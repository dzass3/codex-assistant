use std::{
    fs,
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use codex_assistant_lib::routing::{
    state::{RoutingStateStore, STATE_SCHEMA_VERSION},
    RootRouteState, RoutePhase, RoutingStateEnvelope,
};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

const CANARY: &str = "CANARY PRIVATE TASK CONTENT";

#[tokio::test]
async fn content_fields_are_rejected_at_nested_protocol_and_tool_boundaries_without_leaking() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.schema_version = STATE_SCHEMA_VERSION;
    state.routes.push(RootRouteState {
        route_key,
        conversation_id: Uuid::new_v4(),
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    RoutingStateStore::in_directory(directory.path())
        .expect("store")
        .save(&state)
        .expect("seed state");

    let mut input = vec![json!({
        "jsonrpc": "2.0",
        "id": "private-initialize",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "privacy-test", "version": "1", "prompt": CANARY}
        }
    })];
    input.push(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "privacy-test", "version": "1"}
        }
    }));
    input.push(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    for (index, forbidden) in [
        "prompt",
        "task",
        "response",
        "reasoning",
        "tool_arguments",
        "tool_output",
        "patch",
        "command",
        "cwd",
        "file_path",
        "auth",
        "cookie",
        "secret",
    ]
    .into_iter()
    .enumerate()
    {
        input.push(json!({
            "jsonrpc": "2.0",
            "id": index,
            "method": "tools/call",
            "params": {
                "name": "routing_policy_get",
                "arguments": {
                    "route_key": route_key,
                    forbidden: {"nested": {"value": CANARY}}
                }
            }
        }));
    }

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert_eq!(responses[0]["id"], "private-initialize");
    assert_eq!(responses[0]["error"]["code"], -32602);
    assert_eq!(responses[1]["id"], "initialize");
    for response in &responses[2..] {
        assert_eq!(response["error"]["code"], -32602);
    }
    assert!(!diagnostics.contains(CANARY));
    assert!(!diagnostics.contains("prompt"));
    for bytes in all_file_bytes(directory.path()) {
        assert!(!String::from_utf8_lossy(&bytes).contains(CANARY));
    }
}

#[test]
fn compiled_binary_runs_exact_sidecar_mode_without_stdout_prose_or_content_leaks() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id: Uuid::new_v4(),
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    RoutingStateStore::in_directory(directory.path())
        .expect("store")
        .save(&state)
        .expect("seed state");
    let mut child = Command::new(env!("CARGO_BIN_EXE_codex-assistant"))
        .arg("routing-mcp")
        .env("CODEX_ASSISTANT_ROUTING_STATE_DIR", directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start sidecar binary");
    let requests = [
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "subprocess-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": "list", "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": "policy",
            "method": "tools/call",
            "params": {"name": "routing_policy_get", "arguments": {"route_key": route_key}}
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "private",
            "method": "tools/call",
            "params": {
                "name": "routing_policy_get",
                "arguments": {"route_key": route_key, "task": CANARY}
            }
        }),
    ];
    {
        let stdin = child.stdin.as_mut().expect("sidecar stdin");
        for request in requests {
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(&request).expect("request JSON")
            )
            .expect("write sidecar request");
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("sidecar exit");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("stdout is JSONL only"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 3);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["route_key"],
        route_key.to_string()
    );
    assert_eq!(responses[3]["error"]["code"], -32602);
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(!stderr.contains(CANARY));
    assert!(stderr
        .lines()
        .all(|line| line.starts_with("routing_mcp_error code=") && !line.contains("task")));
    for bytes in all_file_bytes(directory.path()) {
        assert!(!String::from_utf8_lossy(&bytes).contains(CANARY));
    }
}

async fn run_session(state_directory: &Path, input: &[Value]) -> (Vec<Value>, String) {
    let (mut request_writer, request_reader) = tokio::io::duplex(128 * 1024);
    let (response_writer, mut response_reader) = tokio::io::duplex(128 * 1024);
    let (diagnostic_writer, mut diagnostic_reader) = tokio::io::duplex(128 * 1024);
    let directory = state_directory.to_path_buf();
    let server = tokio::spawn(async move {
        codex_assistant_lib::routing_mcp::serve(
            request_reader,
            response_writer,
            diagnostic_writer,
            directory,
        )
        .await
    });
    for message in input {
        request_writer
            .write_all(
                serde_json::to_string(message)
                    .expect("request JSON")
                    .as_bytes(),
            )
            .await
            .expect("write request");
        request_writer.write_all(b"\n").await.expect("newline");
    }
    request_writer.shutdown().await.expect("request EOF");
    server.await.expect("server task").expect("graceful EOF");
    let mut stdout = Vec::new();
    response_reader
        .read_to_end(&mut stdout)
        .await
        .expect("responses");
    let mut stderr = Vec::new();
    diagnostic_reader
        .read_to_end(&mut stderr)
        .await
        .expect("diagnostics");
    (
        String::from_utf8(stdout)
            .expect("UTF-8 responses")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON response"))
            .collect(),
        String::from_utf8(stderr).expect("UTF-8 diagnostics"),
    )
}

fn all_file_bytes(root: &Path) -> Vec<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                bytes.push(fs::read(path).expect("file"));
            }
        }
    }
    bytes
}
