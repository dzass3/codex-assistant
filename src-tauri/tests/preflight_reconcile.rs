use codex_assistant_lib::{
    monitor::model::{AgentStatus, ModelSource},
    preflight::{
        reconcile_attempt, EligibilityKey, NativeObservation, PreflightAttempt, PreflightInput,
        PreflightPhase, PreflightReason, PreflightSignal,
    },
    routing::RouteKind,
};
use uuid::Uuid;

const CODEX_VERSION: &str = "1.2.3";
const PROFILE_VERSION: &str = "routing-v1";
const LUNA: &str = "gpt-5.6-luna";
const TERRA: &str = "gpt-5.6-terra";

fn direct_attempt(root: Uuid) -> PreflightAttempt {
    PreflightAttempt {
        attempt_id: Uuid::new_v4(),
        key: EligibilityKey {
            codex_package_version: CODEX_VERSION.into(),
            profile_version: PROFILE_VERSION.into(),
            requested_model: LUNA.into(),
            route_kind: RouteKind::Direct,
            depth: 1,
        },
        expected_root_id: root,
        expected_parent_id: root,
        started_at_ms: 100,
        deadline_at_ms: 1_000,
        phase: PreflightPhase::AwaitingNativeChild,
    }
}

fn observation(
    thread_id: Uuid,
    parent_thread_id: Option<Uuid>,
    requested_model: Option<&str>,
    effective_model: Option<&str>,
    status: AgentStatus,
    depth: u8,
    started_at_ms: i64,
) -> NativeObservation {
    NativeObservation {
        thread_id,
        parent_thread_id,
        requested_model: requested_model.map(str::to_owned),
        effective_model: effective_model.map(str::to_owned),
        model_source: if effective_model.is_some() {
            ModelSource::TurnContext
        } else {
            ModelSource::RequestedOnly
        },
        status,
        depth,
        started_at_ms,
    }
}

fn reconcile(
    attempt: &PreflightAttempt,
    observations: &[NativeObservation],
    now_ms: i64,
    signal: PreflightSignal,
) -> codex_assistant_lib::preflight::PreflightOutcome {
    reconcile_attempt(PreflightInput {
        attempt,
        observations,
        current_codex_package_version: CODEX_VERSION,
        current_profile_version: PROFILE_VERSION,
        now_ms,
        signal,
    })
}

#[test]
fn direct_child_is_eligible_only_after_authoritative_model_equality_and_idle() {
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let attempt = direct_attempt(root);
    let root_observation = observation(
        root,
        None,
        None,
        Some("gpt-5.6-sol"),
        AgentStatus::Idle,
        0,
        1,
    );
    let running = observation(
        child,
        Some(root),
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Running,
        1,
        120,
    );

    let pending = reconcile(
        &attempt,
        &[root_observation.clone(), running],
        200,
        PreflightSignal::None,
    );
    assert_eq!(pending.phase, PreflightPhase::VerifyingLineage);
    assert_eq!(pending.reason, Some(PreflightReason::ChildStillRunning));

    let mut fallback_only = observation(
        child,
        Some(root),
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Idle,
        1,
        120,
    );
    fallback_only.model_source = ModelSource::StateDatabase;
    let fallback_timeout = reconcile(
        &attempt,
        &[root_observation.clone(), fallback_only],
        1_001,
        PreflightSignal::None,
    );
    assert_eq!(fallback_timeout.phase, PreflightPhase::TimedOut);
    assert_eq!(fallback_timeout.reason, Some(PreflightReason::Timeout));

    let idle = observation(
        child,
        Some(root),
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Idle,
        1,
        120,
    );
    let eligible = reconcile(
        &attempt,
        &[root_observation, idle],
        300,
        PreflightSignal::None,
    );
    assert_eq!(eligible.phase, PreflightPhase::Eligible);
    assert_eq!(eligible.reason, None);
    assert_eq!(eligible.child_thread_id, Some(child));
}

