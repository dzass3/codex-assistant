use std::{fs, path::PathBuf};

use codex_assistant_lib::control_layer::injector::{
    build_control_source, parse_binding_message, preflight_insertion_expression,
    routing_enabled_expression, routing_ready_expression, BindingError, ControlBootstrap,
    ControlEvent, SubmitShortcut,
};
use serde_json::Value;

fn resource(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("control")
        .join(path)
}

#[test]
fn preflight_evaluation_accepts_only_the_fixed_visible_directive_grammar() {
    let attempt = "6e90c53a-b93e-44d7-aeb8-9880ee199388";
    let directive = format!(
        "Codex Assistant preflight {attempt}: create exactly one visible native child from the current root using profile codex_assistant_luna with fork_turns=\"none\". The child performs no user work and reports only native availability."
    );
    let expression = preflight_insertion_expression(&directive).expect("fixed directive");
    assert!(expression.contains("insertPreflightDirective"));
    assert!(expression.contains(attempt));
    assert!(!expression.contains("eval("));
    assert!(
        preflight_insertion_expression(&format!("{directive}\nfetch('https://example.com')"))
            .is_err()
    );
    assert!(
        preflight_insertion_expression(&directive.replace("codex_assistant_luna", "shell"))
            .is_err()
    );
}

#[test]
fn routing_ready_activation_uses_only_the_fixed_namespaced_boolean_call() {
    assert_eq!(
        routing_ready_expression(true),
        "globalThis.__codexAssistantControlV1?.setRoutingReady(true) === true"
    );
    assert_eq!(
        routing_ready_expression(false),
        "globalThis.__codexAssistantControlV1?.setRoutingReady(false) === true"
    );
    assert_eq!(
        routing_enabled_expression(true),
        "globalThis.__codexAssistantControlV1?.syncEnabled(true) === true"
    );
    assert_eq!(
        routing_enabled_expression(false),
        "globalThis.__codexAssistantControlV1?.syncEnabled(false) === true"
    );
}

#[test]
fn injected_control_is_local_namespaced_and_forbids_content_exfiltration_apis() {
    let script = fs::read_to_string(resource("routing-control.js")).expect("routing control JS");
    let stylesheet =
        fs::read_to_string(resource("routing-control.css")).expect("routing control CSS");
    for required in [
        "__codexAssistantControlV1",
        "__codexAssistantBootstrapV1",
        "codexAssistant",
        "data-codex-assistant-control",
        "data-codex-assistant-root",
        "main.main-surface",
        "aside.app-shell-left-panel",
        "[data-codex-composer-root]",
        "[data-codex-composer=\"true\"]",
        ".ProseMirror",
        "document.execCommand(\"insertText\"",
        "[Codex Assistant Routing v1; route=",
        "insertPreflightDirective",
        "setRoutingReady",
        "syncEnabled",
    ] {
        assert!(
            script.contains(required),
            "missing control contract: {required}"
        );
    }
    for forbidden in [
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "http://",
        "https://",
        "eval(",
        "localStorage",
        "sessionStorage",
        "navigator.clipboard",
        "innerHTML",
        "outerHTML",
        "innerText",
        ".textContent",
        "console.",
        "postMessage(",
    ] {
        assert!(
            !script.contains(forbidden),
            "forbidden control API: {forbidden}"
        );
    }
    assert!(script.len() <= 64 * 1024);
    assert!(stylesheet.len() <= 16 * 1024);
    assert!(!stylesheet.contains("html "));
    assert!(!stylesheet.contains("body "));
    assert!(!stylesheet.contains(".ProseMirror"));
    for selector in stylesheet
        .split('}')
        .filter_map(|rule| rule.split_once('{').map(|(selector, _)| selector.trim()))
        .filter(|selector| !selector.is_empty() && !selector.starts_with('@'))
    {
        assert!(
            selector
                .split(',')
                .all(|part| part.trim().starts_with("[data-codex-assistant-root]")),
            "unscoped selector: {selector}"
        );
    }
}

#[test]
fn tauri_bundles_only_runtime_control_assets_not_test_fixtures() {
    let config: Value = serde_json::from_str(
        &fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"))
            .expect("Tauri config"),
    )
    .expect("valid Tauri JSON");
    let resources = config["bundle"]["resources"]
        .as_array()
        .expect("bundle resources")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(resources.contains(&"resources/control/routing-control.js"));
    assert!(resources.contains(&"resources/control/routing-control.css"));
    assert!(resources.iter().all(|path| !path.contains("fixtures")));
}

#[test]
fn rust_builds_owned_bootstrap_and_accepts_only_exact_current_target_messages() {
    let route_id = "7d47a800-c734-4f9a-a56c-55d875ea1cab";
    let route_key = "6e90c53a-b93e-44d7-aeb8-9880ee199388";
    let bootstrap = ControlBootstrap {
        session_id: "session-1".into(),
        target_id: "target-1".into(),
        route_id: route_id.into(),
        route_key: route_key.into(),
        observed: true,
        parent_thread_id: None,
        submit_shortcut: SubmitShortcut::Enter,
    };
    let script = fs::read_to_string(resource("routing-control.js")).unwrap();
    let css = fs::read_to_string(resource("routing-control.css")).unwrap();
    let source = build_control_source(&script, &css, &bootstrap).expect("owned injection source");
    assert!(source.contains("__codexAssistantBootstrapV1"));
    assert!(source.contains(route_id));
    assert!(source.contains(route_key));
    for forbidden in ["prompt", "response", "reasoning", "tool_output"] {
        assert!(!source.contains(forbidden));
    }

    let toggle = format!(
        r#"{{"v":1,"sessionId":"session-1","targetId":"target-1","type":"toggle","routeId":"{route_id}","enabled":true}}"#
    );
    assert_eq!(
        parse_binding_message(&toggle, "session-1", "target-1"),
        Ok(ControlEvent::Toggle {
            route_id: route_id.parse().unwrap(),
            enabled: true,
        })
    );
    let with_prompt = toggle.replace("}", ",\"prompt\":\"PRIVATE\"}");
    assert_eq!(
        parse_binding_message(&with_prompt, "session-1", "target-1"),
        Err(BindingError::Malformed)
    );
    assert_eq!(
        parse_binding_message(&toggle, "other-session", "target-1"),
        Err(BindingError::WrongSession)
    );
    assert_eq!(
        parse_binding_message(&toggle, "session-1", "other-target"),
        Err(BindingError::WrongTarget)
    );
}
