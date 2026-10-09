use crate::backend::{QueryResult, Row};
use crate::query;
use crate::query::mcp;
use crate::records::{CredentialEnvelope, McpServerRecord, McpServerSecretRecord};
use crate::{Store, StoreError};

impl Store {
    pub async fn insert_mcp_server(
        &self,
        input: &crate::records::McpServerInput,
    ) -> Result<i64, StoreError> {
        self.insert(mcp::insert_mcp_server(input)?).await
    }

    pub async fn update_mcp_server(
        &self,
        id: i64,
        input: &crate::records::McpServerInput,
    ) -> Result<bool, StoreError> {
        self.update(mcp::update_mcp_server(id, input)?).await
    }

    pub async fn mcp_server(&self, id: i64) -> Result<Option<McpServerRecord>, StoreError> {
        self.backend()
            .execute(mcp::select_mcp_server(id)?)
            .await?
            .rows
            .into_iter()
            .next()
            .map(parse_mcp_server)
            .transpose()
    }

    pub async fn mcp_server_secret(
        &self,
        id: i64,
    ) -> Result<Option<McpServerSecretRecord>, StoreError> {
        self.backend()
            .execute(mcp::select_mcp_server_secret(id)?)
            .await?
            .rows
            .into_iter()
            .next()
            .map(parse_mcp_server_secret)
            .transpose()
    }

    /// Delete a server and the grants that point at it. A key whose grants are
    /// gone reaches no MCP tools, which is the same meaning as never granted.
    pub async fn delete_mcp_server(&self, id: i64) -> Result<bool, StoreError> {
        let statements = vec![
            mcp::delete_mcp_server_grants(id)?,
            query::delete_by_id("mcp_servers", id)?,
        ];
        Ok(self
            .backend()
            .batch(statements)
            .await?
            .first()
            .is_some_and(|result| result.affected_rows == 1))
    }
}

pub(super) fn mcp_servers(result: QueryResult) -> Result<Vec<McpServerRecord>, StoreError> {
    result.rows.into_iter().map(parse_mcp_server).collect()
}

fn parse_mcp_server(row: Row) -> Result<McpServerRecord, StoreError> {
    let tools = row
        .optional_text("tools_json")?
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| StoreError::InvalidData {
            field: "mcp tools_json",
            message: error.to_string(),
        })?
        .unwrap_or_default();
    Ok(McpServerRecord {
        id: row.i64("id")?,
        name: row.text("name")?.to_owned(),
        url: row.text("url")?.to_owned(),
        auth_kind: row.text("auth_kind")?.to_owned(),
        auth_header_name: row.optional_text("auth_header_name")?.map(str::to_owned),
        enabled: row.i64("enabled")? != 0,
        tools,
        tools_fetched_at: row.optional_i64("tools_fetched_at")?,
    })
}

fn parse_mcp_server_secret(row: Row) -> Result<McpServerSecretRecord, StoreError> {
    let parts = [
        row.optional_blob("ciphertext")?,
        row.optional_blob("wrapped_key")?,
        row.optional_blob("payload_nonce")?,
        row.optional_blob("key_nonce")?,
    ];
    let envelope = match parts {
        [None, None, None, None] => None,
        [Some(ciphertext), Some(wrapped_key), Some(payload_nonce), Some(key_nonce)] => {
            Some(CredentialEnvelope {
                ciphertext: ciphertext.to_vec(),
                wrapped_key: wrapped_key.to_vec(),
                payload_nonce: payload_nonce.to_vec(),
                key_nonce: key_nonce.to_vec(),
            })
        }
        _ => {
            return Err(StoreError::InvalidData {
                field: "mcp server envelope",
                message: "envelope columns are only partially populated".into(),
            });
        }
    };
    Ok(McpServerSecretRecord {
        id: row.i64("id")?,
        envelope,
    })
}
