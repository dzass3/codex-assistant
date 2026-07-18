use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use chrono::Local;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    codex_config::{CodexConfigService, InstallRequest},
    control_layer::cdp::{
        create_owned_session_record, fetch_browser_endpoint, BrowserAnchor, BrowserEndpoint,
        CdpClientError, OwnedSessionRecord, OwnedSessionStore,
    },
    control_layer::injector::{
        insert_preflight_directive_on_pages_detailed, receive_control_event,
        set_control_routing_ready, sync_control_routing_enabled, ControlEvent, ControlReceiveError,
        VisibleControlBinding, VisiblePreflightRequest,
    },
    control_layer::windows_package::{
        discover_store_package, discover_verified_ui_processes, query_process_identity,
        query_tcp_listener, reserve_loopback_port, restart_verified_codex, verify_listener,
        IdentityError, RestartGuard, SetupPhase,
    },
    monitor::model::MonitorSnapshot,
    preflight::{EligibilityKey, PreflightCoordinator, PreflightPhase, PreflightSignal},
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightInsertionRequest {
    pub root_conversation_id: Uuid,
    pub route_key: Uuid,
    pub attempt_id: Uuid,
    pub directive: String,
}

pub struct RoutingApplication {
    config: CodexConfigService,
    routing: RoutingRuntime,
    restart_required: Mutex<bool>,
    restart_blocked: Mutex<bool>,
    preflight: Mutex<PreflightCoordinator>,
    preflight_status: Mutex<RoutingPreflightStatus>,
    cdp_status: Mutex<RoutingCdpStatus>,
    session_store: OwnedSessionStore,
    control_session: Mutex<Option<OwnedSessionRecord>>,
    control_binding: Mutex<Option<VisibleControlBinding>>,
    control_ready: Mutex<bool>,
    control_synced_enabled: Mutex<Option<bool>>,
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
        let session_store = OwnedSessionStore::in_directory(&state_directory)
            .map_err(|_| "Smart Routing control session is unavailable".to_owned())?;
        let routing = RoutingRuntime::load(RoutingStateStore::in_directory(state_directory)?)?;
        Ok(Self {
            config,
            routing,
            restart_required: Mutex::new(false),
            restart_blocked: Mutex::new(false),
            preflight: Mutex::new(PreflightCoordinator::new()),
            preflight_status: Mutex::new(RoutingPreflightStatus::NotStarted),
            cdp_status: Mutex::new(RoutingCdpStatus::Inactive),
            session_store,
            control_session: Mutex::new(None),
            control_binding: Mutex::new(None),
            control_ready: Mutex::new(false),
            control_synced_enabled: Mutex::new(None),
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
                    *lock(&self.control_session) = None;
                    *lock(&self.control_binding) = None;
                    *lock(&self.control_ready) = false;
                    *lock(&self.control_synced_enabled) = None;
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
                    *lock(&self.control_session) = None;
                    *lock(&self.control_binding) = None;
                    *lock(&self.control_ready) = false;
                    *lock(&self.control_synced_enabled) = None;
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
        enabled: bool,
        root_is_observed: bool,
    ) -> OperationReceipt {
        self.set_root_enabled_with_activity(root_conversation_id, enabled, root_is_observed, 0)
    }

    pub fn set_root_enabled_with_activity(
        &self,
        root_conversation_id: &str,
        enabled: bool,
        root_is_observed: bool,
        active_native_children: usize,
    ) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        let Ok(root_id) = Uuid::parse_str(root_conversation_id) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::UnsupportedHost],
                restart_required: *lock(&self.restart_required),
            };
        };
        if root_id.is_nil() || !root_is_observed {
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
        if !enabled && active_native_children != 0 {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Blocked,
                reason_codes: vec![RoutingSetupReasonCode::ActiveChild],
                restart_required: false,
            };
        }
        match self.routing.set_root_enabled(
            root_id,
            enabled,
            chrono::Utc::now().timestamp_millis().max(0),
        ) {
            Ok(changed) => {
                if changed {
                    *lock(&self.control_synced_enabled) = None;
                }
                OperationReceipt {
                    operation_id,
                    status: if changed {
                        OperationStatus::Applied
                    } else {
                        OperationStatus::Noop
                    },
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Err(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                restart_required: false,
            },
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
            "gpt-5.6-terra",
            "gpt-5.3-codex-spark",
            "gpt-5.6-luna",
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
        if self
            .routing
            .ensure_root_route(root_id, started_at_ms)
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
        *lock(&self.preflight) = coordinator;
        *lock(&self.preflight_status) = RoutingPreflightStatus::Running;
        OperationReceipt {
            operation_id,
            status: OperationStatus::Applied,
            reason_codes: Vec::new(),
            restart_required: false,
        }
    }

    pub fn insert_next_preflight_with<F>(&self, insert: F) -> OperationReceipt
    where
        F: FnOnce(&PreflightInsertionRequest) -> Result<bool, RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        if *lock(&self.preflight_status) != RoutingPreflightStatus::Running {
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::PreflightRequired,
                false,
            );
        }
        let mut coordinator = lock(&self.preflight);
        if !coordinator.active_keys().is_empty() {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let Ok(Some((key, root_conversation_id, directive))) = coordinator.next_visible_directive()
        else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let Some(route_key) = self
            .routing
            .snapshot()
            .routes
            .iter()
            .find(|route| route.conversation_id == root_conversation_id)
            .map(|route| route.route_key)
        else {
            *lock(&self.preflight_status) = RoutingPreflightStatus::Degraded;
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                restart_required: false,
            };
        };
        let request = PreflightInsertionRequest {
            root_conversation_id,
            route_key,
            attempt_id: directive.attempt_id,
            directive: directive.text,
        };
        match insert(&request) {
            Ok(true) => {
                let now_ms = chrono::Utc::now().timestamp_millis().max(0);
                if coordinator.mark_visible_command_submitted(&key).is_err()
                    || coordinator
                        .persist_eligibility(&key, now_ms, &self.routing)
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
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Ok(false) => {
                blocked_receipt(operation_id, RoutingSetupReasonCode::UnsupportedHost, false)
            }
            Err(reason) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![reason],
                restart_required: false,
            },
        }
    }

    pub fn insert_next_preflight(&self) -> OperationReceipt {
        let mut verified_binding = None;
        let receipt = self.insert_next_preflight_with(|request| {
            let binding = insert_preflight_into_verified_host(&self.session_store, request)?;
            let inserted = binding.is_some();
            verified_binding = binding;
            Ok(inserted)
        });
        if receipt.status == OperationStatus::Applied {
            if let Some(binding) = verified_binding {
                *lock(&self.control_binding) = Some(binding);
                *lock(&self.control_ready) = false;
                *lock(&self.control_synced_enabled) = None;
            }
        }
        receipt
    }

    pub fn reconcile_preflight_with(
        &self,
        snapshot: &MonitorSnapshot,
        codex_package_version: &str,
        now_ms: i64,
    ) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if *lock(&self.preflight_status) != RoutingPreflightStatus::Running {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let mut coordinator = lock(&self.preflight);
        let keys = coordinator.active_keys();
        if keys.is_empty() {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        for key in &keys {
            let outcome = match coordinator.reconcile_monitor(
                key,
                snapshot,
                codex_package_version,
                ROUTING_PROFILE_VERSION,
                now_ms,
                PreflightSignal::None,
            ) {
                Ok(outcome) => outcome,
                Err(_) => {
                    *lock(&self.preflight_status) = RoutingPreflightStatus::Degraded;
                    return OperationReceipt {
                        operation_id,
                        status: OperationStatus::Failed,
                        reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                        restart_required: false,
                    };
                }
            };
            if coordinator
                .persist_eligibility(key, now_ms, &self.routing)
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
            if key.route_kind == RouteKind::Direct
                && key.requested_model == "gpt-5.6-terra"
                && outcome.phase == PreflightPhase::Eligible
            {
                let Some(parent_id) = outcome.child_thread_id else {
                    *lock(&self.preflight_status) = RoutingPreflightStatus::Degraded;
                    return OperationReceipt {
                        operation_id,
                        status: OperationStatus::Failed,
                        reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                        restart_required: false,
                    };
                };
                let Some(root_id) = coordinator
                    .get(key)
                    .map(|record| record.attempt.expected_root_id)
                else {
                    *lock(&self.preflight_status) = RoutingPreflightStatus::Degraded;
                    return OperationReceipt {
                        operation_id,
                        status: OperationStatus::Failed,
                        reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                        restart_required: false,
                    };
                };
                for model in ["gpt-5.6-luna", "gpt-5.3-codex-spark"] {
                    let nested = EligibilityKey {
                        codex_package_version: codex_package_version.to_owned(),
                        profile_version: ROUTING_PROFILE_VERSION.to_owned(),
                        requested_model: model.to_owned(),
                        route_kind: RouteKind::Nested,
                        depth: 2,
                    };
                    if coordinator.get(&nested).is_none()
                        && (coordinator
                            .begin(nested.clone(), root_id, parent_id, now_ms, 120_000)
                            .is_err()
                            || coordinator
                                .persist_eligibility(&nested, now_ms, &self.routing)
                                .is_err())
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
            }
        }
        if coordinator.is_complete() {
            *lock(&self.preflight_status) = RoutingPreflightStatus::Complete;
        }
        OperationReceipt {
            operation_id,
            status: OperationStatus::Applied,
            reason_codes: Vec::new(),
            restart_required: false,
        }
    }

    pub fn reconcile_preflight(&self, snapshot: &MonitorSnapshot) -> OperationReceipt {
        let version = lock(&self.preflight).host_version();
        let Some(version) = version else {
            return OperationReceipt {
                operation_id: Uuid::new_v4().to_string(),
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        self.reconcile_preflight_with(
            snapshot,
            &version,
            chrono::Utc::now().timestamp_millis().max(0),
        )
    }

    pub fn apply_control_event(
        &self,
        event: ControlEvent,
        active_native_children: usize,
    ) -> OperationReceipt {
        match event {
            ControlEvent::Toggle { route_id, enabled } => {
                let known_root = self
                    .routing
                    .snapshot()
                    .routes
                    .iter()
                    .any(|route| route.conversation_id == route_id);
                if !known_root {
                    return blocked_receipt(
                        Uuid::new_v4().to_string(),
                        RoutingSetupReasonCode::UnsupportedHost,
                        false,
                    );
                }
                self.set_root_enabled_with_activity(
                    &route_id.to_string(),
                    enabled,
                    true,
                    active_native_children,
                )
            }
            ControlEvent::Compatibility { .. }
            | ControlEvent::SubmitIntent { .. }
            | ControlEvent::InsertionResult { .. } => OperationReceipt {
                operation_id: Uuid::new_v4().to_string(),
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            },
        }
    }

    pub fn ensure_control_ready(&self) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if *lock(&self.preflight_status) != RoutingPreflightStatus::Complete
            || *lock(&self.control_ready)
        {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let session = lock(&self.control_session).clone();
        let binding = lock(&self.control_binding).clone();
        let (Some(session), Some(binding)) = (session, binding) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            };
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            };
        };
        match runtime.block_on(set_control_routing_ready(
            &endpoint,
            &binding.target_id,
            true,
            1_500,
        )) {
            Ok(true) => {
                *lock(&self.control_ready) = true;
                *lock(&self.control_synced_enabled) = None;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            _ => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            },
        }
    }

    pub fn poll_control_event(&self, active_native_children: usize) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if !*lock(&self.control_ready) {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let session = lock(&self.control_session).clone();
        let binding = lock(&self.control_binding).clone();
        let (Some(session), Some(binding)) = (session, binding) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return self.unavailable_operation();
        };
        match runtime.block_on(receive_control_event(
            &endpoint,
            &binding.target_id,
            &binding.session_id,
            1_500,
        )) {
            Ok(event) if control_event_route_id(&event) == binding.root_conversation_id => {
                let is_toggle = matches!(event, ControlEvent::Toggle { .. });
                let receipt = self.apply_control_event(event, active_native_children);
                if is_toggle {
                    let enabled = self
                        .routing
                        .snapshot()
                        .routes
                        .iter()
                        .find(|route| route.conversation_id == binding.root_conversation_id)
                        .is_some_and(|route| route.enabled);
                    if runtime.block_on(sync_control_routing_enabled(
                        &endpoint,
                        &binding.target_id,
                        enabled,
                        1_500,
                    )) != Ok(true)
                    {
                        return OperationReceipt {
                            operation_id,
                            status: OperationStatus::Failed,
                            reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                            restart_required: false,
                        };
                    }
                    *lock(&self.control_synced_enabled) = Some(enabled);
                }
                receipt
            }
            Ok(_) => blocked_receipt(operation_id, RoutingSetupReasonCode::UnsupportedHost, false),
            Err(ControlReceiveError::Cdp(CdpClientError::TimedOut))
            | Err(ControlReceiveError::TargetUnavailable)
            | Err(ControlReceiveError::Discovery(_)) => OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            },
            Err(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            },
        }
    }

    pub fn sync_control_state(&self) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if !*lock(&self.control_ready) {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let binding = lock(&self.control_binding).clone();
        let session = lock(&self.control_session).clone();
        let (Some(binding), Some(session)) = (binding, session) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let enabled = self
            .routing
            .snapshot()
            .routes
            .iter()
            .find(|route| route.conversation_id == binding.root_conversation_id)
            .is_some_and(|route| route.enabled);
        if *lock(&self.control_synced_enabled) == Some(enabled) {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            };
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return self.unavailable_operation();
        };
        match runtime.block_on(sync_control_routing_enabled(
            &endpoint,
            &binding.target_id,
            enabled,
            1_500,
        )) {
            Ok(true) => {
                *lock(&self.control_synced_enabled) = Some(enabled);
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            _ => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            },
        }
    }

    pub fn request_restart(&self, active_native_children: usize) -> OperationReceipt {
        self.request_restart_with_session(active_native_children, restart_verified_host)
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

    pub fn request_restart_with_session<F>(
        &self,
        active_native_children: usize,
        restart: F,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<OwnedSessionRecord, RoutingSetupReasonCode>,
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
            Ok(record) if self.session_store.save(&record).is_ok() => {
                *lock(&self.control_session) = Some(record);
                *lock(&self.restart_required) = false;
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Ok(_) => {
                *lock(&self.cdp_status) = RoutingCdpStatus::Degraded;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                    restart_required: true,
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

fn control_event_route_id(event: &ControlEvent) -> Uuid {
    match event {
        ControlEvent::Compatibility { route_id, .. }
        | ControlEvent::Toggle { route_id, .. }
        | ControlEvent::SubmitIntent { route_id, .. }
        | ControlEvent::InsertionResult { route_id, .. } => *route_id,
    }
}

fn verified_control_endpoint(
    session: &OwnedSessionRecord,
) -> Result<BrowserEndpoint, RoutingSetupReasonCode> {
    let listener = query_tcp_listener(session.port).map_err(restart_reason)?;
    if listener.address != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        || listener.port != session.port
        || listener.pid != session.verified_pid
    {
        return Err(RoutingSetupReasonCode::CdpUnavailable);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
    let endpoint = runtime
        .block_on(fetch_browser_endpoint(session.port, 1_000))
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
    if BrowserAnchor::new(&endpoint).hash() != session.browser_id_hash {
        return Err(RoutingSetupReasonCode::CdpUnavailable);
    }
    Ok(endpoint)
}

#[cfg(windows)]
fn restart_verified_host() -> Result<OwnedSessionRecord, RoutingSetupReasonCode> {
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
        let endpoint = runtime.block_on(fetch_browser_endpoint(restarted.port, 1_000));
        if listener_verified {
            if let Ok(endpoint) = endpoint {
                return create_owned_session_record(
                    &endpoint,
                    restarted.process.pid,
                    &package.version,
                    chrono::Utc::now().timestamp_millis().max(0),
                )
                .map_err(|_| RoutingSetupReasonCode::CdpUnavailable);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(RoutingSetupReasonCode::CdpUnavailable);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(windows)]
fn insert_preflight_into_verified_host(
    session_store: &OwnedSessionStore,
    request: &PreflightInsertionRequest,
) -> Result<Option<VisibleControlBinding>, RoutingSetupReasonCode> {
    const CONTROL_SCRIPT: &str = include_str!("../resources/control/routing-control.js");
    const CONTROL_CSS: &str = include_str!("../resources/control/routing-control.css");

    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let [current_process] = processes.as_slice() else {
        return Err(RoutingSetupReasonCode::UnsupportedHost);
    };
    let now_ms = chrono::Utc::now().timestamp_millis().max(0);
    let record = session_store
        .load(current_process.pid, &package.version, now_ms)
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?
        .ok_or(RoutingSetupReasonCode::CdpUnavailable)?;
    let listener = query_tcp_listener(record.port).map_err(restart_reason)?;
    verify_listener(current_process, listener, record.port).map_err(restart_reason)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
    let endpoint = runtime
        .block_on(fetch_browser_endpoint(record.port, 1_000))
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
    if BrowserAnchor::new(&endpoint).hash() != record.browser_id_hash {
        return Err(RoutingSetupReasonCode::CdpUnavailable);
    }
    runtime
        .block_on(insert_preflight_directive_on_pages_detailed(
            &endpoint,
            CONTROL_SCRIPT,
            CONTROL_CSS,
            &VisiblePreflightRequest {
                session_id: format!("session-{}", request.route_key),
                root_conversation_id: request.root_conversation_id,
                route_key: request.route_key,
                directive: request.directive.clone(),
            },
            2_000,
        ))
        .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)
}

#[cfg(not(windows))]
fn insert_preflight_into_verified_host(
    _session_store: &OwnedSessionStore,
    _request: &PreflightInsertionRequest,
) -> Result<Option<VisibleControlBinding>, RoutingSetupReasonCode> {
    Err(RoutingSetupReasonCode::UnsupportedHost)
}

#[cfg(not(windows))]
fn restart_verified_host() -> Result<OwnedSessionRecord, RoutingSetupReasonCode> {
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
