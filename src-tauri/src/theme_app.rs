use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    control_layer::{
        cdp::{
            create_owned_session_record, fetch_browser_endpoint, BrowserAnchor, BrowserEndpoint,
            OwnedSessionRecord, OwnedSessionStore,
        },
        windows_package::{
            discover_store_package, discover_verified_ui_processes, launch_verified_codex,
            query_process_identity, query_tcp_listener, reserve_loopback_port,
            restart_verified_codex, restart_verified_codex_force, root_fingerprint,
            verify_listener, IdentityError, RestartGuard, SetupPhase, VerifiedRootFingerprint,
        },
    },
    local_theme::LocalThemeCatalog,
    monitor::RestartSafetyProjection,
    theme::{
        apply_theme_on_pages_with_asset_for_version, bundled_theme_packs, restore_theme_on_pages,
        ThemeCategory, ThemeEngineError, ThemePack, ThemePreferenceStore, ThemeScriptRegistration,
    },
    theme_environment::{inspect_local_environment, ThemeEnvironmentReport},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeSessionStatus {
    Inactive,
    Paused,
    Ready,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeSessionAction {
    Reuse,
    Launch,
    Restart,
}

pub fn decide_session_action(
    verified_process_count: usize,
    session_reachable: bool,
) -> Result<ThemeSessionAction, ThemeReasonCode> {
    match (verified_process_count, session_reachable) {
        (1, true) => Ok(ThemeSessionAction::Reuse),
        (0, false) => Ok(ThemeSessionAction::Launch),
        (1, false) => Ok(ThemeSessionAction::Restart),
        _ => Err(ThemeReasonCode::UnsupportedHost),
    }
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
    ThemeSession,
    ActivateTheme,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeReasonCode {
    ActiveWork,
    MonitorUncertain,
    UnsupportedHost,
    CdpUnavailable,
    ThemeStateUnavailable,
    ConfirmationRequired,
    ConfirmationExpired,
    ImpactChanged,
    OperationConflict,
    IdentityChanged,
    TerminationFailed,
    OldTreeStillRunning,
    CdpVerificationFailed,
    DomIncompatible,
    MultipleWindows,
    PartialApplyFailed,
    TerminalPartialFailure,
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
    pub reason_codes: Vec<ThemeReasonCode>,
    pub restart_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeUiSnapshot {
    pub contract_version: u32,
    pub session_status: ThemeSessionStatus,
    pub selected_theme_id: Option<String>,
    pub applied_theme_id: Option<String>,
    pub catalog_notice: Option<String>,
    pub packs: Vec<ThemePack>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeImportReceipt {
    pub theme_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ForceRestartImpact {
    pub confirmation_ticket: String,
    pub intent: RestartIntent,
    pub active_work_count: usize,
    pub monitor_confident: bool,
    pub grace_period_ms: u32,
    pub expires_at_ms: i64,
}

#[derive(Clone, Debug)]
struct ForceRestartTicket {
    intent: RestartIntent,
    subject: Option<String>,
    restart_safety: RestartSafetyProjection,
    expires_at_ms: i64,
    fingerprint: VerifiedRootFingerprint,
    cancellation: Arc<AtomicBool>,
}

#[derive(Clone)]
struct ThemeWorkflowState {
    session: Option<OwnedSessionRecord>,
    status: ThemeSessionStatus,
    selected_theme_id: Option<String>,
    applied_theme_id: Option<String>,
    scripts: Vec<ThemeScriptRegistration>,
    force_tickets: HashMap<String, ForceRestartTicket>,
    force_cancellations: HashMap<String, Arc<AtomicBool>>,
    lifecycle_active: bool,
}

pub struct ThemeApplication {
    session_store: OwnedSessionStore,
    workflow: Mutex<ThemeWorkflowState>,
    local_catalog: LocalThemeCatalog,
    preference_store: ThemePreferenceStore,
    catalog_notice: Option<String>,
}

struct LifecycleLease<'a>(&'a Mutex<ThemeWorkflowState>);

impl Drop for LifecycleLease<'_> {
    fn drop(&mut self) {
        lock(self.0).lifecycle_active = false;
    }
}

impl ThemeApplication {
    pub fn default_location() -> Result<Self, String> {
        let state_directory = crate::theme_state::prepare_default_theme_state()?;
        Self::for_state_directory(state_directory)
    }

    pub fn for_state_directory(state_directory: PathBuf) -> Result<Self, String> {
        let session_store = OwnedSessionStore::in_directory(&state_directory)
            .map_err(|_| "Theme session state is unavailable".to_owned())?;
        let preference_store = ThemePreferenceStore::in_directory(&state_directory)?;
        let local_catalog = LocalThemeCatalog::in_directory(&state_directory)?;
        let mut available = bundled_theme_packs();
        let bundled_ids = available
            .iter()
            .map(|pack| pack.id.clone())
            .collect::<std::collections::HashSet<_>>();
        available.extend(
            local_catalog
                .packs()
                .into_iter()
                .filter(|pack| !bundled_ids.contains(&pack.id)),
        );
        let preference = preference_store.load(&available)?;
        let status = if preference.selected_theme_id.is_some() {
            ThemeSessionStatus::Paused
        } else {
            ThemeSessionStatus::Inactive
        };
        Ok(Self {
            session_store,
            workflow: Mutex::new(ThemeWorkflowState {
                session: None,
                status,
                selected_theme_id: preference.selected_theme_id,
                applied_theme_id: None,
                scripts: Vec::new(),
                force_tickets: HashMap::new(),
                force_cancellations: HashMap::new(),
                lifecycle_active: false,
            }),
            local_catalog,
            preference_store,
            catalog_notice: preference
                .removed_missing_bundled_theme
                .then(|| "原主题已下架，请从 12 个新主题中重新选择".to_owned()),
        })
    }

    fn try_lifecycle(&self) -> Option<LifecycleLease<'_>> {
        let mut workflow = lock(&self.workflow);
        if workflow.lifecycle_active {
            return None;
        }
        workflow.lifecycle_active = true;
        drop(workflow);
        Some(LifecycleLease(&self.workflow))
    }

    fn accept_session(&self, record: OwnedSessionRecord) {
        let mut workflow = lock(&self.workflow);
        workflow.session = Some(record);
        workflow.status = ThemeSessionStatus::Ready;
    }

    pub fn snapshot(&self) -> ThemeUiSnapshot {
        let workflow = lock(&self.workflow);
        ThemeUiSnapshot {
            contract_version: 2,
            session_status: workflow.status,
            selected_theme_id: workflow.selected_theme_id.clone(),
            applied_theme_id: workflow.applied_theme_id.clone(),
            catalog_notice: self.catalog_notice.clone(),
            packs: self.theme_packs(),
        }
    }

    pub fn environment_report(&self) -> ThemeEnvironmentReport {
        let reachable = self.reconcile_session();
        inspect_local_environment(lock(&self.workflow).selected_theme_id.clone(), reachable)
    }

    pub fn preview_data_url(&self, theme_id: &str) -> Option<String> {
        self.local_catalog.preview_data_url(theme_id)
    }

    pub fn import_local_theme(
        &self,
        name: &str,
        image_data_url: &str,
    ) -> Result<ThemeImportReceipt, String> {
        if image_data_url.len() > 2_100_000 {
            return Err("Local theme image is too large".to_owned());
        }
        let (header, encoded) = image_data_url
            .split_once(',')
            .ok_or_else(|| "Local theme image is invalid".to_owned())?;
        let mime_type = match header {
            "data:image/jpeg;base64" => "image/jpeg",
            "data:image/png;base64" => "image/png",
            "data:image/webp;base64" => "image/webp",
            _ => return Err("Local theme image is invalid".to_owned()),
        };
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "Local theme image is invalid".to_owned())?;
        let pack = self.local_catalog.import_image(name, mime_type, &bytes)?;
        Ok(ThemeImportReceipt { theme_id: pack.id })
    }

    fn theme_packs(&self) -> Vec<ThemePack> {
        let mut packs = bundled_theme_packs();
        let bundled_ids = packs
            .iter()
            .map(|pack| pack.id.clone())
            .collect::<std::collections::HashSet<_>>();
        packs.extend(
            self.local_catalog
                .packs()
                .into_iter()
                .filter(|pack| !bundled_ids.contains(&pack.id)),
        );
        packs
    }

    pub fn reconcile_session(&self) -> bool {
        if lock(&self.workflow).session.is_none() {
            if let Some(record) = recover_verified_session(&self.session_store) {
                self.accept_session(record);
            }
        }
        self.reconcile_session_with(|record| verified_theme_endpoint(record).is_ok())
    }

    pub fn reconcile_session_with<F>(&self, verify: F) -> bool
    where
        F: FnOnce(&OwnedSessionRecord) -> bool,
    {
        let session = {
            let workflow = lock(&self.workflow);
            if workflow.lifecycle_active {
                return workflow.session.is_some();
            }
            workflow.session.clone()
        };
        if session.as_ref().is_some_and(verify) {
            return true;
        }
        let mut workflow = lock(&self.workflow);
        workflow.session = None;
        workflow.status = if workflow.selected_theme_id.is_some() {
            ThemeSessionStatus::Paused
        } else {
            ThemeSessionStatus::Inactive
        };
        workflow.applied_theme_id = None;
        workflow.scripts.clear();
        false
    }

    pub fn start_session(&self, active_work_count: usize) -> OperationReceipt {
        self.start_session_safety(RestartSafetyProjection::confirmed(active_work_count))
    }

    pub fn start_session_safety(
        &self,
        restart_safety: RestartSafetyProjection,
    ) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        if let Some(record) = recover_verified_session(&self.session_store) {
            self.accept_session(record);
            return receipt(OperationStatus::Noop, Vec::new());
        }
        {
            let mut workflow = lock(&self.workflow);
            workflow.session = None;
            workflow.status = ThemeSessionStatus::Inactive;
        }
        self.start_session_with_safety_inner(restart_safety, restart_verified_host)
    }

    pub fn start_session_with<F>(&self, active_work_count: usize, restart: F) -> OperationReceipt
    where
        F: FnOnce() -> Result<OwnedSessionRecord, ThemeReasonCode>,
    {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        self.start_session_with_safety_inner(
            RestartSafetyProjection::confirmed(active_work_count),
            restart,
        )
    }

    pub fn start_session_with_safety<F>(
        &self,
        restart_safety: RestartSafetyProjection,
        restart: F,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<OwnedSessionRecord, ThemeReasonCode>,
    {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        self.start_session_with_safety_inner(restart_safety, restart)
    }

    fn start_session_with_safety_inner<F>(
        &self,
        restart_safety: RestartSafetyProjection,
        restart: F,
    ) -> OperationReceipt
    where
        F: FnOnce() -> Result<OwnedSessionRecord, ThemeReasonCode>,
    {
        {
            let mut workflow = lock(&self.workflow);
            if workflow.session.is_some() {
                workflow.status = ThemeSessionStatus::Ready;
                return receipt(OperationStatus::Noop, Vec::new());
            }
        }
        if let Some(reason) = restart_block_reason(restart_safety) {
            return blocked(reason);
        }
        match restart() {
            Ok(record) if self.session_store.save(&record).is_ok() => {
                self.accept_session(record);
                receipt(OperationStatus::Applied, Vec::new())
            }
            Ok(_) => {
                lock(&self.workflow).status = ThemeSessionStatus::Degraded;
                failed(ThemeReasonCode::ThemeStateUnavailable)
            }
            Err(reason) => {
                lock(&self.workflow).status = ThemeSessionStatus::Degraded;
                failed(reason)
            }
        }
    }

    pub fn start_session_mode(
        &self,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        active_work_count: usize,
    ) -> OperationReceipt {
        self.start_session_mode_with_safety(
            mode,
            confirmation_ticket,
            RestartSafetyProjection::confirmed(active_work_count),
        )
    }

    pub fn start_session_mode_with_safety(
        &self,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        restart_safety: RestartSafetyProjection,
    ) -> OperationReceipt {
        if mode == RestartMode::Safe {
            return self.start_session_safety(restart_safety);
        }
        let Some(ticket) = confirmation_ticket else {
            return blocked(ThemeReasonCode::ConfirmationRequired);
        };
        self.execute_force_restart(ticket, RestartIntent::ThemeSession, None, restart_safety)
    }

    pub fn activate_mode(
        &self,
        theme_id: &str,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        active_work_count: usize,
    ) -> OperationReceipt {
        self.activate_mode_with_safety(
            theme_id,
            mode,
            confirmation_ticket,
            RestartSafetyProjection::confirmed(active_work_count),
        )
    }

    pub fn activate_mode_with_safety(
        &self,
        theme_id: &str,
        mode: RestartMode,
        confirmation_ticket: Option<&str>,
        restart_safety: RestartSafetyProjection,
    ) -> OperationReceipt {
        lock(&self.workflow).selected_theme_id = Some(theme_id.to_owned());
        if mode == RestartMode::Safe {
            return self.activate_safety(theme_id, restart_safety);
        }
        let Some(ticket) = confirmation_ticket else {
            return blocked(ThemeReasonCode::ConfirmationRequired);
        };
        let restarted = self.execute_force_restart(
            ticket,
            RestartIntent::ActivateTheme,
            Some(theme_id),
            restart_safety,
        );
        if restarted.status != OperationStatus::Applied {
            return restarted;
        }
        self.apply_until_ready(theme_id)
    }

    pub fn activate(&self, theme_id: &str, active_work_count: usize) -> OperationReceipt {
        self.activate_safety(
            theme_id,
            RestartSafetyProjection::confirmed(active_work_count),
        )
    }

    pub fn activate_safety(
        &self,
        theme_id: &str,
        restart_safety: RestartSafetyProjection,
    ) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        lock(&self.workflow).selected_theme_id = Some(theme_id.to_owned());
        if lock(&self.workflow).status != ThemeSessionStatus::Ready {
            if let Some(record) = recover_verified_session(&self.session_store) {
                self.accept_session(record);
            }
            if lock(&self.workflow).status != ThemeSessionStatus::Ready {
                let started =
                    self.start_session_with_safety_inner(restart_safety, restart_verified_host);
                if !matches!(
                    started.status,
                    OperationStatus::Applied | OperationStatus::Noop
                ) {
                    return started;
                }
            }
        }
        self.apply_until_ready(theme_id)
    }

    pub fn activate_with<R, A>(
        &self,
        theme_id: &str,
        active_work_count: usize,
        restart: R,
        apply: A,
    ) -> OperationReceipt
    where
        R: FnOnce() -> Result<OwnedSessionRecord, ThemeReasonCode>,
        A: FnOnce(&ThemePack) -> Result<usize, ThemeReasonCode>,
    {
        self.activate_with_safety(
            theme_id,
            RestartSafetyProjection::confirmed(active_work_count),
            restart,
            apply,
        )
    }

    pub fn activate_with_safety<R, A>(
        &self,
        theme_id: &str,
        restart_safety: RestartSafetyProjection,
        restart: R,
        apply: A,
    ) -> OperationReceipt
    where
        R: FnOnce() -> Result<OwnedSessionRecord, ThemeReasonCode>,
        A: FnOnce(&ThemePack) -> Result<usize, ThemeReasonCode>,
    {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        lock(&self.workflow).selected_theme_id = Some(theme_id.to_owned());
        if lock(&self.workflow).status != ThemeSessionStatus::Ready {
            let started = self.start_session_with_safety_inner(restart_safety, restart);
            if !matches!(
                started.status,
                OperationStatus::Applied | OperationStatus::Noop
            ) {
                return started;
            }
        }
        self.apply_theme_with_inner(theme_id, apply)
    }

    fn apply_until_ready(&self, theme_id: &str) -> OperationReceipt {
        self.retry_theme_application_with(
            5,
            || self.apply_theme(theme_id),
            || std::thread::sleep(std::time::Duration::from_millis(250)),
        )
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
            let result = attempt();
            let transient_renderer_state = result.status == OperationStatus::Failed
                && matches!(
                    result.reason_codes.as_slice(),
                    [ThemeReasonCode::DomIncompatible] | [ThemeReasonCode::PartialApplyFailed]
                );
            if !transient_renderer_state || index + 1 == attempts {
                return result;
            }
            wait();
        }
        unreachable!("at least one theme application attempt is required")
    }

    fn apply_theme(&self, theme_id: &str) -> OperationReceipt {
        let previous_scripts = lock(&self.workflow).scripts.clone();
        let next_scripts = Mutex::new(None);
        let result = self.apply_theme_with_inner(theme_id, |pack| {
            let session = lock(&self.workflow)
                .session
                .clone()
                .ok_or(ThemeReasonCode::CdpUnavailable)?;
            let endpoint = verified_theme_endpoint(&session)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ThemeReasonCode::CdpUnavailable)?;
            let local_asset = (pack.category == ThemeCategory::LocalImport)
                .then(|| self.local_catalog.asset_bytes(&pack.id))
                .flatten();
            let applied = runtime
                .block_on(apply_theme_on_pages_with_asset_for_version(
                    &endpoint,
                    &session.codex_version,
                    pack,
                    local_asset.as_deref(),
                    &previous_scripts,
                    10_000,
                ))
                .map_err(theme_engine_reason)?;
            *lock(&next_scripts) = Some(applied.scripts);
            Ok(applied.applied_pages)
        });
        if result.status == OperationStatus::Applied {
            if let Some(scripts) = lock(&next_scripts).take() {
                lock(&self.workflow).scripts = scripts;
            }
        } else if result
            .reason_codes
            .contains(&ThemeReasonCode::CdpUnavailable)
        {
            let mut workflow = lock(&self.workflow);
            workflow.session = None;
            workflow.status = ThemeSessionStatus::Degraded;
        }
        result
    }

    pub fn apply_theme_with<F>(&self, theme_id: &str, apply: F) -> OperationReceipt
    where
        F: FnOnce(&ThemePack) -> Result<usize, ThemeReasonCode>,
    {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        self.apply_theme_with_inner(theme_id, apply)
    }

    fn apply_theme_with_inner<F>(&self, theme_id: &str, apply: F) -> OperationReceipt
    where
        F: FnOnce(&ThemePack) -> Result<usize, ThemeReasonCode>,
    {
        lock(&self.workflow).selected_theme_id = Some(theme_id.to_owned());
        let available = self.theme_packs();
        if self
            .preference_store
            .save(Some(theme_id), &available)
            .is_err()
        {
            return failed(ThemeReasonCode::ThemeStateUnavailable);
        }
        if lock(&self.workflow).status != ThemeSessionStatus::Ready {
            return blocked(ThemeReasonCode::CdpUnavailable);
        }
        let Some(pack) = available.into_iter().find(|pack| pack.id == theme_id) else {
            return blocked(ThemeReasonCode::UnsupportedHost);
        };
        match apply(&pack) {
            Ok(count) if count != 0 => {
                lock(&self.workflow).applied_theme_id = Some(pack.id);
                receipt(OperationStatus::Applied, Vec::new())
            }
            Ok(_) => failed(ThemeReasonCode::UnsupportedHost),
            Err(reason) => failed(reason),
        }
    }

    pub fn restore(&self) -> OperationReceipt {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        let scripts = lock(&self.workflow).scripts.clone();
        let result = self.restore_with_inner(|| {
            let session = lock(&self.workflow)
                .session
                .clone()
                .ok_or(ThemeReasonCode::CdpUnavailable)?;
            let endpoint = verified_theme_endpoint(&session)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ThemeReasonCode::CdpUnavailable)?;
            runtime
                .block_on(restore_theme_on_pages(&endpoint, &scripts, 2_000))
                .map_err(|_| ThemeReasonCode::CdpUnavailable)
        });
        if result.status == OperationStatus::Applied {
            lock(&self.workflow).scripts.clear();
        }
        result
    }

    pub fn restore_with<F>(&self, restore: F) -> OperationReceipt
    where
        F: FnOnce() -> Result<usize, ThemeReasonCode>,
    {
        let Some(_lease) = self.try_lifecycle() else {
            return blocked(ThemeReasonCode::OperationConflict);
        };
        self.restore_with_inner(restore)
    }

    fn restore_with_inner<F>(&self, restore: F) -> OperationReceipt
    where
        F: FnOnce() -> Result<usize, ThemeReasonCode>,
    {
        let (applied, selected) = {
            let workflow = lock(&self.workflow);
            (
                workflow.applied_theme_id.is_some(),
                workflow.selected_theme_id.is_some(),
            )
        };
        if !applied {
            if selected {
                if self.preference_store.save(None, &[]).is_err() {
                    return failed(ThemeReasonCode::ThemeStateUnavailable);
                }
                let mut workflow = lock(&self.workflow);
                workflow.selected_theme_id = None;
                workflow.status = ThemeSessionStatus::Inactive;
                return receipt(OperationStatus::Applied, Vec::new());
            }
            return receipt(OperationStatus::Noop, Vec::new());
        }
        match restore() {
            Ok(count) if count != 0 => {
                if self.preference_store.save(None, &[]).is_err() {
                    return failed(ThemeReasonCode::ThemeStateUnavailable);
                }
                let mut workflow = lock(&self.workflow);
                workflow.selected_theme_id = None;
                workflow.applied_theme_id = None;
                receipt(OperationStatus::Applied, Vec::new())
            }
            Ok(_) => failed(ThemeReasonCode::UnsupportedHost),
            Err(reason) => failed(reason),
        }
    }

    pub fn prepare_force_restart(
        &self,
        intent: RestartIntent,
        subject: Option<String>,
        active_work_count: usize,
    ) -> Result<ForceRestartImpact, ThemeReasonCode> {
        self.prepare_force_restart_with_safety(
            intent,
            subject,
            RestartSafetyProjection::confirmed(active_work_count),
        )
    }

    pub fn prepare_force_restart_with_safety(
        &self,
        intent: RestartIntent,
        subject: Option<String>,
        restart_safety: RestartSafetyProjection,
    ) -> Result<ForceRestartImpact, ThemeReasonCode> {
        if restart_safety.blocking_reason().is_none() {
            return Err(ThemeReasonCode::ConfirmationRequired);
        }
        if lock(&self.workflow).lifecycle_active {
            return Err(ThemeReasonCode::OperationConflict);
        }
        let fingerprint = current_verified_root_fingerprint()?;
        let confirmation_ticket = Uuid::new_v4().to_string();
        let expires_at_ms = chrono::Utc::now().timestamp_millis().max(0) + 60_000;
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut workflow = lock(&self.workflow);
        workflow.force_tickets.insert(
            confirmation_ticket.clone(),
            ForceRestartTicket {
                intent,
                subject,
                restart_safety,
                expires_at_ms,
                fingerprint,
                cancellation: Arc::clone(&cancellation),
            },
        );
        workflow
            .force_cancellations
            .insert(confirmation_ticket.clone(), cancellation);
        Ok(ForceRestartImpact {
            confirmation_ticket,
            intent,
            active_work_count: restart_safety.active_work_count,
            monitor_confident: restart_safety.monitor_confident,
            grace_period_ms: 5_000,
            expires_at_ms,
        })
    }

    pub fn cancel_force_restart(&self, confirmation_ticket: &str) -> bool {
        let cancellation = lock(&self.workflow)
            .force_cancellations
            .get(confirmation_ticket)
            .cloned();
        if let Some(cancellation) = cancellation {
            cancellation.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn execute_force_restart(
        &self,
        confirmation_ticket: &str,
        intent: RestartIntent,
        subject: Option<&str>,
        restart_safety: RestartSafetyProjection,
    ) -> OperationReceipt {
        let ticket = lock(&self.workflow)
            .force_tickets
            .remove(confirmation_ticket);
        let Some(ticket) = ticket else {
            return blocked(ThemeReasonCode::ConfirmationExpired);
        };
        let invalid = chrono::Utc::now().timestamp_millis().max(0) > ticket.expires_at_ms
            || ticket.intent != intent
            || ticket.subject.as_deref() != subject;
        if invalid {
            lock(&self.workflow)
                .force_cancellations
                .remove(confirmation_ticket);
            return blocked(ThemeReasonCode::ConfirmationExpired);
        }
        if ticket.restart_safety != restart_safety {
            lock(&self.workflow)
                .force_cancellations
                .remove(confirmation_ticket);
            return blocked(ThemeReasonCode::ImpactChanged);
        }
        let current = match current_verified_root_fingerprint() {
            Ok(value) => value,
            Err(reason) => {
                lock(&self.workflow)
                    .force_cancellations
                    .remove(confirmation_ticket);
                return blocked(reason);
            }
        };
        if current != ticket.fingerprint {
            lock(&self.workflow)
                .force_cancellations
                .remove(confirmation_ticket);
            return blocked(ThemeReasonCode::IdentityChanged);
        }
        {
            let mut workflow = lock(&self.workflow);
            if workflow.lifecycle_active {
                workflow.force_cancellations.remove(confirmation_ticket);
                return blocked(ThemeReasonCode::OperationConflict);
            }
            workflow.lifecycle_active = true;
        }
        let restarted = restart_verified_host_force(&ticket.fingerprint, &ticket.cancellation);
        {
            let mut workflow = lock(&self.workflow);
            workflow.lifecycle_active = false;
            workflow.force_cancellations.remove(confirmation_ticket);
        }
        match restarted {
            Ok(record) if self.session_store.save(&record).is_ok() => {
                self.accept_session(record);
                receipt(OperationStatus::Applied, Vec::new())
            }
            Ok(_) => failed(ThemeReasonCode::TerminalPartialFailure),
            Err(reason) => failed(reason),
        }
    }

    pub fn unavailable_operation(&self) -> OperationReceipt {
        failed(ThemeReasonCode::ThemeStateUnavailable)
    }
}

fn receipt(status: OperationStatus, reason_codes: Vec<ThemeReasonCode>) -> OperationReceipt {
    OperationReceipt {
        operation_id: Uuid::new_v4().to_string(),
        status,
        reason_codes,
        restart_required: false,
    }
}

fn blocked(reason: ThemeReasonCode) -> OperationReceipt {
    receipt(OperationStatus::Blocked, vec![reason])
}

fn failed(reason: ThemeReasonCode) -> OperationReceipt {
    receipt(OperationStatus::Failed, vec![reason])
}

fn restart_block_reason(restart_safety: RestartSafetyProjection) -> Option<ThemeReasonCode> {
    if restart_safety.active_work_count != 0 {
        Some(ThemeReasonCode::ActiveWork)
    } else if !restart_safety.monitor_confident {
        Some(ThemeReasonCode::MonitorUncertain)
    } else {
        None
    }
}

fn verified_theme_endpoint(
    session: &OwnedSessionRecord,
) -> Result<BrowserEndpoint, ThemeReasonCode> {
    let listener = query_tcp_listener(session.port).map_err(restart_reason)?;
    if listener.address != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        || listener.port != session.port
        || listener.pid != session.verified_pid
    {
        return Err(ThemeReasonCode::CdpUnavailable);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ThemeReasonCode::CdpUnavailable)?;
    let endpoint = runtime
        .block_on(fetch_browser_endpoint(session.port, 1_000))
        .map_err(|_| ThemeReasonCode::CdpUnavailable)?;
    if BrowserAnchor::new(&endpoint).hash() != session.browser_id_hash {
        return Err(ThemeReasonCode::CdpUnavailable);
    }
    Ok(endpoint)
}

#[cfg(windows)]
fn recover_verified_session(store: &OwnedSessionStore) -> Option<OwnedSessionRecord> {
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
    verified_theme_endpoint(&record).ok()?;
    Some(record)
}

#[cfg(not(windows))]
fn recover_verified_session(_store: &OwnedSessionStore) -> Option<OwnedSessionRecord> {
    None
}

#[cfg(windows)]
fn restart_verified_host() -> Result<OwnedSessionRecord, ThemeReasonCode> {
    restart_verified_host_impl(None)
}

#[cfg(not(windows))]
fn restart_verified_host() -> Result<OwnedSessionRecord, ThemeReasonCode> {
    Err(ThemeReasonCode::UnsupportedHost)
}

#[cfg(windows)]
fn restart_verified_host_force(
    expected_root: &VerifiedRootFingerprint,
    cancellation: &AtomicBool,
) -> Result<OwnedSessionRecord, ThemeReasonCode> {
    restart_verified_host_impl(Some((expected_root, cancellation)))
}

#[cfg(not(windows))]
fn restart_verified_host_force(
    _expected_root: &VerifiedRootFingerprint,
    _cancellation: &AtomicBool,
) -> Result<OwnedSessionRecord, ThemeReasonCode> {
    Err(ThemeReasonCode::UnsupportedHost)
}

#[cfg(windows)]
fn restart_verified_host_impl(
    force: Option<(&VerifiedRootFingerprint, &AtomicBool)>,
) -> Result<OwnedSessionRecord, ThemeReasonCode> {
    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let reservation = reserve_loopback_port().map_err(restart_reason)?;
    let restarted = match (force, processes.as_slice()) {
        (Some((expected, cancellation)), [current_process]) => restart_verified_codex_force(
            RestartGuard {
                verified_ui_processes: 1,
                active_native_children: 1,
                setup_phase: SetupPhase::Committed,
            },
            &package,
            current_process,
            expected,
            cancellation,
            reservation,
            15_000,
        ),
        (None, []) => launch_verified_codex(&package, &current_user.owner_sid, reservation, 15_000),
        (None, [current_process]) => restart_verified_codex(
            RestartGuard {
                verified_ui_processes: 1,
                active_native_children: 0,
                setup_phase: SetupPhase::Committed,
            },
            &package,
            current_process,
            reservation,
            15_000,
        ),
        _ => Err(IdentityError::AmbiguousUiProcess),
    }
    .map_err(restart_reason)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ThemeReasonCode::CdpVerificationFailed)?;
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
                .map_err(|_| ThemeReasonCode::CdpVerificationFailed);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(ThemeReasonCode::CdpVerificationFailed);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(windows)]
