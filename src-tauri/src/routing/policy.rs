use super::{
    Capability, ComplexityBand, ModelTier, RiskBand, RouteAction, RouteDecision, RouteKind,
    RoutePolicyInput, RouteReasonCode, UserOverride,
};

pub const MAX_TRIVIAL_SPAWN_OVERHEAD_MS: u64 = 1_500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteBudget {
    pub active_routed_children: u8,
    pub active_nested_children: u8,
    pub automatic_escalations: u8,
    pub route_kind: RouteKind,
    pub reviewer_is_delegating: bool,
}

pub fn decide_route(input: RoutePolicyInput) -> RouteDecision {
    if matches!(input.user_override, Some(UserOverride::DoNotDelegate)) {
        return keep_in_parent(ModelTier::Sol, vec![RouteReasonCode::DoNotDelegate]);
    }

    let floor = required_floor(input.complexity, input.risk, &input.required_capabilities);
    let mut reasons = vec![complexity_reason(input.complexity)];
    if floor == ModelTier::Sol {
        reasons.push(RouteReasonCode::SolFloorRequired);
    }

    if floor == ModelTier::Spark
        && input.estimated_spawn_overhead_ms > MAX_TRIVIAL_SPAWN_OVERHEAD_MS
        && input.user_override.is_none()
    {
        reasons.push(RouteReasonCode::SpawnOverheadTooHigh);
        return keep_in_parent(
            reviewer_floor(ModelTier::Spark, &input.eligible_tiers),
            reasons,
        );
    }

    let requested = match input.user_override {
        Some(UserOverride::UseTier(tier))
            if tier >= floor && input.eligible_tiers.contains(&tier) =>
        {
            Some(tier)
        }
        Some(UserOverride::UseTier(tier)) if tier >= floor => {
            reasons.push(RouteReasonCode::NoEligibleTier);
            return keep_in_parent(reviewer_floor(tier, &input.eligible_tiers), reasons);
        }
        Some(UserOverride::UseTier(_)) => {
            reasons.push(RouteReasonCode::OverrideBelowFloor);
            None
        }
        _ => None,
    };
    let selected = requested
        .filter(|tier| input.eligible_tiers.contains(tier))
        .or_else(|| lowest_eligible_at_or_above(floor, &input.eligible_tiers));

    match selected {
        Some(tier) => RouteDecision {
            action: RouteAction::Delegate,
            selected_tier: Some(tier),
            reviewer_floor: reviewer_floor(tier, &input.eligible_tiers),
            reason_codes: reasons,
        },
        None => {
            reasons.push(RouteReasonCode::NoEligibleTier);
            keep_in_parent(reviewer_floor(floor, &input.eligible_tiers), reasons)
        }
    }
}

pub fn evaluate_budget(budget: &RouteBudget) -> Result<(), RouteReasonCode> {
    if budget.reviewer_is_delegating {
        return Err(RouteReasonCode::ReviewerRecursionForbidden);
    }
    if budget.active_routed_children >= 3 {
        return Err(RouteReasonCode::ActiveChildLimitReached);
    }
    if budget.route_kind == RouteKind::Nested && budget.active_nested_children >= 1 {
        return Err(RouteReasonCode::NestedChildLimitReached);
    }
    if budget.automatic_escalations >= 2 {
        return Err(RouteReasonCode::EscalationLimitReached);
    }
    Ok(())
}

fn keep_in_parent(reviewer_floor: ModelTier, reason_codes: Vec<RouteReasonCode>) -> RouteDecision {
    RouteDecision {
        action: RouteAction::KeepInParent,
        selected_tier: None,
        reviewer_floor,
        reason_codes,
    }
}

fn required_floor(
    complexity: ComplexityBand,
    risk: RiskBand,
    capabilities: &[Capability],
) -> ModelTier {
    if complexity == ComplexityBand::Architectural
        || matches!(risk, RiskBand::High | RiskBand::Restricted)
        || capabilities.iter().any(|capability| {
            matches!(
                capability,
                Capability::Architecture
                    | Capability::Security
                    | Capability::Deployment
                    | Capability::Credentials
                    | Capability::Destructive
                    | Capability::Ambiguous
            )
        })
    {
        return ModelTier::Sol;
    }
    if complexity == ComplexityBand::CrossLayer
        || capabilities.contains(&Capability::CrossLayerIntegration)
    {
        return ModelTier::Terra;
    }
    if complexity == ComplexityBand::Bounded || capabilities.contains(&Capability::BoundedAnalysis)
    {
        return ModelTier::Luna;
    }
    ModelTier::Spark
}

fn lowest_eligible_at_or_above(
    floor: ModelTier,
    eligible_tiers: &[ModelTier],
) -> Option<ModelTier> {
    [
        ModelTier::Spark,
        ModelTier::Luna,
        ModelTier::Terra,
        ModelTier::Sol,
    ]
    .into_iter()
    .find(|tier| *tier >= floor && eligible_tiers.contains(tier))
}

fn reviewer_floor(selected: ModelTier, eligible_tiers: &[ModelTier]) -> ModelTier {
    match selected {
        ModelTier::Spark if eligible_tiers.contains(&ModelTier::Luna) => ModelTier::Luna,
        ModelTier::Spark | ModelTier::Luna => ModelTier::Terra,
        ModelTier::Terra | ModelTier::Sol => ModelTier::Sol,
    }
}

fn complexity_reason(complexity: ComplexityBand) -> RouteReasonCode {
    match complexity {
        ComplexityBand::Mechanical => RouteReasonCode::MechanicalWork,
        ComplexityBand::Bounded => RouteReasonCode::BoundedWork,
        ComplexityBand::CrossLayer | ComplexityBand::Architectural => {
            RouteReasonCode::CrossLayerWork
        }
    }
}
