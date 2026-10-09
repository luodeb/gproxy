//! End-user MCP gateway for gproxy.
//!
//! The admin control plane ([`gproxy_mcp`]) exposes the gateway *to its
//! operator*. This crate is the other direction: it aggregates MCP servers the
//! operator registered, and serves them to ordinary API keys over one
//! streamable HTTP endpoint. A key sees exactly the tools it was granted —
//! server by server, tool pattern by tool pattern — and `tools/call` is
//! forwarded upstream with the server's own configured credential.
//!
//! Compiled only for native targets: the wasm edge host has no MCP gateway.

#![cfg(not(target_arch = "wasm32"))]

mod catalog;
mod server;

use std::sync::Arc;
use std::time::Duration;

use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

pub use server::GatewayMcp;

/// Path the axum ingress routes to the user-facing MCP transport.
///
/// Sits beside `/v1/*`: same caller, same key, different protocol.
pub const USER_MCP_PATH: &str = "/mcp";

/// Sends keep-alive frames on long-lived SSE streams so intermediaries do not
/// silently drop the connection.
const SSE_KEEP_ALIVE: Duration = Duration::from_secs(30);

/// Upper bound on a single MCP request body. Tool arguments are small; this is
/// generous enough for a large document passed to a tool.
const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Cloneable handle to the user-facing MCP service.
///
/// Thin on purpose: it owns the rmcp service and forwards an axum request into
/// it, converting the boxed response body without buffering so SSE responses
/// keep streaming.
#[derive(Clone)]
pub struct McpGatewayService {
    inner: StreamableHttpService<GatewayMcp, LocalSessionManager>,
}

impl McpGatewayService {
    pub fn new(app: gproxy_app::AppHandle) -> Self {
        let config = rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default()
            .with_legacy_session_mode(true)
            .with_sse_keep_alive(Some(SSE_KEEP_ALIVE))
            .with_max_request_body_bytes(MAX_REQUEST_BODY_BYTES)
            // Authentication is the caller's API key, checked per request, so
            // DNS-rebinding `Origin`/`Host` pinning is deliberately left off:
            // a non-browser client sends no `Origin` at all.
            .disable_allowed_origins()
            .disable_allowed_hosts();
        let inner = StreamableHttpService::new(
            move || Ok(GatewayMcp::new(app.clone())),
            Arc::new(LocalSessionManager::default()),
            config,
        );
        Self { inner }
    }

    pub async fn handle(&self, request: axum::extract::Request) -> axum::response::Response {
        let response = self.inner.handle(request).await;
        let (parts, body) = response.into_parts();
        axum::response::Response::from_parts(parts, axum::body::Body::new(body))
    }
}
