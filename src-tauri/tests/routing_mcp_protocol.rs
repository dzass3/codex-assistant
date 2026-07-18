use std::path::Path;

use codex_assistant_lib::routing::{
    state::RoutingStateStore, EligibilityRecord, EligibilityStatus, ModelTier, QualityOutcome,
    RootRouteState, RouteKind, RoutePhase, RoutingStateEnvelope,
};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[tokio::test]
async fn initialize_then_list_tools_over_jsonl_and_exit_cleanly_at_eof() {
    let directory = tempdir().expect("state directory");
    let input = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {
                    "roots": {"listChanged": true},
                    "sampling": {"context": {}, "tools": {}},
                    "elicitation": {"form": {}, "url": {}},
                    "tasks": {
                        "list": {},
                        "cancel": {},
                        "requests": {
                            "sampling": {"createMessage": {}},
                            "elicitation": {"create": {}}
                        }
                    }
                },
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": "tools", "method": "tools/list", "params": {}}),
    ];
    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert!(diagnostics.is_empty());
    assert_eq!(
        responses.len(),
        2,
        "notifications must not produce responses"
    );
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        responses[0]["result"]["capabilities"]["tools"]["listChanged"],
        false
    );
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "codex-assistant-routing"
    );
    assert_eq!(responses[1]["id"], "tools");
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tool list");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>(),
        [
            "routing_policy_get",
            "routing_route_started",
            "routing_quality_record"
        ]
    );
    assert!(tools.iter().all(|tool| {
        tool["inputSchema"]["type"] == "object"
            && tool["inputSchema"]["additionalProperties"] == false
    }));
}

#[tokio::test]
async fn parse_unknown_method_and_invalid_id_errors_are_structured_and_sanitized() {
    let directory = tempdir().expect("state directory");
    let input = [
        "{not-json".to_owned(),
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }))
        .expect("initialize"),
        serde_json::to_string(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .expect("initialized"),
        serde_json::to_string(
            &json!({"jsonrpc": "2.0", "id": "unknown", "method": "unknown/method"}),
        )
        .expect("unknown method"),
        serde_json::to_string(
            &json!({"jsonrpc": "2.0", "id": true, "method": "tools/list", "params": {}}),
        )
        .expect("invalid id"),
    ];
    let (responses, diagnostics) = run_raw_session(directory.path(), &input).await;

    assert_eq!(responses.len(), 4);
    assert_eq!(responses[0]["id"], Value::Null);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[1]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(responses[2]["id"], "unknown");
    assert_eq!(responses[2]["error"]["code"], -32601);
    assert_eq!(responses[3]["id"], Value::Null);
    assert_eq!(responses[3]["error"]["code"], -32600);
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        [
            "routing_mcp_error code=parse_error count=1",
            "routing_mcp_error code=method_not_found count=2",
            "routing_mcp_error code=invalid_request count=3"
        ]
    );
    assert!(!diagnostics.contains("not-json"));
    assert!(!diagnostics.contains("unknown/method"));
}

#[tokio::test]
async fn unknown_fields_are_rejected_at_initialize_and_tool_call_schema_boundaries() {
    let directory = tempdir().expect("state directory");
    let input = [
        json!({
            "jsonrpc": "2.0",
            "id": "bad-init-params",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"},
                "unexpected": true
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "bad-client-info",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1", "unexpected": true}
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "bad-nested-capability",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {"roots": {"listChanged": false, "unexpected": true}},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": "bad-call",
            "method": "tools/call",
            "params": {"name": "routing_policy_get", "arguments": {}, "unexpected": true}
        }),
    ];

    let (responses, _) = run_session(directory.path(), &input).await;

    assert_eq!(responses[0]["error"]["code"], -32602);
    assert_eq!(responses[1]["error"]["code"], -32602);
    assert_eq!(responses[2]["error"]["code"], -32602);
    assert_eq!(responses[3]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(responses[4]["error"]["code"], -32602);
}

#[tokio::test]
async fn tools_are_unavailable_until_initialize_and_initialized_complete_in_order() {
    let directory = tempdir().expect("state directory");
    let input = [
        json!({"jsonrpc": "2.0", "id": "too-early", "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "id": "awaiting-notification", "method": "tools/list", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": "ready", "method": "tools/list", "params": {}}),
    ];

    let (responses, _) = run_session(directory.path(), &input).await;

    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[1]["id"], "initialize");
    assert_eq!(responses[2]["error"]["code"], -32002);
    assert_eq!(responses[3]["id"], "ready");
    assert!(responses[3]["result"]["tools"].is_array());
}

