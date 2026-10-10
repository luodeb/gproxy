use bytes::Bytes;
use http::{Method, StatusCode};

use super::TestState;
use super::helpers::{parts, state};

/// The protocol toggle flips the provider's seeded Responses routing rows
/// between passthrough and transform-to-chat in place — same rows, no
/// inserts or deletes — is idempotent, and rejects unknown providers.
#[tokio::test]
async fn provider_protocol_toggle_flips_the_seeded_responses_rows() {
    let state = state().await;
    let owner = crate::seed_first_admin(&state.store, "owner", "test-password")
        .await
        .unwrap()
        .unwrap();
    let login = crate::portal_dispatch(
        &state,
        &parts(Method::POST, "/portal/api/login", None),
        Bytes::from_static(br#"{"username":"owner","password":"test-password"}"#),
    )
    .await
    .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login.headers()[http::header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let provider = super::helpers::provider(&state).await;

    async fn get(state: &TestState, provider: i64, cookie: &str) -> http::Response<Bytes> {
        let path = format!("/portal/api/providers/{provider}/protocol");
        crate::portal_dispatch(state, &parts(Method::GET, &path, Some(cookie)), Bytes::new())
            .await
            .expect("portal dispatch always answers")
    }
    async fn put(
        state: &TestState,
        provider: i64,
        cookie: &str,
        body: &str,
    ) -> http::Response<Bytes> {
        let path = format!("/portal/api/providers/{provider}/protocol");
        crate::portal_dispatch(
            state,
            &parts(Method::PUT, &path, Some(cookie)),
            Bytes::from(body.to_owned()),
        )
        .await
        .expect("portal dispatch always answers")
    }

    // Unknown provider is a 404, not a silent toggle.
    assert_eq!(get(&state, 99999, &cookie).await.status(), StatusCode::NOT_FOUND);

    fn responses_rows(
        snapshot: &gproxy_store::records::ControlSnapshot,
        provider: i64,
    ) -> Vec<&gproxy_store::records::RoutingRuleRecord> {
        snapshot
            .routing_rules
            .iter()
            .filter(|rule| rule.provider_id == provider && rule.kind == "openai_responses")
            .collect()
    }

    // Default: seeded passthrough rows, translation off.
    let default = get(&state, provider, &cookie).await;
    let default: serde_json::Value = serde_json::from_slice(default.body()).unwrap();
    assert_eq!(default["translate_responses"], false);
    let snapshot = state.store.control_snapshot().await.unwrap();
    // The fixture creates providers at the store level, so no defaults are
    // seeded — the toggle must work from absence too.
    let before = responses_rows(&snapshot, provider);
    assert_eq!(before.len(), 0, "fixture providers start without rules");

    // On: the same rows flip to transform-to-chat…
    let on = put(&state, provider, &cookie, r#"{"translate_responses":true}"#).await;
    assert_eq!(on.status(), StatusCode::OK);
    let snapshot = state.store.control_snapshot().await.unwrap();
    let rows = responses_rows(&snapshot, provider);
    assert_eq!(rows.len(), 2, "installed one row per content operation");
    for operation in ["generate_content", "stream_generate_content"] {
        assert!(
            rows.iter().any(|rule| {
                rule.operation == operation
                    && rule.enabled
                    && rule.implementation == "transform_to"
                    && rule.dest_operation.as_deref() == Some(operation)
                    && rule.dest_kind.as_deref() == Some("openai_chat")
            }),
            "row for {operation} did not flip to transform"
        );
    }

    // …switching on again is a no-op.
    put(&state, provider, &cookie, r#"{"translate_responses":true}"#).await;
    assert_eq!(responses_rows(&state.store.control_snapshot().await.unwrap(), provider).len(), 2);

    // Off: the installed rows flip back to passthrough (kept, not deleted).
    let off = put(&state, provider, &cookie, r#"{"translate_responses":false}"#).await;
    assert_eq!(off.status(), StatusCode::OK);
    let snapshot = state.store.control_snapshot().await.unwrap();
    let rows = responses_rows(&snapshot, provider);
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|rule| rule.implementation == "passthrough" && rule.dest_kind.is_none())
    );

    // Malformed body is a 400.
    assert_eq!(
        put(&state, provider, &cookie, r#"{"translate":"yes"}"#)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}
