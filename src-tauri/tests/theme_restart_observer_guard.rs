use codex_assistant_lib::{
    control_layer::cdp::OwnedSessionRecord,
    monitor::model::{
        HealthEntry, MonitorSnapshot, ObserverStatus, RestartSafetyProjection, SourceHealth,
        SummaryCounts,
    },
    theme_app::{OperationStatus, ThemeApplication, ThemeReasonCode},
};
use tempfile::tempdir;

fn snapshot(counts: SummaryCounts, health: SourceHealth) -> MonitorSnapshot {
    MonitorSnapshot {
        generated_at_ms: 10,
        codex_running: true,
        session_started_at_ms: Some(1),
        observer_status: ObserverStatus::Live,
        agents: Vec::new(),
        counts,
        health,
    }
}

fn healthy() -> SourceHealth {
    SourceHealth {
        state_database: HealthEntry::healthy("ready", 10),
        rollout_observer: HealthEntry::healthy("ready", 10),
    }
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

#[test]
fn projection_blocks_normal_restart_for_known_active_work() {
    let projection = RestartSafetyProjection::from_snapshot(&snapshot(
        SummaryCounts {
            starting: 1,
            running: 2,
            ..SummaryCounts::default()
        },
        healthy(),
    ));
    assert_eq!(projection.active_work_count, 3);
    assert!(projection.monitor_confident);

    let root = tempdir().unwrap();
    let app = ThemeApplication::for_state_directory(root.path().to_path_buf()).unwrap();
    let result = app.start_session_with_safety(projection, || panic!("must not restart"));
    assert_eq!(result.status, OperationStatus::Blocked);
    assert_eq!(result.reason_codes, vec![ThemeReasonCode::ActiveWork]);
}

#[test]
fn degraded_or_tracking_error_observation_blocks_uncertain_restart() {
    let projection = RestartSafetyProjection::from_snapshot(&snapshot(
        SummaryCounts {
            tracking_errors: 1,
            ..SummaryCounts::default()
        },
        SourceHealth {
            state_database: HealthEntry::healthy("ready", 10),
            rollout_observer: HealthEntry::degraded("private detail", Some(9), 1),
        },
    ));
    assert_eq!(projection.active_work_count, 0);
    assert!(!projection.monitor_confident);

    let root = tempdir().unwrap();
    let app = ThemeApplication::for_state_directory(root.path().to_path_buf()).unwrap();
    let result = app.start_session_with_safety(projection, || panic!("must not restart"));
    assert_eq!(result.status, OperationStatus::Blocked);
    assert_eq!(result.reason_codes, vec![ThemeReasonCode::MonitorUncertain]);
}

#[test]
fn uncertain_monitor_does_not_block_a_no_restart_theme_switch() {
    let root = tempdir().unwrap();
    let app = ThemeApplication::for_state_directory(root.path().to_path_buf()).unwrap();
    app.start_session_with_safety(RestartSafetyProjection::confirmed(0), || Ok(session()));

    let result = app.activate_with_safety(
        "wisteria-bride",
        RestartSafetyProjection {
            active_work_count: 0,
            monitor_confident: false,
        },
        || panic!("verified session must be reused"),
        |_| Ok(1),
    );
    assert_eq!(result.status, OperationStatus::Applied);
}
