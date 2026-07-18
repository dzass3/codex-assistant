use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use uuid::Uuid;

use super::{
    policy::{evaluate_budget, RouteBudget},
    RouteActivity, RouteKind, RoutePhase, RouteReasonCode, RoutingSnapshot, RoutingStateEnvelope,
};

pub const STATE_SCHEMA_VERSION: u32 = 1;
const SETTINGS_DIRECTORY: &str = "codex-agent-monitor";
const STATE_FILE: &str = "routing-state.json";
const PROFILE_VERSION: &str = "routing-v1";

#[derive(Debug, Clone)]
pub struct RoutingStateStore {
    directory: PathBuf,
}

impl RoutingStateStore {
    pub fn in_directory(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)
            .map_err(|_| "Routing state directory is unavailable".to_owned())?;
        Ok(Self { directory })
    }

    pub fn default_location() -> Result<Self, String> {
        let directory = dirs::config_dir()
            .ok_or_else(|| "Routing state directory is unavailable".to_owned())?
            .join(SETTINGS_DIRECTORY);
        Self::in_directory(directory)
    }

    pub fn load(&self) -> Result<RoutingStateEnvelope, String> {
        let state_file = self.state_file();
        if !state_file.exists() {
            let empty = RoutingStateEnvelope::empty(PROFILE_VERSION);
            self.save(&empty)?;
            return Ok(empty);
        }

        match fs::read(&state_file)
            .map_err(|_| "Routing state could not be read".to_owned())
            .and_then(|bytes| {
                serde_json::from_slice::<RoutingStateEnvelope>(&bytes)
                    .map_err(|_| "Routing state is invalid".to_owned())
            })
            .and_then(validate_envelope)
        {
            Ok(state) => Ok(state),
            Err(_) => {
                self.quarantine_corrupt()?;
                let empty = RoutingStateEnvelope::empty(PROFILE_VERSION);
                self.save(&empty)?;
                Ok(empty)
            }
        }
    }

    pub fn save(&self, state: &RoutingStateEnvelope) -> Result<(), String> {
        validate_envelope(state.clone())?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|_| "Routing state could not be serialized".to_owned())?;
        let temporary = self
            .directory
            .join(format!(".routing-state-{}.tmp", Uuid::new_v4()));
        let write_result = (|| {
            let mut file = File::create(&temporary)
                .map_err(|_| "Routing state could not be written".to_owned())?;
            file.write_all(&bytes)
                .map_err(|_| "Routing state could not be written".to_owned())?;
            file.sync_all()
                .map_err(|_| "Routing state could not be written".to_owned())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        fs::rename(&temporary, self.state_file()).map_err(|_| {
            let _ = fs::remove_file(&temporary);
            "Routing state could not be replaced".to_owned()
        })
    }

    fn state_file(&self) -> PathBuf {
        self.directory.join(STATE_FILE)
    }

    fn quarantine_corrupt(&self) -> Result<(), String> {
        let evidence = self
            .directory
            .join(format!("routing-state.corrupt-{}.json", Uuid::new_v4()));
        fs::rename(self.state_file(), evidence)
            .map_err(|_| "Corrupt routing state could not be quarantined".to_owned())
    }
}

pub struct RoutingRuntime {
    store: RoutingStateStore,
    state: Mutex<RoutingStateEnvelope>,
}

impl RoutingRuntime {
    pub fn load(store: RoutingStateStore) -> Result<Self, String> {
        let state = store.load()?;
        Ok(Self {
            store,
            state: Mutex::new(state),
        })
    }

    pub fn snapshot(&self) -> RoutingSnapshot {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .snapshot()
    }

    pub fn replace(&self, next: RoutingStateEnvelope) -> Result<(), String> {
        self.store.save(&next)?;
        *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        Ok(())
    }

    pub fn try_start_activity(
        &self,
        activity: RouteActivity,
        reviewer_is_delegating: bool,
    ) -> Result<(), RouteReasonCode> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = state
            .activity
            .iter()
            .filter(|entry| entry.route_key == activity.route_key && is_active(entry.phase))
            .collect::<Vec<_>>();
        let budget = RouteBudget {
            active_routed_children: active.len().min(u8::MAX as usize) as u8,
            active_nested_children: active
                .iter()
                .filter(|entry| entry.route_kind == RouteKind::Nested)
                .count()
                .min(u8::MAX as usize) as u8,
            automatic_escalations: state
                .activity
                .iter()
                .filter(|entry| {
                    entry.route_key == activity.route_key && entry.subtask_id == activity.subtask_id
                })
                .map(|entry| entry.escalation_count)
                .max()
                .unwrap_or(activity.escalation_count),
            route_kind: activity.route_kind,
            reviewer_is_delegating,
        };
        evaluate_budget(&budget)?;
        let mut next = state.clone();
        next.activity.push(activity);
        self.store
            .save(&next)
            .map_err(|_| RouteReasonCode::NoEligibleTier)?;
        *state = next;
        Ok(())
    }
}

fn validate_envelope(state: RoutingStateEnvelope) -> Result<RoutingStateEnvelope, String> {
    if state.schema_version != STATE_SCHEMA_VERSION || !valid_version(&state.profile_version) {
        return Err("Routing state is invalid".to_owned());
    }
    if state
        .routes
        .iter()
        .any(|route| route.route_key.is_nil() || route.conversation_id.is_nil())
        || state.activity.iter().any(|activity| {
            activity.route_key.is_nil()
                || activity.child_thread_id.is_nil()
                || activity.subtask_id.is_nil()
                || activity.escalation_count > 2
        })
    {
        return Err("Routing state is invalid".to_owned());
    }
    Ok(state)
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn is_active(phase: RoutePhase) -> bool {
    matches!(
        phase,
        RoutePhase::Classifying | RoutePhase::Implementing | RoutePhase::Reviewing
    )
}
