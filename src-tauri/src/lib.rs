pub mod monitor;
pub mod routing;

use std::sync::Arc;

use monitor::{runtime::MonitorRuntime, MonitorSnapshot};
use tauri::{Emitter, Manager};

const MONITOR_EVENT: &str = "monitor://snapshot";

pub fn run() {
    let runtime = Arc::new(MonitorRuntime::default());
    let setup_runtime = Arc::clone(&runtime);

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
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor,
            get_monitor_settings,
            set_codex_home,
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
