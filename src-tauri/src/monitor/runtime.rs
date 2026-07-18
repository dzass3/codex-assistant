use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use super::{
    model::{HealthEntry, MonitorSnapshot, ReconcileInput, SourceHealth, SpawnFact, ThreadFact},
    reconcile::reconcile,
    rollout_source::{RolloutFacts, RolloutIndex},
    sqlite_source::{read_state_db, StateFacts},
};

const SETTINGS_DIRECTORY: &str = "codex-agent-monitor";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub codex_home_label: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredSettings {
    codex_home: PathBuf,
}

struct RuntimeState {
    codex_home: PathBuf,
    default_home: PathBuf,
    rollout_index: RolloutIndex,
    last_state_facts: Option<StateFacts>,
    snapshot: MonitorSnapshot,
    stable_signature: String,
    state_errors: u64,
    rollout_errors: u64,
}

pub struct MonitorRuntime {
    state: Mutex<RuntimeState>,
}

impl Default for MonitorRuntime {
    fn default() -> Self {
        Self::new(load_configured_home())
    }
}

impl MonitorRuntime {
    pub fn new(codex_home: PathBuf) -> Self {
        let default_home = default_codex_home();
        let now_ms = now_ms();
        let snapshot = MonitorSnapshot {
            generated_at_ms: now_ms,
            agents: Vec::new(),
            counts: Default::default(),
            health: SourceHealth {
                state_database: HealthEntry::degraded("Waiting for Codex state", None, 0),
                rollout_observer: HealthEntry::degraded("Waiting for rollout metadata", None, 0),
            },
        };
        Self {
            state: Mutex::new(RuntimeState {
                codex_home,
                default_home,
                rollout_index: RolloutIndex::default(),
                last_state_facts: None,
                stable_signature: stable_signature(&snapshot),
                snapshot,
                state_errors: 0,
                rollout_errors: 0,
            }),
        }
    }

    pub fn snapshot(&self) -> MonitorSnapshot {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot
            .clone()
    }

    pub fn settings(&self) -> MonitorSettings {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        MonitorSettings {
            codex_home_label: if state.codex_home == state.default_home {
                "~/.codex".to_owned()
            } else {
                state
                    .codex_home
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| format!("custom · {name}"))
                    .unwrap_or_else(|| "custom Codex home".to_owned())
            },
            is_default: state.codex_home == state.default_home,
        }
    }

    pub fn set_codex_home(&self, raw_path: &str) -> Result<MonitorSettings, String> {
        let requested = PathBuf::from(raw_path.trim());
        if !valid_codex_home(&requested) {
            return Err("Choose a directory containing state_5.sqlite or sessions".to_owned());
        }
        let canonical = requested
            .canonicalize()
            .map_err(|_| "The selected Codex directory could not be opened".to_owned())?;
        persist_home(&canonical)?;

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.codex_home = canonical;
        state.rollout_index = RolloutIndex::default();
        state.last_state_facts = None;
        state.state_errors = 0;
        state.rollout_errors = 0;
        drop(state);
        Ok(self.settings())
    }

    pub fn refresh(&self) -> (MonitorSnapshot, bool) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = now_ms();

        let (state_facts, state_health) = match read_state_db(&state.codex_home) {
            Ok(facts) => {
                state.last_state_facts = Some(facts.clone());
                (
                    Some(facts),
                    HealthEntry::healthy("State database ready", now),
                )
            }
            Err(error) => {
                state.state_errors = state.state_errors.saturating_add(1);
                let cached = state.last_state_facts.clone();
                let health = if cached.is_some() {
                    HealthEntry::degraded(
                        format!("{}; showing cached metadata", error.message),
                        state.snapshot.health.state_database.last_success_ms,
                        state.state_errors,
                    )
                } else {
                    HealthEntry::error(error.message, state.state_errors)
                };
                (cached, health)
            }
        };

        let (rollout_facts, rollout_health) = if let Some(facts) = state_facts.as_ref() {
            match state.rollout_index.refresh(facts) {
                Ok(rollouts) => {
                    let health = if rollouts.backlog || rollouts.parse_errors > 0 {
                        HealthEntry::degraded(
                            if rollouts.backlog {
                                "Rollout observer is catching up"
                            } else {
                                "Some unsupported rollout records were skipped"
                            },
                            Some(now),
                            rollouts.parse_errors,
                        )
                    } else {
                        HealthEntry::healthy("Rollout observer ready", now)
                    };
                    (rollouts, health)
                }
                Err(error) => {
                    state.rollout_errors = state.rollout_errors.saturating_add(1);
                    (
                        RolloutFacts::default(),
                        HealthEntry::degraded(
                            error.message,
                            state.snapshot.health.rollout_observer.last_success_ms,
                            state.rollout_errors,
                        ),
                    )
                }
            }
        } else {
            (
                RolloutFacts::default(),
                HealthEntry::error("Rollout observer needs Codex state metadata", 1),
            )
        };

        let input = merge_sources(
            state_facts.as_ref(),
            &rollout_facts,
            SourceHealth {
                state_database: state_health,
                rollout_observer: rollout_health,
            },
        );
        let snapshot = reconcile(input, now);
        let signature = stable_signature(&snapshot);
        let changed = signature != state.stable_signature;
        state.stable_signature = signature;
        state.snapshot = snapshot.clone();
        (snapshot, changed)
    }
}

