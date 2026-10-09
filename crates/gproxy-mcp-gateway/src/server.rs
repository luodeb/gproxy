//! The end-user MCP gateway handler.
//!
//! Unlike the admin control plane, this server answers with other people's
//! tools: the caller is an ordinary API key, and what it may see comes from its
//! own grant rows. `tools/list` is served from the discovered catalogue so a
//! dead upstream cannot break a listing; `tools/call` connects upstream and
//! forwards the call, and the upstream's own result is returned untouched so a
//! tool's structured content survives the hop.

use std::time::Duration;

use http::request::Parts;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ErrorData, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler};

use crate::catalog::ToolCatalog;

const INSTRUCTIONS: &str = "\
Tools from the MCP servers your gproxy API key has been granted. The list is \
exactly what this key may call: a tool that is not listed cannot be called. \
Tool names are the upstream's own, so a name you know from a server's \
documentation works here unchanged.";

/// A tool call reaches an upstream, so it is bounded; a server that never
/// answers must not hold a client's request open.
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct GatewayMcp {
    app: gproxy_app::AppHandle,
}

impl GatewayMcp {
    pub fn new(app: gproxy_app::AppHandle) -> Self {
        Self { app }
    }

    /// The caller's identity, resolved from the request that carried the call.
    fn identity(&self, context: &RequestContext<RoleServer>) -> Result<gproxy_channel_api::CallerIdentity, ErrorData> {
        let parts = context
            .extensions
            .get::<Parts>()
            .ok_or_else(|| ErrorData::invalid_request("no transport request", None))?;
        self.app
            .mcp_identity(&parts.headers)
            .map_err(|error| ErrorData::invalid_request(error.to_string(), None))
    }

    fn catalog(&self, identity: &gproxy_channel_api::CallerIdentity) -> ToolCatalog {
        let snapshot = self.app.mcp_snapshot();
        ToolCatalog::build(&snapshot, |server, tool| {
            self.app.mcp_permitted(identity, server, tool)
        })
    }
}

impl ServerHandler for GatewayMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "gproxy-mcp-gateway",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let identity = self.identity(&context)?;
        Ok(ListToolsResult::with_all_items(
            self.catalog(&identity).tools,
        ))
    }

    /// Name lookups cannot be answered without the caller's grants, which this
    /// hook does not receive; clients that need a single tool read it from
    /// `tools/list`, and `call_tool` re-checks the grant regardless.
    fn get_tool(&self, _name: &str) -> Option<rmcp::model::Tool> {
        None
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let identity = self.identity(&context)?;
        let catalog = self.catalog(&identity);
        let tool = request.name.to_string();
        let Some(server_id) = catalog.owner.get(&tool).copied() else {
            // Not listed means not granted (or not granted for this name).
            return Err(ErrorData::invalid_params(
                format!("unknown or unauthorized tool: {tool}"),
                None,
            ));
        };
        if !self.app.mcp_permitted(&identity, server_id, Some(&tool)) {
            return Err(ErrorData::invalid_params(
                format!("unknown or unauthorized tool: {tool}"),
                None,
            ));
        }
        let snapshot = self.app.mcp_snapshot();
        let Some(server) = snapshot
            .mcp_servers
            .iter()
            .find(|server| server.id == server_id)
        else {
            return Err(ErrorData::internal_error("the granting server is gone", None));
        };
        let auth = self
            .app
            .mcp_auth(server_id)
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?
            .unwrap_or(gproxy_upstream::mcp_client::UpstreamMcpAuth::None);
        let client = gproxy_upstream::mcp_client::UpstreamMcp::new(server.url.clone(), auth);
        let result = client
            .call_tool(&tool, request.arguments, CALL_TIMEOUT)
            .await
            .map_err(|error| ErrorData::internal_error(error.detail(), None))?;
        Ok(CallToolResponse::Complete(result))
    }
}
