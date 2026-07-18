use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use fs2::FileExt;
use uuid::Uuid;

use super::{
    EligibilityReasonCode, EligibilityRecord, EligibilityStatus, QualityOutcome, QualityRecord,
    RootRouteState, RouteActivity, RouteKind, RoutePhase, RouteReasonCode, RoutingSnapshot,
    RoutingStateEnvelope,
};

pub const STATE_SCHEMA_VERSION: u32 = 1;
pub const MAX_JS_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
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
        protect_owned_path(&directory)?;
        Ok(Self { directory })
    }

    pub fn default_location() -> Result<Self, String> {
        let directory = dirs::config_dir()
            .ok_or_else(|| "Routing state directory is unavailable".to_owned())?
            .join(SETTINGS_DIRECTORY)
            .join("routing");
        Self::in_directory(directory)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn load(&self) -> Result<RoutingStateEnvelope, String> {
        let state_file = self.state_file();
        if state_file.exists() {
            protect_owned_path(&state_file)?;
        }
        let bytes = match fs::read(&state_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let empty = RoutingStateEnvelope::empty(PROFILE_VERSION);
                self.save(&empty)?;
                return Ok(empty);
            }
            Err(_) => return Err("Routing state could not be read".to_owned()),
        };
        match serde_json::from_slice::<RoutingStateEnvelope>(&bytes).and_then(|mut state| {
            migrate_legacy_eligibility(&mut state);
            validate_envelope(&state).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            })?;
            Ok(state)
        }) {
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
        validate_envelope(state)?;
        protect_owned_path(&self.directory)?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|_| "Routing state could not be serialized".to_owned())?;
        let temporary = self
            .directory
            .join(format!(".routing-state-{}.tmp", Uuid::new_v4()));
        let write_result = (|| {
            let mut file = File::create(&temporary)
                .map_err(|_| "Routing state could not be written".to_owned())?;
            protect_owned_path(&temporary)?;
            file.write_all(&bytes)
                .map_err(|_| "Routing state could not be written".to_owned())?;
            file.sync_all()
                .map_err(|_| "Routing state could not be written".to_owned())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = replace_existing(&temporary, &self.state_file()) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn has_current_user_only_acl(&self, target: &Path) -> Result<bool, String> {
        windows_acl::has_current_user_only_acl(target)
    }

    fn state_file(&self) -> PathBuf {
        self.directory.join(STATE_FILE)
    }

    fn quarantine_corrupt(&self) -> Result<(), String> {
        let evidence = self
            .directory
            .join(format!("routing-state.corrupt-{}.json", Uuid::new_v4()));
        protect_owned_path(&self.state_file())?;
        move_file(&self.state_file(), &evidence)
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
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.store.save(&next)?;
        *state = next;
        Ok(())
    }

    pub fn upsert_eligibility(&self, record: EligibilityRecord) -> Result<(), String> {
        valid_eligibility(&record)?;
        let _file_lock = RoutingStateFileLock::acquire(self.store.directory())
            .map_err(|_| "Routing state lock is unavailable".to_owned())?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut next = self.store.load()?;
        for existing in &mut next.eligibility {
            if existing.tier == record.tier
                && existing.route_kind == record.route_kind
                && existing.depth == record.depth
                && (existing.codex_package_version != record.codex_package_version
                    || existing.profile_version != record.profile_version)
            {
                existing.status = EligibilityStatus::Stale;
                existing.reason = Some(if existing.profile_version != record.profile_version {
                    EligibilityReasonCode::ProfileVersionChanged
                } else {
                    EligibilityReasonCode::HostVersionChanged
                });
            }
        }
        let exact = next.eligibility.iter().position(|existing| {
            existing.codex_package_version == record.codex_package_version
                && existing.profile_version == record.profile_version
                && existing.requested_model == record.requested_model
                && existing.route_kind == record.route_kind
                && existing.depth == record.depth
        });
        if let Some(position) = exact {
            next.eligibility[position] = record;
        } else {
            next.eligibility.push(record);
        }
        validate_envelope(&next)?;
        self.store.save(&next)?;
        *state = next;
        Ok(())
    }

    pub fn try_start_activity(&self, mut activity: RouteActivity) -> Result<(), RouteReasonCode> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let route = state
            .routes
            .iter()
            .find(|route| route.route_key == activity.route_key)
            .ok_or(RouteReasonCode::UnknownRoute)?;
        if let Some(existing) = state
            .activity
            .iter()
            .find(|entry| entry.child_thread_id == activity.child_thread_id)
        {
            return Err(
                if matches!(existing.phase, RoutePhase::Completed | RoutePhase::Degraded) {
                    RouteReasonCode::TerminalChildReactivation
                } else {
                    RouteReasonCode::ChildAlreadyRecorded
                },
            );
        }
        if activity.route_kind == RouteKind::Direct {
            if activity.parent_thread_id != route.conversation_id {
                return Err(RouteReasonCode::ParentLineageMismatch);
            }
        } else {
            let parent = state
                .activity
                .iter()
                .find(|entry| entry.child_thread_id == activity.parent_thread_id)
                .ok_or(RouteReasonCode::ParentLineageMismatch)?;
            if parent.route_key != activity.route_key || parent.route_kind != RouteKind::Direct {
                return Err(RouteReasonCode::ParentLineageMismatch);
            }
            if parent.is_reviewer {
                return Err(RouteReasonCode::ReviewerRecursionForbidden);
            }
            if parent.selected_tier != super::ModelTier::Terra
                || activity.selected_tier >= super::ModelTier::Terra
            {
                return Err(RouteReasonCode::NestedDelegationForbidden);
            }
        }
        let active = state
            .activity
            .iter()
            .filter(|entry| entry.route_key == activity.route_key && is_active(entry.phase))
            .collect::<Vec<_>>();
        if active.len() >= 3 {
            return Err(RouteReasonCode::ActiveChildLimitReached);
        }
        if activity.route_kind == RouteKind::Nested
            && active
                .iter()
                .any(|entry| entry.route_kind == RouteKind::Nested)
        {
            return Err(RouteReasonCode::NestedChildLimitReached);
        }
        let attempts = state
            .activity
            .iter()
            .filter(|entry| {
                entry.route_key == activity.route_key
                    && entry.subtask_id == activity.subtask_id
                    && !entry.is_reviewer
            })
            .collect::<Vec<_>>();
        if !activity.is_reviewer && attempts.iter().any(|entry| is_active(entry.phase)) {
            return Err(RouteReasonCode::PreviousAttemptStillActive);
        }
        let current_escalation = attempts.iter().map(|entry| entry.escalation_count).max();
        if activity.is_reviewer {
            activity.escalation_count =
                current_escalation.ok_or(RouteReasonCode::StatePersistenceFailed)?;
        } else {
            activity.escalation_count = match current_escalation {
                Some(2) => return Err(RouteReasonCode::EscalationLimitReached),
                Some(count) => count.saturating_add(1),
                None => 0,
            };
        }
        let mut next = state.clone();
        next.activity.push(activity);
        validate_envelope(&next).map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        self.store
            .save(&next)
            .map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        *state = next;
        Ok(())
    }

    pub fn record_quality(&self, record: QualityRecord) -> Result<(), RouteReasonCode> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state
            .routes
            .iter()
            .any(|route| route.route_key == record.route_key)
        {
            return Err(RouteReasonCode::UnknownRoute);
        }
        if record.retry_count > 2 {
            return Err(RouteReasonCode::RetryLimitReached);
        }
        if state
            .quality
            .iter()
            .any(|quality| quality.child_thread_id == record.child_thread_id)
        {
            return Err(RouteReasonCode::QualityAlreadyRecorded);
        }
        let position = state
            .activity
            .iter()
            .position(|activity| activity.child_thread_id == record.child_thread_id)
            .ok_or(RouteReasonCode::UnknownChild)?;
        let activity = &state.activity[position];
        if activity.route_key != record.route_key {
            return Err(RouteReasonCode::ParentLineageMismatch);
        }
        if activity.escalation_count != record.escalation_count {
            return Err(RouteReasonCode::EscalationCountMismatch);
        }
        if matches!(activity.phase, RoutePhase::Completed | RoutePhase::Degraded) {
            return Err(RouteReasonCode::TerminalChildReactivation);
        }
        let mut next = state.clone();
        let activity = &mut next.activity[position];
        activity.phase = match record.outcome {
            QualityOutcome::Passed => RoutePhase::Completed,
            QualityOutcome::Failed | QualityOutcome::Degraded => RoutePhase::Degraded,
        };
        activity.updated_at_ms = record.recorded_at_ms;
        next.quality.push(record);
        validate_envelope(&next).map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        self.store
            .save(&next)
            .map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        *state = next;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStateLockError {
    Timeout,
    Unavailable,
}

pub struct RoutingStateFileLock {
    file: File,
}

impl RoutingStateFileLock {
    pub fn acquire(state_directory: &Path) -> Result<Self, RoutingStateLockError> {
        fs::create_dir_all(state_directory).map_err(|_| RoutingStateLockError::Unavailable)?;
        let path = state_directory.join("routing-mcp.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|_| RoutingStateLockError::Unavailable)?;
        protect_owned_path(&path).map_err(|_| RoutingStateLockError::Unavailable)?;
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { file }),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || matches!(error.raw_os_error(), Some(32 | 33)) =>
                {
                    if Instant::now() >= deadline {
                        return Err(RoutingStateLockError::Timeout);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return Err(RoutingStateLockError::Unavailable),
            }
        }
    }
}

impl Drop for RoutingStateFileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn validate_envelope(state: &RoutingStateEnvelope) -> Result<(), String> {
    if state.schema_version != STATE_SCHEMA_VERSION || !valid_profile(&state.profile_version) {
        return Err("Routing state is invalid".to_owned());
    }
    let mut routes = HashMap::new();
    let mut conversation_ids = HashSet::new();
    for route in &state.routes {
        valid_route(route)?;
        if routes.insert(route.route_key, route).is_some()
            || !conversation_ids.insert(route.conversation_id)
        {
            return Err("Routing state is invalid".to_owned());
        }
    }
    let mut eligibility_keys = HashSet::new();
    for eligibility in &state.eligibility {
        valid_eligibility(eligibility)?;
        if !eligibility_keys.insert((
            eligibility.codex_package_version.clone(),
            eligibility.profile_version.clone(),
            eligibility.requested_model.clone(),
            eligibility.route_kind,
            eligibility.depth,
        )) {
            return Err("Routing state is invalid".to_owned());
        }
    }
    let mut activities = HashMap::new();
    for activity in &state.activity {
        valid_activity(activity, &routes)?;
        if activities
            .insert(activity.child_thread_id, activity)
            .is_some()
        {
            return Err("Routing state is invalid".to_owned());
        }
    }
    let mut quality_children = HashSet::new();
    for quality in &state.quality {
        valid_quality(quality)?;
        if !quality_children.insert(quality.child_thread_id) {
            return Err("Routing state is invalid".to_owned());
        }
        let activity = activities
            .get(&quality.child_thread_id)
            .ok_or_else(|| "Routing state is invalid".to_owned())?;
        if activity.route_key != quality.route_key
            || activity.escalation_count != quality.escalation_count
            || activity.updated_at_ms != quality.recorded_at_ms
            || !matches!(
                (quality.outcome, activity.phase),
                (QualityOutcome::Passed, RoutePhase::Completed)
                    | (
                        QualityOutcome::Failed | QualityOutcome::Degraded,
                        RoutePhase::Degraded
                    )
            )
        {
            return Err("Routing state is invalid".to_owned());
        }
    }
    for activity in &state.activity {
        let route = routes
            .get(&activity.route_key)
            .ok_or_else(|| "Routing state is invalid".to_owned())?;
        match activity.route_kind {
            RouteKind::Direct if activity.parent_thread_id != route.conversation_id => {
                return Err("Routing state is invalid".to_owned())
            }
            RouteKind::Nested => {
                let parent = activities
                    .get(&activity.parent_thread_id)
                    .ok_or_else(|| "Routing state is invalid".to_owned())?;
                if parent.route_key != activity.route_key
                    || parent.route_kind != RouteKind::Direct
                    || parent.is_reviewer
                    || parent.selected_tier != super::ModelTier::Terra
                    || activity.selected_tier >= super::ModelTier::Terra
                {
                    return Err("Routing state is invalid".to_owned());
                }
            }
            RouteKind::Direct => {}
        }
    }
    for route_key in routes.keys().copied() {
        let active = state
            .activity
            .iter()
            .filter(|activity| activity.route_key == route_key && is_active(activity.phase))
            .collect::<Vec<_>>();
        let nested = active
            .iter()
            .filter(|activity| activity.route_kind == RouteKind::Nested)
            .count();
        if active.len() > 3 || nested > 1 {
            return Err("Routing state is invalid".to_owned());
        }
    }
    validate_escalations(state)?;
    Ok(())
}

fn valid_route(route: &RootRouteState) -> Result<(), String> {
    valid_uuid(route.route_key)?;
    valid_uuid(route.conversation_id)?;
    valid_timestamp(route.created_at_ms)?;
    valid_timestamp(route.updated_at_ms)
}

fn valid_eligibility(eligibility: &EligibilityRecord) -> Result<(), String> {
    valid_timestamp(eligibility.checked_at_ms)?;
    let expected_depth = match eligibility.route_kind {
        RouteKind::Direct => 1,
        RouteKind::Nested => 2,
    };
    let status_reason_valid = match eligibility.status {
        EligibilityStatus::Unknown | EligibilityStatus::Eligible => eligibility.reason.is_none(),
        EligibilityStatus::Verifying => matches!(
            eligibility.reason,
            Some(
                EligibilityReasonCode::AwaitingVisibleCommand
                    | EligibilityReasonCode::AwaitingNativeChild
                    | EligibilityReasonCode::AwaitingEffectiveModel
                    | EligibilityReasonCode::ChildStillRunning
            )
        ),
        EligibilityStatus::Unavailable => eligibility.reason.is_some(),
        EligibilityStatus::Stale => matches!(
            eligibility.reason,
            Some(
                EligibilityReasonCode::HostVersionChanged
                    | EligibilityReasonCode::ProfileVersionChanged
            )
        ),
    };
    if valid_profile(&eligibility.profile_version)
        && safe_host_version(&eligibility.codex_package_version)
        && eligibility.requested_model == eligibility.tier.model_id()
        && eligibility.depth == expected_depth
        && status_reason_valid
    {
        Ok(())
    } else {
        Err("Routing state is invalid".to_owned())
    }
}

fn migrate_legacy_eligibility(state: &mut RoutingStateEnvelope) {
    for eligibility in &mut state.eligibility {
        if eligibility.codex_package_version.is_empty()
            || eligibility.requested_model.is_empty()
            || eligibility.depth == 0
        {
            eligibility.codex_package_version = "legacy-unverified".to_owned();
            eligibility.requested_model = eligibility.tier.model_id().to_owned();
            eligibility.depth = match eligibility.route_kind {
                RouteKind::Direct => 1,
                RouteKind::Nested => 2,
            };
            eligibility.status = EligibilityStatus::Stale;
            eligibility.reason = Some(EligibilityReasonCode::HostVersionChanged);
        }
    }
}

fn safe_host_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+')
        })
}

