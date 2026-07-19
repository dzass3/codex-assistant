use codex_assistant_lib::{
    control_layer::cdp::OwnedSessionRecord,
    routing_app::{
        OperationStatus, RoutingApplication, RoutingSetupReasonCode, ThemeSessionStatus,
    },
};
use tempfile::tempdir;

fn session() -> OwnedSessionRecord {
    OwnedSessionRecord {
        schema_version: 1,
        port: 41_237,
        verified_pid: 72_000,
        browser_id_hash: "13d7f5f458585fa1a13c163da9f2b337d7a20a1d7852ec8f6709f14408d2a1af".into(),
        codex_version: "26.715.3651.0".into(),
        started_at_ms: 2_000,
        engine_version: "control-v1".into(),
    }
}

#[test]
fn theme_session_apply_and_restore_are_independent_from_routing_installation() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    let initial = app.theme_snapshot();
    assert_eq!(initial.session_status, ThemeSessionStatus::Inactive);
    assert_eq!(initial.packs.len(), 12);
    assert!(initial.selected_theme_id.is_none());
    assert!(initial.applied_theme_id.is_none());

    let started = app.start_theme_session_with(0, || Ok(session()));

    assert_eq!(started.status, OperationStatus::Applied);
    assert_eq!(
        app.theme_snapshot().session_status,
        ThemeSessionStatus::Ready
    );
    let applied = app.apply_theme_with("aurora-grid", |pack| {
        assert_eq!(pack.id, "aurora-grid");
        Ok(1)
    });
    assert_eq!(applied.status, OperationStatus::Applied);
    assert_eq!(
        app.theme_snapshot().applied_theme_id.as_deref(),
        Some("aurora-grid")
    );
    let restored = app.restore_theme_with(|| Ok(1));
    assert_eq!(restored.status, OperationStatus::Applied);
    assert!(app.theme_snapshot().applied_theme_id.is_none());
}

#[test]
fn inactive_theme_activation_restarts_once_and_applies_the_selected_theme() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    let mut restarted = false;

    let receipt = app.activate_theme_with(
        "gothic-horizon",
        0,
        || {
            restarted = true;
            Ok(session())
        },
        |pack| {
            assert_eq!(pack.id, "gothic-horizon");
            Ok(1)
        },
    );

    assert!(restarted);
    assert_eq!(receipt.status, OperationStatus::Applied);
    assert_eq!(
        app.theme_snapshot().applied_theme_id.as_deref(),
        Some("gothic-horizon")
    );
}

#[test]
fn one_click_theme_activation_retries_until_the_main_task_dom_is_ready() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    app.start_theme_session_with(0, || Ok(session()));
    let mut attempts = 0;
    let mut waits = 0;

    let receipt = app.retry_theme_application_with(
        3,
        || {
            attempts += 1;
            app.apply_theme_with("crystal-daylight", |_| {
                if attempts == 1 {
                    Err(RoutingSetupReasonCode::DomIncompatible)
                } else {
                    Ok(1)
                }
            })
        },
        || waits += 1,
    );

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert_eq!(attempts, 2);
    assert_eq!(waits, 1);
    assert_eq!(
        app.theme_snapshot().applied_theme_id.as_deref(),
        Some("crystal-daylight")
    );
}

#[test]
fn failed_theme_switch_keeps_selected_and_applied_state_distinct() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    app.start_theme_session_with(0, || Ok(session()));
    assert_eq!(
        app.apply_theme_with("aurora-grid", |_| Ok(1)).status,
        OperationStatus::Applied
    );

    let failed = app.apply_theme_with("gothic-horizon", |_| {
        Err(RoutingSetupReasonCode::DomIncompatible)
    });

    assert_eq!(failed.status, OperationStatus::Failed);
    let snapshot = app.theme_snapshot();
    assert_eq!(
        snapshot.selected_theme_id.as_deref(),
        Some("gothic-horizon")
    );
    assert_eq!(snapshot.applied_theme_id.as_deref(), Some("aurora-grid"));
}

