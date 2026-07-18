use std::fs;

use codex_assistant_lib::codex_config::{CodexConfigService, FailurePoint, InstallRequest};
use tempfile::tempdir;

#[test]
fn first_install_creates_only_owned_assets_and_preserves_unrelated_config() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    fs::create_dir_all(&codex_home).expect("codex home");
    fs::write(
        codex_home.join("config.toml"),
        b"# keep this comment\r\n[agents]\r\nmax_threads = 9\r\n\r\n[mcp_servers.other]\r\ncommand = \"other\"\r\n",
    )
    .expect("fixture config");
    let executable = std::env::current_exe().expect("current test executable");

    let service = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        skills_root,
        executable,
    ))
    .expect("injected service");
    let receipt = service.install().expect("install");

    assert!(receipt.changed);
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("merged config");
    assert!(config.contains("# keep this comment\r\n"));
    assert!(config.contains("max_threads = 9"));
    assert!(config.contains("[mcp_servers.other]"));
    assert!(config.contains("[agents.codex_assistant_spark]"));
    assert!(codex_home
        .join("agents/codex-assistant/spark.toml")
        .is_file());
    assert!(codex_home
        .join("agents/codex-assistant/manifest.json")
        .is_file());
}

#[test]
fn reinstall_is_idempotent_and_keeps_the_same_owned_bytes() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    let executable = std::env::current_exe().expect("current test executable");
    let request = InstallRequest::new(codex_home.clone(), skills_root.clone(), executable);
    CodexConfigService::new(request.clone())
        .expect("service")
        .install()
        .expect("first install");
    let config_before = fs::read(codex_home.join("config.toml")).expect("config bytes");
    let manifest_before =
        fs::read(codex_home.join("agents/codex-assistant/manifest.json")).expect("manifest bytes");

    let receipt = CodexConfigService::new(request)
        .expect("service")
        .install()
        .expect("second install");

    assert!(!receipt.changed);
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("config bytes"),
        config_before
    );
    assert_eq!(
        fs::read(codex_home.join("agents/codex-assistant/manifest.json")).expect("manifest bytes"),
        manifest_before
    );
}

#[test]
fn idempotent_reinstall_keeps_the_first_preimage_available_for_restore() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    let agent_root = codex_home.join("agents/codex-assistant");
    fs::create_dir_all(&agent_root).expect("agent root");
    fs::write(
        codex_home.join("config.toml"),
        b"# original\r\n[agents]\r\nmax_depth = 1\r\n",
    )
    .expect("config preimage");
    fs::write(agent_root.join("spark.toml"), b"original spark profile").expect("asset preimage");
    let config_before = fs::read(codex_home.join("config.toml")).expect("config bytes");
    let spark_before = fs::read(agent_root.join("spark.toml")).expect("spark bytes");
    let service = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        skills_root,
        std::env::current_exe().expect("executable"),
    ))
    .expect("service");

    service.install().expect("first install");
    assert!(!service.install().expect("idempotent reinstall").changed);
    service.restore().expect("restore first preimages");

    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("restored config"),
        config_before
    );
    assert_eq!(
        fs::read(agent_root.join("spark.toml")).expect("restored spark"),
        spark_before
    );
}