fn valid_activity(
    activity: &RouteActivity,
    routes: &HashMap<Uuid, &RootRouteState>,
) -> Result<(), String> {
    valid_uuid(activity.route_key)?;
    valid_uuid(activity.child_thread_id)?;
    valid_uuid(activity.subtask_id)?;
    valid_uuid(activity.parent_thread_id)?;
    valid_timestamp(activity.started_at_ms)?;
    valid_timestamp(activity.updated_at_ms)?;
    if !routes.contains_key(&activity.route_key)
        || activity.escalation_count > 2
        || activity.reason_codes.is_empty()
    {
        return Err("Routing state is invalid".to_owned());
    }
    Ok(())
}

fn valid_quality(quality: &QualityRecord) -> Result<(), String> {
    valid_uuid(quality.route_key)?;
    valid_uuid(quality.child_thread_id)?;
    valid_timestamp(quality.recorded_at_ms)?;
    if quality.retry_count > 2 || quality.escalation_count > 2 {
        Err("Routing state is invalid".to_owned())
    } else {
        Ok(())
    }
}

fn validate_escalations(state: &RoutingStateEnvelope) -> Result<(), String> {
    let mut implementations: HashMap<(Uuid, Uuid), Vec<&RouteActivity>> = HashMap::new();
    let mut reviewers: HashMap<(Uuid, Uuid), Vec<&RouteActivity>> = HashMap::new();
    for activity in &state.activity {
        let key = (activity.route_key, activity.subtask_id);
        if activity.is_reviewer {
            reviewers.entry(key).or_default().push(activity);
        } else {
            implementations.entry(key).or_default().push(activity);
        }
    }
    for (key, attempts) in &implementations {
        let counts = attempts
            .iter()
            .map(|activity| activity.escalation_count)
            .collect::<HashSet<_>>();
        if attempts.len() > 3
            || counts.len() != attempts.len()
            || !(0..attempts.len() as u8).all(|count| counts.contains(&count))
        {
            return Err("Routing state is invalid".to_owned());
        }
        let active_attempts = attempts
            .iter()
            .filter(|activity| is_active(activity.phase))
            .collect::<Vec<_>>();
        if active_attempts.len() > 1
            || active_attempts.first().is_some_and(|activity| {
                Some(activity.escalation_count) != counts.iter().copied().max()
            })
        {
            return Err("Routing state is invalid".to_owned());
        }
        if let Some(reviews) = reviewers.get(key) {
            if reviews
                .iter()
                .any(|review| !counts.contains(&review.escalation_count))
            {
                return Err("Routing state is invalid".to_owned());
            }
        }
    }
    if reviewers
        .keys()
        .any(|key| !implementations.contains_key(key))
    {
        return Err("Routing state is invalid".to_owned());
    }
    Ok(())
}

