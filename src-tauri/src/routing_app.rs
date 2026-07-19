use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    codex_config::{CodexConfigService, InstallRequest},
    control_layer::cdp::{
        create_owned_session_record, fetch_browser_endpoint, BrowserAnchor, BrowserEndpoint,
        CdpClientError, OwnedSessionRecord, OwnedSessionStore,
    },
    control_layer::injector::{
        bind_routing_controls_on_pages_detailed, insert_preflight_directive_on_pages_detailed,
        receive_control_event, request_visible_agent_stop, set_control_routing_ready,
        sync_control_routing_enabled, ControlEvent, ControlReceiveError, VisibleControlBinding,
        VisiblePreflightRequest, VisibleRootControlRequest,
    },
    control_layer::windows_package::{
        discover_store_package, discover_verified_ui_processes, query_process_identity,
        query_tcp_listener, reserve_loopback_port, restart_verified_codex,
        restart_verified_codex_force, root_fingerprint, verify_listener, IdentityError,
        RestartGuard, SetupPhase,
    },
    monitor::model::MonitorSnapshot,
    preflight::{EligibilityKey, PreflightCoordinator, PreflightPhase, PreflightSignal},
    routing::{
        state::RoutingStateStore, EligibilityStatus, RouteKind, RoutingRuntime, RoutingSnapshot,
    },
    theme::{
        apply_theme_on_pages, bundled_theme_packs, restore_theme_on_pages, ThemeEngineError,
        ThemePack, ThemePreferenceStore, ThemeScriptRegistration,
    },
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
pub enum RoutingActivationStatus {
    Off,
    PendingOpen,
    PendingNextTurn,
    Enabled,
    NeedsRepair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSessionStatus {
    Inactive,
    Paused,
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
    ConfirmationRequired,
    ConfirmationExpired,
    ImpactChanged,
    OperationConflict,
    IdentityChanged,
    GracefulStopUnsupported,
    TerminationFailed,
    OldTreeStillRunning,
    LaunchFailed,
    CdpVerificationFailed,
    DomIncompatible,
    PartialApplyFailed,
    TerminalPartialFailure,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartMode {
    Safe,
    ForceAfterGrace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartIntent {
    RoutingRestart,
    ThemeSession,
    ActivateTheme,
}

pub use crate::control_layer::windows_package::VerifiedRootFingerprint;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ForceRestartImpact {
    pub confirmation_ticket: String,
    pub intent: RestartIntent,
    pub active_native_children: usize,
    pub grace_period_ms: u32,
    pub expires_at_ms: i64,
}

#[derive(Clone, Debug)]
struct ForceRestartTicket {
    intent: RestartIntent,
    subject: Option<String>,
    active_native_children: usize,
    expires_at_ms: i64,
    fingerprint: VerifiedRootFingerprint,
    cancellation: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug)]
pub struct ForceRestartExecution<'a> {
    pub confirmation_ticket: &'a str,
    pub intent: RestartIntent,
    pub subject: Option<&'a str>,
    pub active_native_children: usize,
    pub now_ms: i64,
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
    pub controls: Vec<RootRoutingControlSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RootRoutingControlSnapshot {
    pub conversation_id: Uuid,
    pub status: RoutingActivationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeUiSnapshot {
    pub contract_version: u32,
    pub session_status: ThemeSessionStatus,
    pub selected_theme_id: Option<String>,
    pub applied_theme_id: Option<String>,
    pub packs: Vec<ThemePack>,
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
    root_control_bindings: Mutex<HashMap<Uuid, VisibleControlBinding>>,
    control_ready: Mutex<bool>,
    control_synced_enabled: Mutex<Option<bool>>,
    routing_activation: Mutex<HashMap<Uuid, RoutingActivationStatus>>,
    theme_status: Mutex<ThemeSessionStatus>,
    theme_preference_store: ThemePreferenceStore,
    selected_theme_id: Mutex<Option<String>>,
    applied_theme_id: Mutex<Option<String>>,
    theme_scripts: Mutex<Vec<ThemeScriptRegistration>>,
    theme_reconcile_at_ms: Mutex<i64>,
    force_restart_tickets: Mutex<HashMap<String, ForceRestartTicket>>,
    force_restart_cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    lifecycle_active: Mutex<bool>,
}

struct LifecycleLease<'a>(&'a Mutex<bool>);

impl Drop for LifecycleLease<'_> {
    fn drop(&mut self) {
        *lock(self.0) = false;
    }
}

impl RoutingApplication {
    fn try_lifecycle(&self) -> Option<LifecycleLease<'_>> {
        let mut active = lock(&self.lifecycle_active);
        if *active {
            return None;
        }
        *active = true;
        drop(active);
        Some(LifecycleLease(&self.lifecycle_active))
    }
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
        let theme_preference_store = ThemePreferenceStore::in_directory(&state_directory)?;
        let selected_theme_id = theme_preference_store.load()?;
        let theme_status = if selected_theme_id.is_some() {
            ThemeSessionStatus::Paused
        } else {
            ThemeSessionStatus::Inactive
        };
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
            root_control_bindings: Mutex::new(HashMap::new()),
            control_ready: Mutex::new(false),
            control_synced_enabled: Mutex::new(None),
            routing_activation: Mutex::new(HashMap::new()),
            theme_status: Mutex::new(theme_status),
            theme_preference_store,
            selected_theme_id: Mutex::new(selected_theme_id),
            applied_theme_id: Mutex::new(None),
            theme_scripts: Mutex::new(Vec::new()),
            theme_reconcile_at_ms: Mutex::new(0),
            force_restart_tickets: Mutex::new(HashMap::new()),
            force_restart_cancellations: Mutex::new(HashMap::new()),
            lifecycle_active: Mutex::new(false),
        })
    }

    pub fn prepare_force_restart_with<F>(
        &self,
        intent: RestartIntent,
        active_native_children: usize,
        now_ms: i64,
        fingerprint: F,
    ) -> Result<ForceRestartImpact, RoutingSetupReasonCode>
    where
        F: FnOnce() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode>,
    {
        self.prepare_force_restart_for_with(
            intent,
            None,
            active_native_children,
            now_ms,
            fingerprint,
        )
    }

    pub fn prepare_force_restart_for_with<F>(
        &self,
        intent: RestartIntent,
        subject: Option<String>,
        active_native_children: usize,
        now_ms: i64,
        fingerprint: F,
    ) -> Result<ForceRestartImpact, RoutingSetupReasonCode>
    where
        F: FnOnce() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode>,
    {
        if active_native_children == 0 {
            return Err(RoutingSetupReasonCode::ConfirmationRequired);
        }
        if *lock(&self.lifecycle_active) {
            return Err(RoutingSetupReasonCode::OperationConflict);
        }
        let fingerprint = fingerprint()?;
        let confirmation_ticket = Uuid::new_v4().to_string();
        let expires_at_ms = now_ms.saturating_add(60_000);
        let cancellation = Arc::new(AtomicBool::new(false));
        lock(&self.force_restart_tickets).insert(
            confirmation_ticket.clone(),
            ForceRestartTicket {
                intent,
                subject,
                active_native_children,
                expires_at_ms,
                fingerprint,
                cancellation: Arc::clone(&cancellation),
            },
        );
        lock(&self.force_restart_cancellations).insert(confirmation_ticket.clone(), cancellation);
        Ok(ForceRestartImpact {
            confirmation_ticket,
            intent,
            active_native_children,
            grace_period_ms: 5_000,
            expires_at_ms,
        })
    }

    pub fn force_restart_with<F, R>(
        &self,
        confirmation_ticket: &str,
        intent: RestartIntent,
        active_native_children: usize,
        now_ms: i64,
        fingerprint: F,
        restart: R,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode>,
        R: FnOnce(&VerifiedRootFingerprint, &AtomicBool) -> Result<(), RoutingSetupReasonCode>,
    {
        self.force_restart_for_with(
            ForceRestartExecution {
                confirmation_ticket,
                intent,
                subject: None,
                active_native_children,
                now_ms,
            },
            fingerprint,
            restart,
        )
    }

    pub fn force_restart_for_with<F, R>(
        &self,
        execution: ForceRestartExecution<'_>,
        fingerprint: F,
        restart: R,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode>,
        R: FnOnce(&VerifiedRootFingerprint, &AtomicBool) -> Result<(), RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        let Some(ticket) = lock(&self.force_restart_tickets).remove(execution.confirmation_ticket)
        else {
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::ConfirmationExpired,
                *lock(&self.restart_required),
            );
        };
        if execution.now_ms > ticket.expires_at_ms
            || ticket.intent != execution.intent
            || ticket.subject.as_deref() != execution.subject
        {
            lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::ConfirmationExpired,
                *lock(&self.restart_required),
            );
        }
        if ticket.active_native_children != execution.active_native_children {
            lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::ImpactChanged,
                *lock(&self.restart_required),
            );
        }
        let current = match fingerprint() {
            Ok(current) => current,
            Err(reason) => {
                lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
                return blocked_receipt(operation_id, reason, *lock(&self.restart_required));
            }
        };
        if current != ticket.fingerprint {
            lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
            return blocked_receipt(
                operation_id,
                RoutingSetupReasonCode::IdentityChanged,
                *lock(&self.restart_required),
            );
        }
        {
            let mut active = lock(&self.lifecycle_active);
            if *active {
                lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
                return blocked_receipt(
                    operation_id,
                    RoutingSetupReasonCode::OperationConflict,
                    *lock(&self.restart_required),
                );
            }
            *active = true;
        }
        let result = restart(&ticket.fingerprint, &ticket.cancellation);
        *lock(&self.lifecycle_active) = false;
        lock(&self.force_restart_cancellations).remove(execution.confirmation_ticket);
        match result {
            Ok(()) => {
                *lock(&self.restart_required) = false;
                *lock(&self.restart_blocked) = false;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Err(reason) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![reason],
                restart_required: *lock(&self.restart_required),
            },
        }
    }

    pub fn cancel_force_restart(&self, confirmation_ticket: &str) -> bool {
        let cancellation = lock(&self.force_restart_cancellations)
            .get(confirmation_ticket)
            .cloned();
        if let Some(cancellation) = cancellation {
            cancellation.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn prepare_force_restart(
        &self,
        intent: RestartIntent,
        subject: Option<String>,
        active_native_children: usize,
    ) -> Result<ForceRestartImpact, RoutingSetupReasonCode> {
        self.prepare_force_restart_for_with(
            intent,
            subject,
            active_native_children,
            chrono::Utc::now().timestamp_millis().max(0),
            current_verified_root_fingerprint,
        )
    }

    pub fn request_restart_mode(
        &self,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        active_native_children: usize,
    ) -> OperationReceipt {
        if mode == RestartMode::Safe {
            return self.request_restart(active_native_children);
        }
        let Some(ticket) = confirmation_ticket else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::ConfirmationRequired,
                *lock(&self.restart_required),
            );
        };
        let next_session = Mutex::new(None);
        let receipt = self.force_restart_for_with(
            ForceRestartExecution {
                confirmation_ticket: ticket,
                intent: RestartIntent::RoutingRestart,
                subject: None,
                active_native_children,
                now_ms: chrono::Utc::now().timestamp_millis().max(0),
            },
            current_verified_root_fingerprint,
            |expected, cancellation| {
                self.attempt_graceful_agent_stop();
                *lock(&next_session) = Some(restart_verified_host_force(expected, cancellation)?);
                Ok(())
            },
        );
        if receipt.status == OperationStatus::Applied {
            if let Some(record) = lock(&next_session).take() {
                if self.session_store.save(&record).is_err() {
                    return failed_receipt(
                        receipt.operation_id,
                        RoutingSetupReasonCode::TerminalPartialFailure,
                        false,
                    );
                }
                *lock(&self.control_session) = Some(record);
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
            }
        }
        receipt
    }

    pub fn retry_theme_application_with<A, W>(
        &self,
        maximum_attempts: usize,
        mut attempt: A,
        mut wait: W,
    ) -> OperationReceipt
    where
        A: FnMut() -> OperationReceipt,
        W: FnMut(),
    {
        let attempts = maximum_attempts.max(1);
        for index in 0..attempts {
            let receipt = attempt();
            let dom_not_ready = receipt.status == OperationStatus::Failed
                && receipt.reason_codes == vec![RoutingSetupReasonCode::DomIncompatible];
            if !dom_not_ready || index + 1 == attempts {
                return receipt;
            }
            wait();
        }
        unreachable!("at least one theme application attempt is required")
    }

    fn apply_theme_until_main_ready(&self, theme_id: &str) -> OperationReceipt {
        self.retry_theme_application_with(
            41,
            || self.apply_theme(theme_id),
            || std::thread::sleep(std::time::Duration::from_millis(250)),
        )
    }

    pub fn start_theme_session_mode(
        &self,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        active_native_children: usize,
    ) -> OperationReceipt {
        if mode == RestartMode::Safe {
            return self.start_theme_session(active_native_children);
        }
        let Some(ticket) = confirmation_ticket else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::ConfirmationRequired,
                false,
            );
        };
        let next_session = Mutex::new(None);
        let receipt = self.force_restart_for_with(
            ForceRestartExecution {
                confirmation_ticket: ticket,
                intent: RestartIntent::ThemeSession,
                subject: None,
                active_native_children,
                now_ms: chrono::Utc::now().timestamp_millis().max(0),
            },
            current_verified_root_fingerprint,
            |expected, cancellation| {
                self.attempt_graceful_agent_stop();
                *lock(&next_session) = Some(restart_verified_host_force(expected, cancellation)?);
                Ok(())
            },
        );
        self.commit_theme_session_receipt(receipt, &next_session)
    }

    pub fn activate_theme_mode(
        &self,
        theme_id: &str,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        active_native_children: usize,
    ) -> OperationReceipt {
        *lock(&self.selected_theme_id) = Some(theme_id.to_owned());
        if mode == RestartMode::Safe {
            return self.activate_theme(theme_id, active_native_children);
        }
        let Some(ticket) = confirmation_ticket else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::ConfirmationRequired,
                false,
            );
        };
        let next_session = Mutex::new(None);
        let restarted = self.force_restart_for_with(
            ForceRestartExecution {
                confirmation_ticket: ticket,
                intent: RestartIntent::ActivateTheme,
                subject: Some(theme_id),
                active_native_children,
                now_ms: chrono::Utc::now().timestamp_millis().max(0),
            },
            current_verified_root_fingerprint,
            |expected, cancellation| {
                self.attempt_graceful_agent_stop();
                *lock(&next_session) = Some(restart_verified_host_force(expected, cancellation)?);
                Ok(())
            },
        );
        let started = self.commit_theme_session_receipt(restarted, &next_session);
        if started.status != OperationStatus::Applied {
            return started;
        }
        self.apply_theme_until_main_ready(theme_id)
    }

    fn commit_theme_session_receipt(
        &self,
        receipt: OperationReceipt,
        next_session: &Mutex<Option<OwnedSessionRecord>>,
    ) -> OperationReceipt {
        if receipt.status != OperationStatus::Applied {
            return receipt;
        }
        let Some(record) = lock(next_session).take() else {
            return failed_receipt(
                receipt.operation_id,
                RoutingSetupReasonCode::TerminalPartialFailure,
                false,
            );
        };
        if self.session_store.save(&record).is_err() {
            return failed_receipt(
                receipt.operation_id,
                RoutingSetupReasonCode::TerminalPartialFailure,
                false,
            );
        }
        *lock(&self.control_session) = Some(record);
        *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
        *lock(&self.theme_status) = ThemeSessionStatus::Ready;
        receipt
    }

    fn attempt_graceful_agent_stop(&self) {
        let Some(session) = lock(&self.control_session).clone() else {
            return;
        };
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let _ = runtime.block_on(request_visible_agent_stop(&endpoint, 1_500));
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
        let routing = self.routing.snapshot();
        let activation = lock(&self.routing_activation);
        let controls = routing
            .routes
            .iter()
            .map(|route| RootRoutingControlSnapshot {
                conversation_id: route.conversation_id,
                status: if route.enabled {
                    activation
                        .get(&route.conversation_id)
                        .copied()
                        .unwrap_or(RoutingActivationStatus::PendingOpen)
                } else {
                    RoutingActivationStatus::Off
                },
            })
            .collect();
        RoutingUiSnapshot {
            contract_version: 2,
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
            routing,
            controls,
        }
    }

    pub fn theme_snapshot(&self) -> ThemeUiSnapshot {
        ThemeUiSnapshot {
            contract_version: 2,
            session_status: *lock(&self.theme_status),
            selected_theme_id: lock(&self.selected_theme_id).clone(),
            applied_theme_id: lock(&self.applied_theme_id).clone(),
            packs: bundled_theme_packs(),
        }
    }

    pub fn reconcile_theme_session(&self) -> bool {
        if *lock(&self.theme_status) == ThemeSessionStatus::Paused
            && lock(&self.control_session).is_none()
        {
            return false;
        }
        if lock(&self.control_session).is_none() {
            if let Some(record) = recover_verified_host_session(&self.session_store) {
                *lock(&self.control_session) = Some(record);
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
                *lock(&self.theme_status) = ThemeSessionStatus::Ready;
            }
        }
        self.reconcile_theme_session_with(|record| verified_control_endpoint(record).is_ok())
    }

    pub fn reconcile_theme_session_with<F>(&self, verify: F) -> bool
    where
        F: FnOnce(&OwnedSessionRecord) -> bool,
    {
        let session = lock(&self.control_session).clone();
        if session.as_ref().is_some_and(verify) {
            return true;
        }
        *lock(&self.control_session) = None;
        *lock(&self.control_binding) = None;
        lock(&self.root_control_bindings).clear();
        *lock(&self.control_ready) = false;
        *lock(&self.control_synced_enabled) = None;
        *lock(&self.cdp_status) = RoutingCdpStatus::Inactive;
        *lock(&self.theme_status) = if lock(&self.selected_theme_id).is_some() {
            ThemeSessionStatus::Paused
        } else {
            ThemeSessionStatus::Inactive
        };
        *lock(&self.applied_theme_id) = None;
        lock(&self.theme_scripts).clear();
        false
    }

    pub fn reconcile_active_theme(&self) {
        if *lock(&self.theme_status) != ThemeSessionStatus::Ready || *lock(&self.lifecycle_active) {
            return;
        }
        let now_ms = chrono::Utc::now().timestamp_millis().max(0);
        {
            let mut last = lock(&self.theme_reconcile_at_ms);
            if now_ms.saturating_sub(*last) < 5_000 {
                return;
            }
            *last = now_ms;
        }
        let Some(theme_id) = lock(&self.selected_theme_id).clone() else {
            return;
        };
        let _ = self.apply_theme(&theme_id);
    }

    pub fn reconcile_selected_theme_with<F>(&self, apply: F) -> OperationReceipt
    where
        F: FnOnce(&ThemePack) -> Result<usize, RoutingSetupReasonCode>,
    {
        let Some(theme_id) = lock(&self.selected_theme_id).clone() else {
            return OperationReceipt {
                operation_id: Uuid::new_v4().to_string(),
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        self.apply_theme_with(&theme_id, apply)
    }

    pub fn start_theme_session(&self, active_native_children: usize) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::OperationConflict,
                false,
            );
        };
        if let Some(record) = recover_verified_host_session(&self.session_store) {
            *lock(&self.control_session) = Some(record);
            *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
            *lock(&self.theme_status) = ThemeSessionStatus::Ready;
            return OperationReceipt {
                operation_id: Uuid::new_v4().to_string(),
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        *lock(&self.control_session) = None;
        *lock(&self.theme_status) = ThemeSessionStatus::Inactive;
        self.start_theme_session_with(active_native_children, restart_verified_host)
    }

    pub fn start_theme_session_with<F>(
        &self,
        active_native_children: usize,
        restart: F,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<OwnedSessionRecord, RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        if lock(&self.control_session).is_some() {
            *lock(&self.theme_status) = ThemeSessionStatus::Ready;
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        if active_native_children != 0 {
            return blocked_receipt(operation_id, RoutingSetupReasonCode::ActiveChild, false);
        }
        match restart() {
            Ok(record) if self.session_store.save(&record).is_ok() => {
                *lock(&self.control_session) = Some(record);
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
                *lock(&self.theme_status) = ThemeSessionStatus::Ready;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Ok(_) => {
                *lock(&self.theme_status) = ThemeSessionStatus::Degraded;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                    restart_required: false,
                }
            }
            Err(reason) => {
                *lock(&self.theme_status) = ThemeSessionStatus::Degraded;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![reason],
                    restart_required: false,
                }
            }
        }
    }

    pub fn apply_theme(&self, theme_id: &str) -> OperationReceipt {
        let previous_scripts = lock(&self.theme_scripts).clone();
        let next_scripts = std::sync::Mutex::new(None);
        let receipt = self.apply_theme_with(theme_id, |pack| {
            let session = lock(&self.control_session)
                .clone()
                .ok_or(RoutingSetupReasonCode::CdpUnavailable)?;
            let endpoint = verified_control_endpoint(&session)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
            let result = runtime
                .block_on(apply_theme_on_pages(
                    &endpoint,
                    pack,
                    &previous_scripts,
                    2_000,
                ))
                .map_err(theme_engine_reason)?;
            *lock(&next_scripts) = Some(result.scripts);
            Ok(result.applied_pages)
        });
        if receipt.status == OperationStatus::Applied {
            if let Some(scripts) = lock(&next_scripts).take() {
                *lock(&self.theme_scripts) = scripts;
            }
        } else if receipt
            .reason_codes
            .contains(&RoutingSetupReasonCode::CdpUnavailable)
        {
            *lock(&self.control_session) = None;
            *lock(&self.theme_status) = ThemeSessionStatus::Degraded;
        }
        receipt
    }

    pub fn activate_theme(
        &self,
        theme_id: &str,
        active_native_children: usize,
    ) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::OperationConflict,
                false,
            );
        };
        *lock(&self.selected_theme_id) = Some(theme_id.to_owned());
        if *lock(&self.theme_status) != ThemeSessionStatus::Ready {
            if let Some(record) = recover_verified_host_session(&self.session_store) {
                *lock(&self.control_session) = Some(record);
                *lock(&self.theme_status) = ThemeSessionStatus::Ready;
            }
            let started = if *lock(&self.theme_status) == ThemeSessionStatus::Ready {
                OperationReceipt {
                    operation_id: Uuid::new_v4().to_string(),
                    status: OperationStatus::Noop,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            } else {
                self.start_theme_session_with(active_native_children, restart_verified_host)
            };
            if !matches!(
                started.status,
                OperationStatus::Applied | OperationStatus::Noop
            ) {
                return started;
            }
        }
        self.apply_theme_until_main_ready(theme_id)
    }

    pub fn activate_theme_with<R, A>(
        &self,
        theme_id: &str,
        active_native_children: usize,
        restart: R,
        apply: A,
    ) -> OperationReceipt
    where
        R: FnOnce() -> Result<OwnedSessionRecord, RoutingSetupReasonCode>,
        A: FnOnce(&ThemePack) -> Result<usize, RoutingSetupReasonCode>,
    {
        *lock(&self.selected_theme_id) = Some(theme_id.to_owned());
        if *lock(&self.theme_status) != ThemeSessionStatus::Ready {
            let started = self.start_theme_session_with(active_native_children, restart);
            if !matches!(
                started.status,
                OperationStatus::Applied | OperationStatus::Noop
            ) {
                return started;
            }
        }
        self.apply_theme_with(theme_id, apply)
    }

    pub fn apply_theme_with<F>(&self, theme_id: &str, apply: F) -> OperationReceipt
    where
        F: FnOnce(&ThemePack) -> Result<usize, RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        *lock(&self.selected_theme_id) = Some(theme_id.to_owned());
        if self.theme_preference_store.save(Some(theme_id)).is_err() {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                restart_required: false,
            };
        }
        if *lock(&self.theme_status) != ThemeSessionStatus::Ready {
            return blocked_receipt(operation_id, RoutingSetupReasonCode::CdpUnavailable, false);
        }
        let Some(pack) = bundled_theme_packs()
            .into_iter()
            .find(|pack| pack.id == theme_id)
        else {
            return blocked_receipt(operation_id, RoutingSetupReasonCode::UnsupportedHost, false);
        };
        match apply(&pack) {
            Ok(count) if count != 0 => {
                *lock(&self.applied_theme_id) = Some(pack.id);
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Ok(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::UnsupportedHost],
                restart_required: false,
            },
            Err(reason) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![reason],
                restart_required: false,
            },
        }
    }

    pub fn restore_theme(&self) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::OperationConflict,
                false,
            );
        };
        let scripts = lock(&self.theme_scripts).clone();
        let receipt = self.restore_theme_with(|| {
            let session = lock(&self.control_session)
                .clone()
                .ok_or(RoutingSetupReasonCode::CdpUnavailable)?;
            let endpoint = verified_control_endpoint(&session)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)?;
            runtime
                .block_on(restore_theme_on_pages(&endpoint, &scripts, 2_000))
                .map_err(|_| RoutingSetupReasonCode::CdpUnavailable)
        });
        if receipt.status == OperationStatus::Applied {
            lock(&self.theme_scripts).clear();
        }
        receipt
    }

    pub fn restore_theme_with<F>(&self, restore: F) -> OperationReceipt
    where
        F: FnOnce() -> Result<usize, RoutingSetupReasonCode>,
    {
        let operation_id = Uuid::new_v4().to_string();
        if lock(&self.applied_theme_id).is_none() {
            if lock(&self.selected_theme_id).is_some() {
                if self.theme_preference_store.save(None).is_err() {
                    return OperationReceipt {
                        operation_id,
                        status: OperationStatus::Failed,
                        reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                        restart_required: false,
                    };
                }
                *lock(&self.selected_theme_id) = None;
                *lock(&self.theme_status) = ThemeSessionStatus::Inactive;
                return OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                };
            }
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        match restore() {
            Ok(count) if count != 0 => {
                if self.theme_preference_store.save(None).is_err() {
                    return OperationReceipt {
                        operation_id,
                        status: OperationStatus::Failed,
                        reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                        restart_required: false,
                    };
                }
                *lock(&self.selected_theme_id) = None;
                *lock(&self.applied_theme_id) = None;
                OperationReceipt {
                    operation_id,
                    status: OperationStatus::Applied,
                    reason_codes: Vec::new(),
                    restart_required: false,
                }
            }
            Ok(_) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::UnsupportedHost],
                restart_required: false,
            },
            Err(reason) => OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![reason],
                restart_required: false,
            },
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
                    lock(&self.root_control_bindings).clear();
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
                    lock(&self.root_control_bindings).clear();
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
        _active_native_children: usize,
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
        let now_ms = chrono::Utc::now().timestamp_millis().max(0);
        if self.routing.ensure_root_route(root_id, now_ms).is_err() {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                restart_required: false,
            };
        }
        match self.routing.set_root_enabled(root_id, enabled, now_ms) {
            Ok(changed) => {
                lock(&self.routing_activation).insert(
                    root_id,
                    if enabled {
                        RoutingActivationStatus::PendingOpen
                    } else {
                        RoutingActivationStatus::Off
                    },
                );
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

    pub fn reconcile_persisted_preflight_with(&self, codex_package_version: &str) -> bool {
        let eligibility = self.routing.snapshot().eligibility;
        let required = [
            ("gpt-5.3-codex-spark", RouteKind::Direct, 1),
            ("gpt-5.6-luna", RouteKind::Direct, 1),
            ("gpt-5.6-terra", RouteKind::Direct, 1),
            ("gpt-5.6-sol", RouteKind::Direct, 1),
            ("gpt-5.3-codex-spark", RouteKind::Nested, 2),
            ("gpt-5.6-luna", RouteKind::Nested, 2),
        ];
        let verified = !codex_package_version.is_empty()
            && required.iter().all(|(model, kind, depth)| {
                eligibility.iter().any(|entry| {
                    entry.requested_model == *model
                        && entry.route_kind == *kind
                        && entry.depth == *depth
                        && entry.status == EligibilityStatus::Eligible
                        && entry.profile_version == ROUTING_PROFILE_VERSION
                        && entry.codex_package_version == codex_package_version
                })
            });
        *lock(&self.preflight_status) = if verified {
            RoutingPreflightStatus::Complete
        } else {
            RoutingPreflightStatus::NotStarted
        };
        verified
    }

    pub fn reconcile_persisted_preflight(&self) -> bool {
        persisted_host_version()
            .as_deref()
            .is_some_and(|version| self.reconcile_persisted_preflight_with(version))
    }

    pub fn observe_roots(&self, snapshot: &MonitorSnapshot) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        let existing = self
            .routing
            .snapshot()
            .routes
            .into_iter()
            .map(|route| route.conversation_id)
            .collect::<std::collections::HashSet<_>>();
        let mut changed = false;
        for root_id in snapshot
            .agents
            .iter()
            .filter(|agent| !agent.is_subagent)
            .filter_map(|agent| Uuid::parse_str(&agent.thread_id).ok())
            .filter(|root_id| !root_id.is_nil())
        {
            if existing.contains(&root_id) {
                continue;
            }
            changed = true;
            if self
                .routing
                .ensure_root_route(root_id, snapshot.generated_at_ms.max(0))
                .is_err()
            {
                return OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![RoutingSetupReasonCode::RoutingRuntimeUnavailable],
                    restart_required: false,
                };
            }
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
            ControlEvent::Compatibility {
                route_id,
                compatible,
                reason,
            } => {
                let enabled = self
                    .routing
                    .snapshot()
                    .routes
                    .iter()
                    .find(|route| route.conversation_id == route_id)
                    .map(|route| route.enabled);
                let Some(enabled) = enabled else {
                    return blocked_receipt(
                        Uuid::new_v4().to_string(),
                        RoutingSetupReasonCode::UnsupportedHost,
                        false,
                    );
                };
                let status = if !enabled {
                    RoutingActivationStatus::Off
                } else if compatible
                    && reason == crate::control_layer::injector::CompatibilityReason::Ready
                {
                    RoutingActivationStatus::PendingNextTurn
                } else {
                    RoutingActivationStatus::NeedsRepair
                };
                self.set_routing_activation(route_id, status)
            }
            ControlEvent::SubmitIntent {
                route_id,
                route_key,
                ..
            } => {
                if !self.control_event_matches_route(route_id, route_key) {
                    return blocked_receipt(
                        Uuid::new_v4().to_string(),
                        RoutingSetupReasonCode::UnsupportedHost,
                        false,
                    );
                }
                self.set_routing_activation(route_id, RoutingActivationStatus::PendingNextTurn)
            }
            ControlEvent::InsertionResult {
                route_id,
                route_key,
                result,
                ..
            } => {
                if !self.control_event_matches_route(route_id, route_key) {
                    return blocked_receipt(
                        Uuid::new_v4().to_string(),
                        RoutingSetupReasonCode::UnsupportedHost,
                        false,
                    );
                }
                let status = if result == crate::control_layer::injector::InsertionResult::Inserted
                {
                    RoutingActivationStatus::Enabled
                } else {
                    RoutingActivationStatus::NeedsRepair
                };
                self.set_routing_activation(route_id, status)
            }
        }
    }

    fn control_event_matches_route(&self, route_id: Uuid, route_key: Uuid) -> bool {
        self.routing.snapshot().routes.iter().any(|route| {
            route.conversation_id == route_id && route.route_key == route_key && route.enabled
        })
    }

    fn set_routing_activation(
        &self,
        route_id: Uuid,
        status: RoutingActivationStatus,
    ) -> OperationReceipt {
        let changed = lock(&self.routing_activation).insert(route_id, status) != Some(status);
        OperationReceipt {
            operation_id: Uuid::new_v4().to_string(),
            status: if changed {
                OperationStatus::Applied
            } else {
                OperationStatus::Noop
            },
            reason_codes: Vec::new(),
            restart_required: false,
        }
    }

    pub fn reconcile_root_controls(&self) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        if *lock(&self.preflight_status) != RoutingPreflightStatus::Complete {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        let routes = self.routing.snapshot().routes;
        if routes.is_empty() {
            lock(&self.root_control_bindings).clear();
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        }
        if lock(&self.control_session).is_none() {
            if let Some(record) = recover_verified_host_session(&self.session_store) {
                *lock(&self.control_session) = Some(record);
                *lock(&self.cdp_status) = RoutingCdpStatus::Ready;
            }
        }
        let Some(session) = lock(&self.control_session).clone() else {
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Noop,
                reason_codes: Vec::new(),
                restart_required: false,
            };
        };
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            lock(&self.root_control_bindings).clear();
            return OperationReceipt {
                operation_id,
                status: OperationStatus::Failed,
                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                restart_required: false,
            };
        };
        let requests = routes
            .iter()
            .map(|route| VisibleRootControlRequest {
                root_conversation_id: route.conversation_id,
                route_key: route.route_key,
                enabled: route.enabled,
            })
            .collect::<Vec<_>>();
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return self.unavailable_operation();
        };
        const CONTROL_SCRIPT: &str = include_str!("../resources/control/routing-control.js");
        const CONTROL_CSS: &str = include_str!("../resources/control/routing-control.css");
        let bindings = match runtime.block_on(bind_routing_controls_on_pages_detailed(
            &endpoint,
            CONTROL_SCRIPT,
            CONTROL_CSS,
            &requests,
            1_000,
        )) {
            Ok(bindings) => bindings,
            Err(_) => {
                return OperationReceipt {
                    operation_id,
                    status: OperationStatus::Failed,
                    reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                    restart_required: false,
                };
            }
        };
        let previous = lock(&self.root_control_bindings).clone();
        let mut verified = HashMap::new();
        let mut any_failed = false;
        for binding in bindings {
            let enabled = routes
                .iter()
                .find(|route| route.conversation_id == binding.root_conversation_id)
                .is_some_and(|route| route.enabled);
            let ready = runtime.block_on(set_control_routing_ready(
                &endpoint,
                &binding.target_id,
                true,
                1_000,
            )) == Ok(true);
            let synced = ready
                && runtime.block_on(sync_control_routing_enabled(
                    &endpoint,
                    &binding.target_id,
                    enabled,
                    1_000,
                )) == Ok(true);
            if !synced {
                any_failed = true;
                if enabled {
                    lock(&self.routing_activation).insert(
                        binding.root_conversation_id,
                        RoutingActivationStatus::NeedsRepair,
                    );
                }
                continue;
            }
            let same_binding = previous
                .get(&binding.root_conversation_id)
                .is_some_and(|old| old == &binding);
            let current_status = lock(&self.routing_activation)
                .get(&binding.root_conversation_id)
                .copied();
            lock(&self.routing_activation).insert(
                binding.root_conversation_id,
                if !enabled {
                    RoutingActivationStatus::Off
                } else if same_binding && current_status == Some(RoutingActivationStatus::Enabled) {
                    RoutingActivationStatus::Enabled
                } else {
                    RoutingActivationStatus::PendingNextTurn
                },
            );
            verified.insert(binding.root_conversation_id, binding);
        }
        for route in &routes {
            if route.enabled && !verified.contains_key(&route.conversation_id) {
                lock(&self.routing_activation)
                    .insert(route.conversation_id, RoutingActivationStatus::PendingOpen);
            }
        }
        *lock(&self.root_control_bindings) = verified;
        OperationReceipt {
            operation_id,
            status: if any_failed {
                OperationStatus::Failed
            } else {
                OperationStatus::Applied
            },
            reason_codes: if any_failed {
                vec![RoutingSetupReasonCode::CdpUnavailable]
            } else {
                Vec::new()
            },
            restart_required: false,
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
        let root_bindings = lock(&self.root_control_bindings)
            .values()
            .cloned()
            .collect::<Vec<_>>();
        if !root_bindings.is_empty() {
            return self.poll_root_control_events(root_bindings, active_native_children);
        }
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

    fn poll_root_control_events(
        &self,
        bindings: Vec<VisibleControlBinding>,
        active_native_children: usize,
    ) -> OperationReceipt {
        let operation_id = Uuid::new_v4().to_string();
        let Some(session) = lock(&self.control_session).clone() else {
            return self.unavailable_operation();
        };
        let Ok(endpoint) = verified_control_endpoint(&session) else {
            return self.unavailable_operation();
        };
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return self.unavailable_operation();
        };
        for binding in bindings {
            match runtime.block_on(receive_control_event(
                &endpoint,
                &binding.target_id,
                &binding.session_id,
                100,
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
                            1_000,
                        )) != Ok(true)
                        {
                            lock(&self.routing_activation).insert(
                                binding.root_conversation_id,
                                RoutingActivationStatus::NeedsRepair,
                            );
                            return OperationReceipt {
                                operation_id,
                                status: OperationStatus::Failed,
                                reason_codes: vec![RoutingSetupReasonCode::CdpUnavailable],
                                restart_required: false,
                            };
                        }
                    }
                    return receipt;
                }
                Ok(_) => {
                    lock(&self.routing_activation).insert(
                        binding.root_conversation_id,
                        RoutingActivationStatus::NeedsRepair,
                    );
                }
                Err(ControlReceiveError::Cdp(CdpClientError::TimedOut)) => {}
                Err(ControlReceiveError::TargetUnavailable)
                | Err(ControlReceiveError::Discovery(_)) => {
                    lock(&self.routing_activation).insert(
                        binding.root_conversation_id,
                        RoutingActivationStatus::PendingOpen,
                    );
                }
                Err(_) => {
                    lock(&self.routing_activation).insert(
                        binding.root_conversation_id,
                        RoutingActivationStatus::NeedsRepair,
                    );
                }
            }
        }
        OperationReceipt {
            operation_id,
            status: OperationStatus::Noop,
            reason_codes: Vec::new(),
            restart_required: false,
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
        let Some(_lease) = self.try_lifecycle() else {
            return blocked_receipt(
                Uuid::new_v4().to_string(),
                RoutingSetupReasonCode::OperationConflict,
                *lock(&self.restart_required),
            );
        };
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
fn recover_verified_host_session(store: &OwnedSessionStore) -> Option<OwnedSessionRecord> {
    let package = discover_store_package().ok()?;
    let current_user = query_process_identity(std::process::id()).ok()?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid).ok()?;
    let [current_process] = processes.as_slice() else {
        return None;
    };
    let record = store
        .load(
            current_process.pid,
            &package.version,
            chrono::Utc::now().timestamp_millis().max(0),
        )
        .ok()??;
    verified_control_endpoint(&record).ok()?;
    Some(record)
}

