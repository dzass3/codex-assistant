use std::fs;

use codex_assistant_lib::routing::{
    state::{RoutingStateStore, STATE_SCHEMA_VERSION},
    EligibilityRecord, EligibilityStatus, ModelTier, RootRouteState, RouteActivity, RouteKind,
    RoutePhase, RoutingStateEnvelope,
};
use tempfile::tempdir;
use uuid::Uuid;

fn representative_state() -> RoutingStateEnvelope {
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    RoutingStateEnvelope {
        schema_version: STATE_SCHEMA_VERSION,
        profile_version: "routing-v1".to_owned(),
        routes: vec![RootRouteState {
            route_key,
            conversation_id,
            enabled: true,
            phase: RoutePhase::Implementing,
            created_at_ms: 1,
            updated_at_ms: 2,
        }],
        eligibility: vec![EligibilityRecord {
            tier: ModelTier::Terra,
            route_kind: RouteKind::Direct,
            status: EligibilityStatus::Eligible,
            checked_at_ms: 2,
            profile_version: "routing-v1".to_owned(),
        }],
        activity: vec![RouteActivity {
            route_key,
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Direct,
            phase: RoutePhase::Implementing,
            is_reviewer: false,
            escalation_count: 0,
            started_at_ms: 2,
            updated_at_ms: 3,
        }],
    }
}

#[test]
fn persisted_state_is_metadata_only_and_snapshot_is_sanitized() {
    let state = representative_state();
    let serialized = serde_json::to_string(&state)
        .expect("serializes state")
        .to_lowercase();
    for forbidden in [
        "prompt",
        "response",
        "reasoning",
        "command",
        "patch",
        "path",
        "token",
        "cookie",
        "secret",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "state contains {forbidden}"
        );
    }

    let snapshot = state.snapshot();
    assert_eq!(snapshot.schema_version, STATE_SCHEMA_VERSION);
    assert!(snapshot.routes[0].enabled);
    assert_eq!(
        snapshot.activity[0].child_thread_id,
        state.activity[0].child_thread_id
    );
}

#[test]
fn corrupt_state_is_quarantined_and_recovered_without_destroying_evidence() {
    let directory = tempdir().expect("state directory");
    let state_file = directory.path().join("routing-state.json");
    fs::write(&state_file, b"{ not valid json").expect("corrupt fixture");

    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let loaded = store.load().expect("recovers empty state");
    assert_eq!(loaded.schema_version, STATE_SCHEMA_VERSION);
    assert!(loaded.routes.is_empty());
    assert!(loaded.eligibility.is_empty());
    assert!(loaded.activity.is_empty());
    assert!(state_file.is_file(), "valid empty replacement is written");
    assert!(fs::read_to_string(&state_file)
        .expect("replacement")
        .contains("schema_version"));
    assert!(fs::read_dir(directory.path())
        .expect("directory")
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("routing-state.corrupt-")));
}

#[test]
fn state_with_unknown_content_field_is_quarantined() {
    let directory = tempdir().expect("state directory");
    let state_file = directory.path().join("routing-state.json");
    fs::write(
        &state_file,
        r#"{"schema_version":1,"profile_version":"routing-v1","routes":[],"eligibility":[],"activity":[],"prompt":"CANARY"}"#,
    )
    .expect("content-bearing fixture");

    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    assert!(store.load().expect("recovered").routes.is_empty());
    assert!(fs::read_dir(directory.path())
        .expect("directory")
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("routing-state.corrupt-")));
}
