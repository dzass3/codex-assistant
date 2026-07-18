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
    assert_eq!(initial.packs.len(), 2);
    assert!(initial.active_theme_id.is_none());

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
        app.theme_snapshot().active_theme_id.as_deref(),
        Some("aurora-grid")
    );
    let restored = app.restore_theme_with(|| Ok(1));
    assert_eq!(restored.status, OperationStatus::Applied);
    assert!(app.theme_snapshot().active_theme_id.is_none());
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