fn valid_uuid(value: Uuid) -> Result<(), String> {
    if value.is_nil() {
        Err("Routing state is invalid".to_owned())
    } else {
        Ok(())
    }
}

fn valid_timestamp(value: i64) -> Result<(), String> {
    if (0..=MAX_JS_SAFE_INTEGER).contains(&value) {
        Ok(())
    } else {
        Err("Routing state is invalid".to_owned())
    }
}

fn valid_profile(value: &str) -> bool {
    value == PROFILE_VERSION
}

fn is_active(phase: RoutePhase) -> bool {
    matches!(
        phase,
        RoutePhase::Classifying | RoutePhase::Implementing | RoutePhase::Reviewing
    )
}

#[cfg(not(windows))]
pub(crate) fn protect_owned_path(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if path.is_dir() { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| "Routing state permissions could not be set".to_owned())
}

#[cfg(windows)]
pub(crate) fn protect_owned_path(path: &Path) -> Result<(), String> {
    windows_acl::protect_current_user(path)
}

#[cfg(not(windows))]
pub(crate) fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(temporary, destination).map_err(|_| "Routing state could not be replaced".to_owned())
}

#[cfg(windows)]
pub(crate) fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
    windows_acl::move_file(temporary, destination, true)
}

#[cfg(not(windows))]
fn move_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|_| "Routing state could not be quarantined".to_owned())
}

