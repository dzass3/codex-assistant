pub const CONTROL_BINDING_NAME: &str = "codexAssistant";
const MAX_INJECTION_SCRIPT_BYTES: usize = 262_144;

use serde::Serialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::cdp::{
    fetch_page_targets, BrowserEndpoint, CdpClient, CdpClientError, CdpDiscoveryError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPrimitive {
    RuntimeEnable,
    PageEnable,
    RuntimeAddBinding,
    PageAddScriptOnNewDocument,
    RuntimeEvaluate,
}

pub const REQUIRED_PRIMITIVES: [InjectionPrimitive; 5] = [
    InjectionPrimitive::RuntimeEnable,
    InjectionPrimitive::PageEnable,
    InjectionPrimitive::RuntimeAddBinding,
    InjectionPrimitive::PageAddScriptOnNewDocument,
    InjectionPrimitive::RuntimeEvaluate,
];

#[derive(Debug, Clone, PartialEq)]
pub struct InjectionCommand {
    pub method: &'static str,
    pub params: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPlanError {
    InvalidScript,
}

pub fn injection_plan(script: &str) -> Result<Vec<InjectionCommand>, InjectionPlanError> {
    if script.is_empty()
        || script.len() > MAX_INJECTION_SCRIPT_BYTES
        || script.as_bytes().contains(&0)
    {
        return Err(InjectionPlanError::InvalidScript);
    }
    Ok(vec![
        InjectionCommand {
            method: "Runtime.enable",
            params: json!({}),
        },
        InjectionCommand {
            method: "Page.enable",
            params: json!({}),
        },
        InjectionCommand {
            method: "Runtime.addBinding",
            params: json!({ "name": CONTROL_BINDING_NAME }),
        },
        InjectionCommand {
            method: "Page.addScriptToEvaluateOnNewDocument",
            params: json!({ "source": script }),
        },
        InjectionCommand {
            method: "Runtime.evaluate",
            params: json!({
                "expression": script,
                "awaitPromise": false,
                "returnByValue": false,
            }),
        },
    ])
}

pub async fn apply_injection(
    client: &mut CdpClient,
    script: &str,
) -> Result<(), ApplyInjectionError> {
    let plan = injection_plan(script).map_err(|_| ApplyInjectionError::InvalidScript)?;
    for command in plan {
        client
            .call(command.method, command.params)
            .await
            .map_err(ApplyInjectionError::Cdp)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyInjectionError {
    InvalidScript,
    Cdp(CdpClientError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisiblePreflightRequest {
    pub session_id: String,
    pub root_conversation_id: Uuid,
    pub route_key: Uuid,
    pub directive: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRootControlRequest {
    pub root_conversation_id: Uuid,
    pub route_key: Uuid,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleControlBinding {
    pub session_id: String,
    pub target_id: String,
    pub root_conversation_id: Uuid,
    pub route_key: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisiblePreflightError {
    Discovery(CdpDiscoveryError),
    InvalidControl,
    Injection(ApplyInjectionError),
    Cdp(CdpClientError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlReceiveError {
    Discovery(CdpDiscoveryError),
    TargetUnavailable,
    Cdp(CdpClientError),
    Binding(BindingError),
}

pub async fn receive_control_event(
    endpoint: &BrowserEndpoint,
    expected_target_id: &str,
    expected_session_id: &str,
    timeout_ms: u64,
) -> Result<ControlEvent, ControlReceiveError> {
    let target = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ControlReceiveError::Discovery)?
        .into_iter()
        .find(|target| target.target_id == expected_target_id)
        .ok_or(ControlReceiveError::TargetUnavailable)?;
    let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
        .await
        .map_err(ControlReceiveError::Cdp)?;
    client
        .call("Runtime.enable", json!({}))
        .await
        .map_err(ControlReceiveError::Cdp)?;
    let payload = client
        .next_binding_payload()
        .await
        .map_err(ControlReceiveError::Cdp)?;
    parse_binding_message(&payload, expected_session_id, expected_target_id)
        .map_err(ControlReceiveError::Binding)
}

pub async fn set_control_routing_ready(
    endpoint: &BrowserEndpoint,
    expected_target_id: &str,
    ready: bool,
    timeout_ms: u64,
) -> Result<bool, ControlReceiveError> {
    let target = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ControlReceiveError::Discovery)?
        .into_iter()
        .find(|target| target.target_id == expected_target_id)
        .ok_or(ControlReceiveError::TargetUnavailable)?;
    let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
        .await
        .map_err(ControlReceiveError::Cdp)?;
    client
        .evaluate_boolean(&routing_ready_expression(ready))
        .await
        .map_err(ControlReceiveError::Cdp)
}

pub async fn sync_control_routing_enabled(
    endpoint: &BrowserEndpoint,
    expected_target_id: &str,
    enabled: bool,
    timeout_ms: u64,
) -> Result<bool, ControlReceiveError> {
    let target = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ControlReceiveError::Discovery)?
        .into_iter()
        .find(|target| target.target_id == expected_target_id)
        .ok_or(ControlReceiveError::TargetUnavailable)?;
    let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
        .await
        .map_err(ControlReceiveError::Cdp)?;
    client
        .evaluate_boolean(&routing_enabled_expression(enabled))
        .await
        .map_err(ControlReceiveError::Cdp)
}

pub async fn insert_preflight_directive_on_pages(
    endpoint: &BrowserEndpoint,
    script: &str,
    css: &str,
    request: &VisiblePreflightRequest,
    timeout_ms: u64,
) -> Result<bool, VisiblePreflightError> {
    Ok(
        insert_preflight_directive_on_pages_detailed(endpoint, script, css, request, timeout_ms)
            .await?
            .is_some(),
    )
}

pub async fn insert_preflight_directive_on_pages_detailed(
    endpoint: &BrowserEndpoint,
    script: &str,
    css: &str,
    request: &VisiblePreflightRequest,
    timeout_ms: u64,
) -> Result<Option<VisibleControlBinding>, VisiblePreflightError> {
    let expression = preflight_insertion_expression(&request.directive)
        .map_err(|_| VisiblePreflightError::InvalidControl)?;
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(VisiblePreflightError::Discovery)?;
    for target in targets {
        let bootstrap = ControlBootstrap {
            session_id: request.session_id.clone(),
            target_id: target.target_id.clone(),
            route_id: request.root_conversation_id.to_string(),
            route_key: request.route_key.to_string(),
            observed: true,
            parent_thread_id: None,
            submit_shortcut: SubmitShortcut::Enter,
        };
        let source = build_control_source(script, css, &bootstrap)
            .map_err(|_| VisiblePreflightError::InvalidControl)?;
        let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
            .await
            .map_err(VisiblePreflightError::Cdp)?;
        apply_injection(&mut client, &source)
            .await
            .map_err(VisiblePreflightError::Injection)?;
        if client
            .evaluate_boolean(&expression)
            .await
            .map_err(VisiblePreflightError::Cdp)?
        {
            return Ok(Some(VisibleControlBinding {
                session_id: request.session_id.clone(),
                target_id: target.target_id,
                root_conversation_id: request.root_conversation_id,
                route_key: request.route_key,
            }));
        }
    }
    Ok(None)
}

pub async fn bind_routing_controls_on_pages_detailed(
    endpoint: &BrowserEndpoint,
    script: &str,
    css: &str,
    requests: &[VisibleRootControlRequest],
    timeout_ms: u64,
) -> Result<Vec<VisibleControlBinding>, VisiblePreflightError> {
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(VisiblePreflightError::Discovery)?;
    let mut bindings = Vec::new();
    for target in targets {
        if !safe_bridge_id(&target.target_id) {
            continue;
        }
        let Ok(mut client) = CdpClient::connect_target(&target, endpoint.port(), timeout_ms).await
        else {
            continue;
        };
        for request in requests {
            let expression =
                visible_root_match_expression(&request.root_conversation_id.to_string())
                    .map_err(|_| VisiblePreflightError::InvalidControl)?;
            let matches = client
                .evaluate_boolean(&expression)
                .await
                .map_err(VisiblePreflightError::Cdp)?;
            if !matches {
                continue;
            }
            let session_id = format!("root-{}", request.route_key);
            let bootstrap = ControlBootstrap {
                session_id: session_id.clone(),
                target_id: target.target_id.clone(),
                route_id: request.root_conversation_id.to_string(),
                route_key: request.route_key.to_string(),
                observed: true,
                parent_thread_id: None,
                submit_shortcut: SubmitShortcut::Enter,
            };
            let current_expression = format!(
                "globalThis.__codexAssistantControlV1?.routeId===\"{}\"&&globalThis.__codexAssistantControlV1?.routeKey===\"{}\"&&globalThis.__codexAssistantControlV1?.sessionId===\"{}\"&&globalThis.__codexAssistantControlV1?.targetId===\"{}\"",
                request.root_conversation_id, request.route_key, session_id, target.target_id
            );
            let already_bound = client
                .evaluate_boolean(&current_expression)
                .await
                .map_err(VisiblePreflightError::Cdp)?;
            if !already_bound {
                let source = build_control_source(script, css, &bootstrap)
                    .map_err(|_| VisiblePreflightError::InvalidControl)?;
                apply_injection(&mut client, &source)
                    .await
                    .map_err(VisiblePreflightError::Injection)?;
            }
            let verified_expression = format!(
                "({current_expression})&&document.querySelectorAll(\"[data-codex-assistant-control]\").length===1"
            );
            if !client
                .evaluate_boolean(&verified_expression)
                .await
                .map_err(VisiblePreflightError::Cdp)?
            {
                return Err(VisiblePreflightError::InvalidControl);
            }
            bindings.push(VisibleControlBinding {
                session_id,
                target_id: target.target_id.clone(),
                root_conversation_id: request.root_conversation_id,
                route_key: request.route_key,
            });
            break;
        }
    }
    Ok(bindings)
}

pub async fn request_visible_agent_stop(
    endpoint: &BrowserEndpoint,
    timeout_ms: u64,
) -> Result<usize, VisiblePreflightError> {
    const EXPRESSION: &str = r#"(()=>{const labels=["stop","cancel","停止","取消"];let count=0;for(const button of document.querySelectorAll("button")){const label=`${button.getAttribute("aria-label")||""} ${button.getAttribute("title")||""} ${button.textContent||""}`.trim().toLowerCase();if(labels.some(token=>label===token||label.includes(token))&&!button.disabled&&button.getClientRects().length){button.click();count+=1}}return count>0})()"#;
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(VisiblePreflightError::Discovery)?;
    let mut stopped = 0;
    for target in targets {
        let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
            .await
            .map_err(VisiblePreflightError::Cdp)?;
        if client
            .evaluate_boolean(EXPRESSION)
            .await
            .map_err(VisiblePreflightError::Cdp)?
        {
            stopped += 1;
        }
    }
    Ok(stopped)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubmitShortcut {
    Enter,
    CtrlEnter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlBootstrap {
    pub session_id: String,
    pub target_id: String,
    pub route_id: String,
    pub route_key: String,
    pub observed: bool,
    pub parent_thread_id: Option<String>,
    pub submit_shortcut: SubmitShortcut,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedBootstrap<'a> {
    v: u8,
    session_id: &'a str,
    target_id: &'a str,
    route_id: &'a str,
    route_key: &'a str,
    observed: bool,
    parent_thread_id: Option<&'a str>,
    submit_shortcut: SubmitShortcut,
    css: &'a str,
}

pub fn build_control_source(
    script: &str,
    css: &str,
    bootstrap: &ControlBootstrap,
) -> Result<String, InjectionPlanError> {
    if script.is_empty()
        || script.len() > 65_536
        || css.len() > 16_384
        || script.as_bytes().contains(&0)
        || css.as_bytes().contains(&0)
        || !safe_bridge_id(&bootstrap.session_id)
        || !safe_bridge_id(&bootstrap.target_id)
        || !valid_uuid(&bootstrap.route_id)
        || !valid_uuid(&bootstrap.route_key)
        || !bootstrap.observed
        || bootstrap.parent_thread_id.is_some()
    {
        return Err(InjectionPlanError::InvalidScript);
    }
    let payload = SerializedBootstrap {
        v: 1,
        session_id: &bootstrap.session_id,
        target_id: &bootstrap.target_id,
        route_id: &bootstrap.route_id,
        route_key: &bootstrap.route_key,
        observed: bootstrap.observed,
        parent_thread_id: bootstrap.parent_thread_id.as_deref(),
        submit_shortcut: bootstrap.submit_shortcut,
        css,
    };
    let serialized =
        serde_json::to_string(&payload).map_err(|_| InjectionPlanError::InvalidScript)?;
    let source = format!("globalThis.__codexAssistantBootstrapV1={serialized};\n{script}");
    if source.len() > MAX_INJECTION_SCRIPT_BYTES {
        return Err(InjectionPlanError::InvalidScript);
    }
    Ok(source)
}

pub fn preflight_insertion_expression(directive: &str) -> Result<String, InjectionPlanError> {
    const PREFIX: &str = "Codex Assistant preflight ";
    const ROUTES: [&str; 2] = [
        "from the current root",
        "from the verified visible Terra parent",
    ];
    const PROFILES: [&str; 4] = [
        "codex_assistant_spark",
        "codex_assistant_luna",
        "codex_assistant_terra",
        "codex_assistant_sol",
    ];
    if directive.len() < 80
        || directive.len() > 1024
        || !directive.is_ascii()
        || directive.contains(['\n', '\r', '\0'])
    {
        return Err(InjectionPlanError::InvalidScript);
    }
    let (attempt, instruction) = directive
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.split_once(": "))
        .ok_or(InjectionPlanError::InvalidScript)?;
    let attempt = Uuid::parse_str(attempt).map_err(|_| InjectionPlanError::InvalidScript)?;
    if attempt.is_nil() {
        return Err(InjectionPlanError::InvalidScript);
    }
    let valid = ROUTES.iter().any(|route| {
        PROFILES.iter().any(|profile| {
            instruction
                == format!(
                    "create exactly one visible native child {route} using profile {profile} with fork_turns=\"none\". The child performs no user work and reports only native availability."
                )
        })
    });
    if !valid {
        return Err(InjectionPlanError::InvalidScript);
    }
    let serialized =
        serde_json::to_string(directive).map_err(|_| InjectionPlanError::InvalidScript)?;
    Ok(format!(
        "globalThis.__codexAssistantControlV1?.insertPreflightDirective({serialized}) === true"
    ))
}

pub fn routing_ready_expression(ready: bool) -> String {
    format!("globalThis.__codexAssistantControlV1?.setRoutingReady({ready}) === true")
}

pub fn routing_enabled_expression(enabled: bool) -> String {
    format!("globalThis.__codexAssistantControlV1?.syncEnabled({enabled}) === true")
}

pub fn visible_root_match_expression(route_id: &str) -> Result<String, InjectionPlanError> {
    if !valid_uuid(route_id) {
        return Err(InjectionPlanError::InvalidScript);
    }
    Ok(format!(
        r#"(()=>{{const roots=document.querySelectorAll("[data-codex-composer-root]");const composers=document.querySelectorAll("[data-codex-composer=\"true\"]");return location.pathname==="/local/{route_id}"&&document.querySelectorAll("main.main-surface").length===1&&document.querySelectorAll("aside.app-shell-left-panel").length===1&&roots.length===1&&composers.length===1&&roots[0].contains(composers[0])&&composers[0].querySelectorAll(".ProseMirror").length===1}})()"#
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityReason {
    Ready,
    UnsupportedRoute,
    MalformedRoute,
    IncompatibleShell,
    AmbiguousComposer,
    UnobservedRoot,
    ChildRoute,
    RouteMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionResult {
    Inserted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEvent {
    Compatibility {
        route_id: Uuid,
        compatible: bool,
        reason: CompatibilityReason,
    },
    Toggle {
        route_id: Uuid,
        enabled: bool,
    },
    SubmitIntent {
        route_id: Uuid,
        route_key: Uuid,
        submission_id: String,
    },
    InsertionResult {
        route_id: Uuid,
        route_key: Uuid,
        submission_id: String,
        result: InsertionResult,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingError {
    Malformed,
    WrongSession,
    WrongTarget,
}

pub fn parse_binding_message(
    payload: &str,
    expected_session_id: &str,
    expected_target_id: &str,
) -> Result<ControlEvent, BindingError> {
    if payload.len() > 4_096
        || !safe_bridge_id(expected_session_id)
        || !safe_bridge_id(expected_target_id)
    {
        return Err(BindingError::Malformed);
    }
    let value: Value = serde_json::from_str(payload).map_err(|_| BindingError::Malformed)?;
    let object = value.as_object().ok_or(BindingError::Malformed)?;
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(BindingError::Malformed);
    }
    let session_id = object
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|value| safe_bridge_id(value))
        .ok_or(BindingError::Malformed)?;
    if session_id != expected_session_id {
        return Err(BindingError::WrongSession);
    }
    let target_id = object
        .get("targetId")
        .and_then(Value::as_str)
        .filter(|value| safe_bridge_id(value))
        .ok_or(BindingError::Malformed)?;
    if target_id != expected_target_id {
        return Err(BindingError::WrongTarget);
    }
    match object.get("type").and_then(Value::as_str) {
        Some("compatibility") => parse_compatibility(object),
        Some("toggle") => parse_toggle(object),
        Some("submit_intent") => parse_submit_intent(object),
        Some("insertion_result") => parse_insertion_result(object),
        _ => Err(BindingError::Malformed),
    }
}

fn parse_compatibility(object: &Map<String, Value>) -> Result<ControlEvent, BindingError> {
    if !exact_keys(
        object,
        &[
            "v",
            "sessionId",
            "targetId",
            "type",
            "routeId",
            "compatible",
            "reason",
        ],
    ) {
        return Err(BindingError::Malformed);
    }
    let route_id = required_uuid(object, "routeId")?;
    let compatible = object
        .get("compatible")
        .and_then(Value::as_bool)
        .ok_or(BindingError::Malformed)?;
    let reason = match object.get("reason").and_then(Value::as_str) {
        Some("ready") => CompatibilityReason::Ready,
        Some("unsupported-route") => CompatibilityReason::UnsupportedRoute,
        Some("malformed-route") => CompatibilityReason::MalformedRoute,
        Some("incompatible-shell") => CompatibilityReason::IncompatibleShell,
        Some("ambiguous-composer") => CompatibilityReason::AmbiguousComposer,
        Some("unobserved-root") => CompatibilityReason::UnobservedRoot,
        Some("child-route") => CompatibilityReason::ChildRoute,
        Some("route-mismatch") => CompatibilityReason::RouteMismatch,
        _ => return Err(BindingError::Malformed),
    };
    if compatible != (reason == CompatibilityReason::Ready) {
        return Err(BindingError::Malformed);
    }
    Ok(ControlEvent::Compatibility {
        route_id,
        compatible,
        reason,
    })
}

fn parse_toggle(object: &Map<String, Value>) -> Result<ControlEvent, BindingError> {
    if !exact_keys(
        object,
        &["v", "sessionId", "targetId", "type", "routeId", "enabled"],
    ) {
        return Err(BindingError::Malformed);
    }
    Ok(ControlEvent::Toggle {
        route_id: required_uuid(object, "routeId")?,
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or(BindingError::Malformed)?,
    })
}

fn parse_submit_intent(object: &Map<String, Value>) -> Result<ControlEvent, BindingError> {
    if !exact_keys(
        object,
        &[
            "v",
            "sessionId",
            "targetId",
            "type",
            "routeId",
            "routeKey",
            "submissionId",
        ],
    ) {
        return Err(BindingError::Malformed);
    }
    Ok(ControlEvent::SubmitIntent {
        route_id: required_uuid(object, "routeId")?,
        route_key: required_uuid(object, "routeKey")?,
        submission_id: required_bridge_id(object, "submissionId")?,
    })
}

fn parse_insertion_result(object: &Map<String, Value>) -> Result<ControlEvent, BindingError> {
    if !exact_keys(
        object,
        &[
            "v",
            "sessionId",
            "targetId",
            "type",
            "routeId",
            "routeKey",
            "submissionId",
            "result",
        ],
    ) {
        return Err(BindingError::Malformed);
    }
    let result = match object.get("result").and_then(Value::as_str) {
        Some("inserted") => InsertionResult::Inserted,
        Some("failed") => InsertionResult::Failed,
        _ => return Err(BindingError::Malformed),
    };
    Ok(ControlEvent::InsertionResult {
        route_id: required_uuid(object, "routeId")?,
        route_key: required_uuid(object, "routeKey")?,
        submission_id: required_bridge_id(object, "submissionId")?,
        result,
    })
}

fn required_uuid(object: &Map<String, Value>, key: &str) -> Result<Uuid, BindingError> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(BindingError::Malformed)?;
    let uuid = Uuid::parse_str(value).map_err(|_| BindingError::Malformed)?;
    if uuid.is_nil() {
        Err(BindingError::Malformed)
    } else {
        Ok(uuid)
    }
}

fn required_bridge_id(object: &Map<String, Value>, key: &str) -> Result<String, BindingError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| safe_bridge_id(value))
        .map(str::to_owned)
        .ok_or(BindingError::Malformed)
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|uuid| !uuid.is_nil())
}

fn safe_bridge_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
}
