const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const MAIN_RS: &str = include_str!("../src/main.rs");
const RUNTIME_RS: &str = include_str!("../src/monitor/runtime.rs");

#[test]
fn locks_the_codex_assistant_product_identity() {
    assert!(TAURI_CONF.contains("\"productName\": \"Codex Assistant\""));
    assert!(TAURI_CONF.contains("\"title\": \"Codex Assistant\""));
    assert!(TAURI_CONF.contains("\"version\": \"0.5.0\""));
    assert!(TAURI_CONF.contains("\"identifier\": \"com.codexagentmonitor.desktop\""));

    assert!(CARGO_TOML.contains("name = \"codex-assistant\""));
    assert!(CARGO_TOML.contains("version = \"0.5.0\""));
    assert!(CARGO_TOML.contains("name = \"codex_assistant_lib\""));

    assert!(MAIN_RS.contains("codex_assistant_lib::run()"));
    assert!(RUNTIME_RS.contains("SETTINGS_DIRECTORY: &str = \"codex-agent-monitor\""));
}
