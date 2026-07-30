use serde::{Deserialize, Serialize};

pub type SourceResult<T> = Result<T, SourceError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceErrorCode {
    Missing,
    Busy,
    SchemaMismatch,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceError {
    pub code: SourceErrorCode,
    pub message: String,
}

impl SourceError {
    pub fn new(code: SourceErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatus {
    Starting,
    Running,
    Uncertain,
    HistoricalUnclosed,
    Idle,
    Interrupted,
    TrackingError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSource {
    TurnContext,
    StateDatabase,
    RequestedOnly,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObserverStatus {
    Live,
    Delayed,
    Uncertain,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthEntry {
    pub level: HealthLevel,
    pub message: String,
    pub last_success_ms: Option<i64>,
    pub error_count: u64,
}

impl HealthEntry {
    pub fn healthy(message: impl Into<String>, now_ms: i64) -> Self {
        Self {
            level: HealthLevel::Healthy,
            message: message.into(),
            last_success_ms: Some(now_ms),
            error_count: 0,
        }
    }

    pub fn degraded(message: impl Into<String>, last_success_ms: Option<i64>, errors: u64) -> Self {
        Self {
            level: HealthLevel::Degraded,
            message: message.into(),
            last_success_ms,
            error_count: errors,
        }
    }

    pub fn error(message: impl Into<String>, errors: u64) -> Self {
        Self {
            level: HealthLevel::Error,
            message: message.into(),
            last_success_ms: None,
            error_count: errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealth {
    pub state_database: HealthEntry,
    pub rollout_observer: HealthEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSessionEvidence {
    VerifiedAbsent,
    OneVerifiedOfficial { process_id: u32, started_at_ms: i64 },
    MultipleVerifiedOfficial { process_count: usize },
    DiscoveryUncertain,
    IdentityUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessActivityProjection {
    Current,
    Historical,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSessionConfidence {
    Verified,
    MultipleVerifiedProcesses,
    DiscoveryUncertain,
    IdentityUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessSessionProjection {
    pub process_id: Option<u32>,
    pub codex_running: bool,
    pub session_started_at_ms: Option<i64>,
    pub activity: ProcessActivityProjection,
    pub confidence: ProcessSessionConfidence,
    pub monitor_confident: bool,
}

impl ProcessSessionEvidence {
    pub fn project(self, observed_activity_ms: Option<i64>) -> ProcessSessionProjection {
        match self {
            Self::VerifiedAbsent => ProcessSessionProjection {
                process_id: None,
                codex_running: false,
                session_started_at_ms: None,
                activity: ProcessActivityProjection::Historical,
                confidence: ProcessSessionConfidence::Verified,
                monitor_confident: true,
            },
            Self::OneVerifiedOfficial {
                process_id,
                started_at_ms,
            } => ProcessSessionProjection {
                process_id: Some(process_id),
                codex_running: true,
                session_started_at_ms: Some(started_at_ms),
                activity: if observed_activity_ms.is_some_and(|observed| observed >= started_at_ms)
                {
                    ProcessActivityProjection::Current
                } else {
                    ProcessActivityProjection::Historical
                },
                confidence: ProcessSessionConfidence::Verified,
                monitor_confident: true,
            },
            Self::MultipleVerifiedOfficial { .. } => ProcessSessionProjection {
                process_id: None,
                codex_running: true,
                session_started_at_ms: None,
                activity: ProcessActivityProjection::Uncertain,
                confidence: ProcessSessionConfidence::MultipleVerifiedProcesses,
                monitor_confident: false,
            },
            Self::DiscoveryUncertain => ProcessSessionProjection {
                process_id: None,
                codex_running: false,
                session_started_at_ms: None,
                activity: ProcessActivityProjection::Uncertain,
                confidence: ProcessSessionConfidence::DiscoveryUncertain,
                monitor_confident: false,
            },
            Self::IdentityUncertain => ProcessSessionProjection {
                process_id: None,
                codex_running: false,
                session_started_at_ms: None,
                activity: ProcessActivityProjection::Uncertain,
                confidence: ProcessSessionConfidence::IdentityUncertain,
                monitor_confident: false,
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryCounts {
    pub roots: usize,
    pub subagents: usize,
    pub starting: usize,
    pub running: usize,
    pub uncertain: usize,
    pub historical_unclosed: usize,
    pub idle: usize,
    pub interrupted: usize,
    pub tracking_errors: usize,
    pub model_drifts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentObservation {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub display_name: String,
    pub role: Option<String>,
    pub project: Option<String>,
    pub originator: Option<String>,
    pub requested_model: Option<String>,
    pub effective_model: Option<String>,
    pub model_source: ModelSource,
    pub reasoning_effort: Option<String>,
    pub status: AgentStatus,
    pub model_drift: bool,
    pub is_subagent: bool,
    pub depth: u32,
    pub started_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
    pub freshness_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub generated_at_ms: i64,
    pub codex_running: bool,
    pub session_started_at_ms: Option<i64>,
    pub observer_status: ObserverStatus,
    pub agents: Vec<AgentObservation>,
    pub counts: SummaryCounts,
    pub health: SourceHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestartSafetyProjection {
    pub active_work_count: usize,
    pub monitor_confident: bool,
}

impl RestartSafetyProjection {
    pub fn confirmed(active_work_count: usize) -> Self {
        Self {
            active_work_count,
            monitor_confident: true,
        }
    }

    pub fn from_snapshot(snapshot: &MonitorSnapshot) -> Self {
        let sources_healthy = snapshot.health.state_database.level == HealthLevel::Healthy
            && snapshot.health.rollout_observer.level == HealthLevel::Healthy;
        Self {
            active_work_count: snapshot.counts.starting + snapshot.counts.running,
            monitor_confident: sources_healthy
                && snapshot.counts.tracking_errors == 0
                && snapshot.counts.uncertain == 0
                && snapshot.observer_status == ObserverStatus::Live,
        }
    }

    pub fn blocking_reason(self) -> Option<&'static str> {
        if self.active_work_count != 0 {
            Some("active-work")
        } else if !self.monitor_confident {
            Some("monitor-uncertain")
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThreadFact {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub project: Option<String>,
    pub originator: Option<String>,
    pub database_model: Option<String>,
    pub database_effort: Option<String>,
    pub rollout_model: Option<String>,
    pub rollout_effort: Option<String>,
    pub created_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
    pub latest_task_started_ms: Option<i64>,
    pub latest_task_completed_ms: Option<i64>,
    pub interrupted_at_ms: Option<i64>,
    pub tracking_error: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SpawnFact {
    pub child_thread_id: String,
    pub requested_model: Option<String>,
    pub requested_effort: Option<String>,
    pub occurred_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReconcileInput {
    pub threads: Vec<ThreadFact>,
    pub spawns: Vec<SpawnFact>,
    pub health: SourceHealth,
    pub process_session: ProcessSessionEvidence,
}
