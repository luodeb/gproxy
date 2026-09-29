//! In-process MCP control plane for gproxy.
//!
//! The MCP server is mounted inside the gproxy process itself: the axum ingress
//! forwards `POST /admin/api/mcp` to [`McpService`], which speaks the MCP
//! streamable HTTP transport and dispatches tool calls through
//! [`gproxy_app::AppHandle::admin_dispatch`]. Tool execution therefore reuses
//! the admin API's authentication, authorization, validation, audit trail and
//! quota bookkeeping — there is no second control path to keep in sync.
//!
//! The crate is compiled only for native (non-wasm) targets: the wasm edge host
//! has no use for a control-plane server, and keeping the dependency out of the
//! wasm build avoids pulling `rmcp`, `tokio` and friends into the edge bundle.

#![cfg(not(target_arch = "wasm32"))]

mod catalog;
mod openapi;
mod server;
mod tools;

use std::sync::Arc;
use std::time::Duration;

use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

pub(crate) use openapi::document;
pub use server::GatewayMcp;

/// Path the axum ingress routes to the MCP streamable HTTP transport.
pub const GATEWAY_MCP_PATH: &str = "/admin/api/mcp";

/// Path serving the generated OpenAPI description of the admin API.
pub const OPENAPI_PATH: &str = "/admin/api/openapi.json";

/// Upper bound on a single MCP request body. Tool calls carry admin API
/// payloads (including configuration imports), so the limit mirrors the
/// generous ingress cap rather than rmcp's 4 MiB default.
const MAX_REQUEST_BODY_BYTES: usize = 32 * 1024 * 1024;

/// Sends keep-alive frames on long-lived SSE streams so intermediaries do not
/// silently drop the connection.
const SSE_KEEP_ALIVE: Duration = Duration::from_secs(30);

/// Environment variable holding the comma-separated DNS-rebinding allow list.
const ALLOWED_HOSTS_ENV: &str = "GPROXY_MCP_ALLOWED_HOSTS";

/// Cloneable handle to the MCP streamable HTTP service.
///
/// The type is intentionally thin: it owns the rmcp service and forwards the
/// incoming axum request into it, converting the rmcp boxed response body into
/// an axum body without buffering (SSE responses must keep streaming).
#[derive(Clone)]
pub struct McpService {
    inner: StreamableHttpService<GatewayMcp, LocalSessionManager>,
}

impl McpService {
    /// Build the service for the given application handle.
    ///
    /// Tool calls are dispatched in-process against the admin API, so the
    /// caller's credentials are the credentials used by every tool.
    pub fn new(app: gproxy_app::AppHandle) -> Self {
        let config = StreamableHttpServerConfigBuilder::build();
        let inner = StreamableHttpService::new(
            move || Ok(GatewayMcp::new(app.clone())),
            Arc::new(LocalSessionManager::default()),
            config,
        );
        Self { inner }
    }

    /// Serve one MCP HTTP request.
    pub async fn handle(&self, request: axum::extract::Request) -> axum::response::Response {
        let response = self.inner.handle(request).await;
        let (parts, body) = response.into_parts();
        axum::response::Response::from_parts(parts, axum::body::Body::new(body))
    }

    /// The generated OpenAPI document for the admin API.
    pub fn openapi_json(&self) -> serde_json::Value {
        openapi::document()
    }
}

/// Builds the rmcp transport configuration.
///
/// DNS-rebinding protection stays enabled: `allowed_hosts` defaults to an empty
/// list (allow any `Host`) because the endpoint sits behind the operator's own
/// reverse proxy and is already gated by admin credentials, but operators can
/// pin the expected hosts through `GPROXY_MCP_ALLOWED_HOSTS`.
struct StreamableHttpServerConfigBuilder;

impl StreamableHttpServerConfigBuilder {
    fn build() -> rmcp::transport::streamable_http_server::StreamableHttpServerConfig {
        let config = rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default()
            .with_legacy_session_mode(true)
            .with_sse_keep_alive(Some(SSE_KEEP_ALIVE))
            .with_max_request_body_bytes(MAX_REQUEST_BODY_BYTES)
            .disable_allowed_origins();
        match allowed_hosts() {
            Some(hosts) => config.with_allowed_hosts(hosts),
            None => config.disable_allowed_hosts(),
        }
    }
}

/// Parse `GPROXY_MCP_ALLOWED_HOSTS` into the transport allow list.
///
/// Returns `None` when the variable is unset or empty, which the caller maps to
/// "accept any `Host` header".
fn allowed_hosts() -> Option<Vec<String>> {
    let raw = std::env::var(ALLOWED_HOSTS_ENV).ok()?;
    let hosts = raw
        .split(',')
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if hosts.is_empty() {
        return None;
    }
    tracing::info!(
        hosts = %hosts.join(","),
        "restricting MCP requests to configured Host headers"
    );
    Some(hosts)
}
