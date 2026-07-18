import type {
  EligibilitySnapshot,
  EligibilityStatus,
  ModelTier,
  RootRouteSnapshot,
  RouteActivitySnapshot,
  RouteKind,
  RoutePhase,
  RoutingSnapshot,
} from "../../shared/routing-types";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const FORBIDDEN_FIELD = /prompt|response|reasoning|command|patch|path|token|cookie|secret/i;
const TIERS = new Set<ModelTier>(["spark", "luna", "terra", "sol"]);
const KINDS = new Set<RouteKind>(["direct", "nested"]);
const ELIGIBILITY = new Set<EligibilityStatus>([
  "unknown",
  "verifying",
  "eligible",
  "unavailable",
  "stale",
]);
const PHASES = new Set<RoutePhase>([
  "off",
  "enabled",
  "classifying",
  "implementing",
  "reviewing",
  "completed",
  "degraded",
]);

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return (
    keys.length === expected.length &&
    keys.every((key) => expected.includes(key) && !FORBIDDEN_FIELD.test(key))
  );
}

function string(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function version(value: unknown): string | null {
  const candidate = string(value);
  return candidate !== null && /^[A-Za-z0-9._-]{1,128}$/.test(candidate) ? candidate : null;
}

function uuid(value: unknown): string | null {
  const candidate = string(value);
  return candidate !== null && UUID.test(candidate) ? candidate : null;
}

function integer(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function route(value: unknown): RootRouteSnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "route_key",
      "conversation_id",
      "enabled",
      "phase",
      "created_at_ms",
      "updated_at_ms",
    ])
  ) {
    return null;
  }
  const routeKey = uuid(raw.route_key);
  const conversationId = uuid(raw.conversation_id);
  const phase = string(raw.phase);
  const createdAtMs = integer(raw.created_at_ms);
  const updatedAtMs = integer(raw.updated_at_ms);
  if (
    routeKey === null ||
    conversationId === null ||
    typeof raw.enabled !== "boolean" ||
    phase === null ||
    !PHASES.has(phase as RoutePhase) ||
    createdAtMs === null ||
    updatedAtMs === null
  ) {
    return null;
  }
  return {
    route_key: routeKey,
    conversation_id: conversationId,
    enabled: raw.enabled,
    phase: phase as RoutePhase,
    created_at_ms: createdAtMs,
    updated_at_ms: updatedAtMs,
  };
}

function eligibility(value: unknown): EligibilitySnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, ["tier", "route_kind", "status", "checked_at_ms", "profile_version"])
  ) {
    return null;
  }
  const tier = string(raw.tier);
  const routeKind = string(raw.route_kind);
  const status = string(raw.status);
  const checkedAtMs = integer(raw.checked_at_ms);
  const profileVersion = version(raw.profile_version);
  if (
    tier === null ||
    routeKind === null ||
    status === null ||
    checkedAtMs === null ||
    profileVersion === null ||
    !TIERS.has(tier as ModelTier) ||
    !KINDS.has(routeKind as RouteKind) ||
    !ELIGIBILITY.has(status as EligibilityStatus)
  ) {
    return null;
  }
  return {
    tier: tier as ModelTier,
    route_kind: routeKind as RouteKind,
    status: status as EligibilityStatus,
    checked_at_ms: checkedAtMs,
    profile_version: profileVersion,
  };
}

function activity(value: unknown): RouteActivitySnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "route_key",
      "child_thread_id",
      "subtask_id",
      "route_kind",
      "phase",
      "is_reviewer",
      "escalation_count",
      "started_at_ms",
      "updated_at_ms",
    ])
  ) {
    return null;
  }
  const routeKey = uuid(raw.route_key);
  const childThreadId = uuid(raw.child_thread_id);
  const subtaskId = uuid(raw.subtask_id);
  const routeKind = string(raw.route_kind);
  const phase = string(raw.phase);
  const escalationCount = integer(raw.escalation_count);
  const startedAtMs = integer(raw.started_at_ms);
  const updatedAtMs = integer(raw.updated_at_ms);
  if (
    routeKey === null ||
    childThreadId === null ||
    subtaskId === null ||
    routeKind === null ||
    phase === null ||
    escalationCount === null ||
    escalationCount > 2 ||
    startedAtMs === null ||
    updatedAtMs === null ||
    typeof raw.is_reviewer !== "boolean" ||
    !KINDS.has(routeKind as RouteKind) ||
    !PHASES.has(phase as RoutePhase)
  ) {
    return null;
  }
  return {
    route_key: routeKey,
    child_thread_id: childThreadId,
    subtask_id: subtaskId,
    route_kind: routeKind as RouteKind,
    phase: phase as RoutePhase,
    is_reviewer: raw.is_reviewer,
    escalation_count: escalationCount,
    started_at_ms: startedAtMs,
    updated_at_ms: updatedAtMs,
  };
}

function parsedArray<T>(value: unknown, parser: (entry: unknown) => T | null): T[] | null {
  if (!Array.isArray(value)) return null;
  const parsed = value.map(parser);
  return parsed.every((entry): entry is T => entry !== null) ? parsed : null;
}

export function toRoutingSnapshot(value: unknown): RoutingSnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, ["schema_version", "profile_version", "routes", "eligibility", "activity"])
  ) {
    return null;
  }
  const schemaVersion = integer(raw.schema_version);
  const profileVersion = version(raw.profile_version);
  const routes = parsedArray(raw.routes, route);
  const eligibilityRecords = parsedArray(raw.eligibility, eligibility);
  const activityRecords = parsedArray(raw.activity, activity);
  if (
    schemaVersion !== 1 ||
    profileVersion === null ||
    routes === null ||
    eligibilityRecords === null ||
    activityRecords === null
  ) {
    return null;
  }
  return {
    schema_version: schemaVersion,
    profile_version: profileVersion,
    routes,
    eligibility: eligibilityRecords,
    activity: activityRecords,
  };
}

export const routingApi = { toSnapshot: toRoutingSnapshot };
