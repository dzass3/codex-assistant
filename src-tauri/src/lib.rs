pub mod codex_config;
pub mod control_layer;
pub mod monitor;
pub mod preflight;
pub mod routing;
pub mod routing_app;
pub mod routing_mcp;

use std::sync::Arc;

use monitor::{runtime::MonitorRuntime, MonitorSnapshot};
use routing_app::{OperationReceipt, RoutingApplication, RoutingUiSnapshot};
use tauri::{Emitter, Manager};

const MONITOR_EVENT: &str = "monitor://snapshot";

pub fn run() {
    let runtime = Arc::new(MonitorRuntime::default());
    let setup_runtime = Arc::clone(&runtime);
    let routing_runtime = Arc::new(
        RoutingApplication::default_location()
            .expect("Smart Routing local runtime could not be initialized"),
    );

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
        .manage(runtime)
        .manage(routing_runtime)
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor,
            get_monitor_settings,
            set_codex_home,
            get_routing_snapshot,
            install_routing,
            restore_routing,
            request_codex_restart,
            begin_routing_preflight,
            set_root_routing_enabled,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }

            let handle = app.handle().clone();
            let runtime = Arc::clone(&setup_runtime);
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let worker = Arc::clone(&runtime);
                    let result =
                        tauri::async_runtime::spawn_blocking(move || worker.refresh()).await;
                    let Ok((snapshot, changed)) = result else {
                        continue;
                    };
                    if changed {
                        let _ = handle.emit(MONITOR_EVENT, snapshot);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Codex Assistant");
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
fn get_routing_snapshot(runtime: tauri::State<'_, Arc<RoutingApplication>>) -> RoutingUiSnapshot {
    runtime.snapshot()
}

#[tauri::command]
fn install_routing(runtime: tauri::State<'_, Arc<RoutingApplication>>) -> OperationReceipt {
    runtime.install()
}

#[tauri::command]
fn restore_routing(runtime: tauri::State<'_, Arc<RoutingApplication>>) -> OperationReceipt {
    runtime.restore()
}

#[tauri::command]
async fn request_codex_restart(
    runtime: tauri::State<'_, Arc<RoutingApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> Result<OperationReceipt, String> {
    let active_native_children = monitor
        .snapshot()
        .agents
        .iter()
        .filter(|agent| {
            agent.is_subagent
                && matches!(
                    agent.status,
                    monitor::model::AgentStatus::Starting | monitor::model::AgentStatus::Running
                )
        })
        .count();
    let worker = Arc::clone(runtime.inner());
    let fallback = Arc::clone(&worker);
    match tauri::async_runtime::spawn_blocking(move || {
        worker.request_restart(active_native_children)
    })
    .await
    {
        Ok(receipt) => Ok(receipt),
        Err(_) => Ok(fallback.unavailable_operation()),
    }
}

#[tauri::command]
fn begin_routing_preflight(
    root_conversation_id: String,
    runtime: tauri::State<'_, Arc<RoutingApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> OperationReceipt {
    let root_is_observed = monitor
        .snapshot()
        .agents
        .iter()
        .any(|agent| !agent.is_subagent && agent.thread_id == root_conversation_id);
    runtime.begin_preflight(&root_conversation_id, root_is_observed)
}

#[tauri::command]
fn set_root_routing_enabled(
    root_conversation_id: String,
    enabled: bool,
    routing: tauri::State<'_, Arc<RoutingApplication>>,
    monitor: tauri::State<'_, Arc<MonitorRuntime>>,
) -> OperationReceipt {
    let root_is_observed = monitor
        .snapshot()
        .agents
        .iter()
        .any(|agent| !agent.is_subagent && agent.thread_id == root_conversation_id);
    routing.set_root_enabled(&root_conversation_id, enabled, root_is_observed)
}
