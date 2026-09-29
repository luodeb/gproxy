//! The MCP server handler.
//!
//! The handler is written by hand rather than generated with rmcp's
//! `#[tool_router]` macro so that every tool call can reach the HTTP request
//! that carried it: the ingress injects the caller's [`http::request::Parts`]
//! into the transport's request extensions, and tool execution replays those
//! credentials against the admin API.

use http::request::Parts;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler};

use crate::tools;

/// Instructions handed to clients during initialization.
const INSTRUCTIONS: &str = "\
gproxy control plane. These tools manage an LLM gateway: providers and their \
credentials, model routes and aliases, users and API keys, quotas, pricing, \
usage reporting, request logs and instance settings.

Every tool acts with the credentials the caller connected with, so the granted \
scope is exactly that of the underlying admin account. Start with \
`gproxy_endpoints` to discover routes, or `gproxy_list` to browse an entity. \
`gproxy_request` reaches any admin route not covered by a dedicated tool.

Two facilities are deliberately out of reach: `gproxy_request` refuses the MCP \
endpoint itself, and the host-level `/admin/api/native/` routes (self-update and \
autostart) are not part of the catalog, so nothing here can restart or replace \
the running binary.

Writes take effect immediately and some are destructive — confirm with the \
operator before applying changes outside a maintenance window.";

/// MCP server exposing the gproxy admin API as tools.
#[derive(Clone)]
pub struct GatewayMcp {
    app: gproxy_app::AppHandle,
}

impl GatewayMcp {
    /// Wrap the application handle.
    pub fn new(app: gproxy_app::AppHandle) -> Self {
        Self { app }
    }
}

impl ServerHandler for GatewayMcp {
    fn get_info(&self) -> ServerConfig {
        server_config()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools::tools()))
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools::lookup(name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let caller = context.extensions.get::<Parts>().cloned();
        if caller.is_none() {
            tracing::warn!(
                tool = %request.name,
                "MCP tool call arrived without a transport request; acting without caller credentials"
            );
        }
        let result =
            tools::call(&self.app, &request.name, request.arguments, caller.as_ref()).await?;
        Ok(result.into())
    }
}

/// The capabilities and identity advertised during initialization.
///
/// Split out from [`ServerHandler::get_info`] so it can be asserted without
/// constructing an application handle.
fn server_config() -> ServerConfig {
    ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
        .with_server_info(Implementation::new("gproxy", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
}

#[cfg(test)]
mod tests {
    use super::server_config;

    #[test]
    fn server_identity_and_capabilities_are_advertised() {
        let info = server_config();
        assert_eq!(info.server_info.name, "gproxy");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        assert!(
            info.instructions
                .is_some_and(|text| text.contains("gproxy"))
        );
    }
}
