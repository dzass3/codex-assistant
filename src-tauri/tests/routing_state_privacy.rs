use std::{fs, sync::Arc, thread};

use codex_assistant_lib::routing::{
    state::{RoutingRuntime, RoutingStateStore, STATE_SCHEMA_VERSION},
    EligibilityRecord, EligibilityStatus, ModelTier, RootRouteState, RouteActivity, RouteKind,
    RoutePhase, RouteReasonCode, RoutingStateEnvelope,
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
            parent_thread_id: conversation_id,
            escalation_count: 0,
            selected_tier: ModelTier::Terra,
            requested_tier: Some(ModelTier::Terra),
            effective_tier: Some(ModelTier::Terra),
            reason_codes: vec![codex_assistant_lib::routing::RouteReasonCode::CrossLayerWork],
            started_at_ms: 2,
            updated_at_ms: 3,
        }],
        quality: Vec::new(),
    }
}

#[test]
fn state_rejects_all_content_and_unknown_fields_at_every_level() {
    let directory = tempdir().expect("directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let mut state = serde_json::to_value(representative_state()).expect("state JSON");
    let slots = ["/profile_version", "/eligibility/0/profile_version"];
    for slot in slots {
        let mut candidate = state.clone();
        *candidate.pointer_mut(slot).expect("string slot") =
            serde_json::Value::String("CANARY PRIVATE PROMPT".to_owned());
        let parsed = serde_json::from_value::<RoutingStateEnvelope>(candidate).expect("shape");
        assert!(store.save(&parsed).is_err());
    }
    for pointer in ["", "/routes/0", "/eligibility/0", "/activity/0"] {
        let mut candidate = state.clone();
        let object = candidate
            .pointer_mut(pointer)
            .expect("object")
            .as_object_mut()
            .expect("map");
        object.insert(
            "secret".to_owned(),
            serde_json::Value::String("CANARY".to_owned()),
        );
        assert!(serde_json::from_value::<RoutingStateEnvelope>(candidate).is_err());
    }
    state["routes"][0]["created_at_ms"] = serde_json::json!(-1);
    let parsed = serde_json::from_value::<RoutingStateEnvelope>(state).expect("shape");
    assert!(store.save(&parsed).is_err());
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
fn second_save_replaces_existing_state_without_losing_valid_data() {
    let directory = tempdir().expect("state directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let first = representative_state();
    store.save(&first).expect("first save");
    let mut second = representative_state();
    second.routes[0].updated_at_ms = 77;
    store.save(&second).expect("second save");
    assert_eq!(store.load().expect("state"), second);
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

#[test]
fn malformed_state_is_quarantined_but_read_errors_are_returned() {
    let directory = tempdir().expect("state directory");
    let state_file = directory.path().join("routing-state.json");
    fs::create_dir(&state_file).expect("unreadable state fixture");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    assert!(store.load().is_err());
    assert!(
        state_file.is_dir(),
        "read failure must not quarantine evidence"
    );
}

#[test]
fn state_rejects_invalid_ids_timestamps_and_full_envelope_budgets() {
    let directory = tempdir().expect("directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let mut state = representative_state();
    state.routes[0].route_key = Uuid::nil();
    assert!(store.save(&state).is_err());

    let mut state = representative_state();
    state.routes[0].created_at_ms = 9_007_199_254_740_992;
    assert!(store.save(&state).is_err());

    let mut state = representative_state();
    state.activity[0].parent_thread_id = Uuid::new_v4();
    assert!(store.save(&state).is_err());

    let mut state = representative_state();
    let base = state.activity[0].clone();
    state.activity.extend([base.clone(), base.clone()]);
    for (index, entry) in state.activity.iter_mut().enumerate() {
        entry.child_thread_id = Uuid::new_v4();
        entry.subtask_id = Uuid::new_v4();
        entry.started_at_ms += index as i64;
        entry.updated_at_ms += index as i64;
    }
    assert!(
        store.save(&state).is_ok(),
        "three active children are valid"
    );
    state.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        subtask_id: Uuid::new_v4(),
        started_at_ms: 10,
        updated_at_ms: 10,
        ..base.clone()
    });
    assert!(
        store.save(&state).is_err(),
        "four active children are invalid"
    );

    let mut nested = representative_state();
    let nested_parent = nested.activity[0].child_thread_id;
    nested.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        parent_thread_id: nested_parent,
        subtask_id: Uuid::new_v4(),
        route_kind: RouteKind::Nested,
        started_at_ms: 4,
        updated_at_ms: 4,
        ..base.clone()
    });
    nested.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        parent_thread_id: nested_parent,
        subtask_id: Uuid::new_v4(),
        route_kind: RouteKind::Nested,
        started_at_ms: 5,
        updated_at_ms: 5,
        ..base
    });
    assert!(
        store.save(&nested).is_err(),
        "second nested child is invalid"
    );
}

