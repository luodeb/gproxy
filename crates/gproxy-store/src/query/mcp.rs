use sea_query::{Alias, Expr, ExprTrait, Query};

use crate::StoreError;
use crate::backend::Statement;
use crate::query::common::{insert, json, update, value};
use crate::records::McpServerInput;

const COLUMNS: &[&str] = &[
    "id",
    "name",
    "url",
    "auth_kind",
    "auth_header_name",
    "enabled",
    "tools_json",
    "tools_fetched_at",
];

pub(crate) fn select_mcp_servers() -> Result<Statement, StoreError> {
    let mut query = Query::select();
    query
        .columns(COLUMNS.iter().copied().map(Alias::new))
        .from(Alias::new("mcp_servers"))
        .order_by(Alias::new("id"), sea_query::Order::Asc);
    Statement::query(&query)
}

pub(crate) fn select_mcp_server(id: i64) -> Result<Statement, StoreError> {
    let mut query = Query::select();
    query
        .columns(COLUMNS.iter().copied().map(Alias::new))
        .from(Alias::new("mcp_servers"))
        .and_where(Expr::col(Alias::new("id")).eq(id))
        .limit(1);
    Statement::query(&query)
}

pub(crate) fn select_mcp_server_secret(id: i64) -> Result<Statement, StoreError> {
    let mut query = Query::select();
    query
        .columns(
            ["id", "ciphertext", "wrapped_key", "payload_nonce", "key_nonce"]
                .into_iter()
                .map(Alias::new),
        )
        .from(Alias::new("mcp_servers"))
        .and_where(Expr::col(Alias::new("id")).eq(id))
        .limit(1);
    Statement::query(&query)
}

pub(crate) fn insert_mcp_server(input: &McpServerInput) -> Result<Statement, StoreError> {
    let (ciphertext, wrapped_key, payload_nonce, key_nonce) = envelope_columns(input);
    insert(
        "mcp_servers",
        &[
            "name",
            "url",
            "auth_kind",
            "auth_header_name",
            "ciphertext",
            "wrapped_key",
            "payload_nonce",
            "key_nonce",
            "enabled",
            "tools_json",
            "tools_fetched_at",
        ],
        vec![
            value(input.name.clone()),
            value(input.url.clone()),
            value(input.auth_kind.clone()),
            value(input.auth_header_name.clone()),
            ciphertext,
            wrapped_key,
            payload_nonce,
            key_nonce,
            value(input.enabled),
            tools_json(input)?,
            value(input.tools_fetched_at),
        ],
    )
}

pub(crate) fn update_mcp_server(id: i64, input: &McpServerInput) -> Result<Statement, StoreError> {
    let (ciphertext, wrapped_key, payload_nonce, key_nonce) = envelope_columns(input);
    update(
        "mcp_servers",
        id,
        &[
            "name",
            "url",
            "auth_kind",
            "auth_header_name",
            "ciphertext",
            "wrapped_key",
            "payload_nonce",
            "key_nonce",
            "enabled",
            "tools_json",
            "tools_fetched_at",
        ],
        vec![
            value(input.name.clone()),
            value(input.url.clone()),
            value(input.auth_kind.clone()),
            value(input.auth_header_name.clone()),
            ciphertext,
            wrapped_key,
            payload_nonce,
            key_nonce,
            value(input.enabled),
            tools_json(input)?,
            value(input.tools_fetched_at),
        ],
    )
}

/// A grant row is keyed by the server it points at, so removing the server
/// removes its grants. `permissions` is polymorphic (no declared ownership),
/// which is why this is explicit rather than a cascade.
pub(crate) fn delete_mcp_server_grants(id: i64) -> Result<Statement, StoreError> {
    let mut query = Query::delete();
    query
        .from_table(Alias::new("permissions"))
        .and_where(Expr::col(Alias::new("operation_group")).eq("mcp"))
        .and_where(Expr::col(Alias::new("provider_id")).eq(id));
    Statement::query(&query)
}

/// An absent envelope keeps the stored one: the portal never echoes a secret
/// back, so an update that does not carry a new token must not clear it.
fn envelope_columns(
    input: &McpServerInput,
) -> (
    sea_query::SimpleExpr,
    sea_query::SimpleExpr,
    sea_query::SimpleExpr,
    sea_query::SimpleExpr,
) {
    match &input.envelope {
        Some(envelope) => (
            value(envelope.ciphertext.clone()),
            value(envelope.wrapped_key.clone()),
            value(envelope.payload_nonce.clone()),
            value(envelope.key_nonce.clone()),
        ),
        None => (
            sea_query::Expr::col(Alias::new("ciphertext")).into(),
            sea_query::Expr::col(Alias::new("wrapped_key")).into(),
            sea_query::Expr::col(Alias::new("payload_nonce")).into(),
            sea_query::Expr::col(Alias::new("key_nonce")).into(),
        ),
    }
}

fn tools_json(input: &McpServerInput) -> Result<sea_query::SimpleExpr, StoreError> {
    Ok(match &input.tools {
        Some(tools) => {
            let encoded = serde_json::to_value(tools).map_err(|error| StoreError::InvalidData {
                field: "mcp tools",
                message: error.to_string(),
            })?;
            value(json(&encoded, "mcp tools")?)
        }
        None => value(Option::<String>::None),
    })
}