#[cfg(not(windows))]
fn recover_verified_host_session(_store: &OwnedSessionStore) -> Option<OwnedSessionRecord> {
    None
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
fn current_verified_root_fingerprint() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode> {
    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let [current_process] = processes.as_slice() else {
        return Err(RoutingSetupReasonCode::IdentityChanged);
    };
    root_fingerprint(current_process).map_err(restart_reason)
}

#[cfg(not(windows))]
fn current_verified_root_fingerprint() -> Result<VerifiedRootFingerprint, RoutingSetupReasonCode> {
    Err(RoutingSetupReasonCode::UnsupportedHost)
}

#[cfg(windows)]
fn restart_verified_host_force(
    expected_root: &VerifiedRootFingerprint,
    cancellation: &AtomicBool,
) -> Result<OwnedSessionRecord, RoutingSetupReasonCode> {
    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let [current_process] = processes.as_slice() else {
        return Err(RoutingSetupReasonCode::IdentityChanged);
    };
    let reservation = reserve_loopback_port().map_err(restart_reason)?;
    let restarted = restart_verified_codex_force(
        RestartGuard {
            verified_ui_processes: 1,
            active_native_children: 1,
            setup_phase: SetupPhase::Committed,
        },
        &package,
        current_process,
        expected_root,
        cancellation,
        reservation,
        15_000,
    )
    .map_err(restart_reason)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RoutingSetupReasonCode::CdpVerificationFailed)?;
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
                .map_err(|_| RoutingSetupReasonCode::CdpVerificationFailed);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(RoutingSetupReasonCode::CdpVerificationFailed);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(not(windows))]
