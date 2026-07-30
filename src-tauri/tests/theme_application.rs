use base64::{engine::general_purpose::STANDARD, Engine as _};
use codex_assistant_lib::{
    control_layer::cdp::OwnedSessionRecord,
    theme_app::{
        decide_session_action, OperationStatus, ThemeApplication, ThemeReasonCode,
        ThemeSessionAction, ThemeSessionStatus,
    },
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Barrier};
use tempfile::tempdir;

fn app_at(path: &std::path::Path) -> ThemeApplication {
    ThemeApplication::for_state_directory(path.to_path_buf()).expect("theme application")
}

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

fn write_local_theme(state: &std::path::Path) {
    let directory = state.join("local-themes").join("arina-pink");
    std::fs::create_dir_all(&directory).expect("local theme directory");
    let bytes = b"local-image";
    std::fs::write(directory.join("arina-pink.jpg"), bytes).expect("local asset");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "minimum_engine_version": 1,
        "id": "arina-pink",
        "name": "Arina Pink",
        "description": "User-owned local theme",
        "category": "local-import",
        "preview_path": "local-theme:arina-pink",
        "backdrop": {"kind": "image", "asset_id": "arina-pink", "overlay": "#fff5f6", "focal_x": 72, "focal_y": 45},
        "palette": {"surface": "#fff8f8", "surface_strong": "#fffdfd", "text": "#3b292d", "accent": "#d9637e", "border": "#e7aeba"},
        "effects": {"surface_opacity": 78, "blur_px": 10, "contrast_percent": 96, "motion": false},
        "assets": [{"id": "arina-pink", "mime_type": "image/jpeg", "sha256": format!("{:x}", Sha256::digest(bytes))}],
        "rights": {"source": "User-owned local import", "rightsholder": "User-provided asset", "license": "Local use only", "commercial_redistribution": false, "attribution": "Stored locally by user request", "reviewed_at": "2026-07-19", "manual_signoff": true, "status": "local-only"}
    });
    std::fs::write(
        directory.join("theme.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest"),
    )
    .expect("local manifest");
}

#[test]
fn theme_session_apply_and_restore_use_only_theme_state() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    let initial = app.snapshot();
    assert_eq!(initial.session_status, ThemeSessionStatus::Inactive);
    assert_eq!(initial.packs.len(), 12);

    assert_eq!(
        app.start_session_with(0, || Ok(session())).status,
        OperationStatus::Applied
    );
    assert_eq!(app.snapshot().session_status, ThemeSessionStatus::Ready);
    assert_eq!(
        app.apply_theme_with("wisteria-bride", |pack| {
            assert_eq!(pack.id, "wisteria-bride");
            Ok(1)
        })
        .status,
        OperationStatus::Applied
    );
    assert_eq!(
        app.snapshot().applied_theme_id.as_deref(),
        Some("wisteria-bride")
    );
    assert_eq!(app.restore_with(|| Ok(1)).status, OperationStatus::Applied);
    assert!(app.snapshot().applied_theme_id.is_none());
}

#[test]
fn local_pack_and_import_are_available_without_a_routing_manifest() {
    let root = tempdir().unwrap();
    write_local_theme(root.path());
    let app = app_at(root.path());
    assert!(app
        .snapshot()
        .packs
        .iter()
        .any(|pack| pack.id == "arina-pink"));

    let bytes = include_bytes!("../../public/themes/wisteria-bride.webp");
    let data_url = format!("data:image/webp;base64,{}", STANDARD.encode(bytes));
    let imported = app
        .import_local_theme("My Aurora", &data_url)
        .expect("theme import");
    assert!(imported.theme_id.starts_with("local-"));
    assert!(app
        .import_local_theme("Remote", "https://example.com/a.webp")
        .is_err());
}

#[test]
fn retired_bundled_preference_is_cleared_once_without_touching_local_themes() {
    let root = tempdir().unwrap();
    write_local_theme(root.path());
    std::fs::write(
        root.path().join("theme-state.json"),
        br#"{"schema_version":1,"selected_theme_id":"violet-blade"}"#,
    )
    .unwrap();

    let migrated = app_at(root.path());
    let snapshot = migrated.snapshot();
    assert!(snapshot.selected_theme_id.is_none());
    assert_eq!(
        snapshot.catalog_notice.as_deref(),
        Some("原主题已下架，请从 12 个新主题中重新选择")
    );
    assert!(snapshot.packs.iter().any(|pack| pack.id == "arina-pink"));
    drop(migrated);

    let repeated = app_at(root.path());
    assert!(repeated.snapshot().selected_theme_id.is_none());
    assert!(repeated.snapshot().catalog_notice.is_none());
    assert!(repeated
        .snapshot()
        .packs
        .iter()
        .any(|pack| pack.id == "arina-pink"));
}