#[test]
fn theme_session_restart_is_blocked_while_a_native_child_is_active() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();

    let blocked = app.start_theme_session_with(1, || Ok(session()));

    assert_eq!(blocked.status, OperationStatus::Blocked);
    assert_eq!(
        blocked.reason_codes,
        vec![RoutingSetupReasonCode::ActiveChild]
    );
    assert_eq!(
        app.theme_snapshot().session_status,
        ThemeSessionStatus::Inactive
    );
}

#[test]
fn selected_theme_survives_application_restart_as_paused_not_applied() {
    let root = tempdir().unwrap();
    let codex = root.path().join("codex");
    let skills = root.path().join("skills");
    let state = root.path().join("state");
    {
        let app = RoutingApplication::for_paths(
            codex.clone(),
            skills.clone(),
            std::env::current_exe().unwrap(),
            state.clone(),
        )
        .unwrap();
        app.start_theme_session_with(0, || Ok(session()));
        assert_eq!(
            app.apply_theme_with("violet-afterdark", |_| Ok(1)).status,
            OperationStatus::Applied
        );
    }

    let restarted =
        RoutingApplication::for_paths(codex, skills, std::env::current_exe().unwrap(), state)
            .unwrap();
    let snapshot = restarted.theme_snapshot();

    assert_eq!(snapshot.session_status, ThemeSessionStatus::Paused);
    assert_eq!(
        snapshot.selected_theme_id.as_deref(),
        Some("violet-afterdark")
    );
    assert!(snapshot.applied_theme_id.is_none());
}

#[test]
fn stale_control_session_pauses_theme_and_can_be_reestablished() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    app.start_theme_session_with(0, || Ok(session()));
    app.apply_theme_with("cyan-chorus", |_| Ok(1));

    app.reconcile_theme_session_with(|_| false);

    let paused = app.theme_snapshot();
    assert_eq!(paused.session_status, ThemeSessionStatus::Paused);
    assert_eq!(paused.selected_theme_id.as_deref(), Some("cyan-chorus"));
    assert!(paused.applied_theme_id.is_none());

    let mut restarted = false;
    let resumed = app.activate_theme_with(
        "cyan-chorus",
        0,
        || {
            restarted = true;
            Ok(session())
        },
        |_| Ok(1),
    );
    assert!(restarted);
    assert_eq!(resumed.status, OperationStatus::Applied);
}

#[test]
fn recovered_session_reapplies_saved_theme_without_a_second_user_click() {
    let root = tempdir().unwrap();
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().unwrap(),
        root.path().join("state"),
    )
    .unwrap();
    app.start_theme_session_with(0, || Ok(session()));
    app.apply_theme_with("roseglass-atelier", |_| Ok(1));
    app.reconcile_theme_session_with(|_| false);
    app.start_theme_session_with(0, || Ok(session()));

    let reapplied = app.reconcile_selected_theme_with(|pack| {
        assert_eq!(pack.id, "roseglass-atelier");
        Ok(1)
    });

    assert_eq!(reapplied.status, OperationStatus::Applied);
    assert_eq!(
        app.theme_snapshot().applied_theme_id.as_deref(),
        Some("roseglass-atelier")
    );
}

#[test]
fn restoring_a_paused_theme_clears_the_saved_preference() {
    let root = tempdir().unwrap();
    let codex = root.path().join("codex");
    let skills = root.path().join("skills");
    let state = root.path().join("state");
    let app = RoutingApplication::for_paths(
        codex.clone(),
        skills.clone(),
        std::env::current_exe().unwrap(),
        state.clone(),
    )
    .unwrap();
    app.start_theme_session_with(0, || Ok(session()));
    app.apply_theme_with("noir-stage", |_| Ok(1));
    app.reconcile_theme_session_with(|_| false);

    let restored = app.restore_theme_with(|| panic!("paused theme has no live styles to remove"));

    assert_eq!(restored.status, OperationStatus::Applied);
    let snapshot = app.theme_snapshot();
    assert_eq!(snapshot.session_status, ThemeSessionStatus::Inactive);
    assert!(snapshot.selected_theme_id.is_none());
    assert!(snapshot.applied_theme_id.is_none());
    drop(app);

    let restarted =
        RoutingApplication::for_paths(codex, skills, std::env::current_exe().unwrap(), state)
            .unwrap();
    assert!(restarted.theme_snapshot().selected_theme_id.is_none());
}
