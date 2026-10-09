//! The upstream MCP server registry.
//!
//! An MCP server is an operator-registered upstream the gateway aggregates for
//! end-user API keys. Its credential is sealed with the same envelope shape as
//! provider credentials, so the gateway can open it per call; the discovered
//! tool names ride along as JSON so `tools/list` never waits on an upstream.

use super::{ColumnKind::*, ColumnSpec as Col, SchemaVersion, TableSpec};

pub(crate) const TABLES: &[TableSpec] = &[TableSpec {
    version: SchemaVersion::McpServers,
    name: "mcp_servers",
    columns: &[
        Col::id(),
        Col::required("name", Text).unique(),
        Col::required("url", Text),
        // none | bearer | header. `header` carries the header name alongside the
        // sealed secret; bearer sends `Authorization: Bearer <secret>`.
        Col::required("auth_kind", Text).default("'none'"),
        Col::optional("auth_header_name", Text),
        Col::optional("ciphertext", Blob),
        Col::optional("wrapped_key", Blob),
        Col::optional("payload_nonce", Blob),
        Col::optional("key_nonce", Blob),
        Col::required("enabled", Integer),
        // JSON array of tool names from the last successful discovery, plus when
        // it happened. `tools/list` serves from here; `tools/call` goes upstream.
        Col::optional("tools_json", Text),
        Col::optional("tools_fetched_at", Integer),
    ],
    owns: &[],
    indexes: &[],
}];
