use std::collections::BTreeSet;

const LIB_RS: &str = include_str!("../src/lib.rs");
const MAIN_RS: &str = include_str!("../src/main.rs");
const PERMISSIONS: &str = include_str!("../permissions/default.toml");
const TAURI_CONFIG: &str = include_str!("../tauri.conf.json");
const THEME_API: &str = include_str!("../../src/lib/themeApi.ts");
const MONITOR_API: &str = include_str!("../../src/lib/monitorApi.ts");
const APP: &str = include_str!("../../src/App.tsx");

fn granted_commands() -> BTreeSet<String> {
    let document: toml::Value = toml::from_str(PERMISSIONS).expect("permissions TOML");
    document
        .get("permission")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|entry| {
            entry
                .get("commands")
                .and_then(|commands| commands.get("allow"))
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn handler_commands() -> BTreeSet<String> {
    let body = LIB_RS
        .split_once("tauri::generate_handler![")
        .and_then(|(_, tail)| tail.split_once("])"))
        .map(|(body, _)| body)
        .expect("Tauri handler list");
    body.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn desktop_exposes_only_read_only_observer_and_theme_commands() {
    let expected = BTreeSet::from([
        "activate_theme".to_owned(),
        "cancel_force_restart".to_owned(),
        "get_monitor_settings".to_owned(),
        "get_monitor_snapshot".to_owned(),
        "get_theme_preview_data_url".to_owned(),
        "get_theme_environment".to_owned(),
        "get_theme_snapshot".to_owned(),
        "import_local_theme".to_owned(),
        "prepare_force_restart".to_owned(),
        "refresh_monitor".to_owned(),
        "restore_theme".to_owned(),
        "set_codex_home".to_owned(),
        "start_theme_session".to_owned(),
    ]);
    assert_eq!(granted_commands(), expected);
    assert_eq!(handler_commands(), expected);
    for command in [
        "activate_theme",
        "cancel_force_restart",
        "get_theme_preview_data_url",
        "get_theme_environment",
        "get_theme_snapshot",
        "import_local_theme",
        "prepare_force_restart",
        "restore_theme",
        "start_theme_session",
    ] {
        assert!(THEME_API.contains(&format!("invoke(\"{command}\"")));
    }
    for command in [
        "get_monitor_settings",
        "get_monitor_snapshot",
        "refresh_monitor",
        "set_codex_home",
    ] {
        assert!(MONITOR_API.contains(&format!("invoke(\"{command}\"")));
    }
}

#[test]
fn observer_event_contract_is_namespaced_and_emits_only_changed_snapshots() {
    let event_declaration = "const MONITOR_EVENT: &str = \"monitor://snapshot\"";
    assert!(LIB_RS.contains(event_declaration));
    assert!(MONITOR_API.contains("const MONITOR_EVENT = \"monitor://snapshot\""));
    assert!(LIB_RS.contains("if changed"));
    assert!(LIB_RS.contains("handle.emit(MONITOR_EVENT, snapshot)"));
}

#[test]
fn shipped_entrypoints_and_resources_contain_no_smart_routing_surface() {
    for (name, source) in [
        ("lib.rs", LIB_RS),
        ("main.rs", MAIN_RS),
        ("permissions", PERMISSIONS),
        ("tauri config", TAURI_CONFIG),
        ("App", APP),
    ] {
        let lower = source.to_ascii_lowercase();
        for forbidden in [
            "smart routing",
            "routing-mcp",
            "get_routing_snapshot",
            "begin_routing_preflight",
            "set_root_routing_enabled",
            "resources/routing",
            "routing-control",
        ] {
            assert!(
                !lower.contains(forbidden),
                "{name} still contains {forbidden}"
            );
        }
    }
}
