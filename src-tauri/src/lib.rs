pub mod control_layer;
pub mod local_theme;
pub mod monitor;
pub mod private_state;
pub mod theme;
pub mod theme_app;
pub mod theme_environment;
pub mod theme_state;

use std::sync::Arc;

use monitor::{
    model::{MonitorSnapshot, RestartSafetyProjection},
    runtime::MonitorRuntime,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};
use theme_app::{
    ForceRestartImpact, OperationReceipt, RestartIntent, RestartMode, ThemeApplication,
    ThemeImportReceipt, ThemeUiSnapshot,
};
use theme_environment::ThemeEnvironmentReport;

const MONITOR_EVENT: &str = "monitor://snapshot";

pub fn run() {
    let monitor = Arc::new(MonitorRuntime::default());
    let setup_monitor = Arc::clone(&monitor);
    let themes = Arc::new(
        ThemeApplication::default_location()
            .expect("Codex Assistant theme runtime could not be initialized"),
    );
    let setup_themes = Arc::clone(&themes);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_single_instance::Builder::new()
                .callback(|app, _args, _cwd| {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                })
                .build(),
        )
        .manage(monitor)
        .manage(themes)
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor,
            get_monitor_settings,
            set_codex_home,
            prepare_force_restart,
            cancel_force_restart,
            get_theme_snapshot,
            get_theme_environment,
            get_theme_preview_data_url,
            import_local_theme,
            start_theme_session,
            activate_theme,
            restore_theme,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            let handle = app.handle().clone();
            let (change_sender, mut change_receiver) = tokio::sync::mpsc::unbounded_channel();
            let mut watcher: Option<RecommendedWatcher> =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    if event.is_ok() {
                        let _ = change_sender.send(());
                    }
                })
                .ok();
            let mut watched_root = setup_monitor.watch_root();
            let watch_failed = watcher.as_mut().is_some_and(|active| {
                active
                    .watch(&watched_root, RecursiveMode::Recursive)
                    .is_err()
            });
            if watch_failed {
                watcher = None;
            }
            tauri::async_runtime::spawn(async move {
                let mut fallback = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    tokio::select! {
                        _ = fallback.tick() => {}
                        event = change_receiver.recv() => {
                            if event.is_none() { break; }
                            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                            while change_receiver.try_recv().is_ok() {}
                        }
                    }
                    let current_root = setup_monitor.watch_root();
                    if current_root != watched_root {
                        let watch_failed = watcher.as_mut().is_some_and(|active| {
                            let _ = active.unwatch(&watched_root);
                            active
                                .watch(&current_root, RecursiveMode::Recursive)
                                .is_err()
                        });
                        if watch_failed {
                            watcher = None;
                        }
                        watched_root = current_root;
                    }
                    let monitor_worker = Arc::clone(&setup_monitor);
                    if let Ok((snapshot, changed)) =
                        tauri::async_runtime::spawn_blocking(move || monitor_worker.refresh()).await
                    {
                        if changed {
                            let _ = handle.emit(MONITOR_EVENT, snapshot);
                        }
                    }
                }
            });
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let theme_worker = Arc::clone(&setup_themes);
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        theme_worker.reconcile_session()
                    })
                    .await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Codex Assistant");
}

fn restart_safety(monitor: &MonitorRuntime) -> RestartSafetyProjection {
    let snapshot = monitor.snapshot();
    RestartSafetyProjection::from_snapshot(&snapshot)
}

#[tauri::command]
fn get_monitor_snapshot(runtime: tauri::State<'_, Arc<MonitorRuntime>>) -> MonitorSnapshot {
    runtime.snapshot()
}

#[tauri::command]
async fn refresh_monitor(
    runtime: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<MonitorSnapshot, String> {
    let worker = Arc::clone(runtime.inner());
    tauri::async_runtime::spawn_blocking(move || worker.refresh().0)
        .await
        .map_err(|_| "Monitor refresh failed".to_owned())
}

#[tauri::command]
fn get_monitor_settings(
    runtime: tauri::State<'_, Arc<MonitorRuntime>>,
) -> monitor::runtime::MonitorSettings {
    runtime.settings()
}

#[tauri::command]
fn set_codex_home(
    path: String,
    runtime: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<monitor::runtime::MonitorSettings, String> {
    runtime.set_codex_home(&path)
}

#[tauri::command]
async fn prepare_force_restart(
    intent: RestartIntent,
    subject: Option<String>,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<ForceRestartImpact, String> {
    let safety = restart_safety(monitor.inner());
    let worker = Arc::clone(runtime.inner());
    tauri::async_runtime::spawn_blocking(move || {
        worker.prepare_force_restart_with_safety(intent, subject, safety)
    })
    .await
    .map_err(|_| "operation-conflict".to_owned())?
    .map_err(|reason| format!("{reason:?}"))
}

#[tauri::command]
fn cancel_force_restart(
    confirmation_ticket: String,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
) -> bool {
    runtime.cancel_force_restart(&confirmation_ticket)
}

#[tauri::command]
fn get_theme_snapshot(runtime: tauri::State<'_, Arc<ThemeApplication>>) -> ThemeUiSnapshot {
    runtime.reconcile_session();
    runtime.snapshot()
}

#[tauri::command]
fn get_theme_environment(
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
) -> ThemeEnvironmentReport {
    runtime.environment_report()
}

#[tauri::command]
fn get_theme_preview_data_url(
    theme_id: String,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
) -> Option<String> {
    runtime.preview_data_url(&theme_id)
}

#[tauri::command]
async fn import_local_theme(
    name: String,
    image_data_url: String,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
) -> Result<ThemeImportReceipt, String> {
    let worker = Arc::clone(runtime.inner());
    tauri::async_runtime::spawn_blocking(move || worker.import_local_theme(&name, &image_data_url))
        .await
        .map_err(|_| "Local theme import failed".to_owned())?
}

#[tauri::command]
async fn start_theme_session(
    restart_mode: RestartMode,
    confirmation_ticket: Option<String>,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<OperationReceipt, String> {
    let safety = restart_safety(monitor.inner());
    let worker = Arc::clone(runtime.inner());
    let fallback = Arc::clone(&worker);
    match tauri::async_runtime::spawn_blocking(move || {
        worker.start_session_mode_with_safety(restart_mode, confirmation_ticket.as_deref(), safety)
    })
    .await
    {
        Ok(receipt) => Ok(receipt),
        Err(_) => Ok(fallback.unavailable_operation()),
    }
}

#[tauri::command]
async fn activate_theme(
    theme_id: String,
    restart_mode: RestartMode,
    confirmation_ticket: Option<String>,
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<OperationReceipt, String> {
    let safety = restart_safety(monitor.inner());
    let worker = Arc::clone(runtime.inner());
    let fallback = Arc::clone(&worker);
    match tauri::async_runtime::spawn_blocking(move || {
        worker.activate_mode_with_safety(
            &theme_id,
            restart_mode,
            confirmation_ticket.as_deref(),
            safety,
        )
    })
    .await
    {
        Ok(receipt) => Ok(receipt),
        Err(_) => Ok(fallback.unavailable_operation()),
    }
}

#[tauri::command]
async fn restore_theme(
    runtime: tauri::State<'_, Arc<ThemeApplication>>,
) -> Result<OperationReceipt, String> {
    let worker = Arc::clone(runtime.inner());
    let fallback = Arc::clone(&worker);
    match tauri::async_runtime::spawn_blocking(move || worker.restore()).await {
        Ok(receipt) => Ok(receipt),
        Err(_) => Ok(fallback.unavailable_operation()),
    }
}
