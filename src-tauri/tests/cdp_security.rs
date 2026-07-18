use codex_assistant_lib::control_layer::cdp::{
    browser_endpoint, create_owned_session_record, fetch_browser_endpoint, fetch_page_targets,
    validate_session_record, BrowserAnchor, CdpDiscoveryError, CdpSecurityError,
    OwnedSessionRecord, OwnedSessionStore, SessionStoreError, TargetDescriptor, TargetRegistry,
};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const BROWSER_ID: &str = "7d47a800-c734-4f9a-a56c-55d875ea1cab";

#[test]
fn browser_and_page_websockets_must_be_same_port_loopback_endpoints() {
    let endpoint = browser_endpoint(
        49_321,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:49321/devtools/browser/{BROWSER_ID}"}}"#
        ),
    )
    .expect("verified browser endpoint");
    assert_eq!(endpoint.browser_id, BROWSER_ID);

    let page = endpoint
        .verify_target(TargetDescriptor {
            target_id: "page-1".into(),
            target_type: "page".into(),
            websocket_url: "ws://127.0.0.1:49321/devtools/page/page-1".into(),
        })
        .expect("same endpoint page");
    assert_eq!(page.target_id, "page-1");

    for url in [
        format!("ws://0.0.0.0:49321/devtools/browser/{BROWSER_ID}"),
        format!("ws://192.168.1.8:49321/devtools/browser/{BROWSER_ID}"),
        format!("ws://127.0.0.1:49322/devtools/browser/{BROWSER_ID}"),
    ] {
        let body = format!(r#"{{"webSocketDebuggerUrl":"{url}"}}"#);
        assert!(browser_endpoint(49_321, &body).is_err());
    }
}

#[test]
fn malformed_or_changed_browser_identity_and_unknown_targets_fail_closed() {
    let endpoint = browser_endpoint(
        49_321,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://localhost:49321/devtools/browser/{BROWSER_ID}"}}"#
        ),
    )
    .unwrap();
    let mut anchor = BrowserAnchor::new(&endpoint);
    assert!(anchor.observe(&endpoint).is_ok());
    assert_eq!(
        anchor.observe(&endpoint),
        Err(CdpSecurityError::DuplicateBrowserIdentity)
    );

    let changed = browser_endpoint(
        49_321,
        r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:49321/devtools/browser/6e90c53a-b93e-44d7-aeb8-9880ee199388"}"#,
    )
    .unwrap();
    assert_eq!(
        anchor.verify(&changed),
        Err(CdpSecurityError::BrowserIdentityChanged)
    );

    assert_eq!(
        endpoint.verify_target(TargetDescriptor {
            target_id: "worker-1".into(),
            target_type: "service_worker".into(),
            websocket_url: "ws://127.0.0.1:49321/devtools/page/worker-1".into(),
        }),
        Err(CdpSecurityError::UnknownTargetType)
    );

    let malformed =
        r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:49321/devtools/browser/not-a-uuid"}"#;
    assert_eq!(
        browser_endpoint(49_321, malformed),
        Err(CdpSecurityError::BrowserIdentity)
    );
}

#[test]
fn owned_session_is_bounded_metadata_only_and_stales_by_pid_version_or_time() {
    let record = OwnedSessionRecord {
        schema_version: 1,
        port: 49_321,
        verified_pid: 41_000,
        browser_id_hash: "a".repeat(64),
        codex_version: "26.715.3651.0".into(),
        started_at_ms: 1_000,
        engine_version: "control-v1".into(),
    };
    validate_session_record(&record, 41_000, "26.715.3651.0", 2_000).unwrap();
    assert_eq!(
        validate_session_record(&record, 41_001, "26.715.3651.0", 2_000),
        Err(CdpSecurityError::StaleSession)
    );
    assert_eq!(
        validate_session_record(&record, 41_000, "26.715.3652.0", 2_000),
        Err(CdpSecurityError::StaleSession)
    );
    assert_eq!(
        validate_session_record(&record, 41_000, "26.715.3651.0", 86_402_001),
        Err(CdpSecurityError::StaleSession)
    );

    let serialized = serde_json::to_string(&record).unwrap();
    for forbidden in ["webSocket", "prompt", "response", "cookie", "token", "path"] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn verified_endpoint_creates_a_hashed_metadata_only_session_record() {
    let endpoint = browser_endpoint(
        49_321,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:49321/devtools/browser/{BROWSER_ID}"}}"#
        ),
    )
    .unwrap();

    let record = create_owned_session_record(&endpoint, 41_000, "26.715.3651.0", 1_000)
        .expect("validated record");

    assert_eq!(record.port, 49_321);
    assert_eq!(record.verified_pid, 41_000);
    assert_eq!(record.browser_id_hash.len(), 64);
    assert_ne!(record.browser_id_hash, BROWSER_ID);
    assert!(!serde_json::to_string(&record).unwrap().contains(BROWSER_ID));
}