#[test]
fn any_observed_transaction_failure_restores_exact_preoperation_bytes() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    fs::create_dir_all(&codex_home).expect("codex home");
    fs::write(
        codex_home.join("config.toml"),
        b"[agents]\nmax_threads = 7\n",
    )
    .expect("config");
    let executable = std::env::current_exe().expect("current test executable");
    let base = InstallRequest::new(codex_home.clone(), skills_root.clone(), executable);
    CodexConfigService::new(base.clone())
        .expect("service")
        .install()
        .expect("seed installation");
    let watched = [
        codex_home.join("config.toml"),
        codex_home.join("agents/codex-assistant/spark.toml"),
        skills_root.join("codex-assistant-smart-routing/SKILL.md"),
    ];
    for (index, point) in [
        FailurePoint::Backup,
        FailurePoint::Journal,
        FailurePoint::AssetStaging,
        FailurePoint::ConfigParse,
        FailurePoint::TempSync,
        FailurePoint::ReplaceAsset,
        FailurePoint::ReplaceConfig,
        FailurePoint::PostWriteValidation,
        FailurePoint::CommitWrite,
        FailurePoint::CommitMark,
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(&watched[1], format!("user drift before failure {index}"))
            .expect("force a non-idempotent transaction");
        let before = watched
            .iter()
            .map(|path| fs::read(path).expect("preimage"))
            .collect::<Vec<_>>();
        let request = base
            .clone()
            .with_operation_id(format!("failure-{index}"))
            .fail_at(point);
        assert!(
            CodexConfigService::new(request)
                .expect("service")
                .install()
                .is_err(),
            "{point:?}"
        );
        for (path, expected) in watched.iter().zip(&before) {
            assert_eq!(
                fs::read(path).expect("restored file"),
                *expected,
                "{point:?}: {}",
                path.file_name().unwrap().to_string_lossy()
            );
        }
        CodexConfigService::new(base.clone())
            .expect("recovery service")
            .inspect()
            .expect("recover any journal left before the next failure point");
    }
}

#[test]
fn interrupted_commit_evidence_keeps_a_valid_recoverable_journal() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("codex home");
    fs::write(
        codex_home.join("config.toml"),
        b"# original\n[agents]\nmax_threads = 6\n",
    )
    .expect("config preimage");
    let original = fs::read(codex_home.join("config.toml")).expect("config bytes");
    let base = InstallRequest::new(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("executable"),
    )
    .with_operation_id("commit-write");

    assert!(
        CodexConfigService::new(base.clone().fail_at(FailurePoint::CommitWrite))
            .expect("service")
            .install()
            .is_err()
    );
    let recovered = CodexConfigService::new(base)
        .expect("service")
        .inspect()
        .expect("valid journal remains recoverable");

    assert!(recovered.recovered_incomplete_operation);
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("restored config"),
        original
    );
}

#[test]
fn inspect_recovers_an_incomplete_journal_without_replaying_a_committed_install() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    let base = InstallRequest::new(
        codex_home.clone(),
        skills_root,
        std::env::current_exe().expect("executable"),
    );
    CodexConfigService::new(base.clone())
        .expect("service")
        .install()
        .expect("install");
    let expected = fs::read(codex_home.join("config.toml")).expect("installed config");
    fs::write(
        codex_home.join("agents/codex-assistant/spark.toml"),
        b"user drift before interruption",
    )
    .expect("force a non-idempotent transaction");
    assert!(CodexConfigService::new(
        base.with_operation_id("interrupted")
            .fail_at(FailurePoint::CommitMark)
    )
    .expect("service")
    .install()
    .is_err());

    let receipt = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        root.path().join("agents/skills"),
        std::env::current_exe().expect("executable"),
    ))
    .expect("service")
    .inspect()
    .expect("inspect");

    assert!(receipt.recovered_incomplete_operation);
    assert!(receipt.installed);
    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("config"),
        expected
    );
}

#[test]
fn restore_preserves_user_modified_owned_file_and_reports_only_a_relative_label() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("agents").join("skills");
    fs::create_dir_all(&codex_home).expect("home");
    fs::write(
        codex_home.join("config.toml"),
        "[agents]\nmax_threads = 4\n",
    )
    .expect("config");
    let service = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        skills_root,
        std::env::current_exe().expect("executable"),
    ))
    .expect("service");
    service.install().expect("install");
    let edited = codex_home.join("agents/codex-assistant/spark.toml");
    fs::write(&edited, "user-managed profile").expect("user edit");

    let receipt = service.restore().expect("restore");

    assert!(receipt.conflicts.contains(&"spark.toml".to_owned()));
    assert_eq!(
        fs::read_to_string(&edited).expect("edited profile"),
        "user-managed profile"
    );
    assert!(receipt
        .conflicts
        .iter()
        .all(|label| !label.contains(root.path().to_string_lossy().as_ref())));
    assert!(fs::read_to_string(codex_home.join("config.toml"))
        .expect("config")
        .contains("[agents]"));
    assert!(codex_home
        .join("agents/codex-assistant/manifest.json")
        .is_file());

    fs::remove_file(&edited).expect("resolve conflict by accepting the absent preimage");
    let resolved = service.restore().expect("finish restore");
    assert!(resolved.conflicts.is_empty());
    assert!(!codex_home
        .join("agents/codex-assistant/manifest.json")
        .exists());
}

