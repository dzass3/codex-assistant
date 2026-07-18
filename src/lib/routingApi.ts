import type {
  EligibilitySnapshot,
  EligibilityStatus,
  ModelTier,
  QualityOutcome,
  QualitySnapshot,
  RootRouteSnapshot,
  RouteActivitySnapshot,
  RouteKind,
  RoutePhase,
  RouteReasonCode,
  RoutingSnapshot,
} from "../../shared/routing-types";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const PROFILE_VERSION = "routing-v1";
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
const QUALITY_OUTCOMES = new Set<QualityOutcome>(["passed", "failed", "degraded"]);
const REASONS = new Set<RouteReasonCode>([
  "mechanical-work",
  "bounded-work",
  "cross-layer-work",
  "architectural-work",
  "high-risk-work",
  "restricted-risk-work",
  "sol-floor-required",
  "spawn-overhead-too-high",
  "do-not-delegate",
  "override-below-floor",
  "no-eligible-tier",
  "active-child-limit-reached",
  "nested-child-limit-reached",
  "escalation-limit-reached",
  "reviewer-recursion-forbidden",
  "nested-delegation-forbidden",
  "previous-attempt-still-active",
  "unknown-route",
  "parent-lineage-mismatch",
  "child-already-recorded",
  "unknown-child",
  "terminal-child-reactivation",
  "eligibility-unavailable",
  "quality-already-recorded",
  "escalation-count-mismatch",
  "retry-limit-reached",
  "state-persistence-failed",
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
  return candidate === PROFILE_VERSION ? candidate : null;
}

function uuid(value: unknown): string | null {
  const candidate = string(value);
  return candidate !== null &&
    UUID.test(candidate) &&
    !/^0{8}-0{4}-0{4}-0{4}-0{12}$/i.test(candidate)
    ? candidate
    : null;
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
      "parent_thread_id",
      "escalation_count",
      "selected_tier",
      "requested_tier",
      "effective_tier",
      "reason_codes",
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
  const parentThreadId = uuid(raw.parent_thread_id);
  const escalationCount = integer(raw.escalation_count);
  const selectedTier = string(raw.selected_tier);
  const requestedTier = raw.requested_tier === null ? null : string(raw.requested_tier);
  const effectiveTier = raw.effective_tier === null ? null : string(raw.effective_tier);
  const reasonCodes = Array.isArray(raw.reason_codes) ? raw.reason_codes.map(string) : null;
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
    parentThreadId === null ||
    selectedTier === null ||
    !TIERS.has(selectedTier as ModelTier) ||
    (requestedTier !== null && !TIERS.has(requestedTier as ModelTier)) ||
    (effectiveTier !== null && !TIERS.has(effectiveTier as ModelTier)) ||
    reasonCodes === null ||
    reasonCodes.length === 0 ||
    reasonCodes.some((reason) => reason === null || !REASONS.has(reason as RouteReasonCode)) ||
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
    parent_thread_id: parentThreadId,
    escalation_count: escalationCount,
    selected_tier: selectedTier as ModelTier,
    requested_tier: requestedTier as ModelTier | null,
    effective_tier: effectiveTier as ModelTier | null,
    reason_codes: reasonCodes as RouteReasonCode[],
    started_at_ms: startedAtMs,
    updated_at_ms: updatedAtMs,
  };
}

function quality(value: unknown): QualitySnapshot | null {
  const raw = record(value);
  if (
    raw === null ||
    !exactKeys(raw, [
      "route_key",
      "child_thread_id",
      "outcome",
      "reviewer_tier",
      "retry_count",
      "escalation_count",
      "recorded_at_ms",
    ])
  ) {
    return null;
  }
  const routeKey = uuid(raw.route_key);
  const childThreadId = uuid(raw.child_thread_id);
  const outcome = string(raw.outcome);
  const reviewerTier = raw.reviewer_tier === null ? null : string(raw.reviewer_tier);
  const retryCount = integer(raw.retry_count);
  const escalationCount = integer(raw.escalation_count);
  const recordedAtMs = integer(raw.recorded_at_ms);
  if (
    routeKey === null ||
    childThreadId === null ||
    outcome === null ||
    !QUALITY_OUTCOMES.has(outcome as QualityOutcome) ||
    (reviewerTier !== null && !TIERS.has(reviewerTier as ModelTier)) ||
    retryCount === null ||
    retryCount > 2 ||
    escalationCount === null ||
    escalationCount > 2 ||
    recordedAtMs === null
  ) {
    return null;
  }
  return {
    route_key: routeKey,
    child_thread_id: childThreadId,
    outcome: outcome as QualityOutcome,
    reviewer_tier: reviewerTier as ModelTier | null,
    retry_count: retryCount,
    escalation_count: escalationCount,
    recorded_at_ms: recordedAtMs,
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
    !exactKeys(raw, [
      "schema_version",
      "profile_version",
      "routes",
      "eligibility",
      "activity",
      "quality",
    ])
  ) {
    return null;
  }
  const schemaVersion = integer(raw.schema_version);
  const profileVersion = version(raw.profile_version);
  const routes = parsedArray(raw.routes, route);
  const eligibilityRecords = parsedArray(raw.eligibility, eligibility);
  const activityRecords = parsedArray(raw.activity, activity);
  const qualityRecords = parsedArray(raw.quality, quality);
  if (
    schemaVersion !== 1 ||
    profileVersion === null ||
    routes === null ||
    eligibilityRecords === null ||
    activityRecords === null ||
    qualityRecords === null
  ) {
    return null;
  }
  const snapshot = {
    schema_version: schemaVersion,
    profile_version: profileVersion,
    routes,
    eligibility: eligibilityRecords,
    activity: activityRecords,
    quality: qualityRecords,
  };
  return validRelationships(snapshot) ? snapshot : null;
}

