use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use chrono::Local;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    codex_config::{CodexConfigService, InstallRequest},
    control_layer::cdp::fetch_browser_endpoint,
    control_layer::windows_package::{
        discover_store_package, discover_verified_ui_processes, query_process_identity,
        query_tcp_listener, reserve_loopback_port, restart_verified_codex, verify_listener,
        IdentityError, RestartGuard, SetupPhase,
    },
    preflight::{EligibilityKey, PreflightCoordinator},
    routing::{state::RoutingStateStore, RouteKind, RoutingRuntime, RoutingSnapshot},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingInstallationStatus {
    Uninstalled,
    Installed,
    RestartRequired,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingRestartStatus {
    NotRequired,
    Required,
    BlockedActiveChild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingPreflightStatus {
    NotStarted,
    Running,
    Complete,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingCdpStatus {
    Inactive,
    Ready,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingSetupReasonCode {
    ActiveChild,
    ConfigConflict,
    PreflightRequired,
    UnsupportedHost,
    CdpUnavailable,
    RoutingRuntimeUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum RoutingConfigChange {
    #[serde(rename = "agents.max_depth")]
    AgentsMaxDepth,
    #[serde(rename = "agents.codex_assistant_spark")]
    SparkAgent,
    #[serde(rename = "agents.codex_assistant_luna")]
    LunaAgent,
    #[serde(rename = "agents.codex_assistant_terra")]
    TerraAgent,
    #[serde(rename = "agents.codex_assistant_sol")]
    SolAgent,
    #[serde(rename = "mcp_servers.codex_assistant_routing")]
    RoutingMcp,
    #[serde(rename = "skill.codex-assistant-routing")]
    RoutingSkill,
}

const DESIRED_CONFIG_CHANGES: [RoutingConfigChange; 7] = [
    RoutingConfigChange::AgentsMaxDepth,
    RoutingConfigChange::SparkAgent,
    RoutingConfigChange::LunaAgent,
    RoutingConfigChange::TerraAgent,
    RoutingConfigChange::SolAgent,
    RoutingConfigChange::RoutingMcp,
    RoutingConfigChange::RoutingSkill,
];
const ROUTING_PROFILE_VERSION: &str = "routing-v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RoutingSetupSnapshot {
    pub installation_status: RoutingInstallationStatus,
    pub restart_status: RoutingRestartStatus,
    pub preflight_status: RoutingPreflightStatus,
    pub cdp_status: RoutingCdpStatus,
    pub backup_label: Option<String>,
    pub config_changes: Vec<RoutingConfigChange>,
    pub reason_codes: Vec<RoutingSetupReasonCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RoutingUiSnapshot {
    pub contract_version: u32,
    pub setup: RoutingSetupSnapshot,
    pub routing: RoutingSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationStatus {
    Applied,
    Noop,
    Blocked,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OperationReceipt {
    pub operation_id: String,
    pub status: OperationStatus,
    pub reason_codes: Vec<RoutingSetupReasonCode>,
    pub restart_required: bool,
}

pub struct RoutingApplication {
    config: CodexConfigService,
    routing: RoutingRuntime,
    restart_required: Mutex<bool>,
    restart_blocked: Mutex<bool>,
    preflight: Mutex<PreflightCoordinator>,
    preflight_status: Mutex<RoutingPreflightStatus>,
    cdp_status: Mutex<RoutingCdpStatus>,
}

impl RoutingApplication {
    pub fn default_location() -> Result<Self, String> {
        let user_home = dirs::home_dir()
            .ok_or_else(|| "Smart Routing user directory is unavailable".to_owned())?;
        let state_directory = dirs::config_dir()
            .ok_or_else(|| "Smart Routing state directory is unavailable".to_owned())?
            .join("codex-agent-monitor")
            .join("routing");
        Self::for_paths(
            user_home.join(".codex"),
            user_home.join(".agents").join("skills"),
            std::env::current_exe()
                .map_err(|_| "Codex Assistant executable is unavailable".to_owned())?,
            state_directory,
        )
    }

    pub fn for_paths(
        codex_home: PathBuf,
        global_skill_root: PathBuf,
        current_executable: PathBuf,
        state_directory: PathBuf,
    ) -> Result<Self, String> {
        let config = CodexConfigService::new(InstallRequest::new(
            codex_home,
            global_skill_root,
            current_executable,
        ))
        .map_err(|_| "Smart Routing configuration is unavailable".to_owned())?;
        let routing = RoutingRuntime::load(RoutingStateStore::in_directory(state_directory)?)?;
        Ok(Self {
            config,
            routing,
            restart_required: Mutex::new(false),
            restart_blocked: Mutex::new(false),
            preflight: Mutex::new(PreflightCoordinator::new()),
            preflight_status: Mutex::new(RoutingPreflightStatus::NotStarted),
            cdp_status: Mutex::new(RoutingCdpStatus::Inactive),
        })
    }

    pub fn snapshot(&self) -> RoutingUiSnapshot {
        let inspected = self.config.inspect();
        let restart_required = *lock(&self.restart_required);
        let (installation_status, backup_label, config_changes, reason_codes) = match inspected {
            Ok(receipt) if !receipt.conflicts.is_empty() => (
                RoutingInstallationStatus::Conflict,
                None,
                Vec::new(),
                vec![RoutingSetupReasonCode::ConfigConflict],
            ),
            Ok(receipt) if receipt.installed => (
                if restart_required {
                    RoutingInstallationStatus::RestartRequired
                } else {
                    RoutingInstallationStatus::Installed
                },
                Some(format!("routing-backup-{}", Local::now().format("%Y%m%d"))),
                Vec::new(),
                Vec::new(),
            ),
            Ok(_) => (
                RoutingInstallationStatus::Uninstalled,
                None,
                DESIRED_CONFIG_CHANGES.to_vec(),
                Vec::new(),
            ),
            Err(_) => (
                RoutingInstallationStatus::Conflict,
                None,
                Vec::new(),
                vec![RoutingSetupReasonCode::ConfigConflict],
            ),
        };
        RoutingUiSnapshot {
            contract_version: 1,
            setup: RoutingSetupSnapshot {
                installation_status,
                restart_status: if restart_required {
                    if *lock(&self.restart_blocked) {
                        RoutingRestartStatus::BlockedActiveChild
                    } else {
                        RoutingRestartStatus::Required
                    }
                } else {
                    RoutingRestartStatus::NotRequired
                },
                preflight_status: *lock(&self.preflight_status),
                cdp_status: *lock(&self.cdp_status),
                backup_label,
                config_changes,
                reason_codes,
            },
            routing: self.routing.snapshot(),
        }
    }

    pub fn install(&self) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        match self.config.install() {
            Ok(receipt) => {
                if receipt.changed {
                    *lock(&self.restart_required) = true;
                    *lock(&self.restart_blocked) = false;
                    *lock(&self.cdp_status) = RoutingCdpStatus::Inactive;
                }
                OperationReceipt {
                    operation_id,
                    status: if receipt.changed {
                        OperationStatus::Applied
                    } else {
                        OperationStatus::Noop
                    },
                    reason_codes: Vec::new(),
                    restart_required: *lock(&self.restart_required),
                }
            }
            Err(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::ConfigConflict],
                restart_required: false,
            },
        }
    }

    pub fn restore(&self) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        match self.config.restore() {
            Ok(receipt) if !receipt.conflicts.is_empty() => OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::ConfigConflict],
                restart_required: *lock(&self.restart_required),
            },
            Ok(receipt) => {
                if receipt.changed {
                    *lock(&self.restart_required) = true;
                    *lock(&self.restart_blocked) = false;
                    *lock(&self.cdp_status) = RoutingCdpStatus::Inactive;
                }
                OperationReceipt {
                    operation_id,
                    status: if receipt.changed {
                        OperationStatus::Applied
                    } else {
                        OperationStatus::Noop
                    },
                    reason_codes: Vec::new(),
                    restart_required: *lock(&self.restart_required),
                }
            }
            Err(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::ConfigConflict],
                restart_required: *lock(&self.restart_required),
            },
        }
    }

    pub fn set_root_enabled(
        &self,
        root_conversation_id: &str,
        _enabled: bool,
        root_is_observed: bool,
    ) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if Uuid::parse_str(root_conversation_id).is_err() || !root_is_observed {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::UnsupportedHost],
                restart_required: *lock(&self.restart_required),
            };
        }
        if self.snapshot().setup.preflight_status != RoutingPreflightStatus::Complete {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::PreflightRequired],
                restart_required: *lock(&self.restart_required),
            };
        }
        OperationReceipt {
            operation_id,
            status: OperationStatus::Blocked,
            reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
            restart_required: *lock(&self.restart_required),
        }
    }

    pub fn unavailable_operation(&self) -> OperationReceipt {
        OperationReceipt {
            operation_id: Uuid::new_v4().to_string(),
            status: OperationStatus::Blocked,
            reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
            restart_required: *lock(&self.restart_required),
        }
    }

    pub fn begin_preflight(
        &self,
        root_conversation_id: &str,
        root_is_observed: bool,
    ) -> OperationReceipt {
        match discover_store_package() {
            Ok(package) => {
                self.begin_preflight_with(root_conversation_id, root_is_observed, &package.version)
            }
            Err(_) => OperationReceipt {
                operation_id: Uuid::new_v4().to_string(),
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::UnsupportedHost],
                restart_required: *lock(&self.restart_required),
            },
        }
    }

    pub fn begin_preflight_with(
        &self,
        root_conversation_id: &str,
        root_is_observed: bool,
        codex_package_version: &str,
    ) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        let Ok(root_id) = Uuid::parse_str(root_conversation_id) else {
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::UnsupportedHost,
                *lock(&self.restart_required),
            );
        };
        if root_id.is_nil() || !root_is_observed {
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::UnsupportedHost,
                *lock(&self.restart_required),
            );
        }
        let setup = self.snapshot().setup;
        if setup.installation_status != RoutingInstallationStatus::Installed
            || setup.restart_status != RoutingRestartStatus::NotRequired
        {
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::PreflightRequired,
                setup.restart_status != RoutingRestartStatus::NotRequired,
            );
        }
        if *lock(&self.preflight_status) == RoutingPreflightStatus::Running {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let started_at_ms = chrono::Utc::now().timestamp_millis().max(0);
        let mut coordinator = PreflightCoordinator::new();
        let mut keys = Vec::new();
        for model in [
            "gpt-5.3-codex-spark",
            "gpt-5.6-luna",
            "gpt-5.6-terra",
            "gpt-5.6-sol",
        ] {
            let key = EligibilityKey {
                codex_package_version: codex_package_version.to_owned(),
                profile_version: ROUTING_PROFILE_VERSION.to_owned(),
                requested_model: model.to_owned(),
                route_kind: RouteKind::Direct,
                depth: 1,
            };
            if coordinator
                .begin(key.clone(), root_id, root_id, started_at_ms, 120_000)
                .is_err()
            {
                return blocked_receipt(
                    operation_id,
                    RoutingSetupReasonCode::UnsupportedHost,
                    false,
                );
            }
            keys.push(key);
        }
        for key in &keys {
            if coordinator
                .persist_eligibility(key, started_at_ms, &self.routing)
                .is_err()
            {
                *lock(&self.preflight_status) = RoutingPreflightStatus::Degraded;
                return OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                    restart_required: false,
                };
            }
        }
        *lock(&self.preflight) = coordinator;
        *lock(&self.preflight_status) = RoutingPreflightStatus::Running;
        OperationReceipt {
            operation_id,
            status: OperationStatus::Applied,
            reason_codes: Vec::new(),
            restart_required: false,
        }
    }

    pub fn request_restart(&self, active_native_children: usize) -> OperationReceipt {
        self.request_restart_with(active_native_children, restart_verified_host)
    }

    pub fn request_restart_with<F>(
        &self,
        active_native_children: usize,
        restart: F,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<(), RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        let restart_required = *lock(&self.restart_required);
        if !restart_required {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        if active_native_children != 0 {
            *lock(&self.restart_blocked) = true;
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::ActiveChild],
                restart_required: true,
            };
        }
        *lock(&self.restart_blocked) = false;
        match restart() {
            Ok(()) => {
                *lock(&self.restart_required) = false;
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Err(reason) => {
                *lock(&self.cdp_status) = RoutingCdpStatus::Degraded;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![reason],
                    restart_required: true,
                }
            }
        }
    }
}

