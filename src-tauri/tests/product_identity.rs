use serde_json::Value as JsonValue;

const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const MAIN_RS: &str = include_str!("../src/main.rs");
const RUNTIME_RS: &str = include_str!("../src/monitor/runtime.rs");
const PACKAGE_JSON: &str = include_str!("../../package.json");
const PACKAGE_LOCK_JSON: &str = include_str!("../../package-lock.json");
const INDEX_HTML: &str = include_str!("../../index.html");
const THIRD_PARTY_NOTICES: &str = include_str!("../../THIRD_PARTY_NOTICES.md");
const DEFAULT_CAPABILITY: &str = include_str!("../capabilities/default.json");
const DEFAULT_PERMISSION: &str = include_str!("../permissions/default.toml");
const NSIS_HOOK: &str = include_str!("../windows/installer-hooks.nsh");

fn json(document: &str, label: &str) -> JsonValue {
    serde_json::from_str(document).unwrap_or_else(|error| panic!("invalid {label}: {error}"))
}

fn toml_document(document: &str, label: &str) -> toml::Value {
    toml::from_str(document).unwrap_or_else(|error| panic!("invalid {label}: {error}"))
}

fn nsis_instruction(instruction: &str) -> bool {
    NSIS_HOOK
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with(';'))
        .any(|line| line == instruction)
}

fn nsis_macro_instructions(macro_name: &str) -> Vec<&'static str> {
    let header = format!("!macro {macro_name}");
    NSIS_HOOK
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| *line != "!macroend")
        .filter(|line| !line.is_empty() && !line.starts_with(';'))
        .collect()
}

