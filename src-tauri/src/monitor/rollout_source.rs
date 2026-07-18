use std::{
    collections::hash_map::DefaultHasher,
    collections::HashMap,
    fs::File,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use chrono::DateTime;
use serde::Deserialize;

use super::{
    model::{SourceError, SourceErrorCode, SourceResult, SpawnFact},
    sqlite_source::StateFacts,
};

const MAX_BYTES_PER_FILE_REFRESH: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolloutThreadFact {
    pub thread_id: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub latest_task_started_ms: Option<i64>,
    pub latest_task_completed_ms: Option<i64>,
    pub interrupted_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct RolloutFacts {
    pub threads: HashMap<String, RolloutThreadFact>,
    pub spawns: Vec<SpawnFact>,
    pub parse_errors: u64,
    pub backlog: bool,
}

#[derive(Debug, Clone)]
struct PendingSpawn {
    requested_model: Option<String>,
    requested_effort: Option<String>,
    task_name: Option<String>,
    occurred_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct FileCursor {
    offset: u64,
    prefix_len: u64,
    prefix_hash: Option<u64>,
    tail_hash: Option<u64>,
    pending_spawns: HashMap<String, PendingSpawn>,
}

#[derive(Debug, Default)]
pub struct RolloutIndex {
    cursors: HashMap<PathBuf, FileCursor>,
    facts: RolloutFacts,
}

impl RolloutIndex {
    pub fn refresh(&mut self, state: &StateFacts) -> SourceResult<RolloutFacts> {
        let files: Vec<(String, PathBuf)> = state
            .threads
            .iter()
            .filter_map(|thread| {
                thread
                    .rollout_path
                    .clone()
                    .map(|path| (thread.fact.thread_id.clone(), path))
            })
            .collect();

        let mut had_readable_file = files.is_empty();
        self.facts.backlog = false;
        for (thread_id, path) in files {
            if !path.is_file() {
                continue;
            }
            had_readable_file = true;
            self.refresh_file(&thread_id, &path)?;
        }

        if !had_readable_file {
            return Err(SourceError::new(
                SourceErrorCode::Missing,
                "Codex rollout metadata is unavailable",
            ));
        }
        Ok(self.facts.clone())
    }

    fn refresh_file(&mut self, thread_id: &str, path: &Path) -> SourceResult<()> {
        let metadata = path.metadata().map_err(map_io_error)?;
        let length = metadata.len();
        let tail_hash = tail_fingerprint(path, length)?;
        let cursor = self.cursors.entry(path.to_path_buf()).or_default();
        let prefix_changed = if cursor.prefix_len > 0 && length >= cursor.prefix_len {
            cursor.prefix_hash != Some(range_fingerprint(path, 0, cursor.prefix_len)?)
        } else {
            false
        };
        let same_length_rewritten = length == cursor.offset
            && cursor.tail_hash.is_some()
            && cursor.tail_hash != Some(tail_hash);
        if length < cursor.offset || prefix_changed || same_length_rewritten {
            cursor.offset = 0;
            cursor.prefix_len = 0;
            cursor.prefix_hash = None;
            cursor.pending_spawns.clear();
            self.facts.threads.remove(thread_id);
        }
        if length == cursor.offset {
            cursor.tail_hash = Some(tail_hash);
            return Ok(());
        }

        let file = File::open(path).map_err(map_io_error)?;
        let mut reader = BufReader::new(file);
        reader
            .seek(SeekFrom::Start(cursor.offset))
            .map_err(map_io_error)?;
        let start_offset = cursor.offset;
        let mut line = String::new();

        loop {
            line.clear();
            let bytes = reader.read_line(&mut line).map_err(map_io_error)?;
            if bytes == 0 {
                break;
            }
            cursor.offset = cursor.offset.saturating_add(bytes as u64);
            match parse_line(&line, &mut cursor.pending_spawns) {
                Ok(Some(record)) => apply_record(&mut self.facts, thread_id, record),
                Ok(None) => {}
                Err(()) => self.facts.parse_errors = self.facts.parse_errors.saturating_add(1),
            }
            if cursor.offset.saturating_sub(start_offset) >= MAX_BYTES_PER_FILE_REFRESH {
                self.facts.backlog = cursor.offset < length;
                break;
            }
        }
        cursor.prefix_len = cursor.offset.min(4096);
        cursor.prefix_hash = Some(range_fingerprint(path, 0, cursor.prefix_len)?);
        cursor.tail_hash = Some(tail_hash);
        Ok(())
    }
}

#[derive(Debug)]
enum WhitelistRecord {
    SessionMeta {
        thread_id: Option<String>,
        parent_thread_id: Option<String>,
    },
    TurnContext {
        model: Option<String>,
        effort: Option<String>,
        occurred_at_ms: Option<i64>,
    },
    TaskStarted {
        occurred_at_ms: Option<i64>,
    },
    TaskCompleted {
        occurred_at_ms: Option<i64>,
    },
    SubagentActivity {
        child_thread_id: String,
        kind: String,
        occurred_at_ms: Option<i64>,
    },
    SpawnResolved {
        child_thread_id: String,
        requested_model: Option<String>,
        requested_effort: Option<String>,
        task_name: Option<String>,
        occurred_at_ms: Option<i64>,
    },
    SpawnRequested,
}

#[derive(Deserialize)]
struct Envelope<T> {
    timestamp: Option<String>,
    payload: T,
}

#[derive(Deserialize)]
struct SessionMetaPayload {
    id: Option<String>,
    session_id: Option<String>,
    parent_thread_id: Option<String>,
}

#[derive(Deserialize)]
struct TurnContextPayload {
    model: Option<String>,
    effort: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(Deserialize)]
struct EventPayload {
    #[serde(rename = "type")]
    event_type: String,
    agent_thread_id: Option<String>,
    kind: Option<String>,
    occurred_at_ms: Option<i64>,
}

#[derive(Deserialize)]
struct FunctionCallPayload {
    call_id: String,
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct FunctionOutputPayload {
    call_id: String,
    output: String,
}

#[derive(Deserialize)]
struct SpawnArguments {
    task_name: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(Deserialize)]
struct SpawnOutput {
    agent_id: Option<String>,
}

fn parse_line(
    line: &str,
    pending: &mut HashMap<String, PendingSpawn>,
) -> Result<Option<WhitelistRecord>, ()> {
    if line.contains(r#""type":"turn_context""#) {
        let envelope: Envelope<TurnContextPayload> = serde_json::from_str(line).map_err(|_| ())?;
        return Ok(Some(WhitelistRecord::TurnContext {
            model: clean(envelope.payload.model),
            effort: clean(
                envelope
                    .payload
                    .effort
                    .or(envelope.payload.reasoning_effort),
            ),
            occurred_at_ms: parse_timestamp_ms(envelope.timestamp.as_deref()),
        }));
    }

    if line.contains(r#""type":"session_meta""#) {
        let envelope: Envelope<SessionMetaPayload> = serde_json::from_str(line).map_err(|_| ())?;
        return Ok(Some(WhitelistRecord::SessionMeta {
            thread_id: clean(envelope.payload.session_id.or(envelope.payload.id)),
            parent_thread_id: clean(envelope.payload.parent_thread_id),
        }));
    }

    if line.contains(r#""type":"event_msg""#)
        && (line.contains(r#""type":"task_started""#)
            || line.contains(r#""type":"task_complete""#)
            || line.contains(r#""type":"sub_agent_activity""#))
    {
        let envelope: Envelope<EventPayload> = serde_json::from_str(line).map_err(|_| ())?;
        let outer_time = parse_timestamp_ms(envelope.timestamp.as_deref());
        return match envelope.payload.event_type.as_str() {
            "task_started" => Ok(Some(WhitelistRecord::TaskStarted {
                occurred_at_ms: outer_time,
            })),
            "task_complete" => Ok(Some(WhitelistRecord::TaskCompleted {
                occurred_at_ms: outer_time,
            })),
            "sub_agent_activity" => {
                let Some(child_thread_id) = clean(envelope.payload.agent_thread_id) else {
                    return Err(());
                };
                Ok(Some(WhitelistRecord::SubagentActivity {
                    child_thread_id,
                    kind: clean(envelope.payload.kind).unwrap_or_default(),
                    occurred_at_ms: envelope.payload.occurred_at_ms.or(outer_time),
                }))
            }
            _ => Ok(None),
        };
    }

    if line.contains(r#""type":"function_call""#) && line.contains(r#""name":"spawn_agent""#) {
        let envelope: Envelope<FunctionCallPayload> = serde_json::from_str(line).map_err(|_| ())?;
        if envelope.payload.name != "spawn_agent" {
            return Ok(None);
        }
        let arguments: SpawnArguments =
            serde_json::from_str(&envelope.payload.arguments).map_err(|_| ())?;
        pending.insert(
            envelope.payload.call_id,
            PendingSpawn {
                requested_model: clean(arguments.model),
                requested_effort: clean(arguments.reasoning_effort),
                task_name: clean(arguments.task_name),
                occurred_at_ms: parse_timestamp_ms(envelope.timestamp.as_deref()),
            },
        );
        return Ok(Some(WhitelistRecord::SpawnRequested));
    }

    if line.contains(r#""type":"function_call_output""#)
        && pending.keys().any(|call_id| line.contains(call_id))
    {
        let envelope: Envelope<FunctionOutputPayload> =
            serde_json::from_str(line).map_err(|_| ())?;
        let Some(request) = pending.remove(&envelope.payload.call_id) else {
            return Ok(None);
        };
        let output: SpawnOutput = serde_json::from_str(&envelope.payload.output).map_err(|_| ())?;
        let Some(child_thread_id) = clean(output.agent_id) else {
            return Err(());
        };
        return Ok(Some(WhitelistRecord::SpawnResolved {
            child_thread_id,
            requested_model: request.requested_model,
            requested_effort: request.requested_effort,
            task_name: request.task_name,
            occurred_at_ms: request
                .occurred_at_ms
                .or_else(|| parse_timestamp_ms(envelope.timestamp.as_deref())),
        }));
    }

    Ok(None)
}

fn apply_record(facts: &mut RolloutFacts, thread_id: &str, record: WhitelistRecord) {
    match record {
        WhitelistRecord::SessionMeta {
            thread_id: observed_id,
            parent_thread_id,
        } => {
            let id = observed_id.as_deref().unwrap_or(thread_id);
            let entry = facts.threads.entry(id.to_owned()).or_default();
            entry.thread_id = id.to_owned();
            let _ = parent_thread_id;
        }
        WhitelistRecord::TurnContext {
            model,
            effort,
            occurred_at_ms,
        } => {
            let entry = thread_entry(facts, thread_id);
            if model.is_some() {
                entry.model = model;
            }
            if effort.is_some() {
                entry.reasoning_effort = effort;
            }
            entry.updated_at_ms = entry.updated_at_ms.max(occurred_at_ms);
        }
        WhitelistRecord::TaskStarted { occurred_at_ms } => {
            let entry = thread_entry(facts, thread_id);
            entry.latest_task_started_ms = entry.latest_task_started_ms.max(occurred_at_ms);
            entry.updated_at_ms = entry.updated_at_ms.max(occurred_at_ms);
        }
        WhitelistRecord::TaskCompleted { occurred_at_ms } => {
            let entry = thread_entry(facts, thread_id);
            entry.latest_task_completed_ms = entry.latest_task_completed_ms.max(occurred_at_ms);
            entry.updated_at_ms = entry.updated_at_ms.max(occurred_at_ms);
        }
        WhitelistRecord::SubagentActivity {
            child_thread_id,
            kind,
            occurred_at_ms,
        } => {
            let entry = thread_entry(facts, &child_thread_id);
            if kind == "interrupted" {
                entry.interrupted_at_ms = entry.interrupted_at_ms.max(occurred_at_ms);
            }
            entry.updated_at_ms = entry.updated_at_ms.max(occurred_at_ms);
        }
        WhitelistRecord::SpawnResolved {
            child_thread_id,
            requested_model,
            requested_effort,
            task_name,
            occurred_at_ms,
        } => facts.spawns.push(SpawnFact {
            child_thread_id,
            requested_model,
            requested_effort,
            task_name,
            occurred_at_ms,
        }),
        WhitelistRecord::SpawnRequested => {}
    }
}

fn thread_entry<'a>(facts: &'a mut RolloutFacts, thread_id: &str) -> &'a mut RolloutThreadFact {
    let entry = facts.threads.entry(thread_id.to_owned()).or_default();
    if entry.thread_id.is_empty() {
        entry.thread_id = thread_id.to_owned();
    }
    entry
}

fn clean(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn parse_timestamp_ms(timestamp: Option<&str>) -> Option<i64> {
    timestamp
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp_millis())
}

fn map_io_error(_: std::io::Error) -> SourceError {
    SourceError::new(
        SourceErrorCode::Io,
        "Codex rollout metadata could not be read",
    )
}

fn tail_fingerprint(path: &Path, length: u64) -> SourceResult<u64> {
    const TAIL_BYTES: u64 = 4096;
    let start = length.saturating_sub(TAIL_BYTES);
    range_fingerprint(path, start, length - start)
}

fn range_fingerprint(path: &Path, start: u64, length: u64) -> SourceResult<u64> {
    let mut file = File::open(path).map_err(map_io_error)?;
    file.seek(SeekFrom::Start(start)).map_err(map_io_error)?;
    let mut bytes = vec![0_u8; length as usize];
    file.read_exact(&mut bytes).map_err(map_io_error)?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::monitor::{
        model::ThreadFact,
        sqlite_source::{StateFacts, StateThread},
    };
    use tempfile::tempdir;

    #[test]
    fn rejects_content_records_without_retaining_canaries() {
        let mut pending = HashMap::new();
        let user =
            r#"{"type":"response_item","payload":{"type":"message","content":"CANARY_SECRET"}}"#;
        let output = r#"{"type":"response_item","payload":{"type":"function_call_output","call_id":"other","output":"CANARY_OUTPUT"}}"#;
        let parsed_user = parse_line(user, &mut pending).expect("rejected user record");
        let parsed_output = parse_line(output, &mut pending).expect("rejected output record");
        assert!(parsed_user.is_none());
        assert!(parsed_output.is_none());
        assert!(!format!("{parsed_user:?}{parsed_output:?}{pending:?}").contains("CANARY"));
    }

    #[test]
    fn parses_effective_model_and_task_boundaries() {
        let mut pending = HashMap::new();
        let context = r#"{"timestamp":"2026-07-18T08:00:00Z","type":"turn_context","payload":{"model":"gpt-5.6-terra","effort":"high","summary":"private"}}"#;
        let started = r#"{"timestamp":"2026-07-18T08:00:01Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn"}}"#;
        let completed = r#"{"timestamp":"2026-07-18T08:00:02Z","type":"event_msg","payload":{"type":"task_complete","last_agent_message":"private"}}"#;

        let mut facts = RolloutFacts::default();
        for line in [context, started, completed] {
            apply_record(
                &mut facts,
                "child",
                parse_line(line, &mut pending)
                    .expect("valid record")
                    .expect("whitelisted record"),
            );
        }
        let child = facts.threads.get("child").expect("child facts");
        assert_eq!(child.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(child.reasoning_effort.as_deref(), Some("high"));
        assert!(child.latest_task_started_ms < child.latest_task_completed_ms);
        assert!(!format!("{facts:?}").contains("private"));
    }

    #[test]
    fn correlates_only_registered_spawn_outputs() {
        let mut pending = HashMap::new();
        let call = r#"{"timestamp":"2026-07-18T08:00:00Z","type":"response_item","payload":{"type":"function_call","name":"spawn_agent","arguments":"{\"task_name\":\"review\",\"model\":\"gpt-5.6-terra\",\"reasoning_effort\":\"high\",\"message\":\"CANARY_PROMPT\"}","call_id":"call-1"}}"#;
        let output = r#"{"timestamp":"2026-07-18T08:00:01Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"{\"agent_id\":\"child\",\"nickname\":\"Locke\",\"extra\":\"CANARY_OUTPUT\"}"}}"#;

        assert!(matches!(
            parse_line(call, &mut pending).expect("spawn request"),
            Some(WhitelistRecord::SpawnRequested)
        ));
        let resolved = parse_line(output, &mut pending)
            .expect("spawn output")
            .expect("resolved spawn");
        let debug = format!("{resolved:?}{pending:?}");
        assert!(debug.contains("gpt-5.6-terra"));
        assert!(debug.contains("child"));
        assert!(!debug.contains("CANARY"));
    }

    #[test]
    fn incremental_refresh_handles_growth_and_truncation() {
        let temporary = tempdir().expect("tempdir");
        let rollout = temporary.path().join("child.jsonl");
        fs::write(
            &rollout,
            "{\"timestamp\":\"2026-07-18T08:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-a\",\"effort\":\"high\"}}\n",
        )
        .expect("initial rollout");
        let state = StateFacts {
            threads: vec![StateThread {
                fact: ThreadFact {
                    thread_id: "child".into(),
                    ..ThreadFact::default()
                },
                rollout_path: Some(rollout.clone()),
            }],
            edges: Vec::new(),
            opened_read_only: true,
        };
        let mut index = RolloutIndex::default();
        assert_eq!(
            index.refresh(&state).expect("first refresh").threads["child"]
                .model
                .as_deref(),
            Some("gpt-a")
        );

        fs::write(
            &rollout,
            "{\"timestamp\":\"2026-07-18T08:01:00Z\",\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-b\",\"effort\":\"medium\"}}\n",
        )
        .expect("truncated rollout");
        let refreshed = index.refresh(&state).expect("refresh after truncation");
        assert_eq!(refreshed.threads["child"].model.as_deref(), Some("gpt-b"));
    }
}
