#[cfg(windows)]
use std::{
    fs,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use codex_assistant_lib::monitor::runtime::MonitorRuntime;
#[cfg(windows)]
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::Threading::{GetCurrentProcess, GetProcessTimes},
};

#[cfg(not(windows))]
fn main() {
    eprintln!("monitor_refresh_probe is supported only on Windows");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    const DEBOUNCE: Duration = Duration::from_millis(120);
    const LATENCY_SAMPLES: usize = 24;
    const FALLBACK_SAMPLES: usize = 5;

    let fixture = tempfile::tempdir().expect("create monitor fixture");
    let sessions = fixture.path().join("sessions");
    fs::create_dir_all(&sessions).expect("create fixture sessions");
    let runtime = MonitorRuntime::new(fixture.path().to_path_buf());
    let _ = runtime.refresh();

    let (sender, receiver) = mpsc::channel();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = sender.send(());
            }
        })
        .expect("create Windows filesystem watcher");
    watcher
        .watch(fixture.path(), RecursiveMode::Recursive)
        .expect("watch monitor fixture");
    thread::sleep(Duration::from_millis(100));
    while receiver.try_recv().is_ok() {}

    let mut latencies_ms = Vec::with_capacity(LATENCY_SAMPLES);
    for index in 0..LATENCY_SAMPLES {
        let started = Instant::now();
        fs::write(
            sessions.join(format!("notification-{index}.jsonl")),
            format!("{{\"index\":{index}}}\n"),
        )
        .expect("write notification fixture");
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("receive filesystem notification within fallback budget");
        thread::sleep(DEBOUNCE);
        while receiver.try_recv().is_ok() {}
        let _ = runtime.refresh();
        latencies_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    latencies_ms.sort_by(f64::total_cmp);

    while receiver.try_recv().is_ok() {}
    for index in 0..20 {
        fs::write(sessions.join(format!("burst-{index}.jsonl")), b"{}\n")
            .expect("write burst fixture");
    }
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("receive burst notification");
    thread::sleep(DEBOUNCE);
    let mut coalesced_events = 1_u64;
    while receiver.try_recv().is_ok() {
        coalesced_events += 1;
    }
    let _ = runtime.refresh();

    let manual_started = Instant::now();
    let _ = runtime.refresh();
    let manual_refresh_ms = manual_started.elapsed().as_secs_f64() * 1_000.0;

    let idle_wall_started = Instant::now();
    let idle_cpu_started = process_cpu_100ns();
    for _ in 0..FALLBACK_SAMPLES {
        thread::sleep(Duration::from_secs(1));
        let _ = runtime.refresh();
    }
    let idle_cpu_ended = process_cpu_100ns();
    let idle_wall = idle_wall_started.elapsed();
    let logical_processors = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let idle_cpu_percent = (idle_cpu_ended.saturating_sub(idle_cpu_started) as f64 / 10_000_000.0)
        / idle_wall.as_secs_f64()
        / logical_processors as f64
        * 100.0;

    let p50 = percentile(&latencies_ms, 0.50);
    let p95 = percentile(&latencies_ms, 0.95);
    let maximum = latencies_ms.last().copied().unwrap_or_default();
    let passed =
        p50 <= 500.0 && p95 <= 1_000.0 && manual_refresh_ms <= 1_000.0 && idle_cpu_percent <= 1.5;
    println!(
        "{}",
        serde_json::json!({
            "status": if passed { "passed" } else { "failed" },
            "platform": "windows",
            "latencySamples": LATENCY_SAMPLES,
            "debounceMs": DEBOUNCE.as_millis(),
            "latencyMs": {
                "p50": round_two(p50),
                "p95": round_two(p95),
                "maximum": round_two(maximum)
            },
            "burst": {
                "observedEvents": coalesced_events,
                "refreshes": 1
            },
            "manualRefreshMs": round_two(manual_refresh_ms),
            "fallback": {
                "intervalMs": 1_000,
                "samples": FALLBACK_SAMPLES
            },
            "idle": {
                "wallMs": round_two(idle_wall.as_secs_f64() * 1_000.0),
                "logicalProcessors": logical_processors,
                "normalizedCpuPercent": round_two(idle_cpu_percent),
                "targetPercent": 1.0,
                "gatePercent": 1.5
            }
        })
    );
    if !passed {
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    let index = ((sorted.len().saturating_sub(1)) as f64 * quantile).ceil() as usize;
    sorted.get(index).copied().unwrap_or_default()
}

#[cfg(windows)]
fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(windows)]
fn process_cpu_100ns() -> u64 {
    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let succeeded = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    assert_ne!(succeeded, 0, "read process CPU time");
    filetime_value(kernel) + filetime_value(user)
}

#[cfg(windows)]
fn filetime_value(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
