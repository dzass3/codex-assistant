use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelTier {
    Spark,
    Luna,
    Terra,
    Sol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteKind {
    Direct,
    Nested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexityBand {
    Mechanical,
    Bounded,
    CrossLayer,
    Architectural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskBand {
    Low,
    Meaningful,
    High,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    MechanicalChange,
    BoundedAnalysis,
    CrossLayerIntegration,
    Architecture,
    Security,
    Deployment,
    Credentials,
    Destructive,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EligibilityStatus {
    Unknown,
    Verifying,
    Eligible,
    Unavailable,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutePhase {
    Off,
    Enabled,
    Classifying,
    Implementing,
    Reviewing,
    Completed,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QualityOutcome {
    Passed,
    Failed,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteAction {
    KeepInParent,
    Delegate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserOverride {
    DoNotDelegate,
    UseTier(ModelTier),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteReasonCode {
    MechanicalWork,
    BoundedWork,
    CrossLayerWork,
    ArchitecturalWork,
    HighRiskWork,
    RestrictedRiskWork,
    SolFloorRequired,
    SpawnOverheadTooHigh,
    DoNotDelegate,
    OverrideBelowFloor,
    NoEligibleTier,
    ActiveChildLimitReached,
    NestedChildLimitReached,
    EscalationLimitReached,
    ReviewerRecursionForbidden,
    NestedDelegationForbidden,
    PreviousAttemptStillActive,
    UnknownRoute,
    ParentLineageMismatch,
    ChildAlreadyRecorded,
    UnknownChild,
    TerminalChildReactivation,
    EligibilityUnavailable,
    QualityAlreadyRecorded,
    EscalationCountMismatch,
    RetryLimitReached,
    StatePersistenceFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePolicyInput {
    pub complexity: ComplexityBand,
    pub risk: RiskBand,
    pub required_capabilities: Vec<Capability>,
    pub eligible_tiers: Vec<ModelTier>,
    pub estimated_spawn_overhead_ms: u64,
    pub user_override: Option<UserOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    pub action: RouteAction,
    pub selected_tier: Option<ModelTier>,
    pub reviewer_floor: ModelTier,
    pub reason_codes: Vec<RouteReasonCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootRouteState {
    pub route_key: Uuid,
    pub conversation_id: Uuid,
    pub enabled: bool,
    pub phase: RoutePhase,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EligibilityRecord {
    pub tier: ModelTier,
    pub route_kind: RouteKind,
    pub status: EligibilityStatus,
    pub checked_at_ms: i64,
    pub profile_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteActivity {
    pub route_key: Uuid,
    pub child_thread_id: Uuid,
    pub subtask_id: Uuid,
    pub route_kind: RouteKind,
    pub phase: RoutePhase,
    pub is_reviewer: bool,
    pub parent_thread_id: Uuid,
    pub escalation_count: u8,
    pub selected_tier: ModelTier,
    pub requested_tier: Option<ModelTier>,
    pub effective_tier: Option<ModelTier>,
    pub reason_codes: Vec<RouteReasonCode>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityRecord {
    pub route_key: Uuid,
    pub child_thread_id: Uuid,
    pub outcome: QualityOutcome,
    pub reviewer_tier: Option<ModelTier>,
    pub retry_count: u8,
    pub escalation_count: u8,
    pub recorded_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingStateEnvelope {
    pub schema_version: u32,
    pub profile_version: String,
    pub routes: Vec<RootRouteState>,
    pub eligibility: Vec<EligibilityRecord>,
    pub activity: Vec<RouteActivity>,
    #[serde(default)]
    pub quality: Vec<QualityRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingSnapshot {
    pub schema_version: u32,
    pub profile_version: String,
    pub routes: Vec<RootRouteState>,
    pub eligibility: Vec<EligibilityRecord>,
    pub activity: Vec<RouteActivity>,
    pub quality: Vec<QualityRecord>,
}

impl RoutingStateEnvelope {
    pub fn empty(profile_version: impl Into<String>) -> Self {
        Self {
            schema_version: crate::routing::state::STATE_SCHEMA_VERSION,
            profile_version: profile_version.into(),
            routes: Vec::new(),
            eligibility: Vec::new(),
            activity: Vec::new(),
            quality: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> RoutingSnapshot {
        RoutingSnapshot {
            schema_version: self.schema_version,
            profile_version: self.profile_version.clone(),
            routes: self.routes.clone(),
            eligibility: self.eligibility.clone(),
            activity: self.activity.clone(),
            quality: self.quality.clone(),
        }
    }
}
