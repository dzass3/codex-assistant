use codex_assistant_lib::routing::{
    policy::{decide_route, evaluate_budget, RouteBudget},
    Capability, ComplexityBand, ModelTier, RiskBand, RouteAction, RouteDecision, RouteKind,
    RoutePolicyInput, RouteReasonCode, UserOverride,
};

fn input(
    complexity: ComplexityBand,
    risk: RiskBand,
    eligible_tiers: Vec<ModelTier>,
) -> RoutePolicyInput {
    RoutePolicyInput {
        complexity,
        risk,
        required_capabilities: Vec::new(),
        eligible_tiers,
        estimated_spawn_overhead_ms: 0,
        user_override: None,
    }
}

fn delegated_tier(decision: RouteDecision) -> ModelTier {
    assert_eq!(decision.action, RouteAction::Delegate);
    decision.selected_tier.expect("delegated tier")
}

#[test]
fn quality_first_matrix_selects_only_eligible_lowest_safe_tier() {
    assert_eq!(
        delegated_tier(decide_route(input(
            ComplexityBand::Mechanical,
            RiskBand::Low,
            vec![ModelTier::Spark, ModelTier::Luna, ModelTier::Terra],
        ))),
        ModelTier::Spark
    );
    assert_eq!(
        delegated_tier(decide_route(input(
            ComplexityBand::Mechanical,
            RiskBand::Low,
            vec![ModelTier::Luna, ModelTier::Terra],
        ))),
        ModelTier::Luna,
        "Spark is unavailable until preflight proves it eligible"
    );
    assert_eq!(
        delegated_tier(decide_route(input(
            ComplexityBand::Bounded,
            RiskBand::Low,
            vec![ModelTier::Spark, ModelTier::Luna, ModelTier::Terra],
        ))),
        ModelTier::Luna
    );
    assert_eq!(
        delegated_tier(decide_route(input(
            ComplexityBand::CrossLayer,
            RiskBand::Meaningful,
            vec![ModelTier::Spark, ModelTier::Luna, ModelTier::Terra],
        ))),
        ModelTier::Terra
    );
}

#[test]
fn high_risk_and_restricted_capabilities_stay_with_sol_or_root() {
    for capability in [
        Capability::Security,
        Capability::Deployment,
        Capability::Credentials,
        Capability::Destructive,
        Capability::Architecture,
        Capability::Ambiguous,
    ] {
        let mut request = input(
            ComplexityBand::Mechanical,
            RiskBand::Low,
            vec![ModelTier::Spark, ModelTier::Luna, ModelTier::Terra],
        );
        request.required_capabilities = vec![capability];
        let decision = decide_route(request);
        assert_eq!(decision.action, RouteAction::KeepInParent);
        assert!(decision
            .reason_codes
            .contains(&RouteReasonCode::SolFloorRequired));
    }

    let decision = decide_route(input(
        ComplexityBand::Architectural,
        RiskBand::High,
        vec![ModelTier::Sol],
    ));
    assert_eq!(delegated_tier(decision.clone()), ModelTier::Sol);
    assert!(decision
        .reason_codes
        .contains(&RouteReasonCode::ArchitecturalWork));
    assert!(decision
        .reason_codes
        .contains(&RouteReasonCode::HighRiskWork));

    let decision = decide_route(input(
        ComplexityBand::Mechanical,
        RiskBand::Restricted,
        vec![ModelTier::Sol],
    ));
    assert_eq!(delegated_tier(decision.clone()), ModelTier::Sol);
    assert!(decision
        .reason_codes
        .contains(&RouteReasonCode::RestrictedRiskWork));
}

#[test]
fn overhead_and_user_overrides_cannot_weaken_quality_floors() {
    let mut trivial = input(
        ComplexityBand::Mechanical,
        RiskBand::Low,
        vec![ModelTier::Spark],
    );
    trivial.estimated_spawn_overhead_ms = 2_000;
    let decision = decide_route(trivial);
    assert_eq!(decision.action, RouteAction::KeepInParent);
    assert!(decision
        .reason_codes
        .contains(&RouteReasonCode::SpawnOverheadTooHigh));

    let mut no_delegate = input(
        ComplexityBand::CrossLayer,
        RiskBand::Low,
        vec![ModelTier::Terra],
    );
    no_delegate.user_override = Some(UserOverride::DoNotDelegate);
    assert_eq!(decide_route(no_delegate).action, RouteAction::KeepInParent);

    let mut unsafe_override = input(
        ComplexityBand::Architectural,
        RiskBand::High,
        vec![ModelTier::Spark, ModelTier::Sol],
    );
    unsafe_override.user_override = Some(UserOverride::UseTier(ModelTier::Spark));
    assert_eq!(
        delegated_tier(decide_route(unsafe_override)),
        ModelTier::Sol
    );

    let mut unavailable_override = input(
        ComplexityBand::Mechanical,
        RiskBand::Low,
        vec![ModelTier::Terra],
    );
    unavailable_override.user_override = Some(UserOverride::UseTier(ModelTier::Luna));
    assert_eq!(
        decide_route(unavailable_override).action,
        RouteAction::KeepInParent
    );
}

#[test]
fn budget_limits_bound_fan_out_escalation_and_reviewer_recursion() {
    assert_eq!(
        evaluate_budget(&RouteBudget {
            active_routed_children: 3,
            active_nested_children: 0,
            automatic_escalations: 0,
            route_kind: RouteKind::Direct,
        }),
        Err(RouteReasonCode::ActiveChildLimitReached)
    );
    assert_eq!(
        evaluate_budget(&RouteBudget {
            active_routed_children: 1,
            active_nested_children: 1,
            automatic_escalations: 0,
            route_kind: RouteKind::Nested,
        }),
        Err(RouteReasonCode::NestedChildLimitReached)
    );
    assert_eq!(
        evaluate_budget(&RouteBudget {
            active_routed_children: 0,
            active_nested_children: 0,
            automatic_escalations: 2,
            route_kind: RouteKind::Direct,
        }),
        Ok(())
    );
    assert_eq!(
        evaluate_budget(&RouteBudget {
            active_routed_children: 0,
            active_nested_children: 0,
            automatic_escalations: 3,
            route_kind: RouteKind::Direct,
        }),
        Err(RouteReasonCode::EscalationLimitReached)
    );
}