#[test]
fn state_requires_exact_profile_and_verified_parent_lineage() {
    let directory = tempdir().expect("directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    for version in ["cm91dGluZy12MQ", "routing_v1", "routingv1"] {
        let mut state = representative_state();
        state.profile_version = version.to_owned();
        assert!(store.save(&state).is_err());
        let mut state = representative_state();
        state.eligibility[0].profile_version = version.to_owned();
        assert!(store.save(&state).is_err());
    }

    let mut nested = representative_state();
    nested.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        parent_thread_id: nested.activity[0].child_thread_id,
        subtask_id: Uuid::new_v4(),
        route_kind: RouteKind::Nested,
        selected_tier: ModelTier::Luna,
        requested_tier: Some(ModelTier::Luna),
        effective_tier: Some(ModelTier::Luna),
        started_at_ms: 4,
        updated_at_ms: 4,
        ..nested.activity[0].clone()
    });
    assert!(
        store.save(&nested).is_ok(),
        "direct implementation can parent one nested child"
    );

    nested.activity[0].is_reviewer = true;
    assert!(
        store.save(&nested).is_err(),
        "reviewer cannot parent nested work"
    );
}

#[test]
fn nested_work_requires_a_terra_parent_and_a_lower_tier_child() {
    let base = representative_state();

    let directory = tempdir().expect("non-Terra parent directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store.clone()).expect("runtime");
    let mut luna_parent = base.clone();
    luna_parent.activity[0].selected_tier = ModelTier::Luna;
    luna_parent.activity[0].requested_tier = Some(ModelTier::Luna);
    luna_parent.activity[0].effective_tier = Some(ModelTier::Luna);
    runtime
        .replace(luna_parent.clone())
        .expect("valid direct parent");
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Nested,
            parent_thread_id: luna_parent.activity[0].child_thread_id,
            selected_tier: ModelTier::Spark,
            requested_tier: Some(ModelTier::Spark),
            effective_tier: Some(ModelTier::Spark),
            ..luna_parent.activity[0].clone()
        }),
        Err(RouteReasonCode::NestedDelegationForbidden)
    );

    let directory = tempdir().expect("non-lower child directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store).expect("runtime");
    runtime.replace(base.clone()).expect("valid Terra parent");
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Nested,
            parent_thread_id: base.activity[0].child_thread_id,
            selected_tier: ModelTier::Terra,
            requested_tier: Some(ModelTier::Terra),
            effective_tier: Some(ModelTier::Terra),
            ..base.activity[0].clone()
        }),
        Err(RouteReasonCode::NestedDelegationForbidden)
    );
}

#[test]
fn a_subtask_cannot_start_a_replacement_while_its_previous_attempt_is_active() {
    let state = representative_state();
    let base = state.activity[0].clone();
    let directory = tempdir().expect("state directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store.clone()).expect("runtime");
    runtime.replace(state).expect("seed state");

    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            ..base.clone()
        }),
        Err(RouteReasonCode::PreviousAttemptStillActive)
    );

    let mut tampered = representative_state();
    let tampered_base = tampered.activity[0].clone();
    tampered.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        escalation_count: 1,
        ..tampered_base
    });
    assert!(
        store.save(&tampered).is_err(),
        "persisted state cannot bypass the sequential-attempt invariant"
    );
}

#[test]
fn state_requires_contiguous_implementation_escalations() {
    let directory = tempdir().expect("directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let mut state = representative_state();
    state.activity[0].phase = RoutePhase::Completed;
    let base = state.activity[0].clone();
    for escalation_count in [1, 2] {
        state.activity.push(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            escalation_count,
            started_at_ms: 10 + escalation_count as i64,
            updated_at_ms: 10 + escalation_count as i64,
            ..base.clone()
        });
    }
    assert!(store.save(&state).is_ok(), "attempts 0, 1, and 2 are valid");
    state.activity.push(RouteActivity {
        child_thread_id: Uuid::new_v4(),
        is_reviewer: true,
        escalation_count: 0,
        phase: RoutePhase::Completed,
        started_at_ms: 20,
        updated_at_ms: 20,
        ..base.clone()
    });
    assert!(
        store.save(&state).is_ok(),
        "a historical reviewer remains attached to the implementation it reviewed"
    );
    state.activity[2].escalation_count = 1;
    assert!(
        store.save(&state).is_err(),
        "escalation counts cannot reset or duplicate"
    );
}