#[tokio::test]
async fn lifecycle_requests_require_ids_and_initialized_must_be_a_notification() {
    let directory = tempdir().expect("state directory");
    let input = [
        json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "id": "not-a-notification", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": "still-waiting", "method": "tools/list", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "method": "tools/list", "params": {}}),
        json!({"jsonrpc": "2.0", "id": "ready", "method": "tools/list", "params": {}}),
    ];

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["error"]["code"], -32600);
    assert_eq!(responses[1]["id"], "initialize");
    assert_eq!(responses[2]["error"]["code"], -32600);
    assert_eq!(responses[3]["error"]["code"], -32002);
    assert_eq!(responses[4]["error"]["code"], -32600);
    assert_eq!(responses[5]["id"], "ready");
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        [
            "routing_mcp_error code=invalid_request count=1",
            "routing_mcp_error code=invalid_request count=2",
            "routing_mcp_error code=server_not_initialized count=3",
            "routing_mcp_error code=invalid_request count=4",
        ]
    );
}

#[tokio::test]
async fn duplicate_or_early_lifecycle_messages_are_invalid_without_corrupting_the_session() {
    let directory = tempdir().expect("state directory");
    let input = [
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": "duplicate-initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": "ready", "method": "tools/list", "params": {}}),
    ];

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["id"], "initialize");
    assert_eq!(responses[1]["id"], "duplicate-initialize");
    assert_eq!(responses[1]["error"]["code"], -32600);
    assert_eq!(responses[2]["id"], "ready");
    assert_eq!(
        diagnostics.lines().collect::<Vec<_>>(),
        [
            "routing_mcp_error code=invalid_request count=1",
            "routing_mcp_error code=invalid_request count=2",
            "routing_mcp_error code=invalid_request count=3",
        ]
    );
}

#[tokio::test]
async fn policy_get_returns_only_sanitized_metadata_without_mutating_state() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    state.eligibility.push(EligibilityRecord {
        tier: ModelTier::Luna,
        route_kind: RouteKind::Direct,
        status: EligibilityStatus::Eligible,
        checked_at_ms: 1,
        profile_version: "routing-v1".into(),
        codex_package_version: "1.2.3".into(),
        requested_model: ModelTier::Luna.model_id().into(),
        depth: 1,
        reason: None,
    });
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let state_file = directory.path().join("routing-state.json");
    let before = std::fs::read(&state_file).expect("state bytes");
    let input = initialized_requests(json!({
        "jsonrpc": "2.0",
        "id": "policy",
        "method": "tools/call",
        "params": {
            "name": "routing_policy_get",
            "arguments": {"route_key": route_key}
        }
    }));

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert!(diagnostics.is_empty());
    let result = &responses[1]["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(
        result["structuredContent"]["route_key"],
        route_key.to_string()
    );
    assert_eq!(result["structuredContent"]["profile_version"], "routing-v1");
    assert_eq!(result["structuredContent"]["policy_version"], "routing-v1");
    assert_eq!(
        result["structuredContent"]["budgets"]["max_active_children"],
        3
    );
    assert_eq!(
        result["structuredContent"]["budgets"]["max_nested_children"],
        1
    );
    assert_eq!(
        result["structuredContent"]["budgets"]["max_automatic_escalations"],
        2
    );
    assert_eq!(
        result["structuredContent"]["eligibility"][0]["tier"],
        "luna"
    );
    assert!(result["structuredContent"]["reason_codes"]
        .as_array()
        .expect("reason vocabulary")
        .contains(&json!("no-eligible-tier")));
    let text = result["content"][0]["text"]
        .as_str()
        .expect("metadata text");
    assert_eq!(
        serde_json::from_str::<Value>(text).expect("text JSON"),
        result["structuredContent"]
    );
    assert_eq!(std::fs::read(state_file).expect("state bytes"), before);
}

