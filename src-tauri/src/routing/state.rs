use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use uuid::Uuid;

use super::{
    EligibilityRecord, RootRouteState, RouteActivity, RouteKind, RoutePhase, RouteReasonCode,
    RoutingSnapshot, RoutingStateEnvelope,
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

    pub fn load(&self) -> Result<RoutingStateEnvelope, String> {
        let state_file = self.state_file();
        let bytes = match fs::read(&state_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let empty = RoutingStateEnvelope::empty(PROFILE_VERSION);
                self.save(&empty)?;
                return Ok(empty);
            }
            Err(_) => return Err("Routing state could not be read".to_owned()),
        };
        match serde_json::from_slice::<RoutingStateEnvelope>(&bytes).and_then(|state| {
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
        protect_owned_path(&self.state_file())
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
        move_file(&self.state_file(), &evidence)?;
        protect_owned_path(&evidence)
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

    pub fn try_start_activity(&self, activity: RouteActivity) -> Result<(), RouteReasonCode> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut next = state.clone();
        next.activity.push(activity);
        validate_envelope(&next).map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        self.store
            .save(&next)
            .map_err(|_| RouteReasonCode::StatePersistenceFailed)?;
        *state = next;
        Ok(())
    }
}

fn validate_envelope(state: &RoutingStateEnvelope) -> Result<(), String> {
    if state.schema_version != STATE_SCHEMA_VERSION || !valid_token(&state.profile_version) {
        return Err("Routing state is invalid".to_owned());
    }
    let mut route_keys = HashSet::new();
    let mut conversation_ids = HashSet::new();
    for route in &state.routes {
        valid_route(route)?;
        if !route_keys.insert(route.route_key) || !conversation_ids.insert(route.conversation_id) {
            return Err("Routing state is invalid".to_owned());
        }
    }
    for eligibility in &state.eligibility {
        valid_eligibility(eligibility)?;
    }
    let mut child_ids = HashSet::new();
    for activity in &state.activity {
        valid_activity(activity, &route_keys)?;
        if !child_ids.insert(activity.child_thread_id) {
            return Err("Routing state is invalid".to_owned());
        }
    }
    for route_key in route_keys {
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
        for activity in active {
            if activity.escalation_count > 2 || activity.reviewer_parent {
                return Err("Routing state is invalid".to_owned());
            }
        }
    }
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
    if valid_token(&eligibility.profile_version) {
        Ok(())
    } else {
        Err("Routing state is invalid".to_owned())
    }
}

fn valid_activity(activity: &RouteActivity, route_keys: &HashSet<Uuid>) -> Result<(), String> {
    valid_uuid(activity.route_key)?;
    valid_uuid(activity.child_thread_id)?;
    valid_uuid(activity.subtask_id)?;
    valid_timestamp(activity.started_at_ms)?;
    valid_timestamp(activity.updated_at_ms)?;
    if !route_keys.contains(&activity.route_key)
        || activity.escalation_count > 2
        || activity.reviewer_parent
        || activity.reason_codes.is_empty()
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

fn valid_token(value: &str) -> bool {
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

#[cfg(not(windows))]
fn protect_owned_path(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if path.is_dir() { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| "Routing state permissions could not be set".to_owned())
}

#[cfg(windows)]
fn protect_owned_path(path: &Path) -> Result<(), String> {
    windows_acl::protect_current_user(path)
}

#[cfg(not(windows))]
fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(temporary, destination).map_err(|_| "Routing state could not be replaced".to_owned())
}

#[cfg(windows)]
fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
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
