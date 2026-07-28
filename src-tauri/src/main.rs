#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Codex Assistant terminated unexpectedly: {info}");
    }));

    codex_assistant_lib::run();
}
