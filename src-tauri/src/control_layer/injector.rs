pub const CONTROL_BINDING_NAME: &str = "codexAssistant";
const MAX_INJECTION_SCRIPT_BYTES: usize = 262_144;

use serde::Serialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::cdp::{CdpClient, CdpClientError};

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