#[test]
fn config_merge_keeps_max_threads_and_raises_only_lower_depth() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    fs::create_dir_all(&codex_home).expect("home");
    fs::write(
        codex_home.join("config.toml"),
        "# unrelated\r\n[agents]\r\nmax_depth = 1\r\nmax_threads = 11\r\n",
    )
    .expect("config");
    let service = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("executable"),
    ))
    .expect("service");
    service.install().expect("install");
    let rendered = fs::read_to_string(codex_home.join("config.toml")).expect("config");
    assert!(rendered.contains("max_depth = 2"));
    assert!(rendered.contains("max_threads = 11"));
    assert!(rendered.contains("\r\n"));
}

#[test]
fn malformed_wrong_type_relative_and_invalid_executable_inputs_fail_before_installing() {
    let root = tempdir().expect("temporary root");
    let executable = std::env::current_exe().expect("executable");
    let malformed_home = root.path().join("malformed");
    fs::create_dir_all(&malformed_home).expect("home");
    fs::write(malformed_home.join("config.toml"), "[agents\n").expect("fixture");
    assert!(CodexConfigService::new(InstallRequest::new(
        malformed_home.clone(),
        root.path().join("skills"),
        executable.clone()
    ))
    .expect("service")
    .install()
    .is_err());

    let wrong_type_home = root.path().join("wrong-type");
    fs::create_dir_all(&wrong_type_home).expect("home");
    fs::write(
        wrong_type_home.join("config.toml"),
        "[agents]\nmax_depth = \"two\"\n",
    )
    .expect("fixture");
    assert!(CodexConfigService::new(InstallRequest::new(
        wrong_type_home,
        root.path().join("skills-2"),
        executable.clone()
    ))
    .expect("service")
    .install()
    .is_err());

    assert!(CodexConfigService::new(InstallRequest::new(
        "relative".into(),
        root.path().join("skills-3"),
        executable.clone()
    ))
    .is_err());
    assert!(CodexConfigService::new(InstallRequest::new(
        root.path().join("home"),
        root.path().join("skills-4"),
        root.path().join("missing.exe")
    ))
    .is_err());
    assert!(CodexConfigService::new(InstallRequest::new(
        root.path().join("home"),
        root.path().join("skills-5"),
        root.path().to_path_buf()
    ))
    .is_err());
}

#[test]
fn absent_max_threads_stays_absent_and_traversal_roots_are_rejected() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let executable = std::env::current_exe().expect("executable");
    CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        root.path().join("skills"),
        executable.clone(),
    ))
    .expect("service")
    .install()
    .expect("install");
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("config");
    assert!(!config.contains("max_threads"));

    let traversal = root.path().join("safe").join("..").join("escaped");
    assert!(CodexConfigService::new(InstallRequest::new(
        traversal,
        root.path().join("skills-two"),
        executable
    ))
    .is_err());
}

#[test]
fn restore_returns_config_and_owned_assets_to_their_exact_preinstall_preimages() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("skills");
    fs::create_dir_all(codex_home.join("agents/codex-assistant")).expect("agent root");
    fs::write(
        codex_home.join("config.toml"),
        b"# before\r\n[agents]\r\nmax_depth = 1\r\n",
    )
    .expect("config");
    fs::write(
        codex_home.join("agents/codex-assistant/spark.toml"),
        b"user preimage",
    )
    .expect("preexisting asset");
    let config_before = fs::read(codex_home.join("config.toml")).expect("config preimage");
    let spark_before =
        fs::read(codex_home.join("agents/codex-assistant/spark.toml")).expect("asset preimage");
    let service = CodexConfigService::new(InstallRequest::new(
        codex_home.clone(),
        skills_root.clone(),
        std::env::current_exe().expect("executable"),
    ))
    .expect("service");
    service.install().expect("install");

    service.restore().expect("restore");

    assert_eq!(
        fs::read(codex_home.join("config.toml")).expect("config"),
        config_before
    );
    assert_eq!(
        fs::read(codex_home.join("agents/codex-assistant/spark.toml")).expect("asset"),
        spark_before
    );
    assert!(!skills_root
        .join("codex-assistant-smart-routing/SKILL.md")
        .exists());
}

