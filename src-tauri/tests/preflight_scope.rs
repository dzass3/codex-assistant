use codex_assistant_lib::{
    monitor::model::{
        AgentObservation, AgentStatus, HealthEntry, ModelSource, MonitorSnapshot, SourceHealth,
        SummaryCounts,
    },
    preflight::{
        project_monitor, EligibilityKey, PreflightCoordinator, PreflightPhase, PreflightReason,
        PreflightSignal,
    },
    routing::{
        state::{RoutingRuntime, RoutingStateStore},
        EligibilityReasonCode, EligibilityStatus, ModelTier, RouteKind,
    },
};
use tempfile::tempdir;
use uuid::Uuid;

const CODEX_VERSION: &str = "1.2.3";
const PROFILE_VERSION: &str = "routing-v1";
const LUNA: &str = "gpt-5.6-luna";

fn key(route_kind: RouteKind, depth: u8) -> EligibilityKey {
    EligibilityKey {
        codex_package_version: CODEX_VERSION.into(),
        profile_version: PROFILE_VERSION.into(),
        requested_model: LUNA.into(),
        route_kind,
        depth,
    }
}

fn agent(
    thread_id: Uuid,
    parent_thread_id: Option<Uuid>,
    requested_model: Option<&str>,
    effective_model: Option<&str>,
    status: AgentStatus,
    depth: u32,
    started_at_ms: i64,
) -> AgentObservation {
    AgentObservation {
        thread_id: thread_id.to_string(),
        parent_thread_id: parent_thread_id.map(|id| id.to_string()),
        agent_path: Some("/root/PRIVATE_TASK_NAME".into()),
        display_name: "CANARY PRIVATE TASK".into(),
        role: Some("PRIVATE ROLE".into()),
        project: Some("PRIVATE PROJECT".into()),
        originator: Some("PRIVATE ORIGIN".into()),
        requested_model: requested_model.map(str::to_owned),
        effective_model: effective_model.map(str::to_owned),
        model_source: if effective_model.is_some() {
            ModelSource::TurnContext
        } else {
            ModelSource::RequestedOnly
        },
        reasoning_effort: Some("medium".into()),
        status,
        model_drift: requested_model
            .zip(effective_model)
            .is_some_and(|(left, right)| left != right),
        is_subagent: parent_thread_id.is_some(),
        depth,
        started_at_ms: Some(started_at_ms),
        updated_at_ms: Some(started_at_ms + 1),
        freshness_ms: Some(1),
    }
}

fn snapshot(agents: Vec<AgentObservation>) -> MonitorSnapshot {
    MonitorSnapshot {
        generated_at_ms: 200,
        agents,
        counts: SummaryCounts::default(),
        health: SourceHealth {
            state_database: HealthEntry::healthy("ready", 200),
            rollout_observer: HealthEntry::healthy("ready", 200),
        },
    }
}

#[test]
fn visible_directive_and_submission_advance_one_exact_key_without_task_content() {
    let root = Uuid::new_v4();
    let direct = key(RouteKind::Direct, 1);
    let nested = key(RouteKind::Nested, 2);
    let mut coordinator = PreflightCoordinator::new();
    let attempt_id = coordinator
        .begin(direct.clone(), root, root, 100, 1_000)
        .expect("begin direct preflight");
    coordinator
        .begin(nested.clone(), root, Uuid::new_v4(), 100, 1_000)
        .expect("begin nested preflight");

    assert_eq!(
        coordinator.get(&direct).unwrap().attempt.phase,
        PreflightPhase::AwaitingVisibleCommand
    );
    assert_eq!(
        coordinator.get(&nested).unwrap().attempt.phase,
        PreflightPhase::AwaitingVisibleCommand
    );
    let directive = coordinator.directive(&direct).expect("directive");
    assert_eq!(directive.attempt_id, attempt_id);
    assert!(directive.text.contains("codex_assistant_luna"));
    assert!(directive.text.contains("fork_turns=\"none\""));
    assert!(directive.text.contains("visible native child"));
    for forbidden in ["CANARY", "PRIVATE", "prompt", "reasoning", "tool_output"] {
        assert!(!directive.text.contains(forbidden));
    }

    coordinator
        .mark_visible_command_submitted(&direct)
        .expect("submitted");
    assert_eq!(
        coordinator.get(&direct).unwrap().attempt.phase,
        PreflightPhase::AwaitingNativeChild
    );
    assert_eq!(
        coordinator.get(&nested).unwrap().attempt.phase,
        PreflightPhase::AwaitingVisibleCommand
    );
    assert!(coordinator.begin(direct, root, root, 200, 1_000).is_err());
}

