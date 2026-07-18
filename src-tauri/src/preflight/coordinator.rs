use uuid::Uuid;

use crate::monitor::model::MonitorSnapshot;
use crate::routing::{
    EligibilityReasonCode, EligibilityRecord, EligibilityStatus, ModelTier, RoutingRuntime,
};

use super::{
    project_monitor, reconcile_attempt, EligibilityKey, PreflightAttempt, PreflightInput,
    PreflightOutcome, PreflightPhase, PreflightReason, PreflightSignal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinatorRecord {
    pub attempt: PreflightAttempt,
    pub outcome: PreflightOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightDirective {
    pub attempt_id: Uuid,
    pub text: String,
}

#[derive(Default)]
pub struct PreflightCoordinator {
    records: Vec<CoordinatorRecord>,
}

impl PreflightCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(
        &mut self,
        key: EligibilityKey,
        expected_root_id: Uuid,
        expected_parent_id: Uuid,
        started_at_ms: i64,
        timeout_ms: i64,
    ) -> Result<Uuid, String> {
        validate_start(
            &key,
            expected_root_id,
            expected_parent_id,
            started_at_ms,
            timeout_ms,
        )?;
        if self.records.iter().any(|record| record.attempt.key == key) {
            return Err("Preflight already exists for this eligibility key".to_owned());
        }
        let attempt_id = Uuid::new_v4();
        let phase = PreflightPhase::AwaitingVisibleCommand;
        self.records.push(CoordinatorRecord {
            attempt: PreflightAttempt {
                attempt_id,
                key,
                expected_root_id,
                expected_parent_id,
                started_at_ms,
                deadline_at_ms: started_at_ms.saturating_add(timeout_ms),
                phase,
            },
            outcome: PreflightOutcome {
                phase,
                reason: None,
                child_thread_id: None,
            },
        });
        Ok(attempt_id)
    }

    pub fn get(&self, key: &EligibilityKey) -> Option<&CoordinatorRecord> {
        self.records
            .iter()
            .find(|record| &record.attempt.key == key)
    }

    pub fn eligibility_record(
        &self,
        key: &EligibilityKey,
        checked_at_ms: i64,
    ) -> Result<EligibilityRecord, String> {
        if checked_at_ms < 0 {
            return Err("Preflight timestamp is invalid".to_owned());
        }
        let record = self
            .get(key)
            .ok_or_else(|| "Preflight eligibility key is unknown".to_owned())?;
        let tier = ModelTier::from_model_id(&key.requested_model)
            .ok_or_else(|| "Preflight model is unsupported".to_owned())?;
        let (status, reason) = eligibility_status(&record.outcome);
        Ok(EligibilityRecord {
            tier,
            route_kind: key.route_kind,
            status,
            checked_at_ms,
            profile_version: key.profile_version.clone(),
            codex_package_version: key.codex_package_version.clone(),
            requested_model: key.requested_model.clone(),
            depth: key.depth,
            reason,
        })
    }

    pub fn persist_eligibility(
        &self,
        key: &EligibilityKey,
        checked_at_ms: i64,
        runtime: &RoutingRuntime,
    ) -> Result<(), String> {
        runtime.upsert_eligibility(self.eligibility_record(key, checked_at_ms)?)
    }

    pub fn directive(&self, key: &EligibilityKey) -> Result<PreflightDirective, String> {
        let record = self
            .get(key)
            .ok_or_else(|| "Preflight eligibility key is unknown".to_owned())?;
        if record.attempt.phase != PreflightPhase::AwaitingVisibleCommand {
            return Err("Preflight is not awaiting a visible command".to_owned());
        }
        let profile = profile_for_model(&key.requested_model)
            .ok_or_else(|| "Preflight model is unsupported".to_owned())?;
        let route = match key.route_kind {
            crate::routing::RouteKind::Direct => "from the current root",
            crate::routing::RouteKind::Nested => "from the verified visible Terra parent",
        };
        Ok(PreflightDirective {
            attempt_id: record.attempt.attempt_id,
            text: format!(
                "Codex Assistant preflight {}: create exactly one visible native child {route} using profile {profile} with fork_turns=\"none\". The child performs no user work and reports only native availability.",
                record.attempt.attempt_id
            ),
        })
    }

    pub fn mark_visible_command_submitted(&mut self, key: &EligibilityKey) -> Result<(), String> {
        let record = self
            .records
            .iter_mut()
            .find(|record| &record.attempt.key == key)
            .ok_or_else(|| "Preflight eligibility key is unknown".to_owned())?;
        if record.attempt.phase != PreflightPhase::AwaitingVisibleCommand {
            return Err("Preflight visible command is not pending".to_owned());
        }
        record.attempt.phase = PreflightPhase::AwaitingNativeChild;
        record.outcome = PreflightOutcome {
            phase: PreflightPhase::AwaitingNativeChild,
            reason: None,
            child_thread_id: None,
        };
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconcile_monitor(
        &mut self,
        key: &EligibilityKey,
        snapshot: &MonitorSnapshot,
        current_codex_package_version: &str,
        current_profile_version: &str,
        now_ms: i64,
        signal: PreflightSignal,
    ) -> Result<PreflightOutcome, String> {
        let record = self
            .records
            .iter_mut()
            .find(|record| &record.attempt.key == key)
            .ok_or_else(|| "Preflight eligibility key is unknown".to_owned())?;
        if current_codex_package_version == record.attempt.key.codex_package_version
            && current_profile_version == record.attempt.key.profile_version
            && signal == PreflightSignal::None
            && matches!(
                record.outcome.phase,
                PreflightPhase::Eligible | PreflightPhase::Unavailable | PreflightPhase::TimedOut
            )
        {
            return Ok(record.outcome.clone());
        }
        let observations = project_monitor(snapshot);
        let outcome = reconcile_attempt(PreflightInput {
            attempt: &record.attempt,
            observations: &observations,
            current_codex_package_version,
            current_profile_version,
            now_ms,
            signal,
        });
        record.attempt.phase = outcome.phase;
        record.outcome = outcome.clone();
        Ok(outcome)
    }

    pub fn invalidate_versions(
        &mut self,
        current_codex_package_version: &str,
        current_profile_version: &str,
    ) -> usize {
        let mut changed = 0;
        for record in &mut self.records {
            let reason =
                if record.attempt.key.codex_package_version != current_codex_package_version {
                    Some(PreflightReason::HostVersionChanged)
                } else if record.attempt.key.profile_version != current_profile_version {
                    Some(PreflightReason::ProfileVersionChanged)
                } else {
                    None
                };
            if let Some(reason) = reason {
                let outcome = PreflightOutcome {
                    phase: PreflightPhase::Unavailable,
                    reason: Some(reason),
                    child_thread_id: record.outcome.child_thread_id,
                };
                if record.outcome != outcome {
                    record.attempt.phase = outcome.phase;
                    record.outcome = outcome;
                    changed += 1;
                }
            }
        }
        changed
    }
}

fn validate_start(
    key: &EligibilityKey,
    expected_root_id: Uuid,
    expected_parent_id: Uuid,
    started_at_ms: i64,
    timeout_ms: i64,
) -> Result<(), String> {
    let valid_depth = matches!(
        (key.route_kind, key.depth),
        (crate::routing::RouteKind::Direct, 1) | (crate::routing::RouteKind::Nested, 2)
    );
    if expected_root_id.is_nil()
        || expected_parent_id.is_nil()
        || started_at_ms < 0
        || !(1_000..=300_000).contains(&timeout_ms)
        || !valid_depth
        || profile_for_model(&key.requested_model).is_none()
        || !safe_host_version(&key.codex_package_version)
        || !safe_profile_version(&key.profile_version)
    {
        return Err("Preflight parameters are invalid".to_owned());
    }
    if key.route_kind == crate::routing::RouteKind::Direct && expected_root_id != expected_parent_id
    {
        return Err("Preflight direct lineage is invalid".to_owned());
    }
    if key.route_kind == crate::routing::RouteKind::Nested
        && (expected_root_id == expected_parent_id
            || !matches!(
                key.requested_model.as_str(),
                "gpt-5.3-codex-spark" | "gpt-5.6-luna"
            ))
    {
        return Err("Preflight nested lineage is invalid".to_owned());
    }
    Ok(())
}

fn eligibility_status(
    outcome: &PreflightOutcome,
) -> (EligibilityStatus, Option<EligibilityReasonCode>) {
    match outcome.phase {
        PreflightPhase::NotStarted => (EligibilityStatus::Unknown, None),
        PreflightPhase::AwaitingVisibleCommand => (
            EligibilityStatus::Verifying,
            Some(EligibilityReasonCode::AwaitingVisibleCommand),
        ),
        PreflightPhase::AwaitingNativeChild => (
            EligibilityStatus::Verifying,
            Some(EligibilityReasonCode::AwaitingNativeChild),
        ),
        PreflightPhase::VerifyingLineage => (
            EligibilityStatus::Verifying,
            outcome
                .reason
                .map(map_reason)
                .or(Some(EligibilityReasonCode::AwaitingEffectiveModel)),
        ),
        PreflightPhase::Eligible => (EligibilityStatus::Eligible, None),
        PreflightPhase::Unavailable => {
            let reason = outcome
                .reason
                .map(map_reason)
                .unwrap_or(EligibilityReasonCode::NativeProfileRejected);
            let status = if matches!(
                reason,
                EligibilityReasonCode::HostVersionChanged
                    | EligibilityReasonCode::ProfileVersionChanged
            ) {
                EligibilityStatus::Stale
            } else {
                EligibilityStatus::Unavailable
            };
            (status, Some(reason))
        }
        PreflightPhase::TimedOut => (
            EligibilityStatus::Unavailable,
            Some(EligibilityReasonCode::Timeout),
        ),
    }
}

fn map_reason(reason: PreflightReason) -> EligibilityReasonCode {
    match reason {
        PreflightReason::AwaitingEffectiveModel => EligibilityReasonCode::AwaitingEffectiveModel,
        PreflightReason::ChildStillRunning => EligibilityReasonCode::ChildStillRunning,
        PreflightReason::EffectiveModelMismatch => EligibilityReasonCode::EffectiveModelMismatch,
        PreflightReason::NativeProfileRejected => EligibilityReasonCode::NativeProfileRejected,
        PreflightReason::LineageAmbiguous => EligibilityReasonCode::LineageAmbiguous,
        PreflightReason::DetachedProcess => EligibilityReasonCode::DetachedProcess,
        PreflightReason::UnrelatedRoot => EligibilityReasonCode::UnrelatedRoot,
        PreflightReason::MissingParent => EligibilityReasonCode::MissingParent,
        PreflightReason::ParentNotVerifiedTerra => EligibilityReasonCode::ParentNotVerifiedTerra,
        PreflightReason::Timeout => EligibilityReasonCode::Timeout,
        PreflightReason::HostVersionChanged => EligibilityReasonCode::HostVersionChanged,
        PreflightReason::ProfileVersionChanged => EligibilityReasonCode::ProfileVersionChanged,
    }
}

fn safe_host_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+')
        })
}

fn safe_profile_version(value: &str) -> bool {
    value.strip_prefix("routing-v").is_some_and(|version| {
        !version.is_empty() && version.chars().all(|character| character.is_ascii_digit())
    })
}

fn profile_for_model(model: &str) -> Option<&'static str> {
    match model {
        "gpt-5.3-codex-spark" => Some("codex_assistant_spark"),
        "gpt-5.6-luna" => Some("codex_assistant_luna"),
        "gpt-5.6-terra" => Some("codex_assistant_terra"),
        "gpt-5.6-sol" => Some("codex_assistant_sol"),
        _ => None,
    }
}
