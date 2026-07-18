use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    monitor::model::{AgentStatus, ModelSource, MonitorSnapshot},
    routing::RouteKind,
};

const TERRA_MODEL: &str = "gpt-5.6-terra";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EligibilityKey {
    pub codex_package_version: String,
    pub profile_version: String,
    pub requested_model: String,
    pub route_kind: RouteKind,
    pub depth: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreflightPhase {
    NotStarted,
    AwaitingVisibleCommand,
    AwaitingNativeChild,
    VerifyingLineage,
    Eligible,
    Unavailable,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreflightReason {
    AwaitingEffectiveModel,
    ChildStillRunning,
    EffectiveModelMismatch,
    NativeProfileRejected,
    LineageAmbiguous,
    DetachedProcess,
    UnrelatedRoot,
    MissingParent,
    ParentNotVerifiedTerra,
    Timeout,
    HostVersionChanged,
    ProfileVersionChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightSignal {
    None,
    NativeProfileRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightAttempt {
    pub attempt_id: Uuid,
    pub key: EligibilityKey,
    pub expected_root_id: Uuid,
    pub expected_parent_id: Uuid,
    pub started_at_ms: i64,
    pub deadline_at_ms: i64,
    pub phase: PreflightPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeObservation {
    pub thread_id: Uuid,
    pub parent_thread_id: Option<Uuid>,
    pub requested_model: Option<String>,
    pub effective_model: Option<String>,
    pub model_source: ModelSource,
    pub status: AgentStatus,
    pub depth: u8,
    pub started_at_ms: i64,
}

pub fn project_monitor(snapshot: &MonitorSnapshot) -> Vec<NativeObservation> {
    snapshot
        .agents
        .iter()
        .filter_map(|agent| {
            let thread_id = Uuid::parse_str(&agent.thread_id)
                .ok()
                .filter(|id| !id.is_nil())?;
            let parent_thread_id = match agent.parent_thread_id.as_deref() {
                Some(parent) => Some(Uuid::parse_str(parent).ok().filter(|id| !id.is_nil())?),
                None => None,
            };
            let depth = u8::try_from(agent.depth).ok()?;
            let started_at_ms = agent.started_at_ms.filter(|value| *value >= 0)?;
            let requested_model = sanitize_model(agent.requested_model.as_deref())?;
            let effective_model = sanitize_model(agent.effective_model.as_deref())?;
            Some(NativeObservation {
                thread_id,
                parent_thread_id,
                requested_model,
                effective_model,
                model_source: agent.model_source,
                status: agent.status,
                depth,
                started_at_ms,
            })
        })
        .collect()
}

fn sanitize_model(value: Option<&str>) -> Option<Option<String>> {
    match value {
        None => Some(None),
        Some(model)
            if matches!(
                model,
                "gpt-5.3-codex-spark" | "gpt-5.6-luna" | "gpt-5.6-terra" | "gpt-5.6-sol"
            ) =>
        {
            Some(Some(model.to_owned()))
        }
        Some(_) => None,
    }
}

pub struct PreflightInput<'a> {
    pub attempt: &'a PreflightAttempt,
    pub observations: &'a [NativeObservation],
    pub current_codex_package_version: &'a str,
    pub current_profile_version: &'a str,
    pub now_ms: i64,
    pub signal: PreflightSignal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightOutcome {
    pub phase: PreflightPhase,
    pub reason: Option<PreflightReason>,
    pub child_thread_id: Option<Uuid>,
}

pub fn reconcile_attempt(input: PreflightInput<'_>) -> PreflightOutcome {
    if input.current_codex_package_version != input.attempt.key.codex_package_version {
        return terminal(PreflightReason::HostVersionChanged, None);
    }
    if input.current_profile_version != input.attempt.key.profile_version {
        return terminal(PreflightReason::ProfileVersionChanged, None);
    }
    if input.signal == PreflightSignal::NativeProfileRejected {
        return terminal(PreflightReason::NativeProfileRejected, None);
    }
    if matches!(
        input.attempt.phase,
        PreflightPhase::NotStarted | PreflightPhase::AwaitingVisibleCommand
    ) {
        return PreflightOutcome {
            phase: input.attempt.phase,
            reason: None,
            child_thread_id: None,
        };
    }

    let candidates = input
        .observations
        .iter()
        .filter(|observation| {
            observation.started_at_ms >= input.attempt.started_at_ms
                && observation.requested_model.as_deref()
                    == Some(input.attempt.key.requested_model.as_str())
        })
        .collect::<Vec<_>>();
    if candidates.len() > 1 {
        return terminal(PreflightReason::LineageAmbiguous, None);
    }
    let Some(candidate) = candidates.first().copied() else {
        return waiting_or_timeout(input.now_ms, input.attempt.deadline_at_ms);
    };
    let child_id = Some(candidate.thread_id);
    let Some(parent_id) = candidate.parent_thread_id else {
        return terminal(PreflightReason::DetachedProcess, child_id);
    };
    if parent_id != input.attempt.expected_parent_id {
        return terminal(PreflightReason::UnrelatedRoot, child_id);
    }
    let Some(parent) = input
        .observations
        .iter()
        .find(|observation| observation.thread_id == parent_id)
    else {
        return terminal(PreflightReason::MissingParent, child_id);
    };
    if candidate.depth != input.attempt.key.depth {
        return terminal(PreflightReason::LineageAmbiguous, child_id);
    }
    match input.attempt.key.route_kind {
        RouteKind::Direct => {
            if parent.thread_id != input.attempt.expected_root_id
                || parent.parent_thread_id.is_some()
            {
                return terminal(PreflightReason::UnrelatedRoot, child_id);
            }
        }
        RouteKind::Nested => {
            if parent.parent_thread_id != Some(input.attempt.expected_root_id) {
                return terminal(PreflightReason::UnrelatedRoot, child_id);
            }
            if parent.depth != 1
                || parent.effective_model.as_deref() != Some(TERRA_MODEL)
                || parent.model_source != ModelSource::TurnContext
            {
                return terminal(PreflightReason::ParentNotVerifiedTerra, child_id);
            }
            if !input
                .observations
                .iter()
                .any(|observation| observation.thread_id == input.attempt.expected_root_id)
            {
                return terminal(PreflightReason::MissingParent, child_id);
            }
        }
    }

    if let Some(effective_model) = candidate.effective_model.as_deref() {
        if effective_model != input.attempt.key.requested_model {
            return terminal(PreflightReason::EffectiveModelMismatch, child_id);
        }
    }
    if candidate.effective_model.is_none() || candidate.model_source != ModelSource::TurnContext {
        return if input.now_ms > input.attempt.deadline_at_ms {
            timed_out(child_id)
        } else {
            verifying(PreflightReason::AwaitingEffectiveModel, child_id)
        };
    }
    match candidate.status {
        AgentStatus::Idle => PreflightOutcome {
            phase: PreflightPhase::Eligible,
            reason: None,
            child_thread_id: child_id,
        },
        AgentStatus::Starting | AgentStatus::Running => {
            if input.now_ms > input.attempt.deadline_at_ms {
                timed_out(child_id)
            } else {
                verifying(PreflightReason::ChildStillRunning, child_id)
            }
        }
        AgentStatus::Interrupted | AgentStatus::TrackingError => {
            terminal(PreflightReason::NativeProfileRejected, child_id)
        }
    }
}

fn waiting_or_timeout(now_ms: i64, deadline_at_ms: i64) -> PreflightOutcome {
    if now_ms > deadline_at_ms {
        timed_out(None)
    } else {
        PreflightOutcome {
            phase: PreflightPhase::AwaitingNativeChild,
            reason: None,
            child_thread_id: None,
        }
    }
}

fn verifying(reason: PreflightReason, child_thread_id: Option<Uuid>) -> PreflightOutcome {
    PreflightOutcome {
        phase: PreflightPhase::VerifyingLineage,
        reason: Some(reason),
        child_thread_id,
    }
}

fn terminal(reason: PreflightReason, child_thread_id: Option<Uuid>) -> PreflightOutcome {
    PreflightOutcome {
        phase: PreflightPhase::Unavailable,
        reason: Some(reason),
        child_thread_id,
    }
}

fn timed_out(child_thread_id: Option<Uuid>) -> PreflightOutcome {
    PreflightOutcome {
        phase: PreflightPhase::TimedOut,
        reason: Some(PreflightReason::Timeout),
        child_thread_id,
    }
}