#[tokio::test]
async fn route_started_persists_valid_metadata_and_rejects_bad_lineage_or_enums() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let child_thread_id = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    state.eligibility.push(EligibilityRecord {
        tier: ModelTier::Luna,
        route_kind: RouteKind::Direct,
        status: EligibilityStatus::Eligible,
        checked_at_ms: 1,
        profile_version: "routing-v1".into(),
        codex_package_version: "1.2.3".into(),
        requested_model: ModelTier::Luna.model_id().into(),
        depth: 1,
        reason: None,
    });
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let mut input = initialized_requests(route_started_call(
        "valid",
        route_key,
        child_thread_id,
        conversation_id,
        "bounded-work",
    ));
    input.push(route_started_call(
        "bad-parent",
        route_key,
        Uuid::new_v4(),
        Uuid::new_v4(),
        "bounded-work",
    ));
    input.push(route_started_call(
        "bad-reason",
        route_key,
        Uuid::new_v4(),
        conversation_id,
        "private-task-reason",
    ));

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert_eq!(responses[1]["id"], "valid");
    assert_eq!(responses[1]["result"]["isError"], false);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["recorded"],
        true
    );
    assert_eq!(responses[2]["result"]["isError"], true);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["code"],
        "parent-lineage-mismatch"
    );
    assert_eq!(responses[3]["error"]["code"], -32602);
    assert!(diagnostics
        .lines()
        .all(|line| line.starts_with("routing_mcp_error code=") && !line.contains("private")));

    let persisted = store.load().expect("persisted state");
    assert_eq!(persisted.activity.len(), 1);
    let activity = &persisted.activity[0];
    assert_eq!(activity.route_key, route_key);
    assert_eq!(activity.child_thread_id, child_thread_id);
    assert_eq!(activity.parent_thread_id, conversation_id);
    assert_eq!(activity.selected_tier, ModelTier::Luna);
    assert_eq!(activity.requested_tier, Some(ModelTier::Luna));
    assert_eq!(activity.effective_tier, None);
    assert_eq!(activity.phase, RoutePhase::Implementing);
}

