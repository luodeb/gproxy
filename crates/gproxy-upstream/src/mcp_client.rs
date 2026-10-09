//! A minimal MCP client for one upstream server.
//!
//! The gateway aggregates operator-registered MCP servers for end-user keys.
//! Discovery reads a server's tool list; a call forwards one tool invocation.
//! Both connect per operation and hang up: the servers are few and the volume
//! is low, so a session pool would buy nothing but expiry and re-init handling.
//!
//! Upstreams are not all well-behaved. A failing Zhipu endpoint, for example,
//! answers with its own JSON envelope instead of a JSON-RPC error, which rmcp
//! reports as a protocol error and hides the reason. Every failure here keeps
//! the upstream's raw body (truncated) so the portal can show what actually
//! went wrong.

use std::collections::HashMap;
use std::time::Duration;

use http::{HeaderName, HeaderValue};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    JsonObject, Tool,
};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};

/// How the gateway authenticates to one upstream.
#[derive(Clone, PartialEq, Eq)]
pub enum UpstreamMcpAuth {
    None,
    /// `Authorization: Bearer <token>`.
    Bearer(String),
    /// A custom header, for servers that do not use Bearer.
    Header(String, String),
}

impl std::fmt::Debug for UpstreamMcpAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Bearer(_) => formatter.write_str("Bearer(<redacted>)"),
            Self::Header(name, _) => write!(formatter, "Header({name}, <redacted>)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpUpstreamError {
    #[error("{0}")]
    Transport(String),
    #[error("upstream did not answer within {0:?}")]
    Timeout(Duration),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

impl McpUpstreamError {
    /// One line for the portal, with the upstream's own words when it gave any.
    pub fn detail(&self) -> String {
        self.to_string()
    }
}

/// One registered upstream, ready to connect.
pub struct UpstreamMcp {
    url: String,
    auth: UpstreamMcpAuth,
}

impl UpstreamMcp {
    pub fn new(url: impl Into<String>, auth: UpstreamMcpAuth) -> Self {
        Self {
            url: url.into(),
            auth,
        }
    }

    pub async fn list_tools(&self, timeout: Duration) -> Result<Vec<Tool>, McpUpstreamError> {
        let client = self.connect(timeout).await?;
        let result = tokio::time::timeout(timeout, client.peer().list_tools(None))
            .await
            .map_err(|_| McpUpstreamError::Timeout(timeout))?;
        let tools = result.map_err(|error| McpUpstreamError::Transport(error.to_string()))?;
        client.cancel().await.ok();
        Ok(tools.tools)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
        timeout: Duration,
    ) -> Result<CallToolResult, McpUpstreamError> {
        let client = self.connect(timeout).await?;
        let params = match arguments {
            Some(arguments) => CallToolRequestParams::new(name.to_owned()).with_arguments(arguments),
            None => CallToolRequestParams::new(name.to_owned()),
        };
        let result = tokio::time::timeout(timeout, client.peer().call_tool_once(params))
            .await
            .map_err(|_| McpUpstreamError::Timeout(timeout))?;
        client.cancel().await.ok();
        match result.map_err(|error| McpUpstreamError::Transport(error.to_string()))? {
            rmcp::model::CallToolResponse::Complete(result) => Ok(result),
            other => Err(McpUpstreamError::Transport(format!(
                "upstream returned an unsupported tool response: {other:?}"
            ))),
        }
    }

    /// Handshake, then hand the live session back for one request.
    async fn connect(
        &self,
        connect_timeout: Duration,
    ) -> Result<RunningService<RoleClient, ClientConfig>, McpUpstreamError> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.as_str());
        // The gateway is the client here, so it may not keep a session alive
        // between calls; servers that support stateless mode are told so.
        config.allow_stateless = true;
        config = match &self.auth {
            UpstreamMcpAuth::None => config,
            // rmcp wants the token alone; it writes the `Bearer ` prefix.
            UpstreamMcpAuth::Bearer(token) => config.auth_header(token.clone()),
            UpstreamMcpAuth::Header(name, value) => {
                let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                    McpUpstreamError::Transport(format!("invalid auth header name: {error}"))
                })?;
                let value = HeaderValue::from_str(value).map_err(|error| {
                    McpUpstreamError::Transport(format!("invalid auth header value: {error}"))
                })?;
                config.custom_headers(HashMap::from([(name, value)]))
            }
        };
        let transport = StreamableHttpClientTransport::<reqwest::Client>::from_config(config);
        let info = ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("gproxy", env!("CARGO_PKG_VERSION")),
        );
        tokio::time::timeout(connect_timeout, info.serve(transport))
            .await
            .map_err(|_| McpUpstreamError::Timeout(connect_timeout))?
            .map_err(|error| McpUpstreamError::Transport(error.to_string()))
    }
}
