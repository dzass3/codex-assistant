mod protocol;
mod tools;

use std::{io, path::PathBuf};

use protocol::{contains_forbidden_field, error, request, success, valid_params, SessionPhase};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::routing::state::RoutingStateStore;

pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const COMPATIBLE_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn run_stdio_default() -> io::Result<()> {
    #[cfg(debug_assertions)]
    let state_directory = std::env::var_os("CODEX_ASSISTANT_ROUTING_STATE_DIR")
        .map(PathBuf::from)
        .map_or_else(default_state_directory, Ok)?;
    #[cfg(not(debug_assertions))]
    let state_directory = default_state_directory()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?
        .block_on(serve(
            tokio::io::stdin(),
            tokio::io::stdout(),
            tokio::io::stderr(),
            state_directory,
        ))
}

fn default_state_directory() -> io::Result<PathBuf> {
    RoutingStateStore::default_location()
        .map(|store| store.directory().to_path_buf())
        .map_err(io::Error::other)
}

pub async fn serve<R, W, E>(
    reader: R,
    mut writer: W,
    mut diagnostics: E,
    state_directory: PathBuf,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    E: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut phase = SessionPhase::New;
    let mut diagnostic_count = 0_u64;
    while let Some(line) = lines.next_line().await? {
        let message = match request(&line) {
            Ok(message) => message,
            Err(protocol_error) => {
                diagnostic_count = diagnostic_count.saturating_add(1);
                write_diagnostic(
                    &mut diagnostics,
                    protocol_error.diagnostic_code,
                    diagnostic_count,
                )
                .await?;
                write_response(
                    &mut writer,
                    &error(
                        protocol_error.id,
                        protocol_error.code,
                        protocol_error.message,
                    ),
                )
                .await?;
                continue;
            }
        };
        if message
            .params
            .as_ref()
            .is_some_and(contains_forbidden_field)
        {
            diagnostic_count = diagnostic_count.saturating_add(1);
            write_diagnostic(&mut diagnostics, "invalid_params", diagnostic_count).await?;
            if let Some(id) = message.id {
                write_response(&mut writer, &error(id, -32602, "Invalid params")).await?;
            }
            continue;
        }
        if !valid_params(&message.method, message.params.as_ref()) {
            diagnostic_count = diagnostic_count.saturating_add(1);
            write_diagnostic(&mut diagnostics, "invalid_params", diagnostic_count).await?;
            if let Some(id) = message.id {
                write_response(&mut writer, &error(id, -32602, "Invalid params")).await?;
            }
            continue;
        }
        let mut diagnostic_code = None;
        let response = match (phase, message.method.as_str()) {
            (SessionPhase::New, "initialize") => {
                let requested = message
                    .params
                    .as_ref()
                    .and_then(|params| params.get("protocolVersion"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(LATEST_PROTOCOL_VERSION);
                let negotiated = if matches!(
                    requested,
                    LATEST_PROTOCOL_VERSION | COMPATIBLE_PROTOCOL_VERSION
                ) {
                    requested
                } else {
                    LATEST_PROTOCOL_VERSION
                };
                phase = SessionPhase::AwaitingInitialized;
                message.id.map(|id| {
                    success(
                        id,
                        serde_json::json!({
                            "protocolVersion": negotiated,
                            "capabilities": {"tools": {"listChanged": false}},
                            "serverInfo": {
                                "name": "codex-assistant-routing",
                                "title": "Codex Assistant Routing",
                                "version": env!("CARGO_PKG_VERSION")
                            }
                        }),
                    )
                })
            }
            (SessionPhase::AwaitingInitialized, "notifications/initialized") => {
                phase = SessionPhase::Ready;
                None
            }
            (SessionPhase::AwaitingInitialized | SessionPhase::Ready, "initialize") => {
                diagnostic_code = Some("invalid_request");
                message.id.map(|id| error(id, -32600, "Invalid Request"))
            }
            (SessionPhase::New | SessionPhase::Ready, "notifications/initialized") => {
                diagnostic_code = Some("invalid_request");
                None
            }
            (
                SessionPhase::New | SessionPhase::AwaitingInitialized,
                "tools/list" | "tools/call",
            ) => {
                diagnostic_code = Some("server_not_initialized");
                message
                    .id
                    .map(|id| error(id, -32002, "Server not initialized"))
            }
            (SessionPhase::Ready, "tools/list") => message
                .id
                .map(|id| success(id, serde_json::json!({"tools": tools::definitions()}))),
            (SessionPhase::Ready, "tools/call") => match message.id {
                Some(id) => match tools::call(message.params.as_ref(), &state_directory) {
                    Ok(result) => Some(success(id, result)),
                    Err(tools::CallError::InvalidParams) => {
                        diagnostic_code = Some("invalid_params");
                        Some(error(id, -32602, "Invalid params"))
                    }
                    Err(tools::CallError::Tool(code)) => {
                        diagnostic_code = Some(code);
                        Some(success(id, tools::tool_error(code)))
                    }
                },
                None => {
                    diagnostic_code = Some("invalid_request");
                    None
                }
            },
            _ => {
                diagnostic_code = Some("method_not_found");
                message.id.map(|id| error(id, -32601, "Method not found"))
            }
        };
        if let Some(code) = diagnostic_code {
            diagnostic_count = diagnostic_count.saturating_add(1);
            write_diagnostic(&mut diagnostics, code, diagnostic_count).await?;
        }
        if let Some(response) = response {
            write_response(&mut writer, &response).await?;
        }
    }
    writer.shutdown().await?;
    diagnostics.shutdown().await
}

async fn write_response<W>(writer: &mut W, response: &serde_json::Value) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(
            serde_json::to_string(response)
                .map_err(io::Error::other)?
                .as_bytes(),
        )
        .await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

async fn write_diagnostic<E>(diagnostics: &mut E, code: &str, count: u64) -> io::Result<()>
where
    E: AsyncWrite + Unpin,
{
    diagnostics
        .write_all(format!("routing_mcp_error code={code} count={count}\n").as_bytes())
        .await?;
    diagnostics.flush().await
}