#[tokio::test]
async fn route_started_rejects_profiles_below_the_classified_quality_floor() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    for tier in [
        ModelTier::Spark,
        ModelTier::Luna,
        ModelTier::Terra,
        ModelTier::Sol,
    ] {
        state.eligibility.push(EligibilityRecord {
            tier,
            route_kind: RouteKind::Direct,
            status: EligibilityStatus::Eligible,
            checked_at_ms: 1,
            profile_version: "routing-v1".into(),
            codex_package_version: "1.2.3".into(),
            requested_model: tier.model_id().into(),
            depth: 1,
            reason: None,
        });
    }
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let classified = [
        ("bounded-spark", "spark", "bounded", "low", "bounded-work"),
        (
            "cross-layer-luna",
            "luna",
            "cross-layer",
            "meaningful",
            "cross-layer-work",
        ),
        (
            "high-risk-terra",
            "terra",
            "mechanical",
            "high",
            "high-risk-work",
        ),
        (
            "architectural-terra",
            "terra",
            "architectural",
            "low",
            "architectural-work",
        ),
        (
            "mismatched-reason",
            "spark",
            "mechanical",
            "low",
            "bounded-work",
        ),
        (
            "mechanical-spark",
            "spark",
            "mechanical",
            "low",
            "mechanical-work",
        ),
    ];
    let mut input = initialized_requests(route_started_call_classified(
        classified[0].0,
        route_key,
        Uuid::new_v4(),
        Uuid::new_v4(),
        conversation_id,
        classified[0].1,
        classified[0].2,
        classified[0].3,
        classified[0].4,
    ));
    for (id, profile, complexity, risk, reason) in classified.into_iter().skip(1) {
        input.push(route_started_call_classified(
            id,
            route_key,
            Uuid::new_v4(),
            Uuid::new_v4(),
            conversation_id,
            profile,
            complexity,
            risk,
            reason,
        ));
    }

    let (responses, _) = run_session(directory.path(), &input).await;

    for response in &responses[1..5] {
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["structuredContent"]["code"],
            "override-below-floor"
        );
    }
    assert_eq!(responses[5]["id"], "mismatched-reason");
    assert_eq!(responses[5]["error"]["code"], -32602);
    assert_eq!(responses[6]["id"], "mechanical-spark");
    assert_eq!(responses[6]["result"]["isError"], false);
    assert_eq!(store.load().expect("persisted state").activity.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_sidecars_serialize_state_updates_and_enforce_the_active_child_budget() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    state.eligibility.push(EligibilityRecord {
        tier: ModelTier::Luna,
        route_kind: RouteKind::Direct,
        status: EligibilityStatus::Eligible,
        checked_at_ms: 1,
        profile_version: "routing-v1".into(),
        codex_package_version: "1.2.3".into(),
        requested_model: ModelTier::Luna.model_id().into(),
        depth: 1,
        reason: None,
    });
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let state_directory = directory.path().to_path_buf();
    let tasks = (0..3)
        .map(|index| {
            let state_directory = state_directory.clone();
            tokio::spawn(async move {
                run_session(
                    &state_directory,
                    &initialized_requests(route_started_call(
                        &format!("child-{index}"),
                        route_key,
                        Uuid::new_v4(),
                        conversation_id,
                        "bounded-work",
                    )),
                )
                .await
            })
        })
        .collect::<Vec<_>>();
    for task in tasks {
        let (responses, diagnostics) = task.await.expect("sidecar task");
        assert_eq!(
            responses[1]["result"]["isError"], false,
            "response={:?}, diagnostics={diagnostics}",
            responses[1]
        );
    }
    assert_eq!(store.load().expect("state").activity.len(), 3);

    let (responses, _) = run_session(
        directory.path(),
        &initialized_requests(route_started_call(
            "over-budget",
            route_key,
            Uuid::new_v4(),
            conversation_id,
            "bounded-work",
        )),
    )
    .await;
    assert_eq!(responses[1]["result"]["isError"], true);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["code"],
        "active-child-limit-reached"
    );
    assert_eq!(store.load().expect("state").activity.len(), 3);
}

#[tokio::test]
async fn quality_record_completes_the_child_once_and_persists_bounded_metadata() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let child_thread_id = Uuid::new_v4();
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    state.eligibility.push(EligibilityRecord {
        tier: ModelTier::Luna,
        route_kind: RouteKind::Direct,
        status: EligibilityStatus::Eligible,
        checked_at_ms: 1,
        profile_version: "routing-v1".into(),
        codex_package_version: "1.2.3".into(),
        requested_model: ModelTier::Luna.model_id().into(),
        depth: 1,
        reason: None,
    });
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let mut input = initialized_requests(route_started_call(
        "start",
        route_key,
        child_thread_id,
        conversation_id,
        "bounded-work",
    ));
    input.push(quality_call(
        "quality",
        route_key,
        child_thread_id,
        "passed",
        Some("terra"),
        0,
        0,
    ));
    input.push(route_started_call(
        "reactivate-terminal-child",
        route_key,
        child_thread_id,
        conversation_id,
        "bounded-work",
    ));
    input.push(quality_call(
        "duplicate-quality",
        route_key,
        child_thread_id,
        "failed",
        Some("terra"),
        1,
        0,
    ));

    let (responses, _) = run_session(directory.path(), &input).await;

    assert_eq!(responses[2]["result"]["isError"], false);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["recorded"],
        true
    );
    assert_eq!(responses[3]["result"]["isError"], true);
    assert_eq!(
        responses[3]["result"]["structuredContent"]["code"],
        "terminal-child-reactivation"
    );
    assert_eq!(responses[4]["result"]["isError"], true);
    assert_eq!(
        responses[4]["result"]["structuredContent"]["code"],
        "quality-already-recorded"
    );
    let persisted = store.load().expect("persisted state");
    assert_eq!(persisted.quality.len(), 1);
    assert_eq!(persisted.quality[0].route_key, route_key);
    assert_eq!(persisted.quality[0].child_thread_id, child_thread_id);
    assert_eq!(persisted.quality[0].outcome, QualityOutcome::Passed);
    assert_eq!(persisted.quality[0].reviewer_tier, Some(ModelTier::Terra));
    assert_eq!(persisted.quality[0].retry_count, 0);
    assert_eq!(persisted.quality[0].escalation_count, 0);
    assert_eq!(persisted.activity[0].phase, RoutePhase::Completed);
}

