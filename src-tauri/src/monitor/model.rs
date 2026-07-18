use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatus {
    Starting,
    Running,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealth {
    pub state_database: HealthEntry,
    pub rollout_observer: HealthEntry,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryCounts {
    pub roots: usize,
    pub subagents: usize,
    pub starting: usize,
    pub running: usize,
    pub idle: usize,
    pub interrupted: usize,
    pub tracking_errors: usize,
    pub model_drifts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentObservation {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub agent_path: Option<String>,
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
    pub agents: Vec<AgentObservation>,
    pub counts: SummaryCounts,
    pub health: SourceHealth,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadFact {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub agent_path: Option<String>,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
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
    pub task_name: Option<String>,
    pub occurred_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReconcileInput {
    pub threads: Vec<ThreadFact>,
    pub spawns: Vec<SpawnFact>,
    pub health: SourceHealth,
}
