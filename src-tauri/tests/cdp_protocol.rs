use codex_assistant_lib::control_layer::cdp::{
    browser_endpoint, CdpClient, CdpClientError, CdpProtocol, CdpProtocolError, IncomingMessage,
    MAX_CDP_FRAME_BYTES,
};
use codex_assistant_lib::control_layer::injector::{injection_plan, InjectionPlanError};
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
