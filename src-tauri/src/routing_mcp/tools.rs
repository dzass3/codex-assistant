use std::{
    fs::{File, OpenOptions},
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::routing::{
    policy::decide_route,
    state::{protect_owned_path, RoutingRuntime, RoutingStateStore, MAX_JS_SAFE_INTEGER},
    ComplexityBand, EligibilityStatus, ModelTier, QualityOutcome, QualityRecord, RiskBand,
    RouteAction, RouteActivity, RouteKind, RoutePhase, RoutePolicyInput, RouteReasonCode,
    UserOverride,
};

pub(crate) enum CallError {
    InvalidParams,
    Tool(&'static str),
}

pub(crate) fn definitions() -> Vec<Value> {
    vec![
        tool(
            "routing_policy_get",
            "Return sanitized native-routing eligibility, budgets, versions, and reason codes.",
            json!({
                "type": "object",
                "properties": {"route_key": {"type": "string", "format": "uuid"}},
                "required": ["route_key"],
                "additionalProperties": false
            }),
        ),
        tool(
            "routing_route_started",
            "Record metadata for a verified native child route.",
            json!({
                "type": "object",
                "properties": {
                    "route_key": {"type": "string", "format": "uuid"},
                    "child_thread_id": {"type": "string", "format": "uuid"},
                    "subtask_id": {"type": "string", "format": "uuid"},
                    "parent_thread_id": {"type": "string", "format": "uuid"},
                    "selected_profile": {"enum": ["spark", "luna", "terra", "sol"]},
                    "route_kind": {"enum": ["direct", "nested"]},
                    "complexity_band": {"enum": ["mechanical", "bounded", "cross-layer", "architectural"]},
                    "risk_band": {"enum": ["low", "meaningful", "high", "restricted"]},
                    "reason_codes": {"type": "array", "minItems": 1, "items": {"type": "string"}}
                },
                "required": ["route_key", "child_thread_id", "subtask_id", "parent_thread_id", "selected_profile", "route_kind", "complexity_band", "risk_band", "reason_codes"],
                "additionalProperties": false
            }),
        ),
        tool(
            "routing_quality_record",
            "Record bounded quality metadata for one native child.",
            json!({
                "type": "object",
                "properties": {
                    "route_key": {"type": "string", "format": "uuid"},
                    "child_thread_id": {"type": "string", "format": "uuid"},
                    "outcome": {"enum": ["passed", "failed", "degraded"]},
                    "reviewer_tier": {"type": ["string", "null"], "enum": ["spark", "luna", "terra", "sol", null]},
                    "retry_count": {"type": "integer", "minimum": 0, "maximum": 2},
                    "escalation_count": {"type": "integer", "minimum": 0, "maximum": 2}
                },
                "required": ["route_key", "child_thread_id", "outcome", "reviewer_tier", "retry_count", "escalation_count"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

pub(crate) fn call(params: Option<&Value>, state_directory: &Path) -> Result<Value, CallError> {
    let params = object_with_exact_keys(params, &["name", "arguments"])?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or(CallError::InvalidParams)?;
    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .ok_or(CallError::InvalidParams)?;
    match name {
        "routing_policy_get" => policy_get(arguments, state_directory),
        "routing_route_started" => route_started(arguments, state_directory),
        "routing_quality_record" => quality_record(arguments, state_directory),
        _ => Err(CallError::InvalidParams),
    }
}

fn quality_record(
    arguments: &Map<String, Value>,
    state_directory: &Path,
) -> Result<Value, CallError> {
    if !has_exact_keys(
        arguments,
        &[
            "route_key",
            "child_thread_id",
            "outcome",
            "reviewer_tier",
            "retry_count",
            "escalation_count",
        ],
    ) {
        return Err(CallError::InvalidParams);
    }
    let route_key = uuid_argument(arguments, "route_key")?;
    let child_thread_id = uuid_argument(arguments, "child_thread_id")?;
    let outcome = enum_argument::<QualityOutcome>(arguments, "outcome")?;
    let reviewer_tier = match arguments.get("reviewer_tier") {
        Some(Value::Null) => None,
        Some(_) => Some(enum_argument::<ModelTier>(arguments, "reviewer_tier")?),
        None => return Err(CallError::InvalidParams),
    };
    let retry_count = bounded_counter(arguments, "retry_count")?;
    let escalation_count = bounded_counter(arguments, "escalation_count")?;
    let _lock = StateFileLock::acquire(state_directory)?;
    let store = RoutingStateStore::in_directory(state_directory)
        .map_err(|_| CallError::Tool("state-unavailable"))?;
    let runtime = RoutingRuntime::load(store).map_err(|_| CallError::Tool("state-unavailable"))?;
    runtime
        .record_quality(QualityRecord {
            route_key,
            child_thread_id,
            outcome,
            reviewer_tier,
            retry_count,
            escalation_count,
            recorded_at_ms: now_ms()?,
        })
        .map_err(|reason| CallError::Tool(reason_code(reason)))?;
    Ok(tool_success(json!({
        "recorded": true,
        "route_key": route_key,
        "child_thread_id": child_thread_id,
        "outcome": outcome,
        "reviewer_tier": reviewer_tier,
        "retry_count": retry_count,
        "escalation_count": escalation_count
    })))
}

fn route_started(
    arguments: &Map<String, Value>,
    state_directory: &Path,
) -> Result<Value, CallError> {
    if !has_exact_keys(
        arguments,
        &[
            "route_key",
            "child_thread_id",
            "subtask_id",
            "parent_thread_id",
            "selected_profile",
            "route_kind",
            "complexity_band",
            "risk_band",
            "reason_codes",
        ],
    ) {
        return Err(CallError::InvalidParams);
    }
    let route_key = uuid_argument(arguments, "route_key")?;
    let child_thread_id = uuid_argument(arguments, "child_thread_id")?;
    let subtask_id = uuid_argument(arguments, "subtask_id")?;
    let parent_thread_id = uuid_argument(arguments, "parent_thread_id")?;
    let selected_tier = enum_argument::<ModelTier>(arguments, "selected_profile")?;
    let route_kind = enum_argument::<RouteKind>(arguments, "route_kind")?;
    let complexity = enum_argument::<ComplexityBand>(arguments, "complexity_band")?;
    let risk = enum_argument::<RiskBand>(arguments, "risk_band")?;
    let reason_codes = arguments
        .get("reason_codes")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<RouteReasonCode>>(value).ok())
        .filter(|reasons| !reasons.is_empty())
        .ok_or(CallError::InvalidParams)?;

    let _lock = StateFileLock::acquire(state_directory)?;
    let store = RoutingStateStore::in_directory(state_directory)
        .map_err(|_| CallError::Tool("state-unavailable"))?;
    let state = store
        .load()
        .map_err(|_| CallError::Tool("state-unavailable"))?;
    let route = state
        .routes
        .iter()
        .find(|route| route.route_key == route_key)
        .ok_or(CallError::Tool("unknown-route"))?;
    if !route.enabled {
        return Err(CallError::Tool("route-disabled"));
    }
    match route_kind {
        RouteKind::Direct if parent_thread_id != route.conversation_id => {
            return Err(CallError::Tool("parent-lineage-mismatch"));
        }
        RouteKind::Nested => {
            let valid_parent = state.activity.iter().any(|activity| {
                activity.child_thread_id == parent_thread_id
                    && activity.route_key == route_key
                    && activity.route_kind == RouteKind::Direct
                    && !activity.is_reviewer
            });
            if !valid_parent {
                return Err(CallError::Tool("parent-lineage-mismatch"));
            }
        }
        RouteKind::Direct => {}
    }
    if let Some(activity) = state
        .activity
        .iter()
        .find(|activity| activity.child_thread_id == child_thread_id)
    {
        return Err(CallError::Tool(
            if matches!(activity.phase, RoutePhase::Completed | RoutePhase::Degraded) {
                "terminal-child-reactivation"
            } else {
                "child-already-recorded"
            },
        ));
    }
    let eligible_tiers = state
        .eligibility
        .iter()
        .filter(|eligibility| {
            eligibility.route_kind == route_kind
                && eligibility.status == EligibilityStatus::Eligible
                && eligibility.profile_version == state.profile_version
        })
        .map(|eligibility| eligibility.tier)
        .collect::<Vec<_>>();
    if !eligible_tiers.contains(&selected_tier) {
        return Err(CallError::Tool("eligibility-unavailable"));
    }
    let decision = decide_route(RoutePolicyInput {
        complexity,
        risk,
        required_capabilities: Vec::new(),
        eligible_tiers,
        estimated_spawn_overhead_ms: 0,
        user_override: Some(UserOverride::UseTier(selected_tier)),
    });
    if decision.action != RouteAction::Delegate || decision.selected_tier != Some(selected_tier) {
        return Err(CallError::Tool("override-below-floor"));
    }
    if !reason_codes
        .iter()
        .all(|reason| decision.reason_codes.contains(reason))
    {
        return Err(CallError::InvalidParams);
    }
    let reason_codes = decision.reason_codes;
    let now = now_ms()?;
    let runtime = RoutingRuntime::load(store).map_err(|_| CallError::Tool("state-unavailable"))?;
    runtime
        .try_start_activity(RouteActivity {
            route_key,
            child_thread_id,
            subtask_id,
            route_kind,
            phase: RoutePhase::Implementing,
            is_reviewer: false,
            parent_thread_id,
            escalation_count: 0,
            selected_tier,
            requested_tier: Some(selected_tier),
            effective_tier: None,
            reason_codes,
            started_at_ms: now,
            updated_at_ms: now,
        })
        .map_err(|reason| CallError::Tool(reason_code(reason)))?;
    let escalation_count = runtime
        .snapshot()
        .activity
        .into_iter()
        .find(|activity| activity.child_thread_id == child_thread_id)
        .map(|activity| activity.escalation_count)
        .ok_or(CallError::Tool("state-persistence-failed"))?;
    Ok(tool_success(json!({
        "recorded": true,
        "route_key": route_key,
        "child_thread_id": child_thread_id,
        "subtask_id": subtask_id,
        "escalation_count": escalation_count,
        "selected_profile": selected_tier,
        "route_kind": route_kind
    })))
}

fn policy_get(arguments: &Map<String, Value>, state_directory: &Path) -> Result<Value, CallError> {
    if !has_exact_keys(arguments, &["route_key"]) {
        return Err(CallError::InvalidParams);
    }
    let route_key = arguments
        .get("route_key")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| !value.is_nil())
        .ok_or(CallError::InvalidParams)?;
    let state = RoutingStateStore::in_directory(state_directory)
        .and_then(|store| store.load())
        .map_err(|_| CallError::Tool("state-unavailable"))?;
    if !state
        .routes
        .iter()
        .any(|route| route.route_key == route_key)
    {
        return Err(CallError::Tool("unknown-route"));
    }
    let metadata = json!({
        "route_key": route_key,
        "policy_version": "routing-v1",
        "profile_version": state.profile_version,
        "eligibility": state.eligibility,
        "budgets": {
            "max_active_children": 3,
            "max_nested_children": 1,
            "max_automatic_escalations": 2
        },
        "reason_codes": reason_codes()
    });
    Ok(tool_success(metadata))
}

fn object_with_exact_keys<'a>(
    value: Option<&'a Value>,
    expected: &[&str],
) -> Result<&'a Map<String, Value>, CallError> {
    let object = value
        .and_then(Value::as_object)
        .ok_or(CallError::InvalidParams)?;
    if has_exact_keys(object, expected) {
        Ok(object)
    } else {
        Err(CallError::InvalidParams)
    }
}

fn has_exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && object.keys().all(|key| expected.contains(&key.as_str()))
}

fn uuid_argument(arguments: &Map<String, Value>, key: &str) -> Result<Uuid, CallError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| !value.is_nil())
        .ok_or(CallError::InvalidParams)
}

fn enum_argument<T>(arguments: &Map<String, Value>, key: &str) -> Result<T, CallError>
where
    T: serde::de::DeserializeOwned,
{
    arguments
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or(CallError::InvalidParams)
}

fn bounded_counter(arguments: &Map<String, Value>, key: &str) -> Result<u8, CallError> {
    arguments
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value <= 2)
        .map(|value| value as u8)
        .ok_or(CallError::InvalidParams)
}

