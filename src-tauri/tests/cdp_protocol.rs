use codex_assistant_lib::control_layer::cdp::{
    browser_endpoint, CdpClient, CdpClientError, CdpProtocol, CdpProtocolError, IncomingMessage,
    MAX_CDP_FRAME_BYTES,
};
use codex_assistant_lib::control_layer::injector::{
    injection_plan, insert_preflight_directive_on_pages_detailed, receive_control_event,
    set_control_routing_ready, ControlEvent, InjectionPlanError, VisiblePreflightRequest,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[test]
fn request_ids_are_monotonic_and_only_owned_primitives_are_allowed() {
    let mut protocol = CdpProtocol::new();
    let runtime = protocol
        .request("Runtime.enable", json!({}))
        .expect("runtime request");
    let page = protocol
        .request("Page.enable", json!({}))
        .expect("page request");
    let binding = protocol
        .request("Runtime.addBinding", json!({"name":"codexAssistant"}))
        .expect("binding request");

    assert_eq!(runtime.id, 1);
    assert_eq!(page.id, 2);
    assert_eq!(binding.id, 3);
    assert!(!runtime.text.contains("prompt"));
    assert_eq!(
        protocol.request("Network.getAllCookies", json!({})),
        Err(CdpProtocolError::MethodNotAllowed)
    );
}

#[test]
fn responses_are_correlated_once_and_protocol_errors_are_sanitized() {
    let mut protocol = CdpProtocol::new();
    let request = protocol
        .request(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":"void 0"}),
        )
        .unwrap();
    assert_eq!(
        protocol.accept(&format!(
            r#"{{"id":{},"result":{{"identifier":"script-1"}}}}"#,
            request.id
        )),
        Ok(IncomingMessage::Response { id: request.id })
    );
    assert_eq!(
        protocol.accept(&format!(r#"{{"id":{},"result":{{}}}}"#, request.id)),
        Err(CdpProtocolError::UnknownResponseId)
    );

    let failed = protocol
        .request("Runtime.evaluate", json!({"expression":"void 0"}))
        .unwrap();
    assert_eq!(
        protocol.accept(&format!(
            r#"{{"id":{},"error":{{"code":-32000,"message":"PRIVATE PAGE DATA"}}}}"#,
            failed.id
        )),
        Err(CdpProtocolError::RemoteFailure)
    );
}

#[test]
fn boolean_evaluation_reads_only_the_exact_return_by_value_shape() {
    let mut protocol = CdpProtocol::new();
    let accepted = protocol
        .boolean_evaluation(json!({
            "expression":"globalThis.__codexAssistant.insertPreflightDirective('safe')",
            "returnByValue":true
        }))
        .unwrap();
    assert_eq!(
        protocol.accept(&format!(
            r#"{{"id":{},"result":{{"result":{{"type":"boolean","value":true}}}}}}"#,
            accepted.id
        )),
        Ok(IncomingMessage::BooleanResponse {
            id: accepted.id,
            value: true,
        })
    );

    for result in [
        r#"{"result":{"type":"string","value":"PRIVATE PAGE DATA"}}"#,
        r#"{"result":{"type":"boolean","value":true,"description":"PRIVATE"}}"#,
        r#"{"result":{"type":"boolean","value":true},"exceptionDetails":{}}"#,
    ] {
        let rejected = protocol
            .boolean_evaluation(json!({"expression":"void 0","returnByValue":true}))
            .unwrap();
        assert_eq!(
            protocol.accept(&format!(r#"{{"id":{},"result":{result}}}"#, rejected.id)),
            Err(CdpProtocolError::MalformedEnvelope)
        );
    }
}

#[test]
fn frame_size_unknown_events_and_malformed_envelopes_fail_closed() {
    let mut protocol = CdpProtocol::new();
    assert_eq!(
        protocol.accept(&"x".repeat(MAX_CDP_FRAME_BYTES + 1)),
        Err(CdpProtocolError::FrameTooLarge)
    );
    assert_eq!(
        protocol.accept(r#"{"method":"Network.dataReceived","params":{}}"#),
        Err(CdpProtocolError::EventNotAllowed)
    );
    assert_eq!(
        protocol.accept(r#"{"id":1,"result":{},"extra":"PRIVATE"}"#),
        Err(CdpProtocolError::MalformedEnvelope)
    );
}

#[test]
fn binding_events_accept_only_bounded_routing_metadata() {
    let mut protocol = CdpProtocol::new();
    let payload = r#"{"v":1,"sessionId":"session-1","targetId":"page-1","type":"toggle","routeId":"d2719d93-b823-4a7f-934f-23cbe01c8ab0","enabled":true}"#;
    assert_eq!(
        protocol.accept(&format!(
            r#"{{"method":"Runtime.bindingCalled","params":{{"name":"codexAssistant","payload":{},"executionContextId":7}}}}"#,
            serde_json::to_string(payload).unwrap()
        )),
        Ok(IncomingMessage::BindingCalled {
            payload: payload.into(),
        })
    );
    for rejected in [
        r#"{"method":"Runtime.bindingCalled","params":{"name":"other","payload":"{}","executionContextId":7}}"#,
        r#"{"method":"Runtime.bindingCalled","params":{"name":"codexAssistant","payload":"{\"prompt\":\"PRIVATE PAGE DATA\"}","executionContextId":7}}"#,
        r#"{"method":"Runtime.bindingCalled","params":{"name":"codexAssistant","payload":"{}","executionContextId":7,"private":"PRIVATE"}}"#,
    ] {
        assert_eq!(
            protocol.accept(rejected),
            Err(CdpProtocolError::MalformedEnvelope)
        );
    }
}

#[tokio::test]
async fn loopback_transport_serializes_calls_and_sanitizes_remote_failures() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let browser_id = "7d47a800-c734-4f9a-a56c-55d875ea1cab";
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/{browser_id}"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let first = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let first: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(first["id"], 1);
        assert_eq!(first["method"], "Runtime.enable");
        socket
            .send(Message::Text(
                r#"{"method":"Page.frameNavigated","params":{}}"#.into(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::Text(r#"{"id":1,"result":{}}"#.into()))
            .await
            .unwrap();
        let second = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let second: serde_json::Value = serde_json::from_str(&second).unwrap();
        assert_eq!(second["id"], 2);
        assert_eq!(second["method"], "Runtime.evaluate");
        socket
            .send(Message::Text(
                r#"{"id":2,"error":{"code":-32000,"message":"PRIVATE PAGE DATA"}}"#.into(),
            ))
            .await
            .unwrap();
    });

    let mut client = CdpClient::connect(&endpoint, 1_000).await.unwrap();
    client.call("Runtime.enable", json!({})).await.unwrap();
    let error = client
        .call("Runtime.evaluate", json!({"expression":"void 0"}))
        .await
        .unwrap_err();
    assert_eq!(error, CdpClientError::RemoteFailure);
    assert!(!format!("{error:?}").contains("PRIVATE"));
    server.await.unwrap();
}

#[tokio::test]
async fn verified_page_target_uses_the_same_bounded_sequential_transport() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let target = endpoint
        .verify_target(codex_assistant_lib::control_layer::cdp::TargetDescriptor {
            target_id: "page-1".into(),
            target_type: "page".into(),
            websocket_url: format!("ws://127.0.0.1:{port}/devtools/page/page-1"),
        })
        .unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let call: serde_json::Value = serde_json::from_str(&call).unwrap();
        assert_eq!(call["method"], "Runtime.evaluate");
        socket
            .send(Message::Text(
                format!(r#"{{"id":{},"result":{{}}}}"#, call["id"]).into(),
            ))
            .await
            .unwrap();
    });

    let mut client = CdpClient::connect_target(&target, port, 1_000)
        .await
        .unwrap();
    client
        .call("Runtime.evaluate", json!({"expression":"void 0"}))
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn boolean_evaluation_returns_only_the_verified_boolean() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let call: serde_json::Value = serde_json::from_str(&call).unwrap();
        assert_eq!(call["method"], "Runtime.evaluate");
        assert_eq!(call["params"]["returnByValue"], true);
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
    });

    let mut client = CdpClient::connect(&endpoint, 1_000).await.unwrap();
    assert!(client
        .evaluate_boolean("globalThis.__codexAssistant.insertPreflightDirective('safe')")
        .await
        .unwrap());
    server.await.unwrap();
}

#[tokio::test]
async fn connected_client_waits_for_one_sanitized_binding_event() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let payload = r#"{"v":1,"sessionId":"session-1","targetId":"page-1","type":"toggle","routeId":"d2719d93-b823-4a7f-934f-23cbe01c8ab0","enabled":true}"#.to_owned();
    let expected = payload.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let call: serde_json::Value = serde_json::from_str(&call).unwrap();
        assert_eq!(call["method"], "Runtime.enable");
        socket
            .send(Message::Text(
                format!(r#"{{"id":{},"result":{{}}}}"#, call["id"]).into(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                format!(
                    r#"{{"method":"Runtime.bindingCalled","params":{{"name":"codexAssistant","payload":{},"executionContextId":7}}}}"#,
                    serde_json::to_string(&payload).unwrap()
                )
                .into(),
            ))
            .await
            .unwrap();
    });

    let mut client = CdpClient::connect(&endpoint, 1_000).await.unwrap();
    client.call("Runtime.enable", json!({})).await.unwrap();
    assert_eq!(client.next_binding_payload().await.unwrap(), expected);
    server.await.unwrap();
}

