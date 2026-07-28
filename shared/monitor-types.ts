export type AgentStatus =
  | "starting"
  | "running"
  | "uncertain"
  | "historical-unclosed"
  | "idle"
  | "interrupted"
  | "tracking-error";
export type ModelSource = "turn-context" | "state-database" | "requested-only" | "unknown";
export type HealthLevel = "healthy" | "degraded" | "error";
export type ObserverStatus = "live" | "delayed" | "uncertain" | "error";

export interface HealthEntry {
  level: HealthLevel;
  message: string;
  last_success_ms: number | null;
  error_count: number;
}

export interface AgentObservation {
  thread_id: string;
  parent_thread_id: string | null;
  display_name: string;
  role: string | null;
  project: string | null;
  originator: string | null;
  requested_model: string | null;
  effective_model: string | null;
  model_source: ModelSource;
  reasoning_effort: string | null;
  status: AgentStatus;
  model_drift: boolean;
  is_subagent: boolean;
  depth: number;
  started_at_ms: number | null;
  updated_at_ms: number | null;
  freshness_ms: number | null;
}

export interface SummaryCounts {
  roots: number;
  subagents: number;
  starting: number;
  running: number;
  uncertain: number;
  historical_unclosed: number;
  idle: number;
  interrupted: number;
  tracking_errors: number;
  model_drifts: number;
}

export interface MonitorSnapshot {
  generated_at_ms: number;
  codex_running: boolean;
  session_started_at_ms: number | null;
  observer_status: ObserverStatus;
  agents: AgentObservation[];
  counts: SummaryCounts;
  health: {
    state_database: HealthEntry;
    rollout_observer: HealthEntry;
  };
}

export interface MonitorSettings {
  codex_home_label: string;
  is_default: boolean;
}
