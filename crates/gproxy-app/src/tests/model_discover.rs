use bytes::Bytes;
use gproxy_core::{RequestCtx, ResponseBody, RoutingMode};
use http::{Method, StatusCode};
use serde_json::json;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::setup;

/// Regression: discovery pulls the catalogue through the funnel with the
/// operator's own key, and a key with no `user_key` permission rows reaches
/// nothing — so the pull came back empty and the dialog offered no models.
/// Operator pulls are exempt from key allow-lists, as in admission: the
/// upstream is whatever provider the operator picked, not what the key can see.
#[tokio::test]
async fn operator_discovery_pulls_models_even_when_the_key_has_no_permission_rows() {
    let fixture = setup::fixture().await;
    let app = &fixture.app;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let end = loop {
            let mut buffer = [0; 4096];
            let size = socket.read(&mut buffer).await.unwrap();
            assert_ne!(size, 0);
            received.extend_from_slice(&buffer[..size]);
            if let Some(end) = received.windows(4).position(|part| part == b"\r\n\r\n") {
                break end;
            }
        };
        let headers = std::str::from_utf8(&received[..end]).unwrap();
        assert!(headers.starts_with("GET /v1/models "), "{headers}");
        let body = r#"{"object":"list","data":[{"id":"upstream-model"}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });

    // The operator's own key reaches nothing here (its only row is a deny), the
    // same shape as a freshly created key with no permission rows. Discovery must
    // still pull: the operator picked the provider, the key does not narrow it.
    let permission = fixture
        .app
        .inner
        .host
        .services
        .control
        .current()
        .permissions[0]
        .clone();
    fixture
        .app
        .inner
        .host
        .services
        .store
        .update_permission(
            permission.id,
            &gproxy_store::records::PermissionInput {
                subject_kind: permission.subject_kind,
                subject_id: permission.subject_id,
                provider_id: None,
                operation_group: Some("generate_content".into()),
                model_pattern: None,
                allowed: false,
            },
        )
        .await
        .expect("deny permission");

    fixture
        .app
        .inner
        .host
        .services
        .store
        .update_provider(
            fixture.provider,
            &gproxy_store::records::ProviderInput {
                name: "provider".into(),
                label: None,
                channel: "openai".into(),
                settings: json!({"base_url": format!("http://{address}")}),
                credential_strategy: "round_robin".into(),
                proxy_url: None,
                tls_fingerprint: None,
                enabled: true,
            },
        )
        .await
        .expect("provider update");
    fixture.app.reload().await.expect("reload");

    let discover = RequestCtx {
        request_id: "request-discover".into(),
        client_ip: None,
        method: Method::GET,
        path: "/v1/models".into(),
        query: None,
        headers: setup::request("discover", "", &fixture.client_key).headers,
        body: Bytes::new(),
        upgrade: false,
        force_model_refresh: true,
        mode: RoutingMode::Scoped {
            provider: "provider".into(),
        },
    };
    let outcome = app.execute(discover).await.expect("discovery pull");
    assert_eq!(outcome.status, StatusCode::OK);
    let ResponseBody::Full(body) = outcome.body else {
        panic!("buffered discovery catalogue")
    };
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ids = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| model["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["upstream-model"]);

    upstream.await.expect("upstream task");
}
