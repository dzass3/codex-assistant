use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

use codex_assistant_lib::monitor::runtime::MonitorRuntime;
use rusqlite::{params, Connection};
use serde_json::json;
use tempfile::tempdir;

const PRIVATE_PROMPT: &str = "CANARY_PRIVATE_PROMPT_8F41";
const PRIVATE_RESPONSE: &str = "CANARY_PRIVATE_RESPONSE_09D2";
const PRIVATE_TOOL_ARGS: &str = "CANARY_PRIVATE_TOOL_ARGS_E77A";

#[test]
fn fixture_reports_effective_child_model_without_mutating_or_leaking_content() {
    let home = tempdir().expect("temporary Codex home");
    let root_rollout = home.path().join("root.jsonl");
    let child_rollout = home.path().join("child.jsonl");
    create_state_database(home.path(), &root_rollout, &child_rollout);
    write_rollouts(&root_rollout, &child_rollout);

    let database = home.path().join("state_5.sqlite");
    let before = [
        fingerprint(&database),
        fingerprint(&root_rollout),
        fingerprint(&child_rollout),
    ];

    let runtime = MonitorRuntime::new(home.path().to_path_buf());
    let (snapshot, _) = runtime.refresh();
    let serialized = serde_json::to_string(&snapshot).expect("sanitized snapshot");

    let child = snapshot
        .agents
        .iter()
        .find(|agent| agent.thread_id == "child")
        .expect("child observation");
    assert_eq!(child.parent_thread_id.as_deref(), Some("root"));
    assert_eq!(child.requested_model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(child.effective_model.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(child.reasoning_effort.as_deref(), Some("high"));
    assert!(child.model_drift);
    assert_eq!(child.project.as_deref(), Some("sample-project"));

    for canary in [PRIVATE_PROMPT, PRIVATE_RESPONSE, PRIVATE_TOOL_ARGS] {
        assert!(!serialized.contains(canary));
    }
    assert!(!serialized.contains(home.path().to_string_lossy().as_ref()));

    let after = [
        fingerprint(&database),
        fingerprint(&root_rollout),
        fingerprint(&child_rollout),
    ];
    assert_eq!(before, after, "observer must not mutate Codex files");
}

fn create_state_database(home: &Path, root_rollout: &Path, child_rollout: &Path) {
    let connection = Connection::open(home.join("state_5.sqlite")).expect("fixture database");
    connection
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT,
                source TEXT,
                cwd TEXT,
                title TEXT,
                agent_nickname TEXT,
                agent_role TEXT,
                model TEXT,
                reasoning_effort TEXT,
                agent_path TEXT,
                created_at_ms INTEGER,
                updated_at_ms INTEGER
            );
            CREATE TABLE thread_spawn_edges (
                parent_thread_id TEXT,
                child_thread_id TEXT,
                status TEXT
            );",
        )
        .expect("fixture schema");
    connection
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, NULL, 100, 500)",
            params![
                "root",
                root_rollout.to_string_lossy(),
                "desktop",
                r"C:\private\sample-project",
                "Root task",
                "gpt-5.6-sol",
                "xhigh"
            ],
        )
        .expect("root thread");
    connection
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, 200, 600)",
            params![
                "child",
                child_rollout.to_string_lossy(),
                "desktop",
                r"C:\private\sample-project",
                "implementation",
                "worker",
                "gpt-5.6-sol",
                "high",
                "/root/implementation"
            ],
        )
        .expect("child thread");
    connection
        .execute(
            "INSERT INTO thread_spawn_edges VALUES ('root', 'child', 'open')",
            [],
        )
        .expect("spawn edge");
}

fn write_rollouts(root: &Path, child: &Path) {
    let spawn_arguments = json!({
        "task_name": "implementation",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "message": PRIVATE_PROMPT,
    })
    .to_string();
    let spawn_output = json!({ "agent_id": "child", "message": PRIVATE_RESPONSE }).to_string();
    let root_lines = [
        json!({"timestamp":"2026-07-18T08:00:00Z","type":"session_meta","payload":{"id":"root"}}),
        json!({"timestamp":"2026-07-18T08:00:01Z","type":"turn_context","payload":{"model":"gpt-5.6-sol","effort":"xhigh"}}),
        json!({"timestamp":"2026-07-18T08:00:02Z","type":"function_call","payload":{"call_id":"call-1","name":"spawn_agent","arguments":spawn_arguments}}),
        json!({"timestamp":"2026-07-18T08:00:03Z","type":"function_call_output","payload":{"call_id":"call-1","output":spawn_output}}),
        json!({"timestamp":"2026-07-18T08:00:04Z","type":"response_item","payload":{"content":PRIVATE_RESPONSE}}),
        json!({"timestamp":"2026-07-18T08:00:05Z","type":"custom_tool_call","payload":{"arguments":PRIVATE_TOOL_ARGS}}),
    ];
    let child_lines = [
        json!({"timestamp":"2026-07-18T08:00:03Z","type":"session_meta","payload":{"id":"child","parent_thread_id":"root"}}),
        json!({"timestamp":"2026-07-18T08:00:04Z","type":"turn_context","payload":{"model":"gpt-5.6-terra","effort":"high"}}),
        json!({"timestamp":"2026-07-18T08:00:05Z","type":"event_msg","payload":{"type":"task_started"}}),
        json!({"timestamp":"2026-07-18T08:00:06Z","type":"response_item","payload":{"content":PRIVATE_RESPONSE}}),
    ];
    fs::write(root, join_lines(&root_lines)).expect("root rollout");
    fs::write(child, join_lines(&child_lines)).expect("child rollout");
}

fn join_lines(lines: &[serde_json::Value]) -> String {
    let mut content = lines
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    content.push('\n');
    content
}

fn fingerprint(path: &Path) -> u64 {
    let bytes = fs::read(path).expect("fixture bytes");
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
