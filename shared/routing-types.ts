export type ModelTier = "spark" | "luna" | "terra" | "sol";
export type RouteKind = "direct" | "nested";
export type EligibilityStatus = "unknown" | "verifying" | "eligible" | "unavailable" | "stale";
export type EligibilityReasonCode =
  | "awaiting-visible-command"
  | "awaiting-native-child"
  | "awaiting-effective-model"
  | "child-still-running"
  | "effective-model-mismatch"
  | "native-profile-rejected"
  | "lineage-ambiguous"
  | "detached-process"
  | "unrelated-root"
  | "missing-parent"
  | "parent-not-verified-terra"
  | "timeout"
  | "host-version-changed"
  | "profile-version-changed";
export type QualityOutcome = "passed" | "failed" | "degraded";
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
  | "nested-delegation-forbidden"
  | "previous-attempt-still-active"
  | "unknown-route"
  | "parent-lineage-mismatch"
  | "child-already-recorded"
  | "unknown-child"
  | "terminal-child-reactivation"
  | "eligibility-unavailable"
  | "quality-already-recorded"
  | "escalation-count-mismatch"
  | "retry-limit-reached"
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
  codex_package_version: string;
  requested_model: string;
  depth: number;
  reason: EligibilityReasonCode | null;
}

export interface RouteActivitySnapshot {
  route_key: string;
  child_thread_id: string;
  subtask_id: string;
  route_kind: RouteKind;
  phase: RoutePhase;
  is_reviewer: boolean;
  parent_thread_id: string;
  escalation_count: number;
  selected_tier: ModelTier;
  requested_tier: ModelTier | null;
  effective_tier: ModelTier | null;
  reason_codes: RouteReasonCode[];
  started_at_ms: number;
  updated_at_ms: number;
}

export interface QualitySnapshot {
  route_key: string;
  child_thread_id: string;
  outcome: QualityOutcome;
  reviewer_tier: ModelTier | null;
  retry_count: number;
  escalation_count: number;
  recorded_at_ms: number;
}

export interface RoutingSnapshot {
  schema_version: number;
  profile_version: string;
  routes: RootRouteSnapshot[];
  eligibility: EligibilitySnapshot[];
  activity: RouteActivitySnapshot[];
  quality: QualitySnapshot[];
}

export type RoutingInstallationStatus =
  | "uninstalled"
  | "installed"
  | "restart-required"
  | "conflict";
export type RoutingRestartStatus = "not-required" | "required" | "blocked-active-child";
export type RoutingPreflightStatus = "not-started" | "running" | "complete" | "degraded";
export type RoutingCdpStatus = "inactive" | "ready" | "degraded";
export type RoutingSetupReasonCode =
  | "active-child"
  | "config-conflict"
  | "preflight-required"
  | "unsupported-host"
  | "cdp-unavailable"
  | "routing-runtime-unavailable"
  | "confirmation-required"
  | "confirmation-expired"
  | "impact-changed"
  | "operation-conflict"
  | "identity-changed"
  | "graceful-stop-unsupported"
  | "termination-failed"
  | "old-tree-still-running"
  | "launch-failed"
  | "cdp-verification-failed"
  | "dom-incompatible"
  | "partial-apply-failed"
  | "terminal-partial-failure";
export type RoutingConfigChange =
  | "agents.max_depth"
  | "agents.codex_assistant_spark"
  | "agents.codex_assistant_luna"
  | "agents.codex_assistant_terra"
  | "agents.codex_assistant_sol"
  | "mcp_servers.codex_assistant_routing"
  | "skill.codex-assistant-routing";

export interface RoutingSetupSnapshot {
  installation_status: RoutingInstallationStatus;
  restart_status: RoutingRestartStatus;
  preflight_status: RoutingPreflightStatus;
  cdp_status: RoutingCdpStatus;
  backup_label: string | null;
  config_changes: RoutingConfigChange[];
  reason_codes: RoutingSetupReasonCode[];
}

export type RoutingActivationStatus =
  | "off"
  | "pending-open"
  | "pending-next-turn"
  | "enabled"
  | "needs-repair";

export interface RootRoutingControlSnapshot {
  conversation_id: string;
  status: RoutingActivationStatus;
}

export interface RoutingUiSnapshot {
  contract_version: 2;
  setup: RoutingSetupSnapshot;
  routing: RoutingSnapshot;
  controls: RootRoutingControlSnapshot[];
}

export type RoutingOperationStatus = "applied" | "noop" | "blocked" | "failed";
export type RestartMode = "safe" | "force-after-grace";
export type RestartIntent = "routing-restart" | "theme-session" | "activate-theme";

export interface ForceRestartImpact {
  confirmation_ticket: string;
  intent: RestartIntent;
  active_native_children: number;
  grace_period_ms: 5000;
  expires_at_ms: number;
}

export interface RoutingOperationReceipt {
  operation_id: string;
  status: RoutingOperationStatus;
  reason_codes: RoutingSetupReasonCode[];
  restart_required: boolean;
}