function validRelationships(snapshot: RoutingSnapshot): boolean {
  const routes = new Map<string, RootRouteSnapshot>();
  const conversations = new Set<string>();
  for (const rootRecord of snapshot.routes) {
    if (routes.has(rootRecord.route_key) || conversations.has(rootRecord.conversation_id)) {
      return false;
    }
    routes.set(rootRecord.route_key, rootRecord);
    conversations.add(rootRecord.conversation_id);
  }
  const children = new Map<string, RouteActivitySnapshot>();
  for (const activityRecord of snapshot.activity) {
    if (children.has(activityRecord.child_thread_id) || !routes.has(activityRecord.route_key)) {
      return false;
    }
    children.set(activityRecord.child_thread_id, activityRecord);
  }
  const activeByRoute = new Map<string, RouteActivitySnapshot[]>();
  const implementations = new Map<string, RouteActivitySnapshot[]>();
  for (const activityRecord of snapshot.activity) {
    const rootRecord = routes.get(activityRecord.route_key);
    if (!rootRecord) return false;
    if (activityRecord.route_kind === "direct") {
      if (activityRecord.parent_thread_id !== rootRecord.conversation_id) return false;
    } else {
      const parent = children.get(activityRecord.parent_thread_id);
      if (
        !parent ||
        parent.route_key !== activityRecord.route_key ||
        parent.route_kind !== "direct" ||
        parent.is_reviewer ||
        parent.selected_tier !== "terra" ||
        (activityRecord.selected_tier !== "spark" && activityRecord.selected_tier !== "luna")
      ) {
        return false;
      }
    }
    if (
      activityRecord.phase === "classifying" ||
      activityRecord.phase === "implementing" ||
      activityRecord.phase === "reviewing"
    ) {
      const active = activeByRoute.get(activityRecord.route_key) ?? [];
      active.push(activityRecord);
      activeByRoute.set(activityRecord.route_key, active);
    }
    const key = `${activityRecord.route_key}:${activityRecord.subtask_id}`;
    if (!activityRecord.is_reviewer) {
      const attempts = implementations.get(key) ?? [];
      attempts.push(activityRecord);
      implementations.set(key, attempts);
    }
  }
  for (const active of activeByRoute.values()) {
    if (
      active.length > 3 ||
      active.filter((activeRecord) => activeRecord.route_kind === "nested").length > 1
    )
      return false;
  }
  for (const [key, attempts] of implementations) {
    const counts = attempts
      .map((attempt) => attempt.escalation_count)
      .toSorted((left, right) => left - right);
    if (counts.length > 3 || counts.some((count, index) => count !== index)) return false;
    const activeAttempts = attempts.filter(
      (attempt) =>
        attempt.phase === "classifying" ||
        attempt.phase === "implementing" ||
        attempt.phase === "reviewing",
    );
    if (
      activeAttempts.length > 1 ||
      (activeAttempts.length === 1 &&
        activeAttempts[0].escalation_count !== counts[counts.length - 1])
    ) {
      return false;
    }
    const reviewers = snapshot.activity.filter(
      (reviewRecord) =>
        reviewRecord.is_reviewer && `${reviewRecord.route_key}:${reviewRecord.subtask_id}` === key,
    );
    if (reviewers.some((reviewer) => !counts.includes(reviewer.escalation_count))) return false;
  }
  const qualityChildren = new Set<string>();
  for (const qualityRecord of snapshot.quality) {
    if (qualityChildren.has(qualityRecord.child_thread_id)) return false;
    qualityChildren.add(qualityRecord.child_thread_id);
    const child = children.get(qualityRecord.child_thread_id);
    if (
      !child ||
      child.route_key !== qualityRecord.route_key ||
      child.escalation_count !== qualityRecord.escalation_count ||
      child.updated_at_ms !== qualityRecord.recorded_at_ms ||
      (qualityRecord.outcome === "passed"
        ? child.phase !== "completed"
        : child.phase !== "degraded")
    ) {
      return false;
    }
  }
  return snapshot.activity
    .filter((reviewRecord) => reviewRecord.is_reviewer)
    .every((reviewer) => implementations.has(`${reviewer.route_key}:${reviewer.subtask_id}`));
}

export const routingApi = { toSnapshot: toRoutingSnapshot };
