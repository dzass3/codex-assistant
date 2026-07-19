use std::fs;

use codex_assistant_lib::control_layer::{
    cdp::{OwnedSessionRecord, OwnedSessionStore},
    injector::{CompatibilityReason, ControlEvent, InsertionResult},
};
use codex_assistant_lib::monitor::model::{
    AgentObservation, AgentStatus, HealthEntry, ModelSource, MonitorSnapshot, SourceHealth,
    SummaryCounts,
};
use codex_assistant_lib::routing::{EligibilityReasonCode, EligibilityStatus, RouteKind};
use codex_assistant_lib::routing_app::{
    OperationStatus, RestartIntent, RoutingActivationStatus, RoutingApplication,
    RoutingInstallationStatus, RoutingRestartStatus, RoutingSetupReasonCode,
    VerifiedRootFingerprint,
};
use tempfile::tempdir;

#[test]
fn first_install_returns_a_sanitized_receipt_and_requires_one_restart() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("codex home");
    fs::write(
        codex_home.join("config.toml"),
        "# keep\n[agents]\nmax_threads = 7\n",
    )
    .expect("config fixture");
    let app = RoutingApplication::for_paths(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");

    assert_eq!(
        app.snapshot().setup.installation_status,
        RoutingInstallationStatus::Uninstalled
    );
    let receipt = app.install();

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(receipt.restart_required);
    assert!(uuid::Uuid::parse_str(&receipt.operation_id).is_ok());
    assert!(receipt.reason_codes.is_empty());
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::RestartRequired
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::Required
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("installed config");
    assert!(config.contains("max_threads = 7"));
    assert!(config.contains("[agents.codex_assistant_luna]"));
}

#[test]
fn restore_returns_owned_configuration_to_its_preinstall_state() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("codex home");
    let original = b"# original\r\n[agents]\r\nmax_depth = 1\r\n";
    fs::write(codex_home.join("config.toml"), original).expect("config fixture");
    let app = RoutingApplication::for_paths(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.restore();

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(receipt.restart_required);
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("restored config"),
        original
    );
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::Uninstalled
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::Required
    );
}

#[test]
fn routing_cannot_be_enabled_before_install_and_native_preflight() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");

    let receipt = app.set_root_enabled("d2719d93-b823-4a7f-934f-23cbe01c8ab0", true, true);

    assert_eq!(receipt.status, OperationStatus::Blocked);
    assert_eq!(
        receipt.reason_codes,
        vec![codex_assistant_lib::routing_app::RoutingSetupReasonCode::PreflightRequired]
    );
    assert!(app.snapshot().routing.routes.is_empty());
}

#[test]
fn restart_is_blocked_while_a_native_child_is_active() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.request_restart(1);

    assert_eq!(receipt.status, OperationStatus::Blocked);
    assert_eq!(
        receipt.reason_codes,
        vec![codex_assistant_lib::routing_app::RoutingSetupReasonCode::ActiveChild]
    );
    assert!(receipt.restart_required);
    assert_eq!(
        app.snapshot().setup.restart_status,
        RoutingRestartStatus::BlockedActiveChild
    );
}

fn root_fingerprint() -> VerifiedRootFingerprint {
    VerifiedRootFingerprint {
        pid: 72_000,
        created_at_ticks: 123_456,
        owner_sid: "S-1-5-21-1000".into(),
        canonical_executable: r"C:\Program Files\WindowsApps\OpenAI.Codex\app\ChatGPT.exe".into(),
        package_version: "26.715.3651.0".into(),
    }
}

#[test]
fn force_restart_ticket_is_single_use_bound_to_impact_and_expires() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    let impact = app
        .prepare_force_restart_with(RestartIntent::RoutingRestart, 2, 10_000, || {
            Ok(root_fingerprint())
        })
        .expect("sanitized confirmation impact");
    assert_eq!(impact.active_native_children, 2);
    assert_eq!(impact.grace_period_ms, 5_000);
    assert_eq!(impact.expires_at_ms, 70_000);

    let changed = app.force_restart_with(
        &impact.confirmation_ticket,
        RestartIntent::RoutingRestart,
        1,
        10_001,
        || Ok(root_fingerprint()),
        |_, _| Ok(()),
    );
    assert_eq!(changed.status, OperationStatus::Blocked);
    assert_eq!(
        changed.reason_codes,
        vec![RoutingSetupReasonCode::ImpactChanged]
    );

    let replay = app.force_restart_with(
        &impact.confirmation_ticket,
        RestartIntent::RoutingRestart,
        2,
        10_002,
        || Ok(root_fingerprint()),
        |_, _| Ok(()),
    );
    assert_eq!(
        replay.reason_codes,
        vec![RoutingSetupReasonCode::ConfirmationExpired]
    );
}

