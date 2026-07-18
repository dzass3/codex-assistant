use std::path::{Path, PathBuf};

use rusqlite::{Connection, DatabaseName, Error as SqlError, OpenFlags};

use super::model::{SourceError, SourceErrorCode, SourceResult, ThreadFact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnEdge {
    pub parent_thread_id: String,
    pub child_thread_id: String,
}

#[derive(Debug, Clone)]
pub struct StateThread {
    pub fact: ThreadFact,
    pub rollout_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct StateFacts {
    pub threads: Vec<StateThread>,
    pub edges: Vec<SpawnEdge>,
    pub opened_read_only: bool,
}

pub fn read_state_db(codex_home: &Path) -> SourceResult<StateFacts> {
    let database_path = codex_home.join("state_5.sqlite");
    if !database_path.is_file() {
        return Err(SourceError::new(
            SourceErrorCode::Missing,
            "Codex state database is unavailable",
        ));
    }

    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_sql_error)?;
    connection
        .busy_timeout(std::time::Duration::from_millis(250))
        .map_err(map_sql_error)?;

    let opened_read_only = connection.is_readonly(DatabaseName::Main).unwrap_or(true);
    let edges = read_edges(&connection)?;
    let threads = read_threads(&connection)?;

    Ok(StateFacts {
        threads,
        edges,
        opened_read_only,
    })
}

fn read_edges(connection: &Connection) -> SourceResult<Vec<SpawnEdge>> {
    let mut statement = connection
        .prepare(
            "SELECT parent_thread_id, child_thread_id
             FROM thread_spawn_edges
             ORDER BY parent_thread_id, child_thread_id",
        )
        .map_err(map_sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SpawnEdge {
                parent_thread_id: row.get(0)?,
                child_thread_id: row.get(1)?,
            })
        })
        .map_err(map_sql_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_sql_error)
}

fn read_threads(connection: &Connection) -> SourceResult<Vec<StateThread>> {
    let mut statement = connection
        .prepare(
            "SELECT t.id,
                    t.rollout_path,
                    t.source,
                    t.cwd,
                    t.title,
                    t.agent_nickname,
                    t.agent_role,
                    t.model,
                    t.reasoning_effort,
                    t.agent_path,
                    t.created_at_ms,
                    t.updated_at_ms,
                    edge.parent_thread_id
             FROM threads AS t
             LEFT JOIN thread_spawn_edges AS edge ON edge.child_thread_id = t.id
             WHERE edge.child_thread_id IS NOT NULL
                OR EXISTS (
                    SELECT 1 FROM thread_spawn_edges AS child
                    WHERE child.parent_thread_id = t.id
                )
             ORDER BY COALESCE(t.updated_at_ms, t.created_at_ms, 0) DESC",
        )
        .map_err(map_sql_error)?;

    let rows = statement
        .query_map([], |row| {
            let cwd: Option<String> = row.get(3)?;
            Ok(StateThread {
                fact: ThreadFact {
                    thread_id: row.get(0)?,
                    parent_thread_id: row.get(12)?,
                    nickname: row.get(5)?,
                    role: row.get(6)?,
                    title: row.get(4)?,
                    project: cwd.as_deref().and_then(project_basename),
                    originator: row.get(2)?,
                    database_model: row.get(7)?,
                    database_effort: row.get(8)?,
                    agent_path: row.get(9)?,
                    created_at_ms: row.get(10)?,
                    updated_at_ms: row.get(11)?,
                    ..ThreadFact::default()
                },
                rollout_path: row.get::<_, Option<String>>(1)?.map(PathBuf::from),
            })
        })
        .map_err(map_sql_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_sql_error)
}

fn project_basename(raw: &str) -> Option<String> {
    raw.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .map(str::to_owned)
}

fn map_sql_error(error: SqlError) -> SourceError {
    let code = match &error {
        SqlError::SqliteFailure(inner, _) if inner.code == rusqlite::ErrorCode::DatabaseBusy => {
            SourceErrorCode::Busy
        }
        SqlError::SqliteFailure(inner, _) if inner.code == rusqlite::ErrorCode::DatabaseLocked => {
            SourceErrorCode::Busy
        }
        SqlError::SqliteFailure(_, _) | SqlError::InvalidColumnName(_) => {
            SourceErrorCode::SchemaMismatch
        }
        _ => SourceErrorCode::Io,
    };
    let message = match code {
        SourceErrorCode::Busy => "Codex state database is busy",
        SourceErrorCode::SchemaMismatch => "Codex state schema is not supported",
        SourceErrorCode::Missing => "Codex state database is unavailable",
        SourceErrorCode::Io => "Codex state database could not be read",
    };
    SourceError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::tempdir;

    fn create_fixture(home: &Path) {
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
                params!["root", "root.jsonl", "vscode", r"C:\secret\project", "Root", "gpt-5.6-sol", "xhigh"],
            )
            .expect("root");
        connection
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, 200, 600)",
                params![
                    "child",
                    "child.jsonl",
                    "vscode",
                    r"C:\secret\project",
                    "Locke",
                    "worker",
                    "gpt-5.6-terra",
                    "high",
                    "/root/worker"
                ],
            )
            .expect("child");
        connection
            .execute(
                "INSERT INTO thread_spawn_edges VALUES ('root', 'child', 'open')",
                [],
            )
            .expect("edge");
    }

    #[test]
    fn reads_related_threads_without_exposing_full_project_path() {
        let temporary = tempdir().expect("tempdir");
        create_fixture(temporary.path());

        let facts = read_state_db(temporary.path()).expect("state facts");
        assert!(facts.opened_read_only);
        assert_eq!(facts.threads.len(), 2);
        assert_eq!(facts.edges.len(), 1);
        assert_eq!(facts.edges[0].child_thread_id, "child");
        let child = facts
            .threads
            .iter()
            .find(|thread| thread.fact.thread_id == "child")
            .expect("child thread");
        assert_eq!(child.fact.project.as_deref(), Some("project"));
        assert_eq!(child.fact.database_model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(child.fact.database_effort.as_deref(), Some("high"));
        assert_eq!(child.fact.parent_thread_id.as_deref(), Some("root"));
    }

    #[test]
    fn missing_database_returns_a_sanitized_error() {
        let temporary = tempdir().expect("tempdir");
        let error = read_state_db(temporary.path()).expect_err("missing database");
        assert_eq!(error.code, SourceErrorCode::Missing);
        assert!(!error
            .message
            .contains(temporary.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn incompatible_schema_returns_a_sanitized_error() {
        let temporary = tempdir().expect("tempdir");
        Connection::open(temporary.path().join("state_5.sqlite")).expect("empty database");
        let error = read_state_db(temporary.path()).expect_err("schema mismatch");
        assert_eq!(error.code, SourceErrorCode::SchemaMismatch);
        assert_eq!(error.message, "Codex state schema is not supported");
    }
}
