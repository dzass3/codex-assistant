use std::fs;

use codex_assistant_lib::routing::{EligibilityReasonCode, EligibilityStatus, RouteKind};
use codex_assistant_lib::routing_app::{
    OperationStatus, RoutingApplication, RoutingInstallationStatus, RoutingRestartStatus,
};
use tempfile::tempdir;

#[test]
fn first_install_returns_a_sanitized_receipt_and_requires_one_restart() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("codex home");
    fs::write(
        codex_home.join("config.toml"),
        "# keep\n[agents]\nmax_threads = 7\n",
    )
    .expect("config fixture");
    let app = RoutingApplication::for_paths(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");

    assert_eq!(
        app.snapshot().setup.installation_status,
        RoutingInstallationStatus::Uninstalled
    );
    let receipt = app.install();

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(receipt.restart_required);
    assert!(uuid::Uuid::parse_str(&receipt.operation_id).is_ok());
    assert!(receipt.reason_codes.is_empty());
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::RestartRequired
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::Required
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("installed config");
    assert!(config.contains("max_threads = 7"));
    assert!(config.contains("[agents.codex_assistant_luna]"));
}

#[test]
fn restore_returns_owned_configuration_to_its_preinstall_state() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("codex home");
    let original = b"# original\r\n[agents]\r\nmax_depth = 1\r\n";
    fs::write(codex_home.join("config.toml"), original).expect("config fixture");
    let app = RoutingApplication::for_paths(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.restore();

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(receipt.restart_required);
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("restored config"),
        original
    );
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::Uninstalled
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::Required
    );
}

#[test]
fn routing_cannot_be_enabled_before_install_and_native_preflight() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");

    let receipt = app.set_root_enabled("d2719d93-b823-4a7f-934f-23cbe01c8ab0", true, true);

    assert_eq!(receipt.status, OperationStatus::Blocked);
    assert_eq!(
        receipt.reason_codes,
        vec![codex_assistant_lib::routing_app::RoutingSetupReasonCode::PreflightRequired]
    );
    assert!(app.snapshot().routing.routes.is_empty());
}

#[test]
fn restart_is_blocked_while_a_native_child_is_active() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.request_restart(1);

    assert_eq!(receipt.status, OperationStatus::Blocked);
    assert_eq!(
        receipt.reason_codes,
        vec![codex_assistant_lib::routing_app::RoutingSetupReasonCode::ActiveChild]
    );
    assert!(receipt.restart_required);
    assert_eq!(
        app.snapshot().setup.restart_status,
        RoutingRestartStatus::BlockedActiveChild
    );
}

#[test]
fn successful_verified_restart_clears_the_one_time_restart_requirement() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    assert_eq!(app.install().status, OperationStatus::Applied);

    let receipt = app.request_restart_with(0, || Ok(()));

    assert_eq!(receipt.status, OperationStatus::Applied);
    assert!(!receipt.restart_required);
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.installation_status,
        RoutingInstallationStatus::Installed
    );
    assert_eq!(
        snapshot.setup.restart_status,
        RoutingRestartStatus::NotRequired
    );
}

#[test]
fn preflight_begins_with_visible_direct_native_checks_instead_of_assuming_support() {
    let root = tempdir().expect("temporary root");
    let app = RoutingApplication::for_paths(
        root.path().join("codex"),
        root.path().join("skills"),
        std::env::current_exe().expect("test executable"),
        root.path().join("state"),
    )
    .expect("routing application");
    app.install();
    app.request_restart_with(0, || Ok(()));
    let root_id = "d2719d93-b823-4a7f-934f-23cbe01c8ab0";

    let receipt = app.begin_preflight_with(root_id, true, "26.715.3651.0");

    assert_eq!(receipt.status, OperationStatus::Applied);
    let snapshot = app.snapshot();
    assert_eq!(
        snapshot.setup.preflight_status,
        codex_assistant_lib::routing_app::RoutingPreflightStatus::Running
    );
    assert_eq!(snapshot.routing.eligibility.len(), 4);
    assert!(snapshot.routing.eligibility.iter().all(|entry| {
        entry.route_kind == RouteKind::Direct
            && entry.status == EligibilityStatus::Verifying
            && entry.reason == Some(EligibilityReasonCode::AwaitingVisibleCommand)
    }));
}