#[test]
fn valid_local_preference_survives_bundled_catalog_replacement() {
    let root = tempdir().unwrap();
    write_local_theme(root.path());
    std::fs::write(
        root.path().join("theme-state.json"),
        br#"{"schema_version":1,"selected_theme_id":"arina-pink"}"#,
    )
    .unwrap();

    let app = app_at(root.path());

    assert_eq!(
        app.snapshot().selected_theme_id.as_deref(),
        Some("arina-pink")
    );
    assert!(app.snapshot().catalog_notice.is_none());
}

#[test]
fn inactive_activation_starts_once_and_applies() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    let mut restarted = false;
    let result = app.activate_with(
        "crimson-palace",
        0,
        || {
            restarted = true;
            Ok(session())
        },
        |_| Ok(1),
    );
    assert!(restarted);
    assert_eq!(result.status, OperationStatus::Applied);
    assert_eq!(
        app.snapshot().applied_theme_id.as_deref(),
        Some("crimson-palace")
    );
}

#[test]
fn activation_retries_while_the_main_dom_is_not_ready() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    app.start_session_with(0, || Ok(session()));
    let mut attempts = 0;
    let mut waits = 0;
    let result = app.retry_theme_application_with(
        3,
        || {
            attempts += 1;
            app.apply_theme_with("mint-gentleman", |_| {
                if attempts == 1 {
                    Err(ThemeReasonCode::DomIncompatible)
                } else {
                    Ok(1)
                }
            })
        },
        || waits += 1,
    );
    assert_eq!(result.status, OperationStatus::Applied);
    assert_eq!((attempts, waits), (2, 1));
}

#[test]
fn activation_retries_after_a_rolled_back_partial_application() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    app.start_session_with(0, || Ok(session()));
    let mut attempts = 0;
    let mut waits = 0;
    let result = app.retry_theme_application_with(
        3,
        || {
            attempts += 1;
            app.apply_theme_with("mint-gentleman", |_| {
                if attempts == 1 {
                    Err(ThemeReasonCode::PartialApplyFailed)
                } else {
                    Ok(1)
                }
            })
        },
        || waits += 1,
    );
    assert_eq!(result.status, OperationStatus::Applied);
    assert_eq!((attempts, waits), (2, 1));
}

#[test]
fn activation_does_not_retry_a_terminal_failure() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    app.start_session_with(0, || Ok(session()));
    let mut attempts = 0;
    let mut waits = 0;
    let result = app.retry_theme_application_with(
        3,
        || {
            attempts += 1;
            app.apply_theme_with("mint-gentleman", |_| {
                Err(ThemeReasonCode::CdpVerificationFailed)
            })
        },
        || waits += 1,
    );
    assert_eq!(result.status, OperationStatus::Failed);
    assert_eq!((attempts, waits), (1, 0));
}

#[test]
fn failed_switch_never_misreports_the_new_theme_as_applied() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    app.start_session_with(0, || Ok(session()));
    app.apply_theme_with("wisteria-bride", |_| Ok(1));
    let failed = app.apply_theme_with("crimson-palace", |_| Err(ThemeReasonCode::DomIncompatible));
    assert_eq!(failed.status, OperationStatus::Failed);
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.selected_theme_id.as_deref(),
        Some("crimson-palace")
    );
    assert_eq!(snapshot.applied_theme_id.as_deref(), Some("wisteria-bride"));
}

