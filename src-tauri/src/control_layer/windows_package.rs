use std::{
    net::{IpAddr, Ipv4Addr, TcpListener},
    path::PathBuf,
};

use serde::Deserialize;

pub const CODEX_PACKAGE_NAME: &str = "OpenAI.Codex";
pub const CODEX_PACKAGE_FAMILY: &str = "OpenAI.Codex_2p2nqsd0c76g0";
pub const CODEX_EXECUTABLE_NAME: &str = "ChatGPT.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    TrustedStore,
    TrustedPublisher,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageProbe {
    pub name: String,
    pub package_family: String,
    pub version: String,
    pub canonical_root: PathBuf,
    pub canonical_executable: PathBuf,
    pub signature: SignatureStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage {
    pub version: String,
    pub root: PathBuf,
    pub executable: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessProbe {
    pub pid: u32,
    pub owner_sid: String,
    pub canonical_image_path: PathBuf,
    pub package_family: String,
    pub signature: SignatureStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub owner_sid: String,
    pub canonical_image_path: PathBuf,
    pub package_family: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProcess {
    pub pid: u32,
    pub owner_sid: String,
    pub image_path: PathBuf,
    pub package_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerProbe {
    pub address: IpAddr,
    pub port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedListener {
    pub address: IpAddr,
    pub port: u16,
    pub pid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    PackageMissing,
    AmbiguousPackage,
    PackageQuery,
    PackageName,
    PackageFamily,
    PackageVersion,
    PackageRoot,
    ExecutableOutsidePackage,
    ExecutableName,
    ProcessId,
    ProcessOwner,
    ProcessImage,
    ProcessPackage,
    Signature,
    ListenerAddress,
    ListenerPort,
    ListenerOwner,
    ActiveNativeChild,
    SetupPending,
    AmbiguousUiProcess,
    ReplacementIdentity,
    CloseFailed,
    ExitTimeout,
    LaunchFailed,
    PortUnavailable,
    InvalidPort,
    ProcessQuery,
    ListenerMissing,
    AmbiguousListener,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct PackageQuery {
    pub name: String,
    #[serde(rename = "PackageFamilyName")]
    pub package_family: String,
    pub version: String,
    pub install_location: PathBuf,
    pub signature_kind: String,
}

pub fn parse_package_query(document: &str) -> Result<PackageQuery, IdentityError> {
    if document.len() > 32_768 {
        return Err(IdentityError::PackageQuery);
    }
    let mut records: Vec<PackageQuery> =
        serde_json::from_str(document).map_err(|_| IdentityError::PackageQuery)?;
    if records.is_empty() {
        return Err(IdentityError::PackageMissing);
    }
    if records.len() != 1 {
        return Err(IdentityError::AmbiguousPackage);
    }
    let record = records.pop().ok_or(IdentityError::PackageMissing)?;
    if record.name != CODEX_PACKAGE_NAME {
        return Err(IdentityError::PackageName);
    }
    if record.package_family != CODEX_PACKAGE_FAMILY {
        return Err(IdentityError::PackageFamily);
    }
    if !safe_version(&record.version) {
        return Err(IdentityError::PackageVersion);
    }
    if record.signature_kind != "Store" {
        return Err(IdentityError::Signature);
    }
    if !record.install_location.is_absolute() {
        return Err(IdentityError::PackageRoot);
    }
    Ok(record)
}

#[cfg(windows)]
pub fn discover_store_package() -> Result<VerifiedPackage, IdentityError> {
    use std::{os::windows::process::CommandExt, process::Command};

    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    let system_root = std::env::var_os("SystemRoot").ok_or(IdentityError::PackageQuery)?;
    let powershell = PathBuf::from(system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let powershell = std::fs::canonicalize(powershell).map_err(|_| IdentityError::PackageQuery)?;
    let script = concat!(
        "$records=@(Get-AppxPackage -Name 'OpenAI.Codex' | ForEach-Object {",
        "[pscustomobject]@{Name=$_.Name;PackageFamilyName=$_.PackageFamilyName;",
        "Version=$_.Version.ToString();InstallLocation=$_.InstallLocation;",
        "SignatureKind=$_.SignatureKind.ToString()}});",
        "ConvertTo-Json -InputObject $records -Compress"
    );
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|_| IdentityError::PackageQuery)?;
    if !output.status.success() || output.stdout.len() > 32_768 {
        return Err(IdentityError::PackageQuery);
    }
    let document = String::from_utf8(output.stdout).map_err(|_| IdentityError::PackageQuery)?;
    let query = parse_package_query(document.trim())?;
    let root =
        std::fs::canonicalize(query.install_location).map_err(|_| IdentityError::PackageRoot)?;
    let executable = std::fs::canonicalize(root.join("app").join(CODEX_EXECUTABLE_NAME))
        .map_err(|_| IdentityError::ExecutableOutsidePackage)?;
    verify_package(PackageProbe {
        name: query.name,
        package_family: query.package_family,
        version: query.version,
        canonical_root: root,
        canonical_executable: executable,
        signature: SignatureStatus::TrustedStore,
    })
}

#[cfg(not(windows))]
pub fn discover_store_package() -> Result<VerifiedPackage, IdentityError> {
    Err(IdentityError::PackageMissing)
}

pub fn verified_process_from_pid(
    package: &VerifiedPackage,
    pid: u32,
    current_user_sid: &str,
) -> Result<VerifiedProcess, IdentityError> {
    let identity = query_process_identity(pid)?;
    let package_family = identity
        .package_family
        .ok_or(IdentityError::ProcessPackage)?;
    verify_process(
        package,
        ProcessProbe {
            pid: identity.pid,
            owner_sid: identity.owner_sid,
            canonical_image_path: identity.canonical_image_path,
            package_family,
            signature: SignatureStatus::TrustedStore,
        },
        current_user_sid,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupPhase {
    Committed,
    Installing,
    AwaitingVisibleCommand,
    PreflightRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartGuard {
    pub verified_ui_processes: usize,
    pub active_native_children: usize,
    pub setup_phase: SetupPhase,
}

pub fn authorize_restart(guard: RestartGuard) -> Result<(), IdentityError> {
    if guard.active_native_children != 0 {
        return Err(IdentityError::ActiveNativeChild);
    }
    if guard.setup_phase != SetupPhase::Committed {
        return Err(IdentityError::SetupPending);
    }
    if guard.verified_ui_processes != 1 {
        return Err(IdentityError::AmbiguousUiProcess);
    }
    Ok(())
}

pub fn validate_replacement_set(
    previous_pid: u32,
    launched_pid: u32,
    verified_ui_processes: &[VerifiedProcess],
) -> Result<VerifiedProcess, IdentityError> {
    if previous_pid == 0 || launched_pid == 0 || previous_pid == launched_pid {
        return Err(IdentityError::ReplacementIdentity);
    }
    let [replacement] = verified_ui_processes else {
        return Err(IdentityError::AmbiguousUiProcess);
    };
    if replacement.pid != launched_pid {
        return Err(IdentityError::ReplacementIdentity);
    }
    Ok(replacement.clone())
}

pub struct PortReservation {
    listener: TcpListener,
    address: Ipv4Addr,
    port: u16,
}

impl PortReservation {
    pub const fn address(&self) -> Ipv4Addr {
        self.address
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn release(self) -> u16 {
        let port = self.port;
        drop(self.listener);
        port
    }
}

pub fn reserve_loopback_port() -> Result<PortReservation, IdentityError> {
    let address = Ipv4Addr::LOCALHOST;
    let listener = TcpListener::bind((address, 0)).map_err(|_| IdentityError::PortUnavailable)?;
    let local = listener
        .local_addr()
        .map_err(|_| IdentityError::PortUnavailable)?;
    if local.ip() != IpAddr::V4(address) || local.port() == 0 {
        return Err(IdentityError::PortUnavailable);
    }
    Ok(PortReservation {
        listener,
        address,
        port: local.port(),
    })
}

pub fn cdp_launch_arguments(port: u16) -> Result<[String; 2], IdentityError> {
    if port == 0 {
        return Err(IdentityError::InvalidPort);
    }
    Ok([
        "--remote-debugging-address=127.0.0.1".to_owned(),
        format!("--remote-debugging-port={port}"),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartedSession {
    pub process: VerifiedProcess,
    pub port: u16,
}

#[cfg(windows)]
pub fn restart_verified_codex(
    guard: RestartGuard,
    package: &VerifiedPackage,
    current_process: &VerifiedProcess,
    reservation: PortReservation,
    timeout_ms: u32,
) -> Result<RestartedSession, IdentityError> {
    authorize_restart(guard)?;
    if !(1_000..=30_000).contains(&timeout_ms) {
        return Err(IdentityError::ExitTimeout);
    }
    let fresh_package = discover_store_package()?;
    if fresh_package.version != package.version
        || !same_windows_path(&fresh_package.root, &package.root)
        || !same_windows_path(&fresh_package.executable, &package.executable)
    {
        return Err(IdentityError::PackageVersion);
    }
    close_verified_ui_process(package, current_process, timeout_ms)?;
    let port = reservation.release();
    let launched_pid = launch_cdp_replacement(package, port)?;
    let process = wait_for_exact_replacement(
        package,
        current_process.pid,
        launched_pid,
        &current_process.owner_sid,
        timeout_ms,
    )?;
    Ok(RestartedSession { process, port })
}

#[cfg(not(windows))]
pub fn restart_verified_codex(
    _guard: RestartGuard,
    _package: &VerifiedPackage,
    _current_process: &VerifiedProcess,
    _reservation: PortReservation,
    _timeout_ms: u32,
) -> Result<RestartedSession, IdentityError> {
    Err(IdentityError::LaunchFailed)
}

#[cfg(windows)]
fn launch_cdp_replacement(package: &VerifiedPackage, port: u16) -> Result<u32, IdentityError> {
    use std::process::Command;

    let arguments = cdp_launch_arguments(port)?;
    let child = Command::new(&package.executable)
        .args(arguments)
        .spawn()
        .map_err(|_| IdentityError::LaunchFailed)?;
    let pid = child.id();
    if pid == 0 {
        return Err(IdentityError::LaunchFailed);
    }
    drop(child);
    Ok(pid)
}

#[cfg(windows)]
fn close_verified_ui_process(
    package: &VerifiedPackage,
    process: &VerifiedProcess,
    timeout_ms: u32,
) -> Result<(), IdentityError> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LPARAM, WAIT_OBJECT_0},
        System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
        },
    };

    let current = verified_process_from_pid(package, process.pid, &process.owner_sid)?;
    if current != *process {
        return Err(IdentityError::ReplacementIdentity);
    }
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            process.pid,
        )
    };
    if handle.is_null() {
        return Err(IdentityError::CloseFailed);
    }
    struct CloseContext {
        pid: u32,
        posted: usize,
        failed: bool,
    }
    unsafe extern "system" fn close_window(
        window: windows_sys::Win32::Foundation::HWND,
        parameter: LPARAM,
    ) -> i32 {
        let context = &mut *(parameter as *mut CloseContext);
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let mut pid = 0_u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid == context.pid {
            if PostMessageW(window, WM_CLOSE, 0, 0) == 0 {
                context.failed = true;
                return 0;
            }
            context.posted += 1;
        }
        1
    }
    let mut context = CloseContext {
        pid: process.pid,
        posted: 0,
        failed: false,
    };
    unsafe {
        EnumWindows(
            Some(close_window),
            (&mut context as *mut CloseContext) as LPARAM,
        );
    }
    if context.failed || context.posted == 0 {
        unsafe {
            CloseHandle(handle);
        }
        return Err(IdentityError::CloseFailed);
    }
    let wait = unsafe { WaitForSingleObject(handle, timeout_ms) };
    unsafe {
        CloseHandle(handle);
    }
    if wait != WAIT_OBJECT_0 {
        return Err(IdentityError::ExitTimeout);
    }
    Ok(())
}

#[cfg(windows)]
fn visible_top_level_process_ids() -> Result<Vec<u32>, IdentityError> {
    use windows_sys::Win32::{
        Foundation::LPARAM,
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
    };

    unsafe extern "system" fn collect_window(
        window: windows_sys::Win32::Foundation::HWND,
        parameter: LPARAM,
    ) -> i32 {
        if IsWindowVisible(window) == 0 {
            return 1;
        }
        let processes = &mut *(parameter as *mut Vec<u32>);
        let mut pid = 0_u32;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != 0 {
            processes.push(pid);
        }
        1
    }

    let mut processes = Vec::new();
    let enumerated = unsafe {
        EnumWindows(
            Some(collect_window),
            (&mut processes as *mut Vec<u32>) as LPARAM,
        )
    };
    if enumerated == 0 {
        return Err(IdentityError::ProcessQuery);
    }
    processes.sort_unstable();
    processes.dedup();
    Ok(processes)
}

#[cfg(windows)]
pub fn discover_verified_ui_processes(
    package: &VerifiedPackage,
    current_user_sid: &str,
) -> Result<Vec<VerifiedProcess>, IdentityError> {
    let mut verified = Vec::new();
    for pid in visible_top_level_process_ids()? {
        let Ok(identity) = query_process_identity(pid) else {
            continue;
        };
        if identity.package_family.as_deref() != Some(CODEX_PACKAGE_FAMILY)
            || !same_windows_path(&identity.canonical_image_path, &package.executable)
        {
            continue;
        }
        verified.push(verify_process(
            package,
            ProcessProbe {
                pid: identity.pid,
                owner_sid: identity.owner_sid,
                canonical_image_path: identity.canonical_image_path,
                package_family: CODEX_PACKAGE_FAMILY.to_owned(),
                signature: SignatureStatus::TrustedStore,
            },
            current_user_sid,
        )?);
    }
    Ok(verified)
}

#[cfg(windows)]
fn wait_for_exact_replacement(
    package: &VerifiedPackage,
    previous_pid: u32,
    launched_pid: u32,
    current_user_sid: &str,
    timeout_ms: u32,
) -> Result<VerifiedProcess, IdentityError> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(u64::from(timeout_ms));
    loop {
        let verified = discover_verified_ui_processes(package, current_user_sid)?;
        match validate_replacement_set(previous_pid, launched_pid, &verified) {
            Ok(process) => return Ok(process),
            Err(IdentityError::AmbiguousUiProcess) if verified.is_empty() => {}
            Err(error) => return Err(error),
        }
        if std::time::Instant::now() >= deadline {
            return Err(IdentityError::ExitTimeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub fn verify_package(probe: PackageProbe) -> Result<VerifiedPackage, IdentityError> {
    if probe.name != CODEX_PACKAGE_NAME {
        return Err(IdentityError::PackageName);
    }
    if probe.package_family != CODEX_PACKAGE_FAMILY {
        return Err(IdentityError::PackageFamily);
    }
    if !safe_version(&probe.version) {
        return Err(IdentityError::PackageVersion);
    }
    if !probe.canonical_root.is_absolute() {
        return Err(IdentityError::PackageRoot);
    }
    if !probe.canonical_executable.is_absolute()
        || !probe
            .canonical_executable
            .starts_with(&probe.canonical_root)
    {
        return Err(IdentityError::ExecutableOutsidePackage);
    }
    if !probe
        .canonical_executable
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(CODEX_EXECUTABLE_NAME))
    {
        return Err(IdentityError::ExecutableName);
    }
    if probe.signature != SignatureStatus::TrustedStore {
        return Err(IdentityError::Signature);
    }
    Ok(VerifiedPackage {
        version: probe.version,
        root: probe.canonical_root,
        executable: probe.canonical_executable,
    })
}

pub fn verify_process(
    package: &VerifiedPackage,
    probe: ProcessProbe,
    current_user_sid: &str,
) -> Result<VerifiedProcess, IdentityError> {
    if probe.pid == 0 {
        return Err(IdentityError::ProcessId);
    }
    if !safe_sid(current_user_sid) || probe.owner_sid != current_user_sid {
        return Err(IdentityError::ProcessOwner);
    }
    if !same_windows_path(&probe.canonical_image_path, &package.executable) {
        return Err(IdentityError::ProcessImage);
    }
    if probe.package_family != CODEX_PACKAGE_FAMILY {
        return Err(IdentityError::ProcessPackage);
    }
    if probe.signature != SignatureStatus::TrustedStore {
        return Err(IdentityError::Signature);
    }
    Ok(VerifiedProcess {
        pid: probe.pid,
        owner_sid: probe.owner_sid,
        image_path: probe.canonical_image_path,
        package_version: package.version.clone(),
    })
}

pub fn verify_listener(
    process: &VerifiedProcess,
    probe: ListenerProbe,
    expected_port: u16,
) -> Result<VerifiedListener, IdentityError> {
    if !probe.address.is_loopback() {
        return Err(IdentityError::ListenerAddress);
    }
    if expected_port == 0 || probe.port != expected_port {
        return Err(IdentityError::ListenerPort);
    }
    if probe.pid != process.pid {
        return Err(IdentityError::ListenerOwner);
    }
    Ok(VerifiedListener {
        address: probe.address,
        port: probe.port,
        pid: probe.pid,
    })
}

fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

fn safe_sid(value: &str) -> bool {
    value.starts_with("S-1-")
        && value.len() <= 184
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || character == '-' || character == 'S')
}

fn same_windows_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(windows)]
pub fn query_process_identity(pid: u32) -> Result<ProcessIdentity, IdentityError> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr};

    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, LocalFree, APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER, HANDLE,
            NO_ERROR,
        },
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
            TOKEN_USER,
        },
        Storage::Packaging::Appx::GetPackageFamilyName,
        System::Threading::{
            OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };

    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    if pid == 0 {
        return Err(IdentityError::ProcessId);
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(IdentityError::ProcessQuery);
    }
    let process = OwnedHandle(process);

    let mut image = vec![0_u16; 32_768];
    let mut image_length = u32::try_from(image.len()).map_err(|_| IdentityError::ProcessQuery)?;
    if unsafe { QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut image_length) }
        == 0
    {
        return Err(IdentityError::ProcessQuery);
    }
    let image_length = usize::try_from(image_length).map_err(|_| IdentityError::ProcessQuery)?;
    let image_path = PathBuf::from(OsString::from_wide(&image[..image_length]));
    let canonical_image_path =
        std::fs::canonicalize(image_path).map_err(|_| IdentityError::ProcessQuery)?;

    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) } == 0 || token.is_null() {
        return Err(IdentityError::ProcessQuery);
    }
    let token = OwnedHandle(token);
    let mut required = 0_u32;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required);
    }
    if required == 0 {
        return Err(IdentityError::ProcessQuery);
    }
    let required_usize = usize::try_from(required).map_err(|_| IdentityError::ProcessQuery)?;
    let words = required_usize
        .checked_add(std::mem::size_of::<usize>() - 1)
        .and_then(|value| value.checked_div(std::mem::size_of::<usize>()))
        .ok_or(IdentityError::ProcessQuery)?;
    let mut token_buffer = vec![0_usize; words];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            token_buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    } == 0
    {
        return Err(IdentityError::ProcessQuery);
    }
    let token_user = unsafe { &*token_buffer.as_ptr().cast::<TOKEN_USER>() };
    if token_user.User.Sid.is_null() {
        return Err(IdentityError::ProcessQuery);
    }
    let mut sid_text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_text) } == 0
        || sid_text.is_null()
    {
        return Err(IdentityError::ProcessQuery);
    }
    let mut sid_length = 0_usize;
    while sid_length < 184 && unsafe { *sid_text.add(sid_length) } != 0 {
        sid_length += 1;
    }
    let owner_sid = if sid_length == 184 {
        unsafe {
            LocalFree(sid_text.cast());
        }
        return Err(IdentityError::ProcessQuery);
    } else {
        let value = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, sid_length) })
            .map_err(|_| IdentityError::ProcessQuery)?;
        unsafe {
            LocalFree(sid_text.cast());
        }
        value
    };
    if !safe_sid(&owner_sid) {
        return Err(IdentityError::ProcessQuery);
    }
    let mut family_length = 0_u32;
    let family_probe =
        unsafe { GetPackageFamilyName(process.0, &mut family_length, std::ptr::null_mut()) };
    let package_family = if family_probe == APPMODEL_ERROR_NO_PACKAGE {
        None
    } else {
        if family_probe != ERROR_INSUFFICIENT_BUFFER || !(2..=256).contains(&family_length) {
            return Err(IdentityError::ProcessQuery);
        }
        let mut family = vec![0_u16; family_length as usize];
        if unsafe { GetPackageFamilyName(process.0, &mut family_length, family.as_mut_ptr()) }
            != NO_ERROR
        {
            return Err(IdentityError::ProcessQuery);
        }
        let terminator = family
            .iter()
            .position(|character| *character == 0)
            .ok_or(IdentityError::ProcessQuery)?;
        Some(String::from_utf16(&family[..terminator]).map_err(|_| IdentityError::ProcessQuery)?)
    };
    Ok(ProcessIdentity {
        pid,
        owner_sid,
        canonical_image_path,
        package_family,
    })
}

