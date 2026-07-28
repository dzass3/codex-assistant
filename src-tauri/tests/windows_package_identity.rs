use std::path::PathBuf;

use codex_assistant_lib::control_layer::windows_package::{
    authorize_restart, cdp_launch_arguments, discover_store_package, parse_package_query,
    plan_leaf_first_termination, query_process_identity, query_tcp_listener, reserve_loopback_port,
    store_activation_arguments, validate_app_server_set, validate_no_owned_runtime_processes,
    validate_replacement_set, validate_stable_pid_samples, validate_tree_drain, verify_listener,
    verify_package, verify_process, IdentityError, ListenerProbe, PackageProbe, ProcessProbe,
    ProcessTreeEntry, RestartGuard, RuntimeProcessProbe, SetupPhase, SignatureStatus,
    VerifiedProcess, CODEX_APP_USER_MODEL_ID, CODEX_PACKAGE_FAMILY,
};

fn package() -> PackageProbe {
    PackageProbe {
        name: "OpenAI.Codex".into(),
        package_family: CODEX_PACKAGE_FAMILY.into(),
        version: "26.715.3651.0".into(),
        canonical_root: PathBuf::from(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.715.3651.0_x64__2p2nqsd0c76g0",
        ),
        canonical_executable: PathBuf::from(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.715.3651.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe",
        ),
        signature: SignatureStatus::TrustedStore,
    }
}

#[test]
fn force_termination_plan_is_leaf_first_and_root_last() {
    let entries = [
        ProcessTreeEntry {
            pid: 10,
            parent_pid: 1,
        },
        ProcessTreeEntry {
            pid: 11,
            parent_pid: 10,
        },
        ProcessTreeEntry {
            pid: 12,
            parent_pid: 10,
        },
        ProcessTreeEntry {
            pid: 13,
            parent_pid: 11,
        },
        ProcessTreeEntry {
            pid: 20,
            parent_pid: 1,
        },
    ];
    let plan = plan_leaf_first_termination(10, &entries).expect("complete process tree");
    assert_eq!(plan.last(), Some(&10));
    assert!(
        plan.iter().position(|pid| *pid == 13).unwrap()
            < plan.iter().position(|pid| *pid == 11).unwrap()
    );
    assert!(
        plan.iter().position(|pid| *pid == 11).unwrap()
            < plan.iter().position(|pid| *pid == 10).unwrap()
    );
    assert_eq!(plan.len(), 4);
}

#[test]
fn force_termination_plan_fails_closed_for_missing_or_duplicate_root() {
    assert_eq!(
        plan_leaf_first_termination(
            10,
            &[ProcessTreeEntry {
                pid: 11,
                parent_pid: 10
            }]
        ),
        Err(IdentityError::ProcessTreeIncomplete)
    );
    assert_eq!(
        plan_leaf_first_termination(
            10,
            &[
                ProcessTreeEntry {
                    pid: 10,
                    parent_pid: 1
                },
                ProcessTreeEntry {
                    pid: 10,
                    parent_pid: 2
                },
            ],
        ),
        Err(IdentityError::ProcessTreeIncomplete)
    );
}

#[test]
fn safe_restart_requires_every_original_descendant_to_exit_before_activation() {
    let plan = vec![13, 11, 12, 10];

    assert_eq!(validate_tree_drain(&plan, &[]), Ok(()));
    assert_eq!(
        validate_tree_drain(&plan, &[13]),
        Err(IdentityError::TreeStillRunning)
    );
    assert_eq!(
        validate_tree_drain(&plan, &[11, 12]),
        Err(IdentityError::TreeStillRunning)
    );
}

#[test]
fn theme_session_requires_a_stable_direct_official_app_server() {
    let verified_package = verify_package(package()).expect("official package");
    let root = VerifiedProcess {
        pid: 42_000,
        owner_sid: "S-1-5-21-1000".into(),
        image_path: verified_package.executable.clone(),
        package_version: verified_package.version.clone(),
    };
    let app_server = RuntimeProcessProbe {
        pid: 42_100,
        parent_pid: root.pid,
        owner_sid: root.owner_sid.clone(),
        canonical_image_path: verified_package
            .root
            .join("app")
            .join("resources")
            .join("codex.exe"),
    };

    assert_eq!(
        validate_app_server_set(&verified_package, &root, std::slice::from_ref(&app_server)),
        Ok(app_server.pid)
    );
    assert_eq!(validate_no_owned_runtime_processes(&[]), Ok(()));
    assert_eq!(
        validate_no_owned_runtime_processes(std::slice::from_ref(&app_server)),
        Err(IdentityError::TreeStillRunning)
    );

    let mut orphan = app_server.clone();
    orphan.parent_pid = 41_999;
    assert_eq!(
        validate_app_server_set(&verified_package, &root, &[orphan]),
        Err(IdentityError::AppServerUnavailable)
    );

    let mut wrong_owner = app_server.clone();
    wrong_owner.owner_sid = "S-1-5-21-2000".into();
    assert_eq!(
        validate_app_server_set(&verified_package, &root, &[wrong_owner]),
        Err(IdentityError::AppServerUnavailable)
    );

    assert_eq!(
        validate_stable_pid_samples(&[42_100, 42_100, 42_100], 3),
        Ok(42_100)
    );
    assert_eq!(
        validate_stable_pid_samples(&[42_100, 42_100, 42_101], 3),
        Err(IdentityError::AppServerUnavailable)
    );
    assert_eq!(
        validate_stable_pid_samples(&[42_100, 42_100], 3),
        Err(IdentityError::AppServerUnavailable)
    );
}