#[test]
fn force_restart_rejects_pid_reuse_before_entering_irreversible_work() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    let impact = app
        .prepare_force_restart_with(RestartIntent::RoutingRestart, 1, 10_000, || {
            Ok(root_fingerprint())
        })
        .unwrap();
    let mut reused = root_fingerprint();
    reused.created_at_ticks += 1;
    let receipt = app.force_restart_with(
        &impact.confirmation_ticket,
        RestartIntent::RoutingRestart,
        1,
        10_001,
        || Ok(reused),
        |_, _| panic!("restart must not run after PID reuse"),
    );
    assert_eq!(receipt.status, OperationStatus::Blocked);
    assert_eq!(
        receipt.reason_codes,
        vec![RoutingSetupReasonCode::IdentityChanged]
    );
}

#[test]
fn successful_verified_restart_clears_the_one_time_restart_requirement() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.request_restart_with(0, || Ok(()));

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(!receipt.restart_required);
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::Installed
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::NotRequired
    );
}

#[test]
fn verified_restart_persists_only_revalidatable_control_session_metadata() {
    let root = tempdir().expect("temporary root");
    let state = root.path().join("state");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        state.clone(),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);
    let record = OwnedSessionRecord {
        schema_version: 1,
        port: 41_237,
        verified_pid: 72_000,
        browser_id_hash: "13d7f5f458585fa1a13c163da9f2b337d7a20a1d7852ec8f6709f14408d2a1af".into(),
        codex_version: "26.715.3651.0".into(),
        started_at_ms: 2_000,
        engine_version: "control-v1".into(),
    };

    let receipt = app.request_restart_with_session(0, || Ok(record.clone()));

    assert_eq!(receipt.status, OperationStatus::Applied);
    let stored = OwnedSessionStore::in_directory(&state)
        .unwrap()
        .load(72_000, "26.715.3651.0", 2_001)
        .unwrap()
        .unwrap();
    assert_eq!(stored, record);
    let raw = fs::read_to_string(state.join("control-session.json")).unwrap();
    assert!(!raw.contains("webSocketDebuggerUrl"));
    assert!(!raw.contains("/devtools/"));
}

#[test]
fn preflight_begins_with_visible_direct_native_checks_instead_of_assuming_support() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    app.request_restart_with(0, || Ok(()));
    let root_id = "d2719d93-b823-4a7f-934f-23cbe01c8ab0";

    let receipt = app.begin_preflight_with(root_id, true, "26.715.3651.0");

    assert_eq!(receipt.status, OperationStatus::Applied);
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.preflight_status,
        codex_assistant_lib::routing_app::RoutingPreflightStatus::Running
    );
    assert_eq!(snapshot.routing.eligibility.len(), 4);
    assert_eq!(snapshot.routing.routes.len(), 1);
    assert_eq!(
        snapshot.routing.routes[0].conversation_id.to_string(),
        root_id
    );
    assert!(!snapshot.routing.routes[0].enabled);
    assert!(snapshot.routing.eligibility.iter().all(|entry| {
        entry.route_kind == RouteKind::Direct
            && entry.status == EligibilityStatus::Verifying
            && entry.reason == Some(EligibilityReasonCode::AwaitingVisibleCommand)
    }));
}

