//! The MCP grant surface: store round-trips, and the authorization matrix that
//! decides whether a key reaches a server, and a tool on it.

use gproxy_channel_api::CallerIdentity;
use gproxy_core::ControlPlane;
use gproxy_store::records::{McpServerInput, McpToolRecord, PermissionInput};

use super::setup;

fn identity(user_id: i64, user_key_id: i64) -> CallerIdentity {
    CallerIdentity {
        oauth_access_digest: None,
        user_id,
        user_key_id,
        org_id: None,
        team_id: None,
    }
}

fn server(name: &str, tools: &[&str]) -> McpServerInput {
    McpServerInput {
        name: name.into(),
        url: format!("https://example.test/{name}/mcp"),
        auth_kind: "none".into(),
        auth_header_name: None,
        envelope: None,
        enabled: true,
        tools: Some(
            tools
                .iter()
                .map(|tool| McpToolRecord {
                    name: (*tool).to_string(),
                    description: None,
                    input_schema: serde_json::json!({"type": "object"}),
                })
                .collect(),
        ),
        tools_fetched_at: Some(1),
    }
}

async fn grant(
    store: &gproxy_store::Store,
    key_id: i64,
    server_id: i64,
    pattern: Option<&str>,
    allowed: bool,
) {
    store
        .insert_permission(&PermissionInput {
            subject_kind: "user_key".into(),
            subject_id: key_id,
            provider_id: Some(server_id),
            operation_group: Some("mcp".into()),
            model_pattern: pattern.map(str::to_owned),
            allowed,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_server_round_trips_and_takes_its_grants_with_it() {
    let fixture = setup::fixture().await;
    let store = &fixture.app.inner.host.services.store;
    let app = &fixture.app;

    let id = store
        .insert_mcp_server(&server("reader", &["webReader", "webSearch"]))
        .await
        .unwrap();
    app.reload().await.unwrap();
    let snapshot = store.control_snapshot().await.unwrap();
    let record = snapshot
        .mcp_servers
        .iter()
        .find(|record| record.id == id)
        .expect("registered server");
    assert_eq!(record.name, "reader");
    assert_eq!(record.tools.len(), 2);
    assert_eq!(record.tools[0].name, "webReader");

    grant(store, 1, id, Some("web*"), true).await;
    app.reload().await.unwrap();
    assert_eq!(
        store
            .control_snapshot()
            .await
            .unwrap()
            .permissions
            .iter()
            .filter(|permission| permission.operation_group.as_deref() == Some("mcp"))
            .count(),
        1
    );

    assert!(store.delete_mcp_server(id).await.unwrap());
    app.reload().await.unwrap();
    let snapshot = store.control_snapshot().await.unwrap();
    assert!(snapshot.mcp_servers.iter().all(|record| record.id != id));
    assert!(
        snapshot
            .permissions
            .iter()
            .all(|permission| permission.provider_id != Some(id)),
        "grants must not outlive their server"
    );
}

#[tokio::test]
async fn mcp_grants_gate_servers_tools_and_interact_with_nothing_else() {
    let fixture = setup::fixture().await;
    let store = &fixture.app.inner.host.services.store;
    let app = &fixture.app;
    let server_id = store.insert_mcp_server(&server("reader", &["webReader"])).await.unwrap();
    app.reload().await.unwrap();

    let caller = identity(1, 1);
    // No rows: reaches nothing, not even the server listing.
    assert!(!app.mcp_permitted(&caller, server_id, None));
    assert!(!app.mcp_permitted(&caller, server_id, Some("webReader")));

    // A pattern grants exactly what it matches.
    grant(store, 1, server_id, Some("web*"), true).await;
    app.reload().await.unwrap();
    assert!(app.mcp_permitted(&caller, server_id, None));
    assert!(app.mcp_permitted(&caller, server_id, Some("webReader")));
    assert!(!app.mcp_permitted(&caller, server_id, Some("fs_read")));

    // A row without a pattern allows the whole server.
    store
        .delete_permission(
            store
                .control_snapshot()
                .await
                .unwrap()
                .permissions
                .iter()
                .find(|permission| permission.subject_id == 1)
                .unwrap()
                .id,
        )
        .await
        .unwrap();
    grant(store, 1, server_id, None, true).await;
    app.reload().await.unwrap();
    assert!(app.mcp_permitted(&caller, server_id, Some("anything")));

    // Deny wins over the allow above.
    grant(store, 1, server_id, Some("fs_*"), false).await;
    app.reload().await.unwrap();
    assert!(!app.mcp_permitted(&caller, server_id, Some("fs_read")));

    // Another key is unaffected: grants are per key.
    let other = identity(1, 2);
    assert!(!app.mcp_permitted(&other, server_id, Some("webReader")));

    // MCP rows never widen or narrow model access.
    let model = gproxy_store::records::PermissionRecord {
        id: 0,
        subject_kind: "user_key".into(),
        subject_id: 1,
        provider_id: Some(server_id),
        operation_group: Some("mcp".into()),
        model_pattern: Some("*".into()),
        allowed: true,
    };
    let snapshot = store.control_snapshot().await.unwrap();
    assert!(
        !snapshot.permissions.contains(&model),
        "the fixture's own model rows are untouched by MCP grants"
    );
    assert_eq!(
        snapshot
            .permissions
            .iter()
            .filter(|permission| permission.operation_group.as_deref() == Some("mcp"))
            .count(),
        3
    );
}
