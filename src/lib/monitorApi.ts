import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MONITOR_EVENT } from "../config";
import type {
  AgentObservation,
  AgentStatus,
  HealthEntry,
  HealthLevel,
  ModelSource,
  MonitorSettings,
  MonitorSnapshot,
  SummaryCounts,
} from "../../shared/monitor-types";

const AGENT_STATUSES = new Set<AgentStatus>([
  "starting",
  "running",
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

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function number(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function health(value: unknown): HealthEntry {
  const raw = record(value);
  const level = HEALTH_LEVELS.has(raw.level as HealthLevel) ? (raw.level as HealthLevel) : "error";
  return {
    level,
    message: text(raw.message) ?? "Source status unavailable",
    last_success_ms: number(raw.last_success_ms),
    error_count: number(raw.error_count) ?? 0,
  };
}

function agent(value: unknown): AgentObservation | null {
  const raw = record(value);
  const threadId = text(raw.thread_id);
  const displayName = text(raw.display_name);
  if (!threadId || !displayName) return null;
  const status = AGENT_STATUSES.has(raw.status as AgentStatus)
    ? (raw.status as AgentStatus)
    : "tracking-error";
  const modelSource = MODEL_SOURCES.has(raw.model_source as ModelSource)
    ? (raw.model_source as ModelSource)
    : "unknown";
  return {
    thread_id: threadId,
    parent_thread_id: text(raw.parent_thread_id),
    agent_path: text(raw.agent_path),
    display_name: displayName,
    role: text(raw.role),
    project: text(raw.project),
    originator: text(raw.originator),
    requested_model: text(raw.requested_model),
    effective_model: text(raw.effective_model),
    model_source: modelSource,
    reasoning_effort: text(raw.reasoning_effort),
    status,
    model_drift: raw.model_drift === true,
    is_subagent: raw.is_subagent === true,
    depth: number(raw.depth) ?? 0,
    started_at_ms: number(raw.started_at_ms),
    updated_at_ms: number(raw.updated_at_ms),
    freshness_ms: number(raw.freshness_ms),
  };
}

function counts(value: unknown): SummaryCounts {
  const raw = record(value);
  return {
    roots: number(raw.roots) ?? 0,
    subagents: number(raw.subagents) ?? 0,
    starting: number(raw.starting) ?? 0,
    running: number(raw.running) ?? 0,
    idle: number(raw.idle) ?? 0,
    interrupted: number(raw.interrupted) ?? 0,
    tracking_errors: number(raw.tracking_errors) ?? 0,
    model_drifts: number(raw.model_drifts) ?? 0,
  };
}

export function toMonitorSnapshot(value: unknown): MonitorSnapshot {
  const raw = record(value);
  const rawHealth = record(raw.health);
  return {
    generated_at_ms: number(raw.generated_at_ms) ?? Date.now(),
    agents: Array.isArray(raw.agents)
      ? raw.agents.map(agent).filter((item): item is AgentObservation => item !== null)
      : [],
    counts: counts(raw.counts),
    health: {
      state_database: health(rawHealth.state_database),
      rollout_observer: health(rawHealth.rollout_observer),
    },
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
    return invoke("get_monitor_settings");
  },
  async setCodexHome(path: string): Promise<MonitorSettings> {
    return invoke("set_codex_home", { path });
  },
  async subscribe(handler: (snapshot: MonitorSnapshot) => void): Promise<() => void> {
    return listen<unknown>(MONITOR_EVENT, (event) => handler(toMonitorSnapshot(event.payload)));
  },
};
