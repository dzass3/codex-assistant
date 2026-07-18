export type ModelTier = "spark" | "luna" | "terra" | "sol";
export type RouteKind = "direct" | "nested";
export type EligibilityStatus = "unknown" | "verifying" | "eligible" | "unavailable" | "stale";
export type RoutePhase =
  | "off"
  | "enabled"
  | "classifying"
  | "implementing"
  | "reviewing"
  | "completed"
  | "degraded";
export type RouteReasonCode =
  | "mechanical-work"
  | "bounded-work"
  | "cross-layer-work"
  | "architectural-work"
  | "high-risk-work"
  | "restricted-risk-work"
  | "sol-floor-required"
  | "spawn-overhead-too-high"
  | "do-not-delegate"
  | "override-below-floor"
  | "no-eligible-tier"
  | "active-child-limit-reached"
  | "nested-child-limit-reached"
  | "escalation-limit-reached"
  | "reviewer-recursion-forbidden"
  | "state-persistence-failed";

export interface RootRouteSnapshot {
  route_key: string;
  conversation_id: string;
  enabled: boolean;
  phase: RoutePhase;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface EligibilitySnapshot {
  tier: ModelTier;
  route_kind: RouteKind;
  status: EligibilityStatus;
  checked_at_ms: number;
  profile_version: string;
}

export interface RouteActivitySnapshot {
  route_key: string;
  child_thread_id: string;
  subtask_id: string;
  route_kind: RouteKind;
  phase: RoutePhase;
  is_reviewer: boolean;
  reviewer_parent: boolean;
  escalation_count: number;
  selected_tier: ModelTier;
  requested_tier: ModelTier | null;
  effective_tier: ModelTier | null;
  reason_codes: RouteReasonCode[];
  started_at_ms: number;
  updated_at_ms: number;
}

export interface RoutingSnapshot {
  schema_version: number;
  profile_version: string;
  routes: RootRouteSnapshot[];
  eligibility: EligibilitySnapshot[];
  activity: RouteActivitySnapshot[];
}
