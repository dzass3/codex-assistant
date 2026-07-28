import type {
  AgentObservation,
  AgentStatus,
  HealthEntry,
  HealthLevel,
  ModelSource,
  ObserverStatus,
  MonitorSettings,
  MonitorSnapshot,
  SummaryCounts,
} from "../../shared/monitor-types";
import { invoke } from "./invoke";
import { listen } from "./listen";

const MONITOR_EVENT = "monitor://snapshot";

const AGENT_STATUSES = new Set<AgentStatus>([
  "starting",
  "running",
  "uncertain",
  "historical-unclosed",
  "idle",
  "interrupted",
  "tracking-error",
]);
const MODEL_SOURCES = new Set<ModelSource>([
  "turn-context",
  "state-database",
  "requested-only",
  "unknown",
]);
const HEALTH_LEVELS = new Set<HealthLevel>(["healthy", "degraded", "error"]);
const OBSERVER_STATUSES = new Set<ObserverStatus>(["live", "delayed", "uncertain", "error"]);

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function number(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function nonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : null;
}

type HealthSource = "state_database" | "rollout_observer";

function healthMessage(source: HealthSource, level: HealthLevel) {
  const label = source === "state_database" ? "状态数据库" : "运行记录观察器";
  if (level === "healthy") return `${label}正常`;
  if (level === "degraded") return `${label}数据可能不完整`;
  return `${label}暂时不可用`;
}

function health(value: unknown, source: HealthSource): HealthEntry {
  const raw = record(value);
  const level = HEALTH_LEVELS.has(raw.level as HealthLevel) ? (raw.level as HealthLevel) : "error";
  return {
    level,
    message: healthMessage(source, level),
    last_success_ms: number(raw.last_success_ms),
    error_count: nonNegativeInteger(raw.error_count) ?? 0,
  };
}

function agent(value: unknown): AgentObservation | null {
  const raw = record(value);
  const threadId = text(raw.thread_id);
  const displayName = text(raw.display_name);
  if (!threadId || !displayName) return null;

  const modelSource = MODEL_SOURCES.has(raw.model_source as ModelSource)
    ? (raw.model_source as ModelSource)
    : "unknown";
  const requestedModel = text(raw.requested_model);
  const effectiveModel = modelSource === "requested-only" ? null : text(raw.effective_model);

  return {
    thread_id: threadId,
    parent_thread_id: text(raw.parent_thread_id),
    display_name: displayName,
    role: text(raw.role),
    project: text(raw.project),
    originator: text(raw.originator),
    requested_model: requestedModel,
    effective_model: effectiveModel,
    model_source: modelSource,
    reasoning_effort: text(raw.reasoning_effort),
    status: AGENT_STATUSES.has(raw.status as AgentStatus)
      ? (raw.status as AgentStatus)
      : "tracking-error",
    model_drift: Boolean(requestedModel && effectiveModel && requestedModel !== effectiveModel),
    is_subagent: raw.is_subagent === true,
    depth: nonNegativeInteger(raw.depth) ?? 0,
    started_at_ms: number(raw.started_at_ms),
    updated_at_ms: number(raw.updated_at_ms),
    freshness_ms: number(raw.freshness_ms),
  };
}

function counts(value: unknown): SummaryCounts {
  const raw = record(value);
  return {
    roots: nonNegativeInteger(raw.roots) ?? 0,
    subagents: nonNegativeInteger(raw.subagents) ?? 0,
    starting: nonNegativeInteger(raw.starting) ?? 0,
    running: nonNegativeInteger(raw.running) ?? 0,
    uncertain: nonNegativeInteger(raw.uncertain) ?? 0,
    historical_unclosed: nonNegativeInteger(raw.historical_unclosed) ?? 0,
    idle: nonNegativeInteger(raw.idle) ?? 0,
    interrupted: nonNegativeInteger(raw.interrupted) ?? 0,
    tracking_errors: nonNegativeInteger(raw.tracking_errors) ?? 0,
    model_drifts: nonNegativeInteger(raw.model_drifts) ?? 0,
  };
}

export function toMonitorSnapshot(value: unknown): MonitorSnapshot {
  const raw = record(value);
  const rawHealth = record(raw.health);
  return {
    generated_at_ms: number(raw.generated_at_ms) ?? Date.now(),
    codex_running: raw.codex_running === true,
    session_started_at_ms: number(raw.session_started_at_ms),
    observer_status: OBSERVER_STATUSES.has(raw.observer_status as ObserverStatus)
      ? (raw.observer_status as ObserverStatus)
      : "error",
    agents: Array.isArray(raw.agents)
      ? raw.agents.map(agent).filter((item): item is AgentObservation => item !== null)
      : [],
    counts: counts(raw.counts),
    health: {
      state_database: health(rawHealth.state_database, "state_database"),
      rollout_observer: health(rawHealth.rollout_observer, "rollout_observer"),
    },
  };
}

function toMonitorSettings(value: unknown): MonitorSettings {
  const raw = record(value);
  return {
    codex_home_label: text(raw.codex_home_label) ?? "Codex home unavailable",
    is_default: raw.is_default === true,
  };
}

export const monitorApi = {
  async getSnapshot(): Promise<MonitorSnapshot> {
    return toMonitorSnapshot(await invoke("get_monitor_snapshot"));
  },
  async refresh(): Promise<MonitorSnapshot> {
    return toMonitorSnapshot(await invoke("refresh_monitor"));
  },
  async getSettings(): Promise<MonitorSettings> {
    return toMonitorSettings(await invoke("get_monitor_settings"));
  },
  async setCodexHome(path: string): Promise<MonitorSettings> {
    return toMonitorSettings(await invoke("set_codex_home", { path }));
  },
  async subscribe(handler: (snapshot: MonitorSnapshot) => void): Promise<() => void> {
    return listen<unknown>(MONITOR_EVENT, (event) => handler(toMonitorSnapshot(event.payload)));
  },
};