fn now_ms() -> Result<i64, CallError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CallError::Tool("clock-unavailable"))?
        .as_millis();
    if millis > MAX_JS_SAFE_INTEGER as u128 {
        Err(CallError::Tool("clock-unavailable"))
    } else {
        Ok(millis as i64)
    }
}

fn reason_code(reason: RouteReasonCode) -> &'static str {
    match reason {
        RouteReasonCode::ActiveChildLimitReached => "active-child-limit-reached",
        RouteReasonCode::NestedChildLimitReached => "nested-child-limit-reached",
        RouteReasonCode::EscalationLimitReached => "escalation-limit-reached",
        RouteReasonCode::ReviewerRecursionForbidden => "reviewer-recursion-forbidden",
        RouteReasonCode::NestedDelegationForbidden => "nested-delegation-forbidden",
        RouteReasonCode::PreviousAttemptStillActive => "previous-attempt-still-active",
        RouteReasonCode::UnknownRoute => "unknown-route",
        RouteReasonCode::ParentLineageMismatch => "parent-lineage-mismatch",
        RouteReasonCode::ChildAlreadyRecorded => "child-already-recorded",
        RouteReasonCode::UnknownChild => "unknown-child",
        RouteReasonCode::TerminalChildReactivation => "terminal-child-reactivation",
        RouteReasonCode::EligibilityUnavailable => "eligibility-unavailable",
        RouteReasonCode::QualityAlreadyRecorded => "quality-already-recorded",
        RouteReasonCode::EscalationCountMismatch => "escalation-count-mismatch",
        RouteReasonCode::RetryLimitReached => "retry-limit-reached",
        _ => "state-persistence-failed",
    }
}