#[cfg(not(windows))]
pub fn query_process_identity(_pid: u32) -> Result<ProcessIdentity, IdentityError> {
    Err(IdentityError::ProcessQuery)
}

#[cfg(windows)]
pub fn query_tcp_listener(port: u16) -> Result<ListenerProbe, IdentityError> {
    use windows_sys::Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR},
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
            TCP_TABLE_OWNER_PID_LISTENER,
        },
        Networking::WinSock::AF_INET,
    };

    if port == 0 {
        return Err(IdentityError::InvalidPort);
    }
    let mut required = 0_u32;
    let first = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut required,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if first != ERROR_INSUFFICIENT_BUFFER || required == 0 {
        return Err(IdentityError::ProcessQuery);
    }
    let required_usize = usize::try_from(required).map_err(|_| IdentityError::ProcessQuery)?;
    let words = required_usize
        .checked_add(std::mem::size_of::<usize>() - 1)
        .and_then(|value| value.checked_div(std::mem::size_of::<usize>()))
        .ok_or(IdentityError::ProcessQuery)?;
    let mut buffer = vec![0_usize; words];
    let result = unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr().cast(),
            &mut required,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if result != NO_ERROR {
        return Err(IdentityError::ProcessQuery);
    }
    let table = unsafe { &*buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>() };
    let row_count = usize::try_from(table.dwNumEntries).map_err(|_| IdentityError::ProcessQuery)?;
    let header = std::mem::size_of::<u32>();
    let row_bytes = row_count
        .checked_mul(std::mem::size_of::<MIB_TCPROW_OWNER_PID>())
        .and_then(|value| value.checked_add(header))
        .ok_or(IdentityError::ProcessQuery)?;
    if row_bytes > required_usize {
        return Err(IdentityError::ProcessQuery);
    }
    let rows = unsafe { std::slice::from_raw_parts(table.table.as_ptr(), row_count) };
    let matches = rows
        .iter()
        .filter_map(|row| {
            let row_port = u16::from_be(row.dwLocalPort as u16);
            let address = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
            (row_port == port && address == Ipv4Addr::LOCALHOST).then_some(ListenerProbe {
                address: IpAddr::V4(address),
                port: row_port,
                pid: row.dwOwningPid,
            })
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(IdentityError::ListenerMissing),
        [listener] => Ok(listener.clone()),
        _ => Err(IdentityError::AmbiguousListener),
    }
}

#[cfg(not(windows))]
pub fn query_tcp_listener(_port: u16) -> Result<ListenerProbe, IdentityError> {
    Err(IdentityError::ProcessQuery)
}