#[cfg(windows)]
fn restart_verified_host() -> Result<(), RoutingSetupReasonCode> {
    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let [current_process] = processes.as_slice() else {
        return Err(RoutingSetupReasonCode::UnsupportedHost);
    };
    let reservation = reserve_loopback_port().map_err(restart_reason)?;
    let restarted = restart_verified_codex(
        RestartGuard {
            verified_ui_processes: 1,
            active_native_children: 0,
            setup_phase: SetupPhase::Committed,
        },
        &package,
        current_process,
        reservation,
        15_000,
    )
    .map_err(restart_reason)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
    loop {
        let listener_verified = query_tcp_listener(restarted.port)
            .and_then(|listener| verify_listener(&restarted.process, listener, restarted.port))
            .is_ok();
        let endpoint_verified = runtime
            .block_on(fetch_browser_endpoint(restarted.port, 1_000))
            .is_ok();
        if listener_verified && endpoint_verified {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(RoutingSetupReasonCode::CdpUnavailable);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(not(windows))]
fn restart_verified_host() -> Result<(), RoutingSetupReasonCode> {
    Err(RoutingSetupReasonCode::UnsupportedHost)
}

fn restart_reason(error: IdentityError) -> RoutingSetupReasonCode {
    match error {
        IdentityError::ActiveNativeChild => RoutingSetupReasonCode::ActiveChild,
        IdentityError::PackageMissing
        | IdentityError::AmbiguousPackage
        | IdentityError::PackageQuery
        | IdentityError::PackageName
        | IdentityError::PackageFamily
        | IdentityError::PackageVersion
        | IdentityError::PackageRoot
        | IdentityError::ExecutableOutsidePackage
        | IdentityError::ExecutableName
        | IdentityError::Signature
        | IdentityError::AmbiguousUiProcess
        | IdentityError::ProcessOwner
        | IdentityError::ProcessImage
        | IdentityError::ProcessPackage => RoutingSetupReasonCode::UnsupportedHost,
        _ => RoutingSetupReasonCode::CdpUnavailable,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn blocked_receipt(
    operation_id: String,
    reason: RoutingSetupReasonCode,
    restart_required: bool,
) -> OperationReceipt {
    OperationReceipt {
        operation_id,
        status: OperationStatus::Blocked,
        reason_codes: vec![reason],
        restart_required,
    }
}