#[tokio::test]
async fn one_opaque_subtask_can_escalate_twice_but_never_start_a_fourth_attempt() {
    let directory = tempdir().expect("state directory");
    let route_key = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let subtask_id = Uuid::new_v4();
    let children = [
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ];
    let mut state = RoutingStateEnvelope::empty("routing-v1");
    state.routes.push(RootRouteState {
        route_key,
        conversation_id,
        enabled: true,
        phase: RoutePhase::Enabled,
        created_at_ms: 1,
        updated_at_ms: 1,
    });
    for tier in [ModelTier::Luna, ModelTier::Terra, ModelTier::Sol] {
        state.eligibility.push(EligibilityRecord {
            tier,
            route_kind: RouteKind::Direct,
            status: EligibilityStatus::Eligible,
            checked_at_ms: 1,
            profile_version: "routing-v1".into(),
            codex_package_version: "1.2.3".into(),
            requested_model: tier.model_id().into(),
            depth: 1,
            reason: None,
        });
    }
    let store = RoutingStateStore::in_directory(directory.path()).expect("store");
    store.save(&state).expect("state");
    let mut input = initialized_requests(route_started_call_for_subtask(
        "attempt-0",
        route_key,
        children[0],
        subtask_id,
        conversation_id,
        "luna",
        "bounded-work",
    ));
    input.push(route_started_call_for_subtask(
        "premature-replacement",
        route_key,
        Uuid::new_v4(),
        subtask_id,
        conversation_id,
        "luna",
        "bounded-work",
    ));
    input.push(quality_call(
        "quality-0",
        route_key,
        children[0],
        "failed",
        Some("terra"),
        0,
        0,
    ));
    input.push(route_started_call_classified(
        "attempt-1",
        route_key,
        children[1],
        subtask_id,
        conversation_id,
        "terra",
        "cross-layer",
        "low",
        "cross-layer-work",
    ));
    input.push(quality_call(
        "quality-1",
        route_key,
        children[1],
        "degraded",
        Some("sol"),
        1,
        1,
    ));
    input.push(route_started_call_classified(
        "attempt-2",
        route_key,
        children[2],
        subtask_id,
        conversation_id,
        "sol",
        "mechanical",
        "high",
        "high-risk-work",
    ));
    input.push(quality_call(
        "quality-2",
        route_key,
        children[2],
        "passed",
        Some("sol"),
        0,
        2,
    ));
    input.push(route_started_call_classified(
        "attempt-3",
        route_key,
        children[3],
        subtask_id,
        conversation_id,
        "sol",
        "mechanical",
        "high",
        "high-risk-work",
    ));

    let (responses, diagnostics) = run_session(directory.path(), &input).await;

    assert!(diagnostics.contains("routing_mcp_error code=previous-attempt-still-active count=1"));
    assert!(diagnostics.ends_with("routing_mcp_error code=escalation-limit-reached count=2\n"));
    assert_eq!(
        responses[1]["result"]["structuredContent"]["escalation_count"],
        0
    );
    assert_eq!(
        responses[2]["result"]["structuredContent"]["code"],
        "previous-attempt-still-active"
    );
    assert_eq!(
        responses[4]["result"]["structuredContent"]["escalation_count"],
        1
    );
    assert_eq!(
        responses[6]["result"]["structuredContent"]["escalation_count"],
        2
    );
    assert_eq!(responses[8]["result"]["isError"], true);
    assert_eq!(
        responses[8]["result"]["structuredContent"]["code"],
        "escalation-limit-reached"
    );
    let persisted = store.load().expect("persisted state");
    assert_eq!(persisted.activity.len(), 3);
    assert!(persisted
        .activity
        .iter()
        .all(|activity| activity.subtask_id == subtask_id));
    assert_eq!(
        persisted
            .activity
            .iter()
            .map(|activity| activity.escalation_count)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

fn quality_call(
    id: &str,
    route_key: Uuid,
    child_thread_id: Uuid,
    outcome: &str,
    reviewer_tier: Option<&str>,
    retry_count: u8,
    escalation_count: u8,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "routing_quality_record",
            "arguments": {
                "route_key": route_key,
                "child_thread_id": child_thread_id,
                "outcome": outcome,
                "reviewer_tier": reviewer_tier,
                "retry_count": retry_count,
                "escalation_count": escalation_count
            }
        }
    })
}