#[cfg(windows)]
fn move_file(source: &Path, destination: &Path) -> Result<(), String> {
    windows_acl::move_file(source, destination, false)
}

#[cfg(windows)]
mod windows_acl {
    use std::{ffi::c_void, path::Path};

    use windows_sys::{
        core::PWSTR,
        Win32::{
            Foundation::{CloseHandle, LocalFree},
            Security::{
                AclSizeInformation,
                Authorization::{
                    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                    ConvertStringSidToSidW,
                },
                EqualSid, GetAce, GetAclInformation, GetFileSecurityW,
                GetSecurityDescriptorControl, GetSecurityDescriptorDacl, GetTokenInformation,
                SetFileSecurityW, TokenUser, ACCESS_ALLOWED_ACE, ACL_SIZE_INFORMATION,
                DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY,
                TOKEN_USER,
            },
            Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    pub fn protect_current_user(path: &Path) -> Result<(), String> {
        let sid = current_user_sid()?;
        let descriptor = wide(&format!("D:P(A;;FA;;;{sid})"));
        let mut security_descriptor = std::ptr::null_mut();
        let result = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                descriptor.as_ptr(),
                1,
                &mut security_descriptor,
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err("Routing state permissions could not be set".to_owned());
        }
        let path = wide_path(path)?;
        let result = unsafe {
            SetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                security_descriptor,
            )
        };
        unsafe { LocalFree(security_descriptor) };
        if result == 0 {
            Err("Routing state permissions could not be set".to_owned())
        } else {
            Ok(())
        }
    }

