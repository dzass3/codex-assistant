use codex_assistant_lib::theme_app::ThemeApplication;
use serde::Serialize;

#[derive(Serialize)]
struct ProbeResult {
    session_reachable: bool,
    before: codex_assistant_lib::theme_app::ThemeUiSnapshot,
    receipt: codex_assistant_lib::theme_app::OperationReceipt,
    after: codex_assistant_lib::theme_app::ThemeUiSnapshot,
}

fn main() {
    let theme_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "seaside-blue".to_owned());
    let application =
        ThemeApplication::default_location().expect("theme application state must be available");
    let session_reachable = application.reconcile_session();
    let before = application.snapshot();
    let receipt = application.activate(&theme_id, 0);
    let after = application.snapshot();
    let result = ProbeResult {
        session_reachable,
        before,
        receipt,
        after,
    };
    println!(
        "{}",
        serde_json::to_string(&result).expect("probe result must serialize")
    );
}