#[test]
fn runtime_derives_precise_budget_reason_codes_from_locked_state() {
    let state = representative_state();
    let base = state.activity[0].clone();

    let directory = tempdir().expect("active limit directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store).expect("runtime");
    runtime.replace(state.clone()).expect("seed state");
    for subtask_id in [Uuid::new_v4(), Uuid::new_v4()] {
        runtime
            .try_start_activity(RouteActivity {
                child_thread_id: Uuid::new_v4(),
                subtask_id,
                ..base.clone()
            })
            .expect("within active fan-out limit");
    }
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            ..base.clone()
        }),
        Err(codex_assistant_lib::routing::RouteReasonCode::ActiveChildLimitReached)
    );

    let directory = tempdir().expect("nested limit directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store).expect("runtime");
    runtime.replace(state.clone()).expect("seed state");
    runtime
        .try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Nested,
            parent_thread_id: base.child_thread_id,
            selected_tier: ModelTier::Luna,
            requested_tier: Some(ModelTier::Luna),
            effective_tier: Some(ModelTier::Luna),
            ..base.clone()
        })
        .expect("first nested child");
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Nested,
            parent_thread_id: base.child_thread_id,
            selected_tier: ModelTier::Luna,
            requested_tier: Some(ModelTier::Luna),
            effective_tier: Some(ModelTier::Luna),
            ..base.clone()
        }),
        Err(codex_assistant_lib::routing::RouteReasonCode::NestedChildLimitReached)
    );

    let directory = tempdir().expect("escalation limit directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store).expect("runtime");
    let mut completed = state.clone();
    completed.activity[0].phase = RoutePhase::Completed;
    runtime.replace(completed).expect("seed state");
    for _ in 0..2 {
        runtime
            .try_start_activity(RouteActivity {
                child_thread_id: Uuid::new_v4(),
                phase: RoutePhase::Completed,
                ..base.clone()
            })
            .expect("permitted escalation");
    }
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            phase: RoutePhase::Completed,
            ..base.clone()
        }),
        Err(codex_assistant_lib::routing::RouteReasonCode::EscalationLimitReached)
    );

    let directory = tempdir().expect("reviewer lineage directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = RoutingRuntime::load(store).expect("runtime");
    runtime.replace(state).expect("seed state");
    let reviewer_id = Uuid::new_v4();
    runtime
        .try_start_activity(RouteActivity {
            child_thread_id: reviewer_id,
            is_reviewer: true,
            phase: RoutePhase::Completed,
            ..base.clone()
        })
        .expect("reviewer uses current implementation escalation");
    assert_eq!(
        runtime.try_start_activity(RouteActivity {
            child_thread_id: Uuid::new_v4(),
            subtask_id: Uuid::new_v4(),
            route_kind: RouteKind::Nested,
            parent_thread_id: reviewer_id,
            ..base
        }),
        Err(codex_assistant_lib::routing::RouteReasonCode::ReviewerRecursionForbidden)
    );
}

#[test]
fn runtime_replaces_state_as_one_validated_transaction_under_concurrency() {
    let directory = tempdir().expect("state directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    let runtime = Arc::new(RoutingRuntime::load(store.clone()).expect("runtime"));
    let first = representative_state();
    let mut second = representative_state();
    second.routes[0].updated_at_ms = 99;
    let one = Arc::clone(&runtime);
    let two = Arc::clone(&runtime);
    let first_thread = thread::spawn(move || one.replace(first));
    let second_thread = thread::spawn(move || two.replace(second));
    first_thread.join().expect("thread").expect("replace");
    second_thread.join().expect("thread").expect("replace");
    let memory = runtime.snapshot();
    let disk = store.load().expect("disk state").snapshot();
    assert_eq!(memory, disk);
}

#[cfg(windows)]
#[test]
fn windows_state_artifacts_have_current_user_only_dacls() {
    let directory = tempdir().expect("state directory");
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&representative_state()).expect("state save");
    assert!(store
        .has_current_user_only_acl(directory.path())
        .expect("directory ACL"));
    assert!(store
        .has_current_user_only_acl(&directory.path().join("routing-state.json"))
        .expect("state ACL"));
    fs::write(directory.path().join("routing-state.json"), b"{ invalid").expect("corrupt fixture");
    store.load().expect("recovery");
    let evidence = fs::read_dir(directory.path())
        .expect("directory")
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("routing-state.corrupt-")
        })
        .expect("quarantine evidence");
    assert!(store
        .has_current_user_only_acl(&evidence.path())
        .expect("evidence ACL"));
}