#[test]
fn restart_requires_one_verified_ui_process_and_no_active_or_unsent_work() {
    let ready = RestartGuard {
        verified_ui_processes: 1,
        active_native_children: 0,
        setup_phase: SetupPhase::Committed,
    };
    authorize_restart(ready).expect("safe one-time restart");

    assert_eq!(
        authorize_restart(RestartGuard {
            active_native_children: 1,
            ..ready
        }),
        Err(IdentityError::ActiveNativeChild)
    );
    assert_eq!(
        authorize_restart(RestartGuard {
            setup_phase: SetupPhase::AwaitingVisibleCommand,
            ..ready
        }),
        Err(IdentityError::SetupPending)
    );
    assert_eq!(
        authorize_restart(RestartGuard {
            verified_ui_processes: 2,
            ..ready
        }),
        Err(IdentityError::AmbiguousUiProcess)
    );
}

#[test]
fn replacement_proof_requires_one_new_verified_ui_process() {
    let replacement = VerifiedProcess {
        pid: 42_000,
        owner_sid: "S-1-5-21-1000".into(),
        image_path: package().canonical_executable,
        package_version: "26.715.3651.0".into(),
    };
    assert_eq!(
        validate_replacement_set(41_000, 42_000, std::slice::from_ref(&replacement)),
        Ok(replacement.clone())
    );
    assert_eq!(
        validate_replacement_set(41_000, 41_000, std::slice::from_ref(&replacement)),
        Err(IdentityError::ReplacementIdentity)
    );
    assert_eq!(
        validate_replacement_set(41_000, 42_000, &[]),
        Err(IdentityError::AmbiguousUiProcess)
    );
    assert_eq!(
        validate_replacement_set(41_000, 42_000, &[replacement.clone(), replacement]),
        Err(IdentityError::AmbiguousUiProcess)
    );
}

#[test]
fn cdp_launch_is_random_loopback_only_and_never_shell_encoded() {
    let reservation = reserve_loopback_port().expect("ephemeral loopback reservation");
    assert_eq!(reservation.address().to_string(), "127.0.0.1");
    assert_ne!(reservation.port(), 0);
    let args = cdp_launch_arguments(reservation.port()).expect("launch args");
    assert_eq!(
        args,
        [
            "--remote-debugging-address=127.0.0.1".to_owned(),
            format!("--remote-debugging-port={}", reservation.port()),
        ]
    );
    assert!(cdp_launch_arguments(0).is_err());
    assert!(args
        .iter()
        .all(|arg| !arg.contains('&') && !arg.contains('|')));
}

#[test]
fn store_activation_uses_the_official_aumid_and_one_bounded_argument_string() {
    assert_eq!(CODEX_APP_USER_MODEL_ID, "OpenAI.Codex_2p2nqsd0c76g0!App");
    assert_eq!(
        store_activation_arguments(41_237).expect("activation arguments"),
        "--remote-debugging-address=127.0.0.1 --remote-debugging-port=41237"
    );
    assert_eq!(
        store_activation_arguments(0),
        Err(IdentityError::InvalidPort)
    );
}

#[test]
fn package_query_accepts_one_exact_current_user_store_record_only() {
    let one = r#"[{"Name":"OpenAI.Codex","PackageFamilyName":"OpenAI.Codex_2p2nqsd0c76g0","Version":"26.715.3651.0","InstallLocation":"C:\\Program Files\\WindowsApps\\OpenAI.Codex_26.715.3651.0_x64__2p2nqsd0c76g0","SignatureKind":"Store"}]"#;
    let parsed = parse_package_query(one).expect("one official package record");
    assert_eq!(parsed.package_family, CODEX_PACKAGE_FAMILY);
    assert_eq!(parsed.version, "26.715.3651.0");

    let duplicate = format!("[{},{}]", &one[1..one.len() - 1], &one[1..one.len() - 1]);
    assert_eq!(
        parse_package_query(&duplicate),
        Err(IdentityError::AmbiguousPackage)
    );
    let unpackaged = one.replace("\"Store\"", "\"Developer\"");
    assert_eq!(
        parse_package_query(&unpackaged),
        Err(IdentityError::Signature)
    );
    assert_eq!(
        parse_package_query("[]"),
        Err(IdentityError::PackageMissing)
    );
}

