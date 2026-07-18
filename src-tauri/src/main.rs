// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("routing-mcp"))
        && arguments.next().is_none()
    {
        if codex_assistant_lib::routing_mcp::run_stdio_default().is_err() {
            eprintln!("routing_mcp_error code=server_failed count=1");
            std::process::exit(1);
        }
        return;
    }

    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {info}");
        if let Some(loc) = info.location() {
            eprintln!("  at {}:{}:{}", loc.file(), loc.line(), loc.column());
        }
    }));

    codex_assistant_lib::run()
}
