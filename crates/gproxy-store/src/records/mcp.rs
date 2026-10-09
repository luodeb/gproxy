use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::CredentialEnvelope;

/// One discovered tool, kept whole so `tools/list` can hand clients the real
/// name, description and input schema without calling upstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolRecord {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

/// One operator-registered upstream MCP server.
///
/// `tools` is the last discovered catalogue: the gateway's `tools/list` serves
/// from it so a listing never waits on (or fails with) a slow upstream. The
/// credential envelope is sealed exactly like a provider credential and is
/// deliberately *not* part of the snapshot — it is read per call, like a
/// credential secret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerInput {
    pub name: String,
    pub url: String,
    /// none | bearer | header
    pub auth_kind: String,
    pub auth_header_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope: Option<CredentialEnvelope>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<McpToolRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_fetched_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerRecord {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub auth_kind: String,
    pub auth_header_name: Option<String>,
    pub enabled: bool,
    pub tools: Vec<McpToolRecord>,
    pub tools_fetched_at: Option<i64>,
}

/// The sealed credential of one server, for the paths that must open it.
#[derive(Clone, PartialEq, Eq)]
pub struct McpServerSecretRecord {
    pub id: i64,
    pub envelope: Option<CredentialEnvelope>,
}

impl std::fmt::Debug for McpServerSecretRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpServerSecretRecord")
            .field("id", &self.id)
            .field("envelope", &self.envelope.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}