struct StateFileLock {
    file: File,
}

impl StateFileLock {
    fn acquire(state_directory: &Path) -> Result<Self, CallError> {
        std::fs::create_dir_all(state_directory)
            .map_err(|_| CallError::Tool("state-unavailable"))?;
        let path = state_directory.join("routing-mcp.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|_| CallError::Tool("state-unavailable"))?;
        protect_owned_path(&path).map_err(|_| CallError::Tool("state-unavailable"))?;
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { file }),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || matches!(error.raw_os_error(), Some(32 | 33)) =>
                {
                    if Instant::now() >= deadline {
                        return Err(CallError::Tool("state-lock-timeout"));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return Err(CallError::Tool("state-unavailable")),
            }
        }
    }
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn tool_success(metadata: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": metadata.to_string()}],
        "structuredContent": metadata,
        "isError": false
    })
}

pub(crate) fn tool_error(code: &'static str) -> Value {
    let metadata = json!({"code": code});
    json!({
        "content": [{"type": "text", "text": metadata.to_string()}],
        "structuredContent": metadata,
        "isError": true
    })
}

fn reason_codes() -> Vec<RouteReasonCode> {
    vec![
        RouteReasonCode::MechanicalWork,
        RouteReasonCode::BoundedWork,
        RouteReasonCode::CrossLayerWork,
        RouteReasonCode::ArchitecturalWork,
        RouteReasonCode::HighRiskWork,
        RouteReasonCode::RestrictedRiskWork,
        RouteReasonCode::SolFloorRequired,
        RouteReasonCode::SpawnOverheadTooHigh,
        RouteReasonCode::DoNotDelegate,
        RouteReasonCode::OverrideBelowFloor,
        RouteReasonCode::NoEligibleTier,
        RouteReasonCode::ActiveChildLimitReached,
        RouteReasonCode::NestedChildLimitReached,
        RouteReasonCode::EscalationLimitReached,
        RouteReasonCode::ReviewerRecursionForbidden,
        RouteReasonCode::NestedDelegationForbidden,
        RouteReasonCode::PreviousAttemptStillActive,
        RouteReasonCode::UnknownRoute,
        RouteReasonCode::ParentLineageMismatch,
        RouteReasonCode::ChildAlreadyRecorded,
        RouteReasonCode::UnknownChild,
        RouteReasonCode::TerminalChildReactivation,
        RouteReasonCode::EligibilityUnavailable,
        RouteReasonCode::QualityAlreadyRecorded,
        RouteReasonCode::EscalationCountMismatch,
        RouteReasonCode::RetryLimitReached,
        RouteReasonCode::StatePersistenceFailed,
    ]
}