#[test]
fn monitor_projection_is_metadata_only_and_coordinator_reaches_eligible() {
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let direct = key(RouteKind::Direct, 1);
    let mut coordinator = PreflightCoordinator::new();
    coordinator
        .begin(direct.clone(), root, root, 100, 1_000)
        .unwrap();
    coordinator.mark_visible_command_submitted(&direct).unwrap();
    let monitor = snapshot(vec![
        agent(
            root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        agent(
            child,
            Some(root),
            Some(LUNA),
            Some(LUNA),
            AgentStatus::Idle,
            1,
            120,
        ),
    ]);

    let projected = project_monitor(&monitor);
    let serialized = serde_json::to_string(&projected).expect("projection JSON");
    for forbidden in [
        "CANARY",
        "PRIVATE",
        "agent_path",
        "display_name",
        "project",
        "originator",
    ] {
        assert!(!serialized.contains(forbidden));
    }
    let outcome = coordinator
        .reconcile_monitor(
            &direct,
            &monitor,
            CODEX_VERSION,
            PROFILE_VERSION,
            200,
            PreflightSignal::None,
        )
        .expect("reconcile");
    assert_eq!(outcome.phase, PreflightPhase::Eligible);
    assert_eq!(outcome.child_thread_id, Some(child));
    assert!(coordinator.is_complete());
    let eligibility = coordinator
        .eligibility_record(&direct, 200)
        .expect("scoped eligibility record");
    assert_eq!(eligibility.tier, ModelTier::Luna);
    assert_eq!(eligibility.route_kind, RouteKind::Direct);
    assert_eq!(eligibility.status, EligibilityStatus::Eligible);
    assert_eq!(eligibility.codex_package_version, CODEX_VERSION);
    assert_eq!(eligibility.profile_version, PROFILE_VERSION);
    assert_eq!(eligibility.requested_model, LUNA);
    assert_eq!(eligibility.depth, 1);
    assert_eq!(eligibility.reason, None);
    let directory = tempdir().expect("routing state directory");
    let runtime = RoutingRuntime::load(
        RoutingStateStore::in_directory(directory.path()).expect("routing state store"),
    )
    .expect("routing runtime");
    coordinator
        .persist_eligibility(&direct, 200, &runtime)
        .expect("persist eligibility");
    assert_eq!(runtime.snapshot().eligibility, [eligibility]);
    let still_eligible = coordinator
        .reconcile_monitor(
            &direct,
            &snapshot(Vec::new()),
            CODEX_VERSION,
            PROFILE_VERSION,
            300,
            PreflightSignal::None,
        )
        .expect("terminal outcome is stable");
    assert_eq!(still_eligible.phase, PreflightPhase::Eligible);
}

#[test]
fn direct_check_ignores_same_model_children_from_a_different_depth_and_parent() {
    let root = Uuid::new_v4();
    let terra_parent = Uuid::new_v4();
    let nested_spark = Uuid::new_v4();
    let direct_spark = Uuid::new_v4();
    let mut direct = key(RouteKind::Direct, 1);
    direct.requested_model = "gpt-5.3-codex-spark".into();
    let mut coordinator = PreflightCoordinator::new();
    coordinator
        .begin(direct.clone(), root, root, 100, 1_000)
        .unwrap();
    coordinator.mark_visible_command_submitted(&direct).unwrap();
    let monitor = snapshot(vec![
        agent(
            root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        agent(
            terra_parent,
            Some(root),
            Some("gpt-5.6-terra"),
            Some("gpt-5.6-terra"),
            AgentStatus::Idle,
            1,
            105,
        ),
        agent(
            nested_spark,
            Some(terra_parent),
            Some("gpt-5.3-codex-spark"),
            Some("gpt-5.3-codex-spark"),
            AgentStatus::Idle,
            2,
            110,
        ),
        agent(
            direct_spark,
            Some(root),
            Some("gpt-5.3-codex-spark"),
            Some("gpt-5.3-codex-spark"),
            AgentStatus::Idle,
            1,
            120,
        ),
    ]);

    let outcome = coordinator
        .reconcile_monitor(
            &direct,
            &monitor,
            CODEX_VERSION,
            PROFILE_VERSION,
            200,
            PreflightSignal::None,
        )
        .unwrap();

    assert_eq!(outcome.phase, PreflightPhase::Eligible);
    assert_eq!(outcome.child_thread_id, Some(direct_spark));
}

#[test]
fn invalidation_changes_only_keys_with_stale_host_or_profile_versions() {
    let root = Uuid::new_v4();
    let old_host = key(RouteKind::Direct, 1);
    let mut current_host = key(RouteKind::Nested, 2);
    current_host.codex_package_version = "1.2.4".into();
    let mut current_profile = key(RouteKind::Direct, 1);
    current_profile.codex_package_version = "1.2.4".into();
    current_profile.profile_version = "routing-v2".into();
    current_profile.requested_model = "gpt-5.6-terra".into();
    let mut coordinator = PreflightCoordinator::new();
    coordinator
        .begin(old_host.clone(), root, root, 100, 1_000)
        .unwrap();
    coordinator
        .begin(current_host.clone(), root, Uuid::new_v4(), 100, 1_000)
        .unwrap();
    coordinator
        .begin(current_profile.clone(), root, root, 100, 1_000)
        .unwrap();

    let changed = coordinator.invalidate_versions("1.2.4", "routing-v1");

    assert_eq!(changed, 2);
    assert_eq!(
        coordinator.get(&old_host).unwrap().outcome.reason,
        Some(PreflightReason::HostVersionChanged)
    );
    assert_eq!(
        coordinator.get(&current_host).unwrap().attempt.phase,
        PreflightPhase::AwaitingVisibleCommand
    );
    assert_eq!(
        coordinator.get(&current_profile).unwrap().outcome.reason,
        Some(PreflightReason::ProfileVersionChanged)
    );
    let stale = coordinator
        .eligibility_record(&old_host, 300)
        .expect("stale eligibility record");
    assert_eq!(stale.status, EligibilityStatus::Stale);
    assert_eq!(
        stale.reason,
        Some(EligibilityReasonCode::HostVersionChanged)
    );
}

#[test]
fn malformed_monitor_ids_are_dropped_instead_of_becoming_detached_candidates() {
    let mut malformed = agent(
        Uuid::new_v4(),
        None,
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Idle,
        1,
        120,
    );
    malformed.thread_id = "not-a-uuid".into();
    malformed.parent_thread_id = Some("also-not-a-uuid".into());
    assert!(project_monitor(&snapshot(vec![malformed])).is_empty());
}

#[test]
fn nested_preflight_accepts_only_lower_tiers_below_a_distinct_parent() {
    let root = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let mut coordinator = PreflightCoordinator::new();
    let mut nested_terra = key(RouteKind::Nested, 2);
    nested_terra.requested_model = "gpt-5.6-terra".into();
    assert!(coordinator
        .begin(nested_terra, root, parent, 100, 1_000)
        .is_err());
    assert!(coordinator
        .begin(key(RouteKind::Nested, 2), root, root, 100, 1_000)
        .is_err());
    assert!(coordinator
        .begin(key(RouteKind::Nested, 2), root, parent, 100, 1_000)
        .is_ok());
}