#[tokio::test]
async fn visible_preflight_injects_owned_control_then_reads_one_boolean_result() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let root_id = uuid::Uuid::parse_str("d2719d93-b823-4a7f-934f-23cbe01c8ab0").unwrap();
    let route_key = uuid::Uuid::parse_str("1f81b88f-d14b-4d35-a70b-daa08f671ada").unwrap();
    let attempt_id = uuid::Uuid::parse_str("40dbfd0b-6dc2-4513-a4b0-85e71c3e2ab8").unwrap();
    let directive = format!(
        "Codex Assistant preflight {attempt_id}: create exactly one visible native child from the current root using profile codex_assistant_terra with fork_turns=\"none\". The child performs no user work and reports only native availability."
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /json/list HTTP/1.1\r\n")
        );
        let body = format!(
            r#"[{{"id":"page-1","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/page-1","title":"PRIVATE","url":"codex://PRIVATE"}}]"#
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
        for index in 0..6 {
            let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let call: serde_json::Value = serde_json::from_str(&call).unwrap();
            if index == 3 {
                assert!(call["params"]["source"]
                    .as_str()
                    .unwrap()
                    .contains("routing-control-test"));
            }
            if index == 5 {
                assert!(call["params"]["expression"]
                    .as_str()
                    .unwrap()
                    .contains("insertPreflightDirective"));
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
            } else {
                socket
                    .send(Message::Text(
                        format!(r#"{{"id":{},"result":{{}}}}"#, call["id"]).into(),
                    ))
                    .await
                    .unwrap();
            }
        }
    });

    let binding = insert_preflight_directive_on_pages_detailed(
        &endpoint,
        "routing-control-test",
        ".routing-control-test{}",
        &VisiblePreflightRequest {
            session_id: "session-1".into(),
            root_conversation_id: root_id,
            route_key,
            directive,
        },
        1_000,
    )
    .await
    .unwrap()
    .expect("compatible root target");

    assert_eq!(binding.target_id, "page-1");
    assert_eq!(binding.session_id, "session-1");
    assert_eq!(binding.root_conversation_id, root_id);
    assert_eq!(binding.route_key, route_key);
    server.await.unwrap();
}