#[test]
fn concurrent_restore_is_blocked_until_theme_application_commits() {
    let root = tempdir().unwrap();
    let app = Arc::new(app_at(root.path()));
    app.start_session_with(0, || Ok(session()));
    app.apply_theme_with("wisteria-bride", |_| Ok(1));

    let apply_entered = Arc::new(Barrier::new(2));
    let release_apply = Arc::new(Barrier::new(2));
    let worker = {
        let app = Arc::clone(&app);
        let apply_entered = Arc::clone(&apply_entered);
        let release_apply = Arc::clone(&release_apply);
        std::thread::spawn(move || {
            app.apply_theme_with("crimson-palace", |_| {
                apply_entered.wait();
                release_apply.wait();
                Ok(1)
            })
        })
    };

    apply_entered.wait();
    let restore = app.restore_with(|| Ok(1));
    assert_eq!(restore.status, OperationStatus::Blocked);
    assert_eq!(
        restore.reason_codes,
        vec![ThemeReasonCode::OperationConflict]
    );
    release_apply.wait();
    assert_eq!(worker.join().unwrap().status, OperationStatus::Applied);

    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.selected_theme_id.as_deref(),
        Some("crimson-palace")
    );
    assert_eq!(snapshot.applied_theme_id.as_deref(), Some("crimson-palace"));
}

#[test]
fn session_reconciliation_does_not_clear_an_inflight_theme_commit() {
    let root = tempdir().unwrap();
    let app = Arc::new(app_at(root.path()));
    app.start_session_with(0, || Ok(session()));
    app.apply_theme_with("wisteria-bride", |_| Ok(1));

    let apply_entered = Arc::new(Barrier::new(2));
    let release_apply = Arc::new(Barrier::new(2));
    let worker = {
        let app = Arc::clone(&app);
        let apply_entered = Arc::clone(&apply_entered);
        let release_apply = Arc::clone(&release_apply);
        std::thread::spawn(move || {
            app.apply_theme_with("crimson-palace", |_| {
                apply_entered.wait();
                release_apply.wait();
                Ok(1)
            })
        })
    };

    apply_entered.wait();
    assert!(app.reconcile_session_with(|_| false));
    release_apply.wait();
    assert_eq!(worker.join().unwrap().status, OperationStatus::Applied);

    let snapshot = app.snapshot();
    assert_eq!(snapshot.session_status, ThemeSessionStatus::Ready);
    assert_eq!(
        snapshot.selected_theme_id.as_deref(),
        Some("crimson-palace")
    );
    assert_eq!(snapshot.applied_theme_id.as_deref(), Some("crimson-palace"));
}

#[test]
fn session_restart_is_blocked_while_native_work_is_active() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    let blocked = app.start_session_with(1, || Ok(session()));
    assert_eq!(blocked.status, OperationStatus::Blocked);
    assert_eq!(blocked.reason_codes, vec![ThemeReasonCode::ActiveWork]);
}

#[test]
fn selected_theme_survives_application_restart_as_paused() {
    let root = tempdir().unwrap();
    {
        let app = app_at(root.path());
        app.start_session_with(0, || Ok(session()));
        app.apply_theme_with("sakura-moon", |_| Ok(1));
    }
    let restarted = app_at(root.path());
    let snapshot = restarted.snapshot();
    assert_eq!(snapshot.session_status, ThemeSessionStatus::Paused);
    assert_eq!(snapshot.selected_theme_id.as_deref(), Some("sakura-moon"));
    assert!(snapshot.applied_theme_id.is_none());
}

#[test]
fn stale_session_pauses_and_restore_clears_the_saved_preference() {
    let root = tempdir().unwrap();
    let app = app_at(root.path());
    app.start_session_with(0, || Ok(session()));
    app.apply_theme_with("fuji-autumn", |_| Ok(1));
    app.reconcile_session_with(|_| false);
    assert_eq!(app.snapshot().session_status, ThemeSessionStatus::Paused);
    let restored = app.restore_with(|| panic!("paused theme has no live style"));
    assert_eq!(restored.status, OperationStatus::Applied);
    assert!(app.snapshot().selected_theme_id.is_none());
}

#[test]
fn session_action_launches_cold_codex_restarts_unmanaged_and_fails_closed_on_ambiguity() {
    assert_eq!(
        decide_session_action(0, false),
        Ok(ThemeSessionAction::Launch)
    );
    assert_eq!(
        decide_session_action(1, false),
        Ok(ThemeSessionAction::Restart)
    );
    assert_eq!(
        decide_session_action(1, true),
        Ok(ThemeSessionAction::Reuse)
    );
    assert_eq!(
        decide_session_action(2, false),
        Err(ThemeReasonCode::UnsupportedHost)
    );
}