#[test]
fn failed_install_leaves_no_owned_operation_temp_files() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("skills");
    let service = CodexConfigService::new(
        InstallRequest::new(
            codex_home.clone(),
            skills_root.clone(),
            std::env::current_exe().expect("executable"),
        )
        .with_operation_id("no-temp")
        .fail_at(FailurePoint::AssetStaging),
    )
    .expect("service");
    assert!(service.install().is_err());
    for root in [codex_home, skills_root] {
        if root.exists() {
            assert!(walkdir_names(&root)
                .iter()
                .all(|name| !name.contains(".no-temp.tmp")));
        }
    }
}

#[test]
fn tampered_manifest_destinations_fail_closed_for_every_public_operation() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let skills_root = root.path().join("skills");
    let request = InstallRequest::new(
        codex_home.clone(),
        skills_root.clone(),
        std::env::current_exe().expect("executable"),
    );
    CodexConfigService::new(request.clone())
        .expect("service")
        .install()
        .expect("install");
    let manifest_path = codex_home.join("agents/codex-assistant/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest json");
    manifest["files"]["spark.toml"]["relative_destination"] =
        serde_json::Value::String("agent:../outside.txt".into());
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("tampered manifest"),
    )
    .expect("write tampered manifest");
    let outside = codex_home.join("agents/outside.txt");
    fs::write(&outside, b"must stay untouched").expect("outside sentinel");

    for operation in ["inspect", "install", "restore"] {
        let service = CodexConfigService::new(request.clone()).expect("service");
        let failed = match operation {
            "inspect" => service.inspect().is_err(),
            "install" => service.install().is_err(),
            "restore" => service.restore().is_err(),
            _ => unreachable!(),
        };
        assert!(failed, "{operation} accepted a path-escaping manifest");
        assert_eq!(
            fs::read(&outside).expect("outside sentinel"),
            b"must stay untouched"
        );
    }
}

#[test]
fn incomplete_ownership_manifest_fails_closed_instead_of_forgetting_owned_files() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let request = InstallRequest::new(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("executable"),
    );
    CodexConfigService::new(request.clone())
        .expect("service")
        .install()
        .expect("install");
    let manifest_path = codex_home.join("agents/codex-assistant/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest json");
    manifest["files"]
        .as_object_mut()
        .expect("manifest file map")
        .remove("spark.toml");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("incomplete manifest"),
    )
    .expect("write incomplete manifest");

    let service = CodexConfigService::new(request).expect("service");
    assert!(service.inspect().is_err());
    assert!(service.install().is_err());
    assert!(service.restore().is_err());
    assert!(codex_home
        .join("agents/codex-assistant/spark.toml")
        .is_file());
}

#[test]
fn owned_journal_directory_may_not_be_replaced_by_a_link_or_reparse_point() {
    let root = tempdir().expect("temporary root");
    let codex_home = root.path().join("codex");
    let request = InstallRequest::new(
        codex_home.clone(),
        root.path().join("skills"),
        std::env::current_exe().expect("executable"),
    );
    CodexConfigService::new(request.clone())
        .expect("service")
        .install()
        .expect("install");
    let journal = codex_home.join("codex-assistant-journal");
    let external = root.path().join("external-journal");
    fs::rename(&journal, &external).expect("move journal outside owned directory");
    create_dir_link(&external, &journal).expect("create journal directory link");

    assert!(CodexConfigService::new(request)
        .expect("service")
        .inspect()
        .is_err());
}

#[cfg(windows)]
fn create_dir_link(original: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    let output = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(original)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}

#[cfg(unix)]
fn create_dir_link(original: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

fn walkdir_names(root: &std::path::Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("directory") {
            let path = entry.expect("entry").path();
            names.push(path.file_name().unwrap().to_string_lossy().into_owned());
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    names
}