#[tokio::test]
async fn control_listener_reconnects_to_one_verified_target_and_parses_toggle_metadata() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/7d47a800-c734-4f9a-a56c-55d875ea1cab"}}"#
        ),
    )
    .unwrap();
    let route_id = uuid::Uuid::parse_str("d2719d93-b823-4a7f-934f-23cbe01c8ab0").unwrap();
    let expected_route_id = route_id;
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let _ = stream.read(&mut request).await.unwrap();
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
        let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let call: serde_json::Value = serde_json::from_str(&call).unwrap();
        assert_eq!(call["method"], "Runtime.enable");
        socket
            .send(Message::Text(
                format!(r#"{{"id":{},"result":{{}}}}"#, call["id"]).into(),
            ))
            .await
            .unwrap();
        let payload = json!({
            "v":1,
            "sessionId":"session-1",
            "targetId":"page-1",
            "type":"toggle",
            "routeId":expected_route_id,
            "enabled":true
        })
        .to_string();
        socket
            .send(Message::Text(
                json!({
                    "method":"Runtime.bindingCalled",
                    "params":{
                        "name":"codexAssistant",
                        "payload":payload,
                        "executionContextId":7
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
    });

    let event = receive_control_event(&endpoint, "page-1", "session-1", 1_000)
        .await
        .unwrap();

    assert_eq!(
        event,
        ControlEvent::Toggle {
            route_id,
            enabled: true
        }
    );
    server.await.unwrap();
}

#[tokio::test]
async fn verified_target_activation_reads_only_the_fixed_boolean_acknowledgement() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
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
        let _ = stream.read(&mut request).await.unwrap();
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
        let call = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let call: serde_json::Value = serde_json::from_str(&call).unwrap();
        assert_eq!(call["method"], "Runtime.evaluate");
        assert_eq!(
            call["params"]["expression"],
            "globalThis.__codexAssistantControlV1?.setRoutingReady(true) === true"
        );
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
    });

    assert!(set_control_routing_ready(&endpoint, "page-1", true, 1_000)
        .await
        .unwrap());
    server.await.unwrap();
}

#[test]
fn injection_plan_uses_only_the_five_owned_cdp_primitives_in_order() {
    let script = "globalThis.__codexAssistantInstalled = true;";
    let plan = injection_plan(script).expect("bounded owned script");
    assert_eq!(
        plan.iter()
            .map(|command| command.method)
            .collect::<Vec<_>>(),
        [
            "Runtime.enable",
            "Page.enable",
            "Runtime.addBinding",
            "Page.addScriptToEvaluateOnNewDocument",
            "Runtime.evaluate",
        ]
    );
    assert_eq!(plan[2].params, json!({"name":"codexAssistant"}));
    assert_eq!(plan[3].params, json!({"source":script}));
    assert_eq!(
        plan[4].params,
        json!({"expression":script,"awaitPromise":false,"returnByValue":false})
    );
    assert_eq!(injection_plan(""), Err(InjectionPlanError::InvalidScript));
    assert_eq!(
        injection_plan(&"x".repeat(262_145)),
        Err(InjectionPlanError::InvalidScript)
    );
}