#[tokio::test]
async fn discovery_fetches_only_the_fixed_loopback_version_route_without_redirects() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..count]);
        assert!(request.starts_with("GET /json/version HTTP/1.1\r\n"));
        let body = format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/{BROWSER_ID}"}}"#
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    let endpoint = fetch_browser_endpoint(port, 1_000).await.unwrap();
    assert_eq!(endpoint.browser_id, BROWSER_ID);
    server.await.unwrap();

    let redirect = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let redirect_port = redirect.local_addr().unwrap().port();
    let redirect_server = tokio::spawn(async move {
        let (mut stream, _) = redirect.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let _ = stream.read(&mut request).await.unwrap();
        stream
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://example.com/json/version\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
    });
    assert_eq!(
        fetch_browser_endpoint(redirect_port, 1_000).await,
        Err(CdpDiscoveryError::UnexpectedStatus)
    );
    redirect_server.await.unwrap();
}

#[tokio::test]
async fn target_discovery_projects_only_verified_page_identity_from_the_fixed_list_route() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = browser_endpoint(
        port,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/browser/{BROWSER_ID}"}}"#
        ),
    )
    .unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 2_048];
        let count = stream.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..count]);
        assert!(request.starts_with("GET /json/list HTTP/1.1\r\n"));
        let body = format!(
            r#"[{{"id":"page-1","type":"page","webSocketDebuggerUrl":"ws://127.0.0.1:{port}/devtools/page/page-1","url":"http://localhost/local/private-task","title":"PRIVATE PAGE TITLE"}}]"#
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });

    let targets = fetch_page_targets(&endpoint, 1_000).await.unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].target_id, "page-1");
    assert!(!format!("{targets:?}").contains("PRIVATE PAGE TITLE"));
    assert!(!format!("{targets:?}").contains("private-task"));
    server.await.unwrap();
}

#[test]
fn target_registry_reconciles_attach_detach_and_rejects_duplicate_snapshots() {
    let endpoint = browser_endpoint(
        49_321,
        &format!(
            r#"{{"webSocketDebuggerUrl":"ws://127.0.0.1:49321/devtools/browser/{BROWSER_ID}"}}"#
        ),
    )
    .unwrap();
    let page = TargetDescriptor {
        target_id: "page-1".into(),
        target_type: "page".into(),
        websocket_url: "ws://127.0.0.1:49321/devtools/page/page-1".into(),
    };
    let mut registry = TargetRegistry::new(&endpoint);
    let attached = registry.reconcile(&endpoint, vec![page.clone()]).unwrap();
    assert_eq!(attached.attach.len(), 1);
    assert!(attached.detach.is_empty());
    let unchanged = registry.reconcile(&endpoint, vec![page.clone()]).unwrap();
    assert!(unchanged.attach.is_empty());
    assert!(unchanged.detach.is_empty());
    let detached = registry.reconcile(&endpoint, Vec::new()).unwrap();
    assert_eq!(detached.detach, ["page-1"]);

    assert_eq!(
        registry.reconcile(&endpoint, vec![page.clone(), page]),
        Err(CdpSecurityError::DuplicateTargetIdentity)
    );
}

#[test]
fn owned_session_store_round_trips_only_validated_metadata_and_rejects_stale_state() {
    let directory = tempdir().unwrap();
    let store = OwnedSessionStore::in_directory(directory.path()).unwrap();
    let record = OwnedSessionRecord {
        schema_version: 1,
        port: 49_321,
        verified_pid: 41_000,
        browser_id_hash: "b".repeat(64),
        codex_version: "26.715.3651.0".into(),
        started_at_ms: 1_000,
        engine_version: "control-v1".into(),
    };
    store.save(&record).unwrap();
    assert_eq!(
        store.load(41_000, "26.715.3651.0", 2_000).unwrap(),
        Some(record.clone())
    );
    let persisted = std::fs::read_to_string(store.path()).unwrap();
    for forbidden in ["webSocket", "prompt", "response", "cookie", "token", "path"] {
        assert!(!persisted.contains(forbidden));
    }
    assert_eq!(
        store.load(41_001, "26.715.3651.0", 2_000),
        Err(SessionStoreError::Stale)
    );
}
