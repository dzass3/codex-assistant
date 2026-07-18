use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

pub const MAX_CDP_FRAME_BYTES: usize = 1_048_576;
const MAX_SESSION_AGE_MS: i64 = 86_400_000;
const SESSION_SCHEMA_VERSION: u32 = 1;
const ENGINE_VERSION: &str = "control-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpSecurityError {
    MalformedVersion,
    EndpointScheme,
    EndpointAddress,
    EndpointPort,
    BrowserIdentity,
    BrowserIdentityChanged,
    DuplicateBrowserIdentity,
    DuplicateTargetIdentity,
    UnknownTargetType,
    TargetIdentity,
    StaleSession,
    InvalidSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEndpoint {
    pub browser_id: String,
    websocket_url: String,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDescriptor {
    pub target_id: String,
    pub target_type: String,
    pub websocket_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTarget {
    pub target_id: String,
    websocket_url: String,
}

impl BrowserEndpoint {
    pub fn websocket_url(&self) -> &str {
        &self.websocket_url
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn verify_target(
        &self,
        descriptor: TargetDescriptor,
    ) -> Result<VerifiedTarget, CdpSecurityError> {
        if descriptor.target_type != "page" {
            return Err(CdpSecurityError::UnknownTargetType);
        }
        if !safe_target_id(&descriptor.target_id) {
            return Err(CdpSecurityError::TargetIdentity);
        }
        let parsed = parse_loopback_websocket(&descriptor.websocket_url, self.port)?;
        if parsed.path != format!("/devtools/page/{}", descriptor.target_id) {
            return Err(CdpSecurityError::TargetIdentity);
        }
        Ok(VerifiedTarget {
            target_id: descriptor.target_id,
            websocket_url: descriptor.websocket_url,
        })
    }
}

impl VerifiedTarget {
    pub fn websocket_url(&self) -> &str {
        &self.websocket_url
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetChanges {
    pub attach: Vec<VerifiedTarget>,
    pub detach: Vec<String>,
}

pub struct TargetRegistry {
    anchor: BrowserAnchor,
    target_url_hashes: HashMap<String, String>,
}

impl TargetRegistry {
    pub fn new(endpoint: &BrowserEndpoint) -> Self {
        Self {
            anchor: BrowserAnchor::new(endpoint),
            target_url_hashes: HashMap::new(),
        }
    }

    pub fn reconcile(
        &mut self,
        endpoint: &BrowserEndpoint,
        descriptors: Vec<TargetDescriptor>,
    ) -> Result<TargetChanges, CdpSecurityError> {
        self.anchor.verify(endpoint)?;
        let mut next = HashMap::new();
        for descriptor in descriptors {
            let target = endpoint.verify_target(descriptor)?;
            let url_hash = hash_browser_id(target.websocket_url());
            if next
                .insert(target.target_id.clone(), (url_hash, target))
                .is_some()
            {
                return Err(CdpSecurityError::DuplicateTargetIdentity);
            }
        }
        for (target_id, (url_hash, _)) in &next {
            if self
                .target_url_hashes
                .get(target_id)
                .is_some_and(|existing| existing != url_hash)
            {
                return Err(CdpSecurityError::TargetIdentity);
            }
        }
        let mut attach = next
            .iter()
            .filter(|(target_id, _)| !self.target_url_hashes.contains_key(*target_id))
            .map(|(_, (_, target))| target.clone())
            .collect::<Vec<_>>();
        attach.sort_by(|left, right| left.target_id.cmp(&right.target_id));
        let mut detach = self
            .target_url_hashes
            .keys()
            .filter(|target_id| !next.contains_key(*target_id))
            .cloned()
            .collect::<Vec<_>>();
        detach.sort();
        self.target_url_hashes = next
            .into_iter()
            .map(|(target_id, (url_hash, _))| (target_id, url_hash))
            .collect();
        Ok(TargetChanges { attach, detach })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVersionDocument {
    web_socket_debugger_url: String,
}

pub fn browser_endpoint(
    expected_port: u16,
    document: &str,
) -> Result<BrowserEndpoint, CdpSecurityError> {
    if document.len() > MAX_CDP_FRAME_BYTES || expected_port == 0 {
        return Err(CdpSecurityError::MalformedVersion);
    }
    let version: BrowserVersionDocument =
        serde_json::from_str(document).map_err(|_| CdpSecurityError::MalformedVersion)?;
    let parsed = parse_loopback_websocket(&version.web_socket_debugger_url, expected_port)?;
    let browser_id = parsed
        .path
        .strip_prefix("/devtools/browser/")
        .ok_or(CdpSecurityError::BrowserIdentity)?;
    let uuid = Uuid::parse_str(browser_id).map_err(|_| CdpSecurityError::BrowserIdentity)?;
    if uuid.is_nil() || uuid.to_string() != browser_id.to_ascii_lowercase() {
        return Err(CdpSecurityError::BrowserIdentity);
    }
    Ok(BrowserEndpoint {
        browser_id: browser_id.to_owned(),
        websocket_url: version.web_socket_debugger_url,
        port: expected_port,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpDiscoveryError {
    InvalidTimeout,
    RequestFailed,
    UnexpectedStatus,
    BodyTooLarge,
    InvalidBody,
    InvalidEndpoint,
}

pub async fn fetch_browser_endpoint(
    port: u16,
    timeout_ms: u64,
) -> Result<BrowserEndpoint, CdpDiscoveryError> {
    if port == 0 || !(100..=30_000).contains(&timeout_ms) {
        return Err(CdpDiscoveryError::InvalidTimeout);
    }
    let operation_timeout = Duration::from_millis(timeout_ms);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(operation_timeout)
        .timeout(operation_timeout)
        .build()
        .map_err(|_| CdpDiscoveryError::RequestFailed)?;
    let expected_url = format!("http://127.0.0.1:{port}/json/version");
    let response = client
        .get(&expected_url)
        .send()
        .await
        .map_err(|_| CdpDiscoveryError::RequestFailed)?;
    if response.status() != reqwest::StatusCode::OK || response.url().as_str() != expected_url {
        return Err(CdpDiscoveryError::UnexpectedStatus);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_CDP_FRAME_BYTES as u64)
    {
        return Err(CdpDiscoveryError::BodyTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| CdpDiscoveryError::RequestFailed)?;
        let next_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or(CdpDiscoveryError::BodyTooLarge)?;
        if next_length > MAX_CDP_FRAME_BYTES {
            return Err(CdpDiscoveryError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    let document = String::from_utf8(body).map_err(|_| CdpDiscoveryError::InvalidBody)?;
    browser_endpoint(port, &document).map_err(|_| CdpDiscoveryError::InvalidEndpoint)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetListEntry {
    id: String,
    #[serde(rename = "type")]
    target_type: String,
    web_socket_debugger_url: String,
}

pub async fn fetch_page_targets(
    endpoint: &BrowserEndpoint,
    timeout_ms: u64,
) -> Result<Vec<VerifiedTarget>, CdpDiscoveryError> {
    if !(100..=30_000).contains(&timeout_ms) {
        return Err(CdpDiscoveryError::InvalidTimeout);
    }
    let operation_timeout = Duration::from_millis(timeout_ms);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(operation_timeout)
        .timeout(operation_timeout)
        .build()
        .map_err(|_| CdpDiscoveryError::RequestFailed)?;
    let expected_url = format!("http://127.0.0.1:{}/json/list", endpoint.port());
    let response = client
        .get(&expected_url)
        .send()
        .await
        .map_err(|_| CdpDiscoveryError::RequestFailed)?;
    if response.status() != reqwest::StatusCode::OK || response.url().as_str() != expected_url {
        return Err(CdpDiscoveryError::UnexpectedStatus);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_CDP_FRAME_BYTES as u64)
    {
        return Err(CdpDiscoveryError::BodyTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| CdpDiscoveryError::RequestFailed)?;
        if body.len().saturating_add(chunk.len()) > MAX_CDP_FRAME_BYTES {
            return Err(CdpDiscoveryError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    let entries: Vec<TargetListEntry> =
        serde_json::from_slice(&body).map_err(|_| CdpDiscoveryError::InvalidBody)?;
    if entries.len() > 64 {
        return Err(CdpDiscoveryError::BodyTooLarge);
    }
    entries
        .into_iter()
        .filter(|entry| entry.target_type == "page")
        .map(|entry| {
            endpoint
                .verify_target(TargetDescriptor {
                    target_id: entry.id,
                    target_type: entry.target_type,
                    websocket_url: entry.web_socket_debugger_url,
                })
                .map_err(|_| CdpDiscoveryError::InvalidEndpoint)
        })
        .collect()
}

struct ParsedWebSocket {
    path: String,
}

fn parse_loopback_websocket(
    value: &str,
    expected_port: u16,
) -> Result<ParsedWebSocket, CdpSecurityError> {
    let without_scheme = value
        .strip_prefix("ws://")
        .ok_or(CdpSecurityError::EndpointScheme)?;
    let (authority, path) = without_scheme
        .split_once('/')
        .ok_or(CdpSecurityError::MalformedVersion)?;
    if authority.contains('@') || authority.contains('?') || authority.contains('#') {
        return Err(CdpSecurityError::EndpointAddress);
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or(CdpSecurityError::EndpointPort)?;
    if !matches!(host, "127.0.0.1" | "localhost") {
        return Err(CdpSecurityError::EndpointAddress);
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| CdpSecurityError::EndpointPort)?;
    if port != expected_port {
        return Err(CdpSecurityError::EndpointPort);
    }
    Ok(ParsedWebSocket {
        path: format!("/{path}"),
    })
}

fn safe_target_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

pub struct BrowserAnchor {
    browser_id_hash: String,
    observed: bool,
}

impl BrowserAnchor {
    pub fn new(endpoint: &BrowserEndpoint) -> Self {
        Self {
            browser_id_hash: hash_browser_id(&endpoint.browser_id),
            observed: false,
        }
    }

    pub fn verify(&self, endpoint: &BrowserEndpoint) -> Result<(), CdpSecurityError> {
        if hash_browser_id(&endpoint.browser_id) == self.browser_id_hash {
            Ok(())
        } else {
            Err(CdpSecurityError::BrowserIdentityChanged)
        }
    }

    pub fn observe(&mut self, endpoint: &BrowserEndpoint) -> Result<(), CdpSecurityError> {
        self.verify(endpoint)?;
        if self.observed {
            return Err(CdpSecurityError::DuplicateBrowserIdentity);
        }
        self.observed = true;
        Ok(())
    }

    pub fn hash(&self) -> &str {
        &self.browser_id_hash
    }
}

fn hash_browser_id(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedSessionRecord {
    pub schema_version: u32,
    pub port: u16,
    pub verified_pid: u32,
    pub browser_id_hash: String,
    pub codex_version: String,
    pub started_at_ms: i64,
    pub engine_version: String,
}

pub fn create_owned_session_record(
    endpoint: &BrowserEndpoint,
    verified_pid: u32,
    codex_version: &str,
    started_at_ms: i64,
) -> Result<OwnedSessionRecord, CdpSecurityError> {
    let record = OwnedSessionRecord {
        schema_version: SESSION_SCHEMA_VERSION,
        port: endpoint.port(),
        verified_pid,
        browser_id_hash: hash_browser_id(&endpoint.browser_id),
        codex_version: codex_version.to_owned(),
        started_at_ms,
        engine_version: ENGINE_VERSION.to_owned(),
    };
    validate_session_record(&record, verified_pid, codex_version, started_at_ms)?;
    Ok(record)
}

pub fn validate_session_record(
    record: &OwnedSessionRecord,
    current_pid: u32,
    current_codex_version: &str,
    now_ms: i64,
) -> Result<(), CdpSecurityError> {
    let structurally_valid = record.schema_version == SESSION_SCHEMA_VERSION
        && record.port != 0
        && record.verified_pid != 0
        && record.browser_id_hash.len() == 64
        && record
            .browser_id_hash
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        && safe_version(&record.codex_version)
        && record.started_at_ms >= 0
        && record.engine_version == ENGINE_VERSION;
    if !structurally_valid {
        return Err(CdpSecurityError::InvalidSession);
    }
    if current_pid != record.verified_pid
        || current_codex_version != record.codex_version
        || now_ms < record.started_at_ms
        || now_ms.saturating_sub(record.started_at_ms) > MAX_SESSION_AGE_MS
    {
        return Err(CdpSecurityError::StaleSession);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStoreError {
    Unavailable,
    Invalid,
    Stale,
}

pub struct OwnedSessionStore {
    directory: PathBuf,
    session_file: PathBuf,
}

impl OwnedSessionStore {
    pub fn in_directory(directory: &Path) -> Result<Self, SessionStoreError> {
        if directory.exists()
            && directory
                .symlink_metadata()
                .map_err(|_| SessionStoreError::Unavailable)?
                .file_type()
                .is_symlink()
        {
            return Err(SessionStoreError::Unavailable);
        }
        fs::create_dir_all(directory).map_err(|_| SessionStoreError::Unavailable)?;
        crate::routing::state::protect_owned_path(directory)
            .map_err(|_| SessionStoreError::Unavailable)?;
        let session_file = directory.join("control-session.json");
        if session_file.exists()
            && session_file
                .symlink_metadata()
                .map_err(|_| SessionStoreError::Unavailable)?
                .file_type()
                .is_symlink()
        {
            return Err(SessionStoreError::Unavailable);
        }
        Ok(Self {
            directory: directory.to_path_buf(),
            session_file,
        })
    }

    pub fn path(&self) -> &Path {
        &self.session_file
    }

    pub fn save(&self, record: &OwnedSessionRecord) -> Result<(), SessionStoreError> {
        validate_session_record(
            record,
            record.verified_pid,
            &record.codex_version,
            record.started_at_ms,
        )
        .map_err(|_| SessionStoreError::Invalid)?;
        let bytes = serde_json::to_vec(record).map_err(|_| SessionStoreError::Invalid)?;
        let temporary = self
            .directory
            .join(format!(".control-session-{}.tmp", Uuid::new_v4()));
        let write_result = (|| {
            let mut file = File::create(&temporary).map_err(|_| SessionStoreError::Unavailable)?;
            crate::routing::state::protect_owned_path(&temporary)
                .map_err(|_| SessionStoreError::Unavailable)?;
            file.write_all(&bytes)
                .map_err(|_| SessionStoreError::Unavailable)?;
            file.sync_all().map_err(|_| SessionStoreError::Unavailable)
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if crate::routing::state::replace_existing(&temporary, &self.session_file).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(SessionStoreError::Unavailable);
        }
        Ok(())
    }

    pub fn load(
        &self,
        current_pid: u32,
        current_codex_version: &str,
        now_ms: i64,
    ) -> Result<Option<OwnedSessionRecord>, SessionStoreError> {
        let bytes = match fs::read(&self.session_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(SessionStoreError::Unavailable),
        };
        if bytes.len() > 4_096 {
            return Err(SessionStoreError::Invalid);
        }
        let record: OwnedSessionRecord =
            serde_json::from_slice(&bytes).map_err(|_| SessionStoreError::Invalid)?;
        match validate_session_record(&record, current_pid, current_codex_version, now_ms) {
            Ok(()) => Ok(Some(record)),
            Err(CdpSecurityError::StaleSession) => Err(SessionStoreError::Stale),
            Err(_) => Err(SessionStoreError::Invalid),
        }
    }
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundRequest {
    pub id: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingMessage {
    Response { id: u64 },
    BooleanResponse { id: u64, value: bool },
    BindingCalled { payload: String },
    Event { method: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpProtocolError {
    MethodNotAllowed,
    InvalidParams,
    FrameTooLarge,
    MalformedEnvelope,
    UnknownResponseId,
    RemoteFailure,
    EventNotAllowed,
}

pub struct CdpProtocol {
    next_id: u64,
    pending: HashSet<u64>,
    boolean_pending: HashSet<u64>,
}

impl Default for CdpProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl CdpProtocol {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            pending: HashSet::new(),
            boolean_pending: HashSet::new(),
        }
    }

    pub fn request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<OutboundRequest, CdpProtocolError> {
        if !allowed_method(method) {
            return Err(CdpProtocolError::MethodNotAllowed);
        }
        if !params.is_object() {
            return Err(CdpProtocolError::InvalidParams);
        }
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(CdpProtocolError::InvalidParams)?;
        let text = json!({ "id": id, "method": method, "params": params }).to_string();
        if text.len() > MAX_CDP_FRAME_BYTES {
            return Err(CdpProtocolError::FrameTooLarge);
        }
        self.pending.insert(id);
        Ok(OutboundRequest { id, text })
    }

    pub fn boolean_evaluation(
        &mut self,
        params: Value,
    ) -> Result<OutboundRequest, CdpProtocolError> {
        let object = params.as_object().ok_or(CdpProtocolError::InvalidParams)?;
        if !keys_are(object, &["expression", "returnByValue"])
            || object.get("returnByValue") != Some(&Value::Bool(true))
            || object
                .get("expression")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(CdpProtocolError::InvalidParams);
        }
        let outbound = self.request("Runtime.evaluate", params)?;
        self.boolean_pending.insert(outbound.id);
        Ok(outbound)
    }

    pub fn accept(&mut self, frame: &str) -> Result<IncomingMessage, CdpProtocolError> {
        if frame.len() > MAX_CDP_FRAME_BYTES {
            return Err(CdpProtocolError::FrameTooLarge);
        }
        let value: Value =
            serde_json::from_str(frame).map_err(|_| CdpProtocolError::MalformedEnvelope)?;
        let object = value
            .as_object()
            .ok_or(CdpProtocolError::MalformedEnvelope)?;
        if object.contains_key("id") {
            self.accept_response(object)
        } else {
            self.accept_event(object)
        }
    }

    fn accept_response(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<IncomingMessage, CdpProtocolError> {
        if !keys_are(object, &["id", "result"]) && !keys_are(object, &["id", "error"]) {
            return Err(CdpProtocolError::MalformedEnvelope);
        }
        let id = object
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(CdpProtocolError::MalformedEnvelope)?;
        if !self.pending.remove(&id) {
            return Err(CdpProtocolError::UnknownResponseId);
        }
        if object.contains_key("error") {
            self.boolean_pending.remove(&id);
            return Err(CdpProtocolError::RemoteFailure);
        }
        if self.boolean_pending.remove(&id) {
            let result = object
                .get("result")
                .and_then(Value::as_object)
                .ok_or(CdpProtocolError::MalformedEnvelope)?;
            if !keys_are(result, &["result"]) {
                return Err(CdpProtocolError::MalformedEnvelope);
            }
            let remote = result
                .get("result")
                .and_then(Value::as_object)
                .ok_or(CdpProtocolError::MalformedEnvelope)?;
            if !keys_are(remote, &["type", "value"])
                || remote.get("type").and_then(Value::as_str) != Some("boolean")
            {
                return Err(CdpProtocolError::MalformedEnvelope);
            }
            let value = remote
                .get("value")
                .and_then(Value::as_bool)
                .ok_or(CdpProtocolError::MalformedEnvelope)?;
            return Ok(IncomingMessage::BooleanResponse { id, value });
        }
        Ok(IncomingMessage::Response { id })
    }

    fn accept_event(
        &self,
        object: &Map<String, Value>,
    ) -> Result<IncomingMessage, CdpProtocolError> {
        if !keys_are(object, &["method", "params"]) {
            return Err(CdpProtocolError::MalformedEnvelope);
        }
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .ok_or(CdpProtocolError::MalformedEnvelope)?;
        if !allowed_event(method) {
            return Err(CdpProtocolError::EventNotAllowed);
        }
        if method == "Runtime.bindingCalled" {
            let params = object
                .get("params")
                .and_then(Value::as_object)
                .ok_or(CdpProtocolError::MalformedEnvelope)?;
            if !keys_are(params, &["name", "payload", "executionContextId"])
                || params.get("name").and_then(Value::as_str) != Some("codexAssistant")
                || params
                    .get("executionContextId")
                    .and_then(Value::as_u64)
                    .is_none_or(|id| id == 0)
            {
                return Err(CdpProtocolError::MalformedEnvelope);
            }
            let payload = params
                .get("payload")
                .and_then(Value::as_str)
                .filter(|payload| safe_binding_payload(payload))
                .ok_or(CdpProtocolError::MalformedEnvelope)?;
            return Ok(IncomingMessage::BindingCalled {
                payload: payload.to_owned(),
            });
        }
        Ok(IncomingMessage::Event {
            method: method.to_owned(),
        })
    }
}

fn allowed_method(method: &str) -> bool {
    matches!(
        method,
        "Runtime.enable"
            | "Page.enable"
            | "Runtime.addBinding"
            | "Page.addScriptToEvaluateOnNewDocument"
            | "Runtime.evaluate"
            | "Target.setDiscoverTargets"
            | "Target.attachToTarget"
            | "Target.detachFromTarget"
    )
}

fn allowed_event(method: &str) -> bool {
    matches!(
        method,
        "Target.targetCreated"
            | "Target.targetDestroyed"
            | "Target.attachedToTarget"
            | "Target.detachedFromTarget"
            | "Runtime.bindingCalled"
            | "Page.frameNavigated"
    )
}

fn keys_are(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn safe_binding_payload(payload: &str) -> bool {
    if payload.is_empty() || payload.len() > 4_096 || !payload.is_ascii() {
        return false;
    }
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(payload) else {
        return false;
    };
    let base_valid = object.get("v").and_then(Value::as_u64) == Some(1)
        && object
            .get("sessionId")
            .and_then(Value::as_str)
            .is_some_and(safe_metadata_id)
        && object
            .get("targetId")
            .and_then(Value::as_str)
            .is_some_and(safe_metadata_id)
        && object
            .get("routeId")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some_and(|value| !value.is_nil());
    if !base_valid {
        return false;
    }
    match object.get("type").and_then(Value::as_str) {
        Some("toggle") => {
            keys_are(
                &object,
                &["v", "sessionId", "targetId", "type", "routeId", "enabled"],
            ) && object.get("enabled").and_then(Value::as_bool).is_some()
        }
        Some("compatibility") => {
            keys_are(
                &object,
                &[
                    "v",
                    "sessionId",
                    "targetId",
                    "type",
                    "routeId",
                    "compatible",
                    "reason",
                ],
            ) && object.get("compatible").and_then(Value::as_bool).is_some()
                && object
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| {
                        matches!(
                            reason,
                            "ready"
                                | "unsupported-route"
                                | "malformed-route"
                                | "incompatible-shell"
                                | "ambiguous-composer"
                                | "unobserved-root"
                                | "child-route"
                                | "route-mismatch"
                        )
                    })
        }
        Some("submit_intent") => safe_submission_payload(&object, false),
        Some("insertion_result") => {
            safe_submission_payload(&object, true)
                && object
                    .get("result")
                    .and_then(Value::as_str)
                    .is_some_and(|result| matches!(result, "inserted" | "failed"))
        }
        _ => false,
    }
}

fn safe_submission_payload(object: &Map<String, Value>, with_result: bool) -> bool {
    let expected = if with_result {
        &[
            "v",
            "sessionId",
            "targetId",
            "type",
            "routeId",
            "routeKey",
            "submissionId",
            "result",
        ][..]
    } else {
        &[
            "v",
            "sessionId",
            "targetId",
            "type",
            "routeId",
            "routeKey",
            "submissionId",
        ][..]
    };
    keys_are(object, expected)
        && object
            .get("routeKey")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some_and(|value| !value.is_nil())
        && object
            .get("submissionId")
            .and_then(Value::as_str)
            .is_some_and(safe_metadata_id)
}

fn safe_metadata_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpClientError {
    InvalidTimeout,
    ConnectFailed,
    HandshakeFailed,
    WriteFailed,
    ReadFailed,
    TimedOut,
    ConnectionClosed,
    FrameTooLarge,
    ProtocolViolation,
    RemoteFailure,
}

pub struct CdpClient {
    socket: WebSocketStream<TcpStream>,
    protocol: CdpProtocol,
    timeout: Duration,
}

impl CdpClient {
    pub async fn connect(
        endpoint: &BrowserEndpoint,
        timeout_ms: u64,
    ) -> Result<Self, CdpClientError> {
        Self::connect_websocket(endpoint.websocket_url(), endpoint.port(), timeout_ms).await
    }

    pub async fn connect_target(
        target: &VerifiedTarget,
        expected_port: u16,
        timeout_ms: u64,
    ) -> Result<Self, CdpClientError> {
        let parsed = parse_loopback_websocket(target.websocket_url(), expected_port)
            .map_err(|_| CdpClientError::ConnectFailed)?;
        if parsed.path != format!("/devtools/page/{}", target.target_id) {
            return Err(CdpClientError::ConnectFailed);
        }
        Self::connect_websocket(target.websocket_url(), expected_port, timeout_ms).await
    }

    async fn connect_websocket(
        websocket_url: &str,
        port: u16,
        timeout_ms: u64,
    ) -> Result<Self, CdpClientError> {
        if !(100..=30_000).contains(&timeout_ms) {
            return Err(CdpClientError::InvalidTimeout);
        }
        let operation_timeout = Duration::from_millis(timeout_ms);
        let stream = timeout(
            operation_timeout,
            TcpStream::connect((Ipv4Addr::LOCALHOST, port)),
        )
        .await
        .map_err(|_| CdpClientError::TimedOut)?
        .map_err(|_| CdpClientError::ConnectFailed)?;
        let (socket, _) = timeout(operation_timeout, client_async(websocket_url, stream))
            .await
            .map_err(|_| CdpClientError::TimedOut)?
            .map_err(|_| CdpClientError::HandshakeFailed)?;
        Ok(Self {
            socket,
            protocol: CdpProtocol::new(),
            timeout: operation_timeout,
        })
    }

    pub async fn call(&mut self, method: &str, params: Value) -> Result<(), CdpClientError> {
        let outbound = self
            .protocol
            .request(method, params)
            .map_err(map_protocol_error)?;
        timeout(
            self.timeout,
            self.socket.send(Message::Text(outbound.text.into())),
        )
        .await
        .map_err(|_| CdpClientError::TimedOut)?
        .map_err(|_| CdpClientError::WriteFailed)?;
        loop {
            let incoming = timeout(self.timeout, self.socket.next())
                .await
                .map_err(|_| CdpClientError::TimedOut)?
                .ok_or(CdpClientError::ConnectionClosed)?
                .map_err(|_| CdpClientError::ReadFailed)?;
            if incoming.len() > MAX_CDP_FRAME_BYTES {
                return Err(CdpClientError::FrameTooLarge);
            }
            match incoming {
                Message::Text(text) => match self
                    .protocol
                    .accept(text.as_str())
                    .map_err(map_protocol_error)?
                {
                    IncomingMessage::Response { id } if id == outbound.id => return Ok(()),
                    IncomingMessage::Response { .. } => {
                        return Err(CdpClientError::ProtocolViolation)
                    }
                    IncomingMessage::BooleanResponse { .. } => {
                        return Err(CdpClientError::ProtocolViolation)
                    }
                    IncomingMessage::BindingCalled { .. } => {}
                    IncomingMessage::Event { .. } => {}
                },
                Message::Ping(payload) => {
                    timeout(self.timeout, self.socket.send(Message::Pong(payload)))
                        .await
                        .map_err(|_| CdpClientError::TimedOut)?
                        .map_err(|_| CdpClientError::WriteFailed)?;
                }
                Message::Pong(_) => {}
                Message::Close(_) => return Err(CdpClientError::ConnectionClosed),
                Message::Binary(_) | Message::Frame(_) => {
                    return Err(CdpClientError::ProtocolViolation)
                }
            }
        }
    }

    pub async fn evaluate_boolean(&mut self, expression: &str) -> Result<bool, CdpClientError> {
        let outbound = self
            .protocol
            .boolean_evaluation(json!({
                "expression": expression,
                "returnByValue": true,
            }))
            .map_err(map_protocol_error)?;
        timeout(
            self.timeout,
            self.socket.send(Message::Text(outbound.text.into())),
        )
        .await
        .map_err(|_| CdpClientError::TimedOut)?
        .map_err(|_| CdpClientError::WriteFailed)?;
        loop {
            let incoming = timeout(self.timeout, self.socket.next())
                .await
                .map_err(|_| CdpClientError::TimedOut)?
                .ok_or(CdpClientError::ConnectionClosed)?
                .map_err(|_| CdpClientError::ReadFailed)?;
            if incoming.len() > MAX_CDP_FRAME_BYTES {
                return Err(CdpClientError::FrameTooLarge);
            }
            match incoming {
                Message::Text(text) => match self
                    .protocol
                    .accept(text.as_str())
                    .map_err(map_protocol_error)?
                {
                    IncomingMessage::BooleanResponse { id, value } if id == outbound.id => {
                        return Ok(value)
                    }
                    IncomingMessage::Response { .. } | IncomingMessage::BooleanResponse { .. } => {
                        return Err(CdpClientError::ProtocolViolation)
                    }
                    IncomingMessage::BindingCalled { .. } => {}
                    IncomingMessage::Event { .. } => {}
                },
                Message::Ping(payload) => {
                    timeout(self.timeout, self.socket.send(Message::Pong(payload)))
                        .await
                        .map_err(|_| CdpClientError::TimedOut)?
                        .map_err(|_| CdpClientError::WriteFailed)?;
                }
                Message::Pong(_) => {}
                Message::Close(_) => return Err(CdpClientError::ConnectionClosed),
                Message::Binary(_) | Message::Frame(_) => {
                    return Err(CdpClientError::ProtocolViolation)
                }
            }
        }
    }

    pub async fn next_binding_payload(&mut self) -> Result<String, CdpClientError> {
        loop {
            let incoming = timeout(self.timeout, self.socket.next())
                .await
                .map_err(|_| CdpClientError::TimedOut)?
                .ok_or(CdpClientError::ConnectionClosed)?
                .map_err(|_| CdpClientError::ReadFailed)?;
            if incoming.len() > MAX_CDP_FRAME_BYTES {
                return Err(CdpClientError::FrameTooLarge);
            }
            match incoming {
                Message::Text(text) => match self
                    .protocol
                    .accept(text.as_str())
                    .map_err(map_protocol_error)?
                {
                    IncomingMessage::BindingCalled { payload } => return Ok(payload),
                    IncomingMessage::Event { .. } => {}
                    IncomingMessage::Response { .. } | IncomingMessage::BooleanResponse { .. } => {
                        return Err(CdpClientError::ProtocolViolation)
                    }
                },
                Message::Ping(payload) => {
                    timeout(self.timeout, self.socket.send(Message::Pong(payload)))
                        .await
                        .map_err(|_| CdpClientError::TimedOut)?
                        .map_err(|_| CdpClientError::WriteFailed)?;
                }
                Message::Pong(_) => {}
                Message::Close(_) => return Err(CdpClientError::ConnectionClosed),
                Message::Binary(_) | Message::Frame(_) => {
                    return Err(CdpClientError::ProtocolViolation)
                }
            }
        }
    }
}

fn map_protocol_error(error: CdpProtocolError) -> CdpClientError {
    match error {
        CdpProtocolError::FrameTooLarge => CdpClientError::FrameTooLarge,
        CdpProtocolError::RemoteFailure => CdpClientError::RemoteFailure,
        CdpProtocolError::MethodNotAllowed
        | CdpProtocolError::InvalidParams
        | CdpProtocolError::MalformedEnvelope
        | CdpProtocolError::UnknownResponseId
        | CdpProtocolError::EventNotAllowed => CdpClientError::ProtocolViolation,
    }
}