#[test]
fn locks_the_codex_assistant_product_identity() {
    let tauri = json(TAURI_CONF, "tauri.conf.json");
    let package = json(PACKAGE_JSON, "package.json");
    let lockfile = json(PACKAGE_LOCK_JSON, "package-lock.json");
    let cargo = toml_document(CARGO_TOML, "Cargo.toml");
    let permissions = toml_document(DEFAULT_PERMISSION, "permissions/default.toml");
    let capability = json(DEFAULT_CAPABILITY, "capabilities/default.json");

    assert_eq!(tauri["productName"].as_str(), Some("Codex Assistant"));
    assert_eq!(tauri["version"].as_str(), Some("0.5.0"));
    assert_eq!(
        tauri["identifier"].as_str(),
        Some("com.codexagentmonitor.desktop")
    );
    assert_eq!(
        tauri["app"]["windows"]
            .as_array()
            .and_then(|windows| windows.first())
            .and_then(|window| window["title"].as_str()),
        Some("Codex Assistant")
    );
    assert_eq!(
        tauri["bundle"]["windows"]["nsis"]["installMode"].as_str(),
        Some("currentUser")
    );
    assert_eq!(
        tauri["bundle"]["windows"]["nsis"]["installerHooks"].as_str(),
        Some("./windows/installer-hooks.nsh")
    );

    assert_eq!(package["name"].as_str(), Some("codex-assistant"));
    assert_eq!(package["version"].as_str(), Some("0.5.0"));
    assert_eq!(lockfile["name"].as_str(), Some("codex-assistant"));
    assert_eq!(lockfile["version"].as_str(), Some("0.5.0"));
    assert_eq!(
        lockfile["packages"][""]["name"].as_str(),
        Some("codex-assistant")
    );
    assert_eq!(lockfile["packages"][""]["version"].as_str(), Some("0.5.0"));

    assert_eq!(cargo["package"]["name"].as_str(), Some("codex-assistant"));
    assert_eq!(cargo["package"]["version"].as_str(), Some("0.5.0"));
    assert_eq!(
        cargo["bin"]
            .as_array()
            .and_then(|binaries| binaries.first())
            .and_then(|binary| binary["name"].as_str()),
        Some("codex-assistant")
    );
    assert_eq!(cargo["lib"]["name"].as_str(), Some("codex_assistant_lib"));

    assert!(MAIN_RS.contains("codex_assistant_lib::run()"));
    assert!(RUNTIME_RS.contains("SETTINGS_DIRECTORY: &str = \"codex-agent-monitor\""));
    assert!(INDEX_HTML.contains("<title>Codex Assistant</title>"));
    assert!(THIRD_PARTY_NOTICES.contains("Codex Assistant is based"));
    assert_eq!(
        capability["description"].as_str(),
        Some("Minimal permissions for Codex Assistant")
    );
    assert_eq!(
        permissions["default"]["description"].as_str(),
        Some("Allow the sanitized Codex Assistant command surface")
    );

    assert!(nsis_instruction("!macro NSIS_HOOK_PREINSTALL"));
    assert!(nsis_instruction(
        "!define LEGACY_PRODUCT_NAME \"Codex Agent Monitor\""
    ));
    assert!(nsis_instruction("!define LEGACY_VERSION \"0.4.0\""));

    let preinstall = nsis_macro_instructions("NSIS_HOOK_PREINSTALL");
    let postinstall = nsis_macro_instructions("NSIS_HOOK_POSTINSTALL");
    assert!(nsis_instruction(
        "ReadRegStr $R0 HKCU \"${LEGACY_UNINST_KEY}\" \"DisplayName\""
    ));
    assert!(nsis_instruction(
        "ReadRegStr $R3 HKCU \"${LEGACY_UNINST_KEY}\" \"MainBinaryName\""
    ));
    assert!(preinstall.contains(&"ReadRegStr $R6 HKCU \"${LEGACY_UNINST_KEY}\" \"DisplayVersion\""));
    assert!(preinstall.contains(&"StrCmp $R6 \"${LEGACY_VERSION}\" 0 legacy_pre_done"));
    assert!(preinstall
        .contains(&"CopyFiles /SILENT \"$R4\\uninstall.exe\" \"$R4\\${LEGACY_UNINSTALL_BACKUP}\""));
    assert!(preinstall.contains(
        &"WriteRegStr HKCU \"${LEGACY_UNINST_KEY}\" \"UninstallString\" \"$\\\"$R4\\${LEGACY_UNINSTALL_BACKUP}$\\\"\""
    ));
    assert!(!preinstall.iter().any(|line| line.starts_with("ExecWait ")));
    assert!(!preinstall
        .iter()
        .any(|line| line == &"Delete \"$R4\\${LEGACY_MAIN_BINARY}\""));
    assert!(!preinstall
        .iter()
        .any(|line| line == &"DeleteRegKey HKCU \"${LEGACY_UNINST_KEY}\""));

    assert!(postinstall.contains(
        &"!insertmacro IsShortcutTarget \"$SMPROGRAMS\\${LEGACY_PRODUCT_NAME}.lnk\" \"$R4\\${LEGACY_MAIN_BINARY}\""
    ));
    assert!(postinstall.contains(
        &"!insertmacro IsShortcutTarget \"$DESKTOP\\${LEGACY_PRODUCT_NAME}.lnk\" \"$R4\\${LEGACY_MAIN_BINARY}\""
    ));
    assert!(postinstall.contains(&"Delete \"$R4\\${LEGACY_MAIN_BINARY}\""));
    assert!(postinstall.contains(
        &"System::Call 'kernel32::GetFileAttributesW(w \"$R4\\${MAINBINARYNAME}.exe\") i .r7'"
    ));
    assert!(postinstall.contains(&"Delete \"$R4\\${LEGACY_UNINSTALL_BACKUP}\""));
    assert!(postinstall.contains(&"DeleteRegKey HKCU \"${LEGACY_UNINST_KEY}\""));
    let current_version_guard = postinstall
        .iter()
        .position(|line| line == &"StrCmp $1 \"${VERSION}\" 0 legacy_post_rollback")
        .expect("missing current-version POSTINSTALL guard");
    let legacy_delete = postinstall
        .iter()
        .position(|line| line == &"Delete \"$R4\\${LEGACY_MAIN_BINARY}\"")
        .expect("missing legacy executable cleanup");
    assert!(current_version_guard < legacy_delete);
    assert!(preinstall.contains(&"StrCpy $INSTDIR \"$R4\""));
    assert!(preinstall.contains(&"SetOutPath \"$INSTDIR\""));
}
