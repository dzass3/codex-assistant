use toml::Value;

const SPARK: &str = include_str!("../resources/routing/agents/spark.toml");
const LUNA: &str = include_str!("../resources/routing/agents/luna.toml");
const TERRA: &str = include_str!("../resources/routing/agents/terra.toml");
const SOL: &str = include_str!("../resources/routing/agents/sol.toml");
const SKILL: &str = include_str!("../resources/routing/skill/SKILL.md");
const POLICY: &str = include_str!("../resources/routing/skill/references/policy.md");
const TAURI_CONF: &str = include_str!("../tauri.conf.json");

#[test]
fn bundled_profiles_follow_current_native_custom_agent_schema_and_exact_models() {
    for (source, name, model) in [
        (SPARK, "codex_assistant_spark", "gpt-5.3-codex-spark"),
        (LUNA, "codex_assistant_luna", "gpt-5.6-luna"),
        (TERRA, "codex_assistant_terra", "gpt-5.6-terra"),
        (SOL, "codex_assistant_sol", "gpt-5.6-sol"),
    ] {
        let profile: Value = toml::from_str(source).expect("valid profile TOML");
        assert_eq!(profile["name"].as_str(), Some(name));
        assert_eq!(profile["model"].as_str(), Some(model));
        assert!(profile["description"].as_str().is_some());
        assert!(profile["developer_instructions"].as_str().is_some());
        assert!(!source.contains("codex exec"));
    }
}

#[test]
fn routing_skill_requires_native_eligibility_quality_gates_and_metadata_privacy() {
    assert!(SKILL.starts_with("---\n"));
    for required in [
        "eligibility",
        "native",
        "three routed children",
        "depth-two",
        "two automatic escalations",
        "self-verifies",
        "high-risk or complex code-changing",
        "fork_turns=\"none\"",
        "Never send prompts",
    ] {
        assert!(SKILL.contains(required), "missing {required}");
    }
    assert!(!SKILL.contains("Any code-changing result receives an independent"));
    assert!(SPARK.contains("must not delegate"));
    assert!(LUNA.contains("must not delegate"));
    assert!(TERRA.contains("one lower-tier native child"));
    assert!(POLICY.contains("Requested/effective-model drift is unavailable"));
    assert!(!SKILL.contains("routing-mcp --"));
}

#[test]
fn tauri_bundles_every_native_routing_resource() {
    let config: serde_json::Value = serde_json::from_str(TAURI_CONF).expect("valid Tauri JSON");
    let resources = config["bundle"]["resources"]
        .as_array()
        .expect("routing resources bundled");
    assert!(resources
        .iter()
        .any(|entry| entry.as_str() == Some("resources/routing/**/*")));
}
