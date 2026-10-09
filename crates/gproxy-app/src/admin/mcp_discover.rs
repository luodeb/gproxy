//! Discover one upstream MCP server's tools and cache them.
//!
//! Discovery is the only place the gateway talks to an upstream on the
//! operator's behalf. It is read-only: the names are shown for the operator to
//! grant, and a failed pull changes nothing. Upstreams are not all
//! well-behaved — a rejected Zhipu call answers with its own JSON envelope
//! rather than a JSON-RPC error — so the message carries whatever the upstream
//! actually said instead of a generic protocol complaint.

use std::time::Duration;

use gproxy_admin::AdminError;
use gproxy_admin::dto::PortalMcpDiscoverResponse;

use crate::AppHandle;

/// A discovery pull reaches out to a third party, so it is bounded: a server
/// that never answers must not hold the portal request open.
const DISCOVER_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) async fn run(
    app: &AppHandle,
    server_id: i64,
) -> Result<PortalMcpDiscoverResponse, AdminError> {
    let services = &app.inner.host.services;
    let Some(server) = services.store.mcp_server(server_id).await? else {
        return Err(AdminError::NotFound);
    };
    let auth = app
        .mcp_auth(server_id)
        .await
        .map_err(|error| AdminError::Internal(error.to_string()))?
        .unwrap_or(gproxy_upstream::mcp_client::UpstreamMcpAuth::None);
    let client = gproxy_upstream::mcp_client::UpstreamMcp::new(server.url.clone(), auth);
    match client.list_tools(DISCOVER_TIMEOUT).await {
        Ok(tools) => {
            let discovered = tools
                .into_iter()
                .map(|tool| gproxy_store::records::McpToolRecord {
                    name: tool.name.to_string(),
                    description: tool.description.as_ref().map(|text| text.to_string()),
                    input_schema: serde_json::Value::Object((*tool.input_schema).clone()),
                })
                .collect::<Vec<_>>();
            let names = discovered
                .iter()
                .map(|tool| tool.name.clone())
                .collect::<Vec<_>>();
            let fetched_at = now_ms();
            // Persist through the same input shape an edit uses, so nothing is
            // lost: only the cached catalogue moves.
            let input = gproxy_store::records::McpServerInput {
                name: server.name.clone(),
                url: server.url.clone(),
                auth_kind: server.auth_kind.clone(),
                auth_header_name: server.auth_header_name.clone(),
                envelope: None,
                enabled: server.enabled,
                tools: Some(discovered),
                tools_fetched_at: Some(fetched_at),
            };
            services.store.update_mcp_server(server_id, &input).await?;
            app.reload().await.map_err(internal)?;
            Ok(PortalMcpDiscoverResponse {
                ok: true,
                tools: names,
                message: None,
            })
        }
        Err(error) => Ok(PortalMcpDiscoverResponse {
            ok: false,
            tools: Vec::new(),
            message: Some(error.detail()),
        }),
    }
}

fn internal(error: impl std::fmt::Display) -> AdminError {
    AdminError::Internal(error.to_string())
}

fn now_ms() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or_default()
}