#[cfg(windows)]
#[test]
fn windows_api_probes_current_process_owner_image_and_listener_pid() {
    let current = query_process_identity(std::process::id()).expect("current process identity");
    assert_eq!(current.pid, std::process::id());
    assert!(current.canonical_image_path.is_absolute());
    assert!(current.owner_sid.starts_with("S-1-"));

    let reservation = reserve_loopback_port().expect("owned listener");
    let listener = query_tcp_listener(reservation.port()).expect("TCP owner table listener");
    assert_eq!(listener.address.to_string(), "127.0.0.1");
    assert_eq!(listener.port, reservation.port());
    assert_eq!(listener.pid, std::process::id());
}

#[cfg(windows)]
#[test]
#[ignore = "requires the Microsoft Store Codex package"]
fn discovers_the_real_installed_store_package_without_mutating_it() {
    let package = discover_store_package().expect("installed official Codex package");
    assert_eq!(package.version.split('.').count(), 4);
    assert!(package.root.is_absolute());
    assert_eq!(
        package
            .executable
            .file_name()
            .and_then(|name| name.to_str()),
        Some("ChatGPT.exe")
    );
}

fn process(image_path: PathBuf) -> ProcessProbe {
    ProcessProbe {
        pid: 41_000,
        owner_sid: "S-1-5-21-1000".into(),
        canonical_image_path: image_path,
        package_family: CODEX_PACKAGE_FAMILY.into(),
        signature: SignatureStatus::TrustedStore,
    }
}

#[test]
fn exact_store_package_process_owner_signature_and_listener_are_required() {
    let verified_package = verify_package(package()).expect("official Store package");
    let verified_process = verify_process(
        &verified_package,
        process(verified_package.executable.clone()),
        "S-1-5-21-1000",
    )
    .expect("same-user official process");
    let listener = verify_listener(
        &verified_process,
        ListenerProbe {
            address: "127.0.0.1".parse().unwrap(),
            port: 49_321,
            pid: verified_process.pid,
        },
        49_321,
    )
    .expect("owned loopback listener");

    assert_eq!(listener.pid, verified_process.pid);
    assert_eq!(listener.port, 49_321);
}

#[test]
fn similarly_named_or_out_of_root_executables_fail_closed() {
    let mut wrong_family = package();
    wrong_family.package_family = "OpenAI.Codex.evil_2p2nqsd0c76g0".into();
    assert_eq!(
        verify_package(wrong_family),
        Err(IdentityError::PackageFamily)
    );

    let mut escaped = package();
    escaped.canonical_executable = PathBuf::from(r"C:\Temp\ChatGPT.exe");
    assert_eq!(
        verify_package(escaped),
        Err(IdentityError::ExecutableOutsidePackage)
    );

    let mut wrong_basename = package();
    wrong_basename.canonical_executable =
        wrong_basename.canonical_root.join("app").join("Codex.exe");
    assert_eq!(
        verify_package(wrong_basename),
        Err(IdentityError::ExecutableName)
    );
}

#[test]
fn untrusted_wrong_user_or_mismatched_listener_is_rejected() {
    let verified_package = verify_package(package()).unwrap();
    let mut untrusted = process(verified_package.executable.clone());
    untrusted.signature = SignatureStatus::Invalid;
    assert_eq!(
        verify_process(&verified_package, untrusted, "S-1-5-21-1000"),
        Err(IdentityError::Signature)
    );

    let mut wrong_package = process(verified_package.executable.clone());
    wrong_package.package_family = "OpenAI.Codex.evil_2p2nqsd0c76g0".into();
    assert_eq!(
        verify_process(&verified_package, wrong_package, "S-1-5-21-1000"),
        Err(IdentityError::ProcessPackage)
    );

    let wrong_user = process(verified_package.executable.clone());
    assert_eq!(
        verify_process(&verified_package, wrong_user, "S-1-5-21-2000"),
        Err(IdentityError::ProcessOwner)
    );

    let verified_process = verify_process(
        &verified_package,
        process(verified_package.executable.clone()),
        "S-1-5-21-1000",
    )
    .unwrap();
    assert_eq!(
        verify_listener(
            &verified_process,
            ListenerProbe {
                address: "127.0.0.1".parse().unwrap(),
                port: 49_321,
                pid: 99,
            },
            49_321,
        ),
        Err(IdentityError::ListenerOwner)
    );
    assert_eq!(
        verify_listener(
            &verified_process,
            ListenerProbe {
                address: "0.0.0.0".parse().unwrap(),
                port: 49_321,
                pid: verified_process.pid,
            },
            49_321,
        ),
        Err(IdentityError::ListenerAddress)
    );
}