fn merge_sources(
    state: Option<&StateFacts>,
    rollouts: &RolloutFacts,
    health: SourceHealth,
) -> ReconcileInput {
    let mut threads: Vec<ThreadFact> = state
        .map(|facts| {
            facts
                .threads
                .iter()
                .map(|thread| thread.fact.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut positions: HashMap<String, usize> = threads
        .iter()
        .enumerate()
        .map(|(index, thread)| (thread.thread_id.clone(), index))
        .collect();

    for (thread_id, rollout) in &rollouts.threads {
        let position = if let Some(position) = positions.get(thread_id) {
            *position
        } else {
            let position = threads.len();
            threads.push(ThreadFact {
                thread_id: thread_id.clone(),
                ..ThreadFact::default()
            });
            positions.insert(thread_id.clone(), position);
            position
        };
        let thread = &mut threads[position];
        thread.rollout_model = rollout.model.clone();
        thread.rollout_effort = rollout.reasoning_effort.clone();
        thread.latest_task_started_ms = rollout.latest_task_started_ms;
        thread.latest_task_completed_ms = rollout.latest_task_completed_ms;
        thread.interrupted_at_ms = rollout.interrupted_at_ms;
        thread.updated_at_ms = thread.updated_at_ms.max(rollout.updated_at_ms);
    }

    ReconcileInput {
        threads,
        spawns: deduplicate_spawns(&rollouts.spawns),
        health,
    }
}

fn deduplicate_spawns(spawns: &[SpawnFact]) -> Vec<SpawnFact> {
    let mut by_child: HashMap<&str, &SpawnFact> = HashMap::new();
    for spawn in spawns {
        let replace = by_child
            .get(spawn.child_thread_id.as_str())
            .is_none_or(|existing| spawn.occurred_at_ms >= existing.occurred_at_ms);
        if replace {
            by_child.insert(spawn.child_thread_id.as_str(), spawn);
        }
    }
    by_child.into_values().cloned().collect()
}

fn stable_signature(snapshot: &MonitorSnapshot) -> String {
    serde_json::to_string(&(
        &snapshot.agents,
        &snapshot.counts,
        snapshot.health.state_database.level,
        &snapshot.health.state_database.message,
        snapshot.health.state_database.error_count,
        snapshot.health.rollout_observer.level,
        &snapshot.health.rollout_observer.message,
        snapshot.health.rollout_observer.error_count,
    ))
    .unwrap_or_default()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn default_codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn load_configured_home() -> PathBuf {
    settings_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<StoredSettings>(&bytes).ok())
        .map(|settings| settings.codex_home)
        .filter(|path| valid_codex_home(path))
        .unwrap_or_else(default_codex_home)
}

fn valid_codex_home(path: &Path) -> bool {
    path.is_dir() && (path.join("state_5.sqlite").is_file() || path.join("sessions").is_dir())
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|directory| directory.join(SETTINGS_DIRECTORY).join(SETTINGS_FILE))
}

fn persist_home(path: &Path) -> Result<(), String> {
    let settings_path =
        settings_path().ok_or_else(|| "Settings directory is unavailable".to_owned())?;
    let parent = settings_path
        .parent()
        .ok_or_else(|| "Settings directory is unavailable".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "Settings could not be saved".to_owned())?;
    let bytes = serde_json::to_vec_pretty(&StoredSettings {
        codex_home: path.to_owned(),
    })
    .map_err(|_| "Settings could not be saved".to_owned())?;
    fs::write(settings_path, bytes).map_err(|_| "Settings could not be saved".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{
        model::{HealthLevel, SourceHealth},
        rollout_source::RolloutThreadFact,
        sqlite_source::{SpawnEdge, StateThread},
    };

    #[test]
    fn merge_applies_rollout_model_and_deduplicates_spawn_intent() {
        let state = StateFacts {
            threads: vec![StateThread {
                fact: ThreadFact {
                    thread_id: "child".into(),
                    database_model: Some("database-model".into()),
                    ..ThreadFact::default()
                },
                rollout_path: None,
            }],
            edges: vec![SpawnEdge {
                parent_thread_id: "root".into(),
                child_thread_id: "child".into(),
            }],
            opened_read_only: true,
        };
        let rollouts = RolloutFacts {
            threads: HashMap::from([(
                "child".into(),
                RolloutThreadFact {
                    thread_id: "child".into(),
                    model: Some("turn-model".into()),
                    reasoning_effort: Some("high".into()),
                    ..RolloutThreadFact::default()
                },
            )]),
            spawns: vec![
                SpawnFact {
                    child_thread_id: "child".into(),
                    requested_model: Some("old".into()),
                    occurred_at_ms: Some(1),
                    ..SpawnFact::default()
                },
                SpawnFact {
                    child_thread_id: "child".into(),
                    requested_model: Some("new".into()),
                    occurred_at_ms: Some(2),
                    ..SpawnFact::default()
                },
            ],
            ..RolloutFacts::default()
        };
        let input = merge_sources(
            Some(&state),
            &rollouts,
            SourceHealth {
                state_database: HealthEntry::healthy("ok", 1),
                rollout_observer: HealthEntry::healthy("ok", 1),
            },
        );
        assert_eq!(
            input.threads[0].rollout_model.as_deref(),
            Some("turn-model")
        );
        assert_eq!(input.spawns.len(), 1);
        assert_eq!(input.spawns[0].requested_model.as_deref(), Some("new"));
        assert_eq!(input.health.state_database.level, HealthLevel::Healthy);
    }
}