fn current_verified_root_fingerprint() -> Result<VerifiedRootFingerprint, ThemeReasonCode> {
    let package = discover_store_package().map_err(restart_reason)?;
    let current_user = query_process_identity(std::process::id()).map_err(restart_reason)?;
    let processes = discover_verified_ui_processes(&package, &current_user.owner_sid)
        .map_err(restart_reason)?;
    let [current_process] = processes.as_slice() else {
        return Err(ThemeReasonCode::IdentityChanged);
    };
    root_fingerprint(current_process).map_err(restart_reason)
}

#[cfg(not(windows))]
fn current_verified_root_fingerprint() -> Result<VerifiedRootFingerprint, ThemeReasonCode> {
    Err(ThemeReasonCode::UnsupportedHost)
}

fn restart_reason(error: IdentityError) -> ThemeReasonCode {
    match error {
        IdentityError::ActiveNativeChild => ThemeReasonCode::ActiveWork,
        IdentityError::ProcessIdentityChanged => ThemeReasonCode::IdentityChanged,
        IdentityError::ProcessTreeIncomplete => ThemeReasonCode::ImpactChanged,
        IdentityError::TerminationFailed => ThemeReasonCode::TerminationFailed,
        IdentityError::TreeStillRunning => ThemeReasonCode::OldTreeStillRunning,
        IdentityError::OperationCancelled => ThemeReasonCode::ConfirmationRequired,
        IdentityError::LaunchFailed | IdentityError::ActivationFailed => {
            ThemeReasonCode::TerminalPartialFailure
        }
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
        | IdentityError::ProcessPackage => ThemeReasonCode::UnsupportedHost,
        _ => ThemeReasonCode::CdpUnavailable,
    }
}

fn theme_engine_reason(error: ThemeEngineError) -> ThemeReasonCode {
    match error {
        ThemeEngineError::DomIncompatible => ThemeReasonCode::DomIncompatible,
        ThemeEngineError::UnsupportedVersion => ThemeReasonCode::UnsupportedHost,
        ThemeEngineError::AmbiguousPrimaryTarget => ThemeReasonCode::MultipleWindows,
        ThemeEngineError::PartialApplication => ThemeReasonCode::PartialApplyFailed,
        ThemeEngineError::InvalidPack(_) => ThemeReasonCode::UnsupportedHost,
        ThemeEngineError::Discovery(_) | ThemeEngineError::Cdp(_) => {
            ThemeReasonCode::CdpUnavailable
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
