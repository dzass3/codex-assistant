use codex_assistant_lib::theme_environment::{
    classify_environment, ThemeEnvironmentProbe, ThemeEnvironmentStatus, ThemeNextAction,
};

fn probe() -> ThemeEnvironmentProbe {
    ThemeEnvironmentProbe {
        platform_supported: true,
        os_build: Some(19_045),
        architecture: "x64".into(),
        package_version: Some("26.715.8383.0".into()),
        verified_process_count: 1,
        session_reachable: true,
        selected_theme_id: Some("aurora-grid".into()),
    }
}

#[test]
fn reports_contract_two_without_a_launcher_requirement() {
    let report = classify_environment(probe());
    assert_eq!(report.contract_version, 2);
    assert_eq!(report.status, ThemeEnvironmentStatus::Ready);
    assert_eq!(report.next_action, ThemeNextAction::ApplyNow);
    assert!(report.can_apply_now);
    assert_eq!(report.checks.len(), 7);
}

#[test]
fn windows_and_architecture_matrix_fail_closed() {
    for (build, architecture) in [
        (Some(19_044), "x64"),
        (Some(22_621), "x86"),
        (None, "arm64"),
    ] {
        let report = classify_environment(ThemeEnvironmentProbe {
            os_build: build,
            architecture: architecture.into(),
            ..probe()
        });
        assert_eq!(report.status, ThemeEnvironmentStatus::Unsupported);
        assert_eq!(report.next_action, ThemeNextAction::UseSupportedWindows);
        assert!(!report.can_apply_now);
    }
    for (build, architecture) in [(19_045, "x64"), (22_621, "arm64")] {
        let report = classify_environment(ThemeEnvironmentProbe {
            os_build: Some(build),
            architecture: architecture.into(),
            ..probe()
        });
        assert_eq!(report.status, ThemeEnvironmentStatus::Ready);
    }
}

#[test]
fn unknown_official_build_is_actionable_and_never_applyable() {
    let report = classify_environment(ThemeEnvironmentProbe {
        package_version: Some("27.1.0.0".into()),
        ..probe()
    });
    assert_eq!(report.status, ThemeEnvironmentStatus::Unsupported);
    assert_eq!(report.next_action, ThemeNextAction::UpdateAssistant);
    assert_eq!(report.codex_version.as_deref(), Some("27.1.0.0"));
    assert!(!report.can_apply_now);
}

#[test]
fn stale_session_requires_explicit_restart_confirmation() {
    let report = classify_environment(ThemeEnvironmentProbe {
        session_reachable: false,
        ..probe()
    });
    assert_eq!(report.status, ThemeEnvironmentStatus::RestartRequired);
    assert_eq!(report.next_action, ThemeNextAction::ConfirmRestart);
    assert!(!report.can_apply_now);
}

#[test]
fn stopped_official_app_can_be_launched_only_by_the_current_user_action() {
    let report = classify_environment(ThemeEnvironmentProbe {
        verified_process_count: 0,
        session_reachable: false,
        ..probe()
    });
    assert_eq!(report.status, ThemeEnvironmentStatus::CodexNotRunning);
    assert_eq!(report.next_action, ThemeNextAction::LaunchCodexForTheme);
    assert!(!report.can_apply_now);
}

#[test]
fn unsupported_and_ambiguous_environments_fail_closed() {
    let missing = classify_environment(ThemeEnvironmentProbe {
        package_version: None,
        verified_process_count: 0,
        session_reachable: false,
        selected_theme_id: None,
        ..probe()
    });
    assert_eq!(missing.status, ThemeEnvironmentStatus::Unsupported);
    assert_eq!(missing.next_action, ThemeNextAction::InstallCodex);

    let ambiguous = classify_environment(ThemeEnvironmentProbe {
        verified_process_count: 2,
        session_reachable: false,
        ..probe()
    });
    assert_eq!(ambiguous.status, ThemeEnvironmentStatus::Unsupported);
    assert_eq!(ambiguous.next_action, ThemeNextAction::CloseExtraWindows);
}
