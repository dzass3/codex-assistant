use codex_assistant_lib::theme_state::migrate_theme_state;
use tempfile::tempdir;

#[test]
fn migration_moves_only_known_theme_state_and_preserves_unrelated_legacy_files() {
    let root = tempdir().unwrap();
    let legacy = root.path().join("state").join("routing");
    let themes = root.path().join("state").join("themes");
    let codex_config = root.path().join("codex").join("config.toml");
    let global_skill = root
        .path()
        .join("skills")
        .join("user-skill")
        .join("SKILL.md");

    std::fs::create_dir_all(legacy.join("local-themes").join("my-theme")).unwrap();
    std::fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
    std::fs::create_dir_all(global_skill.parent().unwrap()).unwrap();
    std::fs::write(
        legacy.join("theme-state.json"),
        br#"{"selected_theme_id":"my-theme"}"#,
    )
    .unwrap();
    std::fs::write(legacy.join("control-session.json"), b"session").unwrap();
    std::fs::write(
        legacy
            .join("local-themes")
            .join("my-theme")
            .join("theme.json"),
        b"theme",
    )
    .unwrap();
    std::fs::write(legacy.join("routing-state.json"), b"routing").unwrap();
    std::fs::write(legacy.join("routing-mcp.lock"), b"lock").unwrap();
    std::fs::write(legacy.join("unrelated.txt"), b"keep me").unwrap();
    std::fs::write(&codex_config, b"user config").unwrap();
    std::fs::write(&global_skill, b"user skill").unwrap();

    migrate_theme_state(&legacy, &themes).unwrap();

    assert_eq!(
        std::fs::read(themes.join("theme-state.json")).unwrap(),
        br#"{"selected_theme_id":"my-theme"}"#
    );
    assert_eq!(
        std::fs::read(themes.join("control-session.json")).unwrap(),
        b"session"
    );
    assert_eq!(
        std::fs::read(
            themes
                .join("local-themes")
                .join("my-theme")
                .join("theme.json")
        )
        .unwrap(),
        b"theme"
    );
    assert_eq!(
        std::fs::read(legacy.join("routing-state.json")).unwrap(),
        b"routing"
    );
    assert_eq!(
        std::fs::read(legacy.join("routing-mcp.lock")).unwrap(),
        b"lock"
    );
    assert_eq!(
        std::fs::read(legacy.join("unrelated.txt")).unwrap(),
        b"keep me"
    );
    assert_eq!(std::fs::read(codex_config).unwrap(), b"user config");
    assert_eq!(std::fs::read(global_skill).unwrap(), b"user skill");
}