#[test]
fn successful_visible_insertion_advances_only_the_first_terra_preflight_attempt() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    app.request_restart_with(0, || Ok(()));
    let root_id = "d2719d93-b823-4a7f-934f-23cbe01c8ab0";
    app.begin_preflight_with(root_id, true, "26.715.3651.0");

    let receipt = app.insert_next_preflight_with(|request| {
        assert_eq!(request.root_conversation_id.to_string(), root_id);
        assert!(!request.route_key.is_nil());
        assert!(request.directive.contains("profile codex_assistant_terra"));
        assert!(request.directive.contains(&request.attempt_id.to_string()));
        Ok(true)
    });

    assert_eq!(receipt.status, OperationStatus::Applied);
    let snapshot = app.snapshot();
    let terra = snapshot
        .routing
        .eligibility
        .iter()
        .find(|entry| entry.requested_model == "gpt-5.6-terra")
        .unwrap();
    assert_eq!(terra.status, EligibilityStatus::Verifying);
    assert_eq!(
        terra.reason,
        Some(EligibilityReasonCode::AwaitingNativeChild)
    );
    assert_eq!(
        snapshot
            .routing
            .eligibility
            .iter()
            .filter(|entry| { entry.reason == Some(EligibilityReasonCode::AwaitingVisibleCommand) })
            .count(),
        3
    );
    let overlapping = app.insert_next_preflight_with(|_| {
        panic!("a second visible command must not be inserted while Terra is active")
    });
    assert_eq!(overlapping.status, OperationStatus::Noop);
}

