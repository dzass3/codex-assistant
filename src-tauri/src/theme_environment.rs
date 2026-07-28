use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeEnvironmentStatus {
    Ready,
    CodexNotRunning,
    RestartRequired,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeNextAction {
    ApplyNow,
    LaunchCodexForTheme,
    ConfirmRestart,
    InstallCodex,
    CloseExtraWindows,
    UpdateAssistant,
    UseSupportedWindows,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeEnvironmentCheckCode {
    SupportedWindows,
    SupportedArchitecture,
    OfficialStoreCodex,
    CompatibleAdapter,
    SingleCodexWindow,
    VerifiedThemeSession,
    SavedTheme,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeEnvironmentCheckState {
    Pass,
    Action,
    Fail,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeEnvironmentCheck {
    pub code: ThemeEnvironmentCheckCode,
    pub state: ThemeEnvironmentCheckState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeEnvironmentProbe {
    pub platform_supported: bool,
    pub os_build: Option<u32>,
    pub architecture: String,
    pub package_version: Option<String>,
    pub verified_process_count: usize,
    pub session_reachable: bool,
    pub selected_theme_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeEnvironmentReport {
    pub contract_version: u32,
    pub status: ThemeEnvironmentStatus,
    pub checks: Vec<ThemeEnvironmentCheck>,
    pub os_build: Option<u32>,
    pub architecture: String,
    pub codex_version: Option<String>,
    pub verified_process_count: usize,
    pub session_reachable: bool,
    pub selected_theme_id: Option<String>,
    pub next_action: ThemeNextAction,
    pub can_apply_now: bool,
}

pub fn classify_environment(probe: ThemeEnvironmentProbe) -> ThemeEnvironmentReport {
    let supported_architecture = matches!(probe.architecture.as_str(), "x64" | "arm64");
    let supported_windows =
        probe.platform_supported && probe.os_build.is_some_and(|build| build >= 19_045);
    let adapter_compatible = probe
        .package_version
        .as_deref()
        .is_some_and(|version| crate::theme::select_theme_adapter(version).is_some());
    let (status, next_action, can_apply_now) = if !supported_windows || !supported_architecture {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::UseSupportedWindows,
            false,
        )
    } else if probe.package_version.is_none() {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::InstallCodex,
            false,
        )
    } else if !adapter_compatible {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::UpdateAssistant,
            false,
        )
    } else if probe.verified_process_count > 1 {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::CloseExtraWindows,
            false,
        )
    } else if probe.verified_process_count == 0 {
        (
            ThemeEnvironmentStatus::CodexNotRunning,
            ThemeNextAction::LaunchCodexForTheme,
            false,
        )
    } else if !probe.session_reachable {
        (
            ThemeEnvironmentStatus::RestartRequired,
            ThemeNextAction::ConfirmRestart,
            false,
        )
    } else {
        (
            ThemeEnvironmentStatus::Ready,
            ThemeNextAction::ApplyNow,
            true,
        )
    };

    ThemeEnvironmentReport {
        contract_version: 2,
        status,
        checks: checks(&probe),
        os_build: probe.os_build,
        architecture: probe.architecture,
        codex_version: probe.package_version,
        verified_process_count: probe.verified_process_count,
        session_reachable: probe.session_reachable,
        selected_theme_id: probe.selected_theme_id,
        next_action,
        can_apply_now,
    }
}

fn checks(probe: &ThemeEnvironmentProbe) -> Vec<ThemeEnvironmentCheck> {
    let package_available = probe.package_version.is_some();
    vec![
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::SupportedWindows,
            state: pass_or_fail(
                probe.platform_supported && probe.os_build.is_some_and(|build| build >= 19_045),
            ),
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::SupportedArchitecture,
            state: pass_or_fail(matches!(probe.architecture.as_str(), "x64" | "arm64")),
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::OfficialStoreCodex,
            state: pass_or_fail(package_available),
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::CompatibleAdapter,
            state: pass_or_fail(
                probe
                    .package_version
                    .as_deref()
                    .is_some_and(|version| crate::theme::select_theme_adapter(version).is_some()),
            ),
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::SingleCodexWindow,
            state: if probe.verified_process_count <= 1 {
                ThemeEnvironmentCheckState::Pass
            } else {
                ThemeEnvironmentCheckState::Fail
            },
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::VerifiedThemeSession,
            state: if probe.session_reachable {
                ThemeEnvironmentCheckState::Pass
            } else if package_available && probe.verified_process_count <= 1 {
                ThemeEnvironmentCheckState::Action
            } else {
                ThemeEnvironmentCheckState::Fail
            },
        },
        ThemeEnvironmentCheck {
            code: ThemeEnvironmentCheckCode::SavedTheme,
            state: if probe.selected_theme_id.is_some() {
                ThemeEnvironmentCheckState::Pass
            } else {
                ThemeEnvironmentCheckState::Action
            },
        },
    ]
}

fn pass_or_fail(value: bool) -> ThemeEnvironmentCheckState {
    if value {
        ThemeEnvironmentCheckState::Pass
    } else {
        ThemeEnvironmentCheckState::Fail
    }
}

#[cfg(windows)]
pub fn inspect_local_environment(
    selected_theme_id: Option<String>,
    session_reachable: bool,
) -> ThemeEnvironmentReport {
    use crate::control_layer::windows_package::{
        discover_store_package, discover_verified_ui_processes, query_process_identity,
    };

    let package = discover_store_package().ok();
    let processes = package
        .as_ref()
        .and_then(|package| {
            let current_user = query_process_identity(std::process::id()).ok()?;
            discover_verified_ui_processes(package, &current_user.owner_sid).ok()
        })
        .unwrap_or_default();
    classify_environment(ThemeEnvironmentProbe {
        platform_supported: true,
        os_build: windows_build(),
        architecture: host_architecture().to_owned(),
        package_version: package.map(|package| package.version),
        verified_process_count: processes.len(),
        session_reachable: session_reachable && processes.len() == 1,
        selected_theme_id,
    })
}

#[cfg(not(windows))]
pub fn inspect_local_environment(
    selected_theme_id: Option<String>,
    _session_reachable: bool,
) -> ThemeEnvironmentReport {
    classify_environment(ThemeEnvironmentProbe {
        platform_supported: false,
        os_build: None,
        architecture: host_architecture().to_owned(),
        package_version: None,
        verified_process_count: 0,
        session_reachable: false,
        selected_theme_id,
    })
}

fn host_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => "unsupported",
    }
}

#[cfg(windows)]
fn windows_build() -> Option<u32> {
    #[repr(C)]
    struct RtlOsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version: *mut RtlOsVersionInfo) -> i32;
    }
    let mut version = RtlOsVersionInfo {
        size: std::mem::size_of::<RtlOsVersionInfo>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service_pack: [0; 128],
    };
    let status = unsafe { RtlGetVersion(&mut version) };
    (status == 0 && version.major == 10).then_some(version.build)
}