fn route_started_call(
    id: &str,
    route_key: Uuid,
    child_thread_id: Uuid,
    parent_thread_id: Uuid,
    reason: &str,
) -> Value {
    route_started_call_for_subtask(
        id,
        route_key,
        child_thread_id,
        child_thread_id,
        parent_thread_id,
        "luna",
        reason,
    )
}

fn route_started_call_for_subtask(
    id: &str,
    route_key: Uuid,
    child_thread_id: Uuid,
    subtask_id: Uuid,
    parent_thread_id: Uuid,
    selected_profile: &str,
    reason: &str,
) -> Value {
    route_started_call_classified(
        id,
        route_key,
        child_thread_id,
        subtask_id,
        parent_thread_id,
        selected_profile,
        "bounded",
        "low",
        reason,
    )
}

#[allow(clippy::too_many_arguments)]
fn route_started_call_classified(
    id: &str,
    route_key: Uuid,
    child_thread_id: Uuid,
    subtask_id: Uuid,
    parent_thread_id: Uuid,
    selected_profile: &str,
    complexity_band: &str,
    risk_band: &str,
    reason: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "routing_route_started",
            "arguments": {
                "route_key": route_key,
                "child_thread_id": child_thread_id,
                "subtask_id": subtask_id,
                "parent_thread_id": parent_thread_id,
                "selected_profile": selected_profile,
                "route_kind": "direct",
                "complexity_band": complexity_band,
                "risk_band": risk_band,
                "reason_codes": [reason]
            }
        }
    })
}

fn initialized_requests(call: Value) -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "protocol-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        call,
    ]
}

async fn run_session(state_directory: &Path, input: &[Value]) -> (Vec<Value>, String) {
    let lines = input
        .iter()
        .map(|message| serde_json::to_string(message).expect("request JSON"))
        .collect::<Vec<_>>();
    run_raw_session(state_directory, &lines).await
}

async fn run_raw_session(state_directory: &Path, input: &[String]) -> (Vec<Value>, String) {
    let (mut request_writer, request_reader) = tokio::io::duplex(64 * 1024);
    let (response_writer, mut response_reader) = tokio::io::duplex(64 * 1024);
    let (diagnostic_writer, mut diagnostic_reader) = tokio::io::duplex(64 * 1024);
    let directory = state_directory.to_path_buf();
    let server = tokio::spawn(async move {
        codex_assistant_lib::routing_mcp::serve(
            request_reader,
            response_writer,
            diagnostic_writer,
            directory,
        )
        .await
    });
    for line in input {
        request_writer
            .write_all(line.as_bytes())
            .await
            .expect("write request");
        request_writer.write_all(b"\n").await.expect("newline");
    }
    request_writer.shutdown().await.expect("request EOF");
    server.await.expect("server task").expect("graceful EOF");

    let mut stdout = Vec::new();
    response_reader
        .read_to_end(&mut stdout)
        .await
        .expect("read responses");
    let mut stderr = Vec::new();
    diagnostic_reader
        .read_to_end(&mut stderr)
        .await
        .expect("read diagnostics");
    let responses = String::from_utf8(stdout)
        .expect("UTF-8 responses")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON response per line"))
        .collect();
    (
        responses,
        String::from_utf8(stderr).expect("UTF-8 diagnostics"),
    )
}