#[test]
fn monitor_reconciliation_marks_the_exact_visible_terra_child_eligible() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    app.request_restart_with(0, || Ok(()));
    let root_id = uuid::Uuid::parse_str("d2719d93-b823-4a7f-934f-23cbe01c8ab0").unwrap();
    let child_id = uuid::Uuid::parse_str("85fb8317-3539-4714-ab7f-815fecf3e66f").unwrap();
    app.begin_preflight_with(&root_id.to_string(), true, "26.715.3651.0");
    app.insert_next_preflight_with(|_| Ok(true));
    let now_ms = chrono::Utc::now().timestamp_millis().max(0);
    let observation = |thread_id: uuid::Uuid,
                       parent_thread_id: Option<uuid::Uuid>,
                       requested_model: Option<&str>,
                       effective_model: &str,
                       depth: u32| AgentObservation {
        thread_id: thread_id.to_string(),
        parent_thread_id: parent_thread_id.map(|id| id.to_string()),
        agent_path: Some("/root/PRIVATE".into()),
        display_name: "PRIVATE".into(),
        role: None,
        project: Some("PRIVATE".into()),
        originator: None,
        requested_model: requested_model.map(str::to_owned),
        effective_model: Some(effective_model.into()),
        model_source: ModelSource::TurnContext,
        reasoning_effort: None,
        status: AgentStatus::Idle,
        model_drift: false,
        is_subagent: parent_thread_id.is_some(),
        depth,
        started_at_ms: Some(now_ms),
        updated_at_ms: Some(now_ms),
        freshness_ms: Some(0),
    };
    let monitor = MonitorSnapshot {
        generated_at_ms: now_ms,
        agents: vec![
            observation(root_id, None, None, "gpt-5.6-sol", 0),
            observation(
                child_id,
                Some(root_id),
                Some("gpt-5.6-terra"),
                "gpt-5.6-terra",
                1,
            ),
        ],
        counts: SummaryCounts::default(),
        health: SourceHealth {
            state_database: HealthEntry::healthy("ready", now_ms),
            rollout_observer: HealthEntry::healthy("ready", now_ms),
        },
    };

    let receipt = app.reconcile_preflight_with(&monitor, "26.715.3651.0", now_ms);

    assert_eq!(receipt.status, OperationStatus::Applied);
    let terra = app
        .snapshot()
        .routing
        .eligibility
        .into_iter()
        .find(|entry| entry.requested_model == "gpt-5.6-terra")
        .unwrap();
    assert_eq!(terra.status, EligibilityStatus::Eligible);
    assert_eq!(terra.reason, None);
    let nested = app
        .snapshot()
        .routing
        .eligibility
        .into_iter()
        .filter(|entry| entry.route_kind == RouteKind::Nested)
        .collect::<Vec<_>>();
    assert_eq!(nested.len(), 2);
    assert!(nested.iter().all(|entry| {
        matches!(
            entry.requested_model.as_str(),
            "gpt-5.6-luna" | "gpt-5.3-codex-spark"
        ) && entry.status == EligibilityStatus::Verifying
            && entry.reason == Some(EligibilityReasonCode::AwaitingVisibleCommand)
    }));
    let next = app.insert_next_preflight_with(|request| {
        assert!(request
            .directive
            .contains("from the verified visible Terra parent"));
        assert!(request.directive.contains("profile codex_assistant_luna"));
        Ok(true)
    });
    assert_eq!(next.status, OperationStatus::Applied);

    let nested_luna = uuid::Uuid::parse_str("af9ba8a4-e270-44e1-93cc-083bf7a1c171").unwrap();
    let nested_spark = uuid::Uuid::parse_str("d53db52e-f7f0-43f1-8ac4-7fdb96bda61e").unwrap();
    let direct_spark = uuid::Uuid::parse_str("22d2a288-6bf6-473f-bf3d-182c318d3b72").unwrap();
    let direct_luna = uuid::Uuid::parse_str("eff4ddd7-fdf4-4925-9f99-e23d8d2c96d5").unwrap();
    let direct_sol = uuid::Uuid::parse_str("1fd3c18b-945d-442c-a4f0-e8604cc238b8").unwrap();
    let mut agents = monitor.agents.clone();
    let monitor_with = |agents: Vec<AgentObservation>, generated_at_ms: i64| MonitorSnapshot {
        generated_at_ms,
        agents,
        counts: SummaryCounts::default(),
        health: SourceHealth {
            state_database: HealthEntry::healthy("ready", generated_at_ms),
            rollout_observer: HealthEntry::healthy("ready", generated_at_ms),
        },
    };

    agents.push(observation(
        nested_luna,
        Some(child_id),
        Some("gpt-5.6-luna"),
        "gpt-5.6-luna",
        2,
    ));
    app.reconcile_preflight_with(
        &monitor_with(agents.clone(), now_ms + 1),
        "26.715.3651.0",
        now_ms + 1,
    );
    assert_eq!(
        app.insert_next_preflight_with(|request| {
            assert!(request.directive.contains("profile codex_assistant_spark"));
            assert!(request
                .directive
                .contains("from the verified visible Terra parent"));
            Ok(true)
        })
        .status,
        OperationStatus::Applied
    );

    agents.push(observation(
        nested_spark,
        Some(child_id),
        Some("gpt-5.3-codex-spark"),
        "gpt-5.3-codex-spark",
        2,
    ));
    app.reconcile_preflight_with(
        &monitor_with(agents.clone(), now_ms + 2),
        "26.715.3651.0",
        now_ms + 2,
    );
    assert_eq!(
        app.insert_next_preflight_with(|request| {
            assert!(request.directive.contains("profile codex_assistant_spark"));
            assert!(request.directive.contains("from the current root"));
            Ok(true)
        })
        .status,
        OperationStatus::Applied
    );

    agents.push(observation(
        direct_spark,
        Some(root_id),
        Some("gpt-5.3-codex-spark"),
        "gpt-5.3-codex-spark",
        1,
    ));
    app.reconcile_preflight_with(
        &monitor_with(agents.clone(), now_ms + 3),
        "26.715.3651.0",
        now_ms + 3,
    );
    assert_eq!(
        app.insert_next_preflight_with(|request| {
            assert!(request.directive.contains("profile codex_assistant_luna"));
            assert!(request.directive.contains("from the current root"));
            Ok(true)
        })
        .status,
        OperationStatus::Applied
    );

    agents.push(observation(
        direct_luna,
        Some(root_id),
        Some("gpt-5.6-luna"),
        "gpt-5.6-luna",
        1,
    ));
    app.reconcile_preflight_with(
        &monitor_with(agents.clone(), now_ms + 4),
        "26.715.3651.0",
        now_ms + 4,
    );
    assert_eq!(
        app.insert_next_preflight_with(|request| {
            assert!(request.directive.contains("profile codex_assistant_sol"));
            assert!(request.directive.contains("from the current root"));
            Ok(true)
        })
        .status,
        OperationStatus::Applied
    );

    agents.push(observation(
        direct_sol,
        Some(root_id),
        Some("gpt-5.6-sol"),
        "gpt-5.6-sol",
        1,
    ));
    app.reconcile_preflight_with(
        &monitor_with(agents, now_ms + 5),
        "26.715.3651.0",
        now_ms + 5,
    );

    let completed = app.snapshot();
    assert_eq!(
        completed.setup.preflight_status,
        codex_assistant_lib::routing_app::RoutingPreflightStatus::Complete
    );
    assert_eq!(completed.routing.eligibility.len(), 6);
    assert!(completed
        .routing
        .eligibility
        .iter()
        .all(|entry| entry.status == EligibilityStatus::Eligible));
    let restored = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("restored routing application");
    assert!(restored.reconcile_persisted_preflight_with("26.715.3651.0"));
    assert_eq!(
        restored.snapshot().setup.preflight_status,
        codex_assistant_lib::routing_app::RoutingPreflightStatus::Complete
    );
    let discovered_root = uuid::Uuid::parse_str("b7786c3b-a608-4a57-afd5-a31e29f6ef48").unwrap();
    let observed = app.observe_roots(&monitor_with(
        vec![observation(discovered_root, None, None, "gpt-5.6-sol", 0)],
        now_ms + 6,
    ));
    assert_eq!(observed.status, OperationStatus::Applied);
    assert!(app
        .snapshot()
        .routing
        .routes
        .iter()
        .any(|route| route.conversation_id == discovered_root && !route.enabled));
    assert_eq!(
        app.set_root_enabled(&root_id.to_string(), true, true)
            .status,
        OperationStatus::Applied
    );
    assert!(app.snapshot().routing.routes[0].enabled);

    let unopened_root = uuid::Uuid::parse_str("a087a511-1635-437a-9de8-4019b479aa13").unwrap();
    assert_eq!(
        app.set_root_enabled(&unopened_root.to_string(), true, true)
            .status,
        OperationStatus::Applied
    );
    let unopened_control = app
        .snapshot()
        .controls
        .into_iter()
        .find(|control| control.conversation_id == unopened_root)
        .expect("new observed root should gain a control state");
    assert_eq!(
        unopened_control.status,
        RoutingActivationStatus::PendingOpen
    );
    let unopened_route_key = app
        .snapshot()
        .routing
        .routes
        .into_iter()
        .find(|route| route.conversation_id == unopened_root)
        .expect("new root route")
        .route_key;
    assert_eq!(
        app.apply_control_event(
            ControlEvent::Compatibility {
                route_id: unopened_root,
                compatible: true,
                reason: CompatibilityReason::Ready,
            },
            0,
        )
        .status,
        OperationStatus::Applied
    );
    assert_eq!(
        app.snapshot()
            .controls
            .into_iter()
            .find(|control| control.conversation_id == unopened_root)
            .unwrap()
            .status,
        RoutingActivationStatus::PendingNextTurn
    );
    assert_eq!(
        app.apply_control_event(
            ControlEvent::InsertionResult {
                route_id: unopened_root,
                route_key: unopened_route_key,
                submission_id: "submission-1".to_owned(),
                result: InsertionResult::Inserted,
            },
            0,
        )
        .status,
        OperationStatus::Applied
    );
    assert_eq!(
        app.snapshot()
            .controls
            .into_iter()
            .find(|control| control.conversation_id == unopened_root)
            .unwrap()
            .status,
        RoutingActivationStatus::Enabled
    );

    let disabled = app.set_root_enabled_with_activity(&root_id.to_string(), false, true, 1);
    assert_eq!(disabled.status, OperationStatus::Applied);
    assert!(disabled.reason_codes.is_empty());
    assert!(!app.snapshot().routing.routes[0].enabled);
    assert_eq!(
        app.apply_control_event(
            ControlEvent::Toggle {
                route_id: root_id,
                enabled: false,
            },
            0,
        )
        .status,
        OperationStatus::Noop
    );
    assert!(!app.snapshot().routing.routes[0].enabled);
    let wrong_root = uuid::Uuid::parse_str("c1f7ed94-63ce-4a60-bab7-23b855e843b8").unwrap();
    assert_eq!(
        app.apply_control_event(
            ControlEvent::Toggle {
                route_id: wrong_root,
                enabled: true,
            },
            0,
        )
        .status,
        OperationStatus::Blocked
    );
}