fn restart_verified_host_force(
    _expected_root: &VerifiedRootFingerprint,
    _cancellation: &AtomicBool,
) -> Result<OwnedSessionRecord, RoutingSetupReasonCode> {
    Err(RoutingSetupReasonCode::UnsupportedHost)
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
        IdentityError::ProcessIdentityChanged => RoutingSetupReasonCode::IdentityChanged,
        IdentityError::ProcessTreeIncomplete => RoutingSetupReasonCode::ImpactChanged,
        IdentityError::TerminationFailed => RoutingSetupReasonCode::TerminationFailed,
        IdentityError::TreeStillRunning => RoutingSetupReasonCode::OldTreeStillRunning,
        IdentityError::OperationCancelled => RoutingSetupReasonCode::ConfirmationRequired,
        IdentityError::LaunchFailed => RoutingSetupReasonCode::TerminalPartialFailure,
        _ => RoutingSetupReasonCode::CdpUnavailable,
    }
}

#[cfg(windows)]
fn persisted_host_version() -> Option<String> {
    discover_store_package().ok().map(|package| package.version)
}

#[cfg(not(windows))]
fn persisted_host_version() -> Option<String> {
    None
}

fn theme_engine_reason(error: ThemeEngineError) -> RoutingSetupReasonCode {
    match error {
        ThemeEngineError::DomIncompatible => RoutingSetupReasonCode::DomIncompatible,
        ThemeEngineError::PartialApplication => RoutingSetupReasonCode::PartialApplyFailed,
        ThemeEngineError::InvalidPack(_) => RoutingSetupReasonCode::UnsupportedHost,
        ThemeEngineError::Discovery(_) | ThemeEngineError::Cdp(_) => {
            RoutingSetupReasonCode::CdpUnavailable
        }
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

fn failed_receipt(
    operation_id: String,
    reason: RoutingSetupReasonCode,
    restart_required: bool,
) -> OperationReceipt {
    OperationReceipt {
        operation_id,
        status: OperationStatus::Failed,
        reason_codes: vec![reason],
        restart_required,
    }
}