    pub fn move_file(source: &Path, destination: &Path, replace: bool) -> Result<(), String> {
        let source = wide_path(source)?;
        let destination = wide_path(destination)?;
        let mut flags = MOVEFILE_WRITE_THROUGH;
        if replace {
            flags |= MOVEFILE_REPLACE_EXISTING;
        }
        let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
        if result == 0 {
            Err("Routing state could not be replaced".to_owned())
        } else {
            Ok(())
        }
    }

    pub fn has_current_user_only_acl(path: &Path) -> Result<bool, String> {
        let path = wide_path(path)?;
        let mut size = 0;
        unsafe {
            GetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                0,
                &mut size,
            );
        }
        if size == 0 {
            return Err("Routing state permissions could not be verified".to_owned());
        }
        let mut descriptor = vec![0_u8; size as usize];
        let result = unsafe {
            GetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION,
                descriptor.as_mut_ptr() as *mut c_void,
                size,
                &mut size,
            )
        };
        if result == 0 {
            return Err("Routing state permissions could not be verified".to_owned());
        }
        let mut control = 0_u16;
        let mut revision = 0_u32;
        if unsafe {
            GetSecurityDescriptorControl(
                descriptor.as_mut_ptr() as *mut c_void,
                &mut control,
                &mut revision,
            )
        } == 0
            || control & 0x1000 == 0
        {
            return Ok(false);
        }
        let mut present = 0;
        let mut dacl = std::ptr::null_mut();
        let mut defaulted = 0;
        let result = unsafe {
            GetSecurityDescriptorDacl(
                descriptor.as_mut_ptr() as *mut c_void,
                &mut present,
                &mut dacl,
                &mut defaulted,
            )
        };
        if result == 0 || present == 0 || dacl.is_null() {
            return Ok(false);
        }
        let mut info = ACL_SIZE_INFORMATION {
            AceCount: 0,
            AclBytesInUse: 0,
            AclBytesFree: 0,
        };
        let result = unsafe {
            GetAclInformation(
                dacl,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        };
        if result == 0 || info.AceCount != 1 {
            return Ok(false);
        }
        let mut ace = std::ptr::null_mut();
        if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
            return Ok(false);
        }
        let allowed = ace as *const ACCESS_ALLOWED_ACE;
        if unsafe { (*allowed).Header.AceType } != 0 || unsafe { (*allowed).Mask } != 0x1F01FF {
            return Ok(false);
        }
        let ace_sid = unsafe { &(*allowed).SidStart as *const u32 as *mut c_void };
        let sid = string_sid(&current_user_sid()?)?;
        let equal = unsafe { EqualSid(ace_sid, sid) != 0 };
        unsafe { LocalFree(sid) };
        Ok(equal)
    }

    fn current_user_sid() -> Result<String, String> {
        let mut token = std::ptr::null_mut();
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        if opened == 0 {
            return Err("Routing state permissions could not be set".to_owned());
        }
        let mut length = 0;
        unsafe {
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut length);
        }
        if length == 0 {
            unsafe { CloseHandle(token) };
            return Err("Routing state permissions could not be set".to_owned());
        }
        let mut bytes = vec![0_u8; length as usize];
        let read = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                bytes.as_mut_ptr() as *mut c_void,
                length,
                &mut length,
            )
        };
        unsafe { CloseHandle(token) };
        if read == 0 {
            return Err("Routing state permissions could not be set".to_owned());
        }
        let token_user = bytes.as_ptr() as *const TOKEN_USER;
        let mut sid: PWSTR = std::ptr::null_mut();
        let converted = unsafe { ConvertSidToStringSidW((*token_user).User.Sid, &mut sid) };
        if converted == 0 || sid.is_null() {
            return Err("Routing state permissions could not be set".to_owned());
        }
        let result = unsafe {
            let length = (0..).take_while(|index| *sid.add(*index) != 0).count();
            String::from_utf16_lossy(std::slice::from_raw_parts(sid, length))
        };
        unsafe { LocalFree(sid as *mut c_void) };
        Ok(result)
    }

    fn string_sid(value: &str) -> Result<*mut c_void, String> {
        let value = wide(value);
        let mut sid = std::ptr::null_mut();
        let converted = unsafe { ConvertStringSidToSidW(value.as_ptr(), &mut sid) };
        if converted == 0 || sid.is_null() {
            Err("Routing state permissions could not be verified".to_owned())
        } else {
            Ok(sid)
        }
    }

    fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
        path.to_str()
            .map(wide)
            .ok_or_else(|| "Routing state path is invalid".to_owned())
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