#[test]
fn requested_effective_drift_is_unavailable_not_a_substitute() {
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let attempt = direct_attempt(root);
    let observations = [
        observation(
            root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        observation(
            child,
            Some(root),
            Some(LUNA),
            Some(TERRA),
            AgentStatus::Idle,
            1,
            120,
        ),
    ];

    let outcome = reconcile(&attempt, &observations, 200, PreflightSignal::None);

    assert_eq!(outcome.phase, PreflightPhase::Unavailable);
    assert_eq!(
        outcome.reason,
        Some(PreflightReason::EffectiveModelMismatch)
    );
    assert_eq!(outcome.child_thread_id, Some(child));
}

#[test]
fn detached_unrelated_missing_and_duplicate_candidates_fail_closed() {
    let root = Uuid::new_v4();
    let child = Uuid::new_v4();
    let other_root = Uuid::new_v4();
    let attempt = direct_attempt(root);

    let detached = [observation(
        child,
        None,
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Idle,
        0,
        120,
    )];
    assert_eq!(
        reconcile(&attempt, &detached, 200, PreflightSignal::None).reason,
        Some(PreflightReason::DetachedProcess)
    );

    let unrelated = [
        observation(
            other_root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        observation(
            child,
            Some(other_root),
            Some(LUNA),
            Some(LUNA),
            AgentStatus::Idle,
            1,
            120,
        ),
    ];
    assert_eq!(
        reconcile(&attempt, &unrelated, 200, PreflightSignal::None).reason,
        Some(PreflightReason::UnrelatedRoot)
    );

    let missing_parent = [observation(
        child,
        Some(root),
        Some(LUNA),
        Some(LUNA),
        AgentStatus::Idle,
        1,
        120,
    )];
    assert_eq!(
        reconcile(&attempt, &missing_parent, 200, PreflightSignal::None).reason,
        Some(PreflightReason::MissingParent)
    );

    let duplicate = [
        observation(
            root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        observation(
            child,
            Some(root),
            Some(LUNA),
            Some(LUNA),
            AgentStatus::Idle,
            1,
            120,
        ),
        observation(
            Uuid::new_v4(),
            Some(root),
            Some(LUNA),
            Some(LUNA),
            AgentStatus::Idle,
            1,
            130,
        ),
    ];
    assert_eq!(
        reconcile(&attempt, &duplicate, 200, PreflightSignal::None).reason,
        Some(PreflightReason::LineageAmbiguous)
    );
}

#[test]
fn nested_luna_is_eligible_only_below_the_expected_verified_terra_parent() {
    let root = Uuid::new_v4();
    let terra_parent = Uuid::new_v4();
    let luna_child = Uuid::new_v4();
    let attempt = PreflightAttempt {
        attempt_id: Uuid::new_v4(),
        key: EligibilityKey {
            codex_package_version: CODEX_VERSION.into(),
            profile_version: PROFILE_VERSION.into(),
            requested_model: LUNA.into(),
            route_kind: RouteKind::Nested,
            depth: 2,
        },
        expected_root_id: root,
        expected_parent_id: terra_parent,
        started_at_ms: 100,
        deadline_at_ms: 1_000,
        phase: PreflightPhase::AwaitingNativeChild,
    };
    let valid = [
        observation(
            root,
            None,
            None,
            Some("gpt-5.6-sol"),
            AgentStatus::Idle,
            0,
            1,
        ),
        observation(
            terra_parent,
            Some(root),
            Some(TERRA),
            Some(TERRA),
            AgentStatus::Idle,
            1,
            20,
        ),
        observation(
            luna_child,
            Some(terra_parent),
            Some(LUNA),
            Some(LUNA),
            AgentStatus::Idle,
            2,
            120,
        ),
    ];
    assert_eq!(
        reconcile(&attempt, &valid, 200, PreflightSignal::None).phase,
        PreflightPhase::Eligible
    );

    let mut wrong_parent_model = valid;
    wrong_parent_model[1].effective_model = Some(LUNA.into());
    assert_eq!(
        reconcile(&attempt, &wrong_parent_model, 200, PreflightSignal::None).reason,
        Some(PreflightReason::ParentNotVerifiedTerra)
    );
}

#[test]
fn rejection_timeout_and_version_changes_have_distinct_terminal_reasons() {
    let root = Uuid::new_v4();
    let attempt = direct_attempt(root);
    assert_eq!(
        reconcile(&attempt, &[], 200, PreflightSignal::NativeProfileRejected).reason,
        Some(PreflightReason::NativeProfileRejected)
    );
    let timed_out = reconcile(&attempt, &[], 1_001, PreflightSignal::None);
    assert_eq!(timed_out.phase, PreflightPhase::TimedOut);
    assert_eq!(timed_out.reason, Some(PreflightReason::Timeout));

    let stale_host = reconcile_attempt(PreflightInput {
        attempt: &attempt,
        observations: &[],
        current_codex_package_version: "1.2.4",
        current_profile_version: PROFILE_VERSION,
        now_ms: 200,
        signal: PreflightSignal::None,
    });
    assert_eq!(stale_host.reason, Some(PreflightReason::HostVersionChanged));

    let stale_profile = reconcile_attempt(PreflightInput {
        attempt: &attempt,
        observations: &[],
        current_codex_package_version: CODEX_VERSION,
        current_profile_version: "routing-v2",
        now_ms: 200,
        signal: PreflightSignal::None,
    });
    assert_eq!(
        stale_profile.reason,
        Some(PreflightReason::ProfileVersionChanged)
    );
}
