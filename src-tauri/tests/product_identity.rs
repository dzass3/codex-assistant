const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const MAIN_RS: &str = include_str!("../src/main.rs");
const RUNTIME_RS: &str = include_str!("../src/monitor/runtime.rs");

fn toml_section<'a>(document: &'a str, header: &str) -> &'a str {
    let (_, section_and_rest) = document
        .split_once(header)
        .unwrap_or_else(|| panic!("missing TOML section {header}"));

    section_and_rest
        .split_once("\n[")
        .map_or(section_and_rest, |(section, _)| section)
}

#[test]
fn locks_the_codex_assistant_product_identity() {
    assert!(TAURI_CONF.contains("\"productName\": \"Codex Assistant\""));
    assert!(TAURI_CONF.contains("\"title\": \"Codex Assistant\""));
    assert!(TAURI_CONF.contains("\"version\": \"0.5.0\""));
    assert!(TAURI_CONF.contains("\"identifier\": \"com.codexagentmonitor.desktop\""));

    let package = toml_section(CARGO_TOML, "[package]");
    let binary = toml_section(CARGO_TOML, "[[bin]]");
    let library = toml_section(CARGO_TOML, "[lib]");

    assert!(package.contains("name = \"codex-assistant\""));
    assert!(package.contains("version = \"0.5.0\""));
    assert!(binary.contains("name = \"codex-assistant\""));
    assert!(library.contains("name = \"codex_assistant_lib\""));

    assert!(MAIN_RS.contains("codex_assistant_lib::run()"));
    assert!(RUNTIME_RS.contains("SETTINGS_DIRECTORY: &str = \"codex-agent-monitor\""));
}
