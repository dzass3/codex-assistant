use codex_assistant_lib::theme::{
    apply_theme_on_pages, bundled_theme_packs, theme_application_source, theme_restore_source,
    validate_theme_pack, RightsStatus, ThemeBackdrop, ThemeCategory, ThemeEngineError,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[test]
fn bundled_themes_are_declarative_project_owned_and_pass_the_rights_gate() {
    let packs = bundled_theme_packs();
    assert_eq!(
        packs
            .iter()
            .map(|pack| pack.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "aurora-grid",
            "observatory-muse",
            "gothic-horizon",
            "roseglass-atelier",
            "blush-circuit",
            "fortune-foundry",
            "crimson-relay",
            "crystal-daylight",
            "pocket-cosmos",
            "violet-afterdark",
            "cyan-chorus",
            "noir-stage",
        ]
    );
    assert!(packs
        .iter()
        .any(|pack| pack.category == ThemeCategory::Abstract));
    assert!(packs
        .iter()
        .any(|pack| pack.category == ThemeCategory::OriginalCharacter));
    for pack in packs {
        validate_theme_pack(&pack, true).expect("bundled theme rights gate");
        assert_eq!(pack.rights.status, RightsStatus::Verified);
        assert!(pack.rights.commercial_redistribution);
        assert!(!pack.rights.attribution.is_empty());
        assert!(pack.assets.iter().all(|asset| {
            asset.sha256.len() == 64
                && asset
                    .sha256
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        }));
        let serialized = serde_json::to_string(&pack).unwrap();
        for forbidden in [
            "<script",
            "javascript:",
            "http://",
            "https://",
            "powershell",
            "Arina Hashimoto",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "forbidden pack content: {forbidden}"
            );
        }
    }
}

#[test]
fn theme_engine_generates_only_owned_dom_presentation_and_exact_restore() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Gradient { .. }))
        .unwrap();
    let source = theme_application_source(&pack).expect("validated engine source");
    for required in [
        "__codexAssistantThemeV1",
        "data-codex-assistant-theme",
        "main.main-surface",
        "aside.app-shell-left-panel",
        "prefers-reduced-motion",
    ] {
        assert!(
            source.contains(required),
            "missing engine contract: {required}"
        );
    }
    for forbidden in [
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "eval(",
        "localStorage",
        "sessionStorage",
        "navigator.clipboard",
        "innerHTML",
        "innerText",
        ".textContent",
        "http://",
        "https://",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden engine API: {forbidden}"
        );
    }
    let restore = theme_restore_source();
    assert!(restore.contains("__codexAssistantThemeV1"));
    assert!(restore.contains("remove"));
    assert!(!restore.contains("querySelectorAll('*')"));
}

#[test]
fn bundled_gate_rejects_local_only_or_unreviewed_redistribution() {
    let mut pack = bundled_theme_packs().remove(0);
    pack.rights.status = RightsStatus::LocalOnly;
    assert!(validate_theme_pack(&pack, true).is_err());
    assert!(validate_theme_pack(&pack, false).is_ok());
    pack.rights.commercial_redistribution = false;
    assert!(validate_theme_pack(&pack, true).is_err());
}

#[tokio::test]
async fn theme_apply_uses_verified_page_targets_and_boolean_compatibility_acknowledgement() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"page-1","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/page-1"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let compatibility = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let compatibility: serde_json::Value = serde_json::from_str(&compatibility).unwrap();
        assert_eq!(compatibility["method"], "Runtime.evaluate");
        socket
            .send(Message::Text(
                format!(
                    r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                    compatibility["id"]
                )
                .into(),
            ))
            .await
            .unwrap();
        drop(socket);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..4 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            match index {
                0 => assert_eq!(call["method"], "Page.enable"),
                1 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    assert!(call["params"]["source"]
                        .as_str()
                        .unwrap()
                        .contains("data-codex-assistant-theme"));
                }
                2 => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    socket
                        .send(Message::Text(
                            format!(
                                r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                                call["id"]
                            )
                            .into(),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    assert!(call["params"]["expression"]
                        .as_str()
                        .unwrap()
                        .contains("getComputedStyle"));
                    socket
                        .send(Message::Text(
                            format!(
                                r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                                call["id"]
                            )
                            .into(),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
            }
            let result = if index == 1 {
                r#"{"identifier":"theme-script-1"}"#
            } else {
                "{}"
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);

    let result = apply_theme_on_pages(&endpoint, &pack, &[], 1_000)
        .await
        .unwrap();
    assert_eq!(result.applied_pages, 1);
    assert_eq!(result.scripts.len(), 1);
    assert_eq!(result.scripts[0].target_id, "page-1");
    assert_eq!(result.scripts[0].identifier, "theme-script-1");
    server.await.unwrap();
}

#[tokio::test]
async fn incompatible_utility_page_does_not_block_main_task_theme() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"main-task","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-task"}},{{"id":"utility","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/utility"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);

        for compatible in [true, false] {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let request = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["method"], "Runtime.evaluate");
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":{compatible}}}}}}}"#,
                        request["id"]
                    )
                    .into(),
                ))
                .await
                .unwrap();
        }

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..4 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            let result = match index {
                0 => {
                    assert_eq!(call["method"], "Page.enable");
                    "{}"
                }
                1 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    r#"{"identifier":"theme-script-main"}"#
                }
                2 => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    assert!(call["params"]["expression"]
                        .as_str()
                        .unwrap()
                        .contains("getComputedStyle"));
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);

    let result = apply_theme_on_pages(&endpoint, &pack, &[], 1_000)
        .await
        .unwrap();

    assert_eq!(result.applied_pages, 1);
    assert_eq!(result.scripts.len(), 1);
    assert_eq!(result.scripts[0].target_id, "main-task");
    server.await.unwrap();
}

#[tokio::test]
async fn theme_is_not_applied_until_computed_style_is_verified() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = codex_assistant_lib::control_layer::cdp::browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"main-task","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/main-task"}}]"#
        );
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        drop(stream);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let compatibility = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let compatibility: serde_json::Value = serde_json::from_str(&compatibility).unwrap();
        socket
            .send(Message::Text(
                format!(
                    r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
                    compatibility["id"]
                )
                .into(),
            ))
            .await
            .unwrap();
        drop(socket);

        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        for index in 0..6 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            let result = match index {
                0 => {
                    assert_eq!(call["method"], "Page.enable");
                    "{}"
                }
                1 => {
                    assert_eq!(call["method"], "Page.addScriptToEvaluateOnNewDocument");
                    r#"{"identifier":"theme-script-main"}"#
                }
                2 => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
                3 => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    assert!(call["params"]["expression"]
                        .as_str()
                        .unwrap()
                        .contains("getComputedStyle"));
                    r#"{"result":{"type":"boolean","value":false}}"#
                }
                4 => {
                    assert_eq!(call["method"], "Page.removeScriptToEvaluateOnNewDocument");
                    assert_eq!(call["params"]["identifier"], "theme-script-main");
                    "{}"
                }
                _ => {
                    assert_eq!(call["method"], "Runtime.evaluate");
                    r#"{"result":{"type":"boolean","value":true}}"#
                }
            };
            socket
                .send(Message::Text(
                    format!(r#"{{"id":{},"result":{result}}}"#, call["id"]).into(),
                ))
                .await
                .unwrap();
        }
    });
    let pack = bundled_theme_packs().remove(0);

    let result = apply_theme_on_pages(&endpoint, &pack, &[], 1_000).await;

    assert!(matches!(result, Err(ThemeEngineError::PartialApplication)));
    server.await.unwrap();
}
