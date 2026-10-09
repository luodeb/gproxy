//! Portal surface for the MCP gateway.
//!
//! Two things live here: the upstream server registry the operator maintains,
//! and the per-key grants that decide which of those tools an API key may call.
//! Both mirror the provider and per-key model surfaces beside them — same
//! `State` helpers, same ownership check, same "replace the whole set" writes.

use bytes::Bytes;
use gproxy_store::records::{ControlSnapshot, McpServerInput, McpServerRecord, PermissionInput};
use http::request::Parts;
use http::{Response, StatusCode};

use super::PortalIdentity;
use crate::auth::verify_same_origin;
use crate::dto::{
    PortalKeyMcpDto, PortalKeyMcpGrantDto, PortalKeyMcpRequest, PortalMcpDiscoverResponse,
    PortalMcpServerDto, PortalMcpServerToolsDto, PortalMcpServerWriteRequest,
};
use crate::{AdminError, State, response};

/// One page of the registry. Tools come from the cached discovery, never a
/// live upstream call — the portal must not hang because a server is down.
pub(super) async fn list_servers(state: &impl State) -> Result<Response<Bytes>, AdminError> {
    let snapshot = state.store().control_snapshot().await?;
    response::json(
        StatusCode::OK,
        &snapshot
            .mcp_servers
            .iter()
            .map(server_dto)
            .collect::<Vec<_>>(),
    )
}

pub(super) async fn create_server(
    state: &impl State,
    parts: &Parts,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    let request: PortalMcpServerWriteRequest = parse(body)?;
    let input = input_from(state, &request, None)?;
    let id = state.store().insert_mcp_server(&input).await?;
    state.reload().await?;
    let server = state
        .store()
        .mcp_server(id)
        .await?
        .ok_or(AdminError::NotFound)?;
    response::json(StatusCode::CREATED, &server_dto(&server))
}

pub(super) async fn update_server(
    state: &impl State,
    parts: &Parts,
    id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    let existing = state
        .store()
        .mcp_server(id)
        .await?
        .ok_or(AdminError::NotFound)?;
    let request: PortalMcpServerWriteRequest = parse(body)?;
    // The catalogue is discovered, not written: an edit keeps what was found.
    let input = input_from(state, &request, Some(&existing))?;
    state.store().update_mcp_server(id, &input).await?;
    state.reload().await?;
    response::json(StatusCode::OK, &serde_json::json!({ "id": id }))
}

/// Removing a server removes the grants pointing at it; a key that was granted
/// nothing else then reaches no MCP tools.
pub(super) async fn delete_server(
    state: &impl State,
    parts: &Parts,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    if !state.store().delete_mcp_server(id).await? {
        return Err(AdminError::NotFound);
    }
    state.reload().await?;
    Ok(response::empty(StatusCode::NO_CONTENT))
}

/// Ask the server what it serves, and remember the answer.
pub(super) async fn discover_server(
    state: &impl State,
    parts: &Parts,
    identity: &PortalIdentity,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    // A pull that fails is still a successful request: the response carries the
    // upstream's own message for the operator to read.
    let result: PortalMcpDiscoverResponse = state.discover_mcp_tools(identity.user_id, id).await?;
    response::json(StatusCode::OK, &result)
}

/// The tools one key may call, and the servers it already reaches.
pub(super) async fn key_mcp(
    state: &impl State,
    identity: &PortalIdentity,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    super::keys::ensure_key(state, identity, id).await?;
    let snapshot = state.store().control_snapshot().await?;
    response::json(StatusCode::OK, &key_mcp_dto(&snapshot, id))
}

/// Replace one key's MCP grants. An empty list clears them, and an empty list
/// means this key reaches no MCP tools — there is no account fallback here.
pub(super) async fn set_key_mcp(
    state: &impl State,
    identity: &PortalIdentity,
    parts: &Parts,
    id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    super::keys::ensure_key(state, identity, id).await?;
    let request: PortalKeyMcpRequest = parse(body)?;
    let snapshot = state.store().control_snapshot().await?;
    // Grants name servers the way the portal does; an unknown name is a bug in
    // the caller, not something to silently drop.
    let mut resolved = Vec::new();
    for grant in &request.grants {
        let server = snapshot
            .mcp_servers
            .iter()
            .find(|server| server.name == grant.server)
            .ok_or_else(|| AdminError::BadRequest(format!("unknown MCP server: {}", grant.server)))?;
        resolved.push((server.id, grant.patterns.clone()));
    }
    for permission in snapshot.permissions.iter().filter(|permission| {
        permission.subject_kind == "user_key"
            && permission.subject_id == id
            && permission.operation_group.as_deref() == Some("mcp")
    }) {
        state.store().delete_permission(permission.id).await?;
    }
    for (server_id, patterns) in resolved {
        if patterns.is_empty() {
            continue;
        }
        for pattern in patterns {
            state
                .store()
                .insert_permission(&PermissionInput {
                    subject_kind: "user_key".into(),
                    subject_id: id,
                    provider_id: Some(server_id),
                    operation_group: Some("mcp".into()),
                    model_pattern: Some(pattern),
                    allowed: true,
                })
                .await?;
        }
    }
    state.reload().await?;
    let snapshot = state.store().control_snapshot().await?;
    response::json(StatusCode::OK, &key_mcp_dto(&snapshot, id))
}

fn server_dto(server: &McpServerRecord) -> PortalMcpServerDto {
    PortalMcpServerDto {
        id: server.id,
        name: server.name.clone(),
        url: server.url.clone(),
        auth_kind: server.auth_kind.clone(),
        auth_header_name: server.auth_header_name.clone(),
        enabled: server.enabled,
        tools: server.tools.iter().map(|tool| tool.name.clone()).collect(),
        tools_fetched_at: server.tools_fetched_at,
    }
}

fn key_mcp_dto(snapshot: &ControlSnapshot, key_id: i64) -> PortalKeyMcpDto {
    let available = snapshot
        .mcp_servers
        .iter()
        .map(|server| PortalMcpServerToolsDto {
            name: server.name.clone(),
            enabled: server.enabled,
            tools: server.tools.iter().map(|tool| tool.name.clone()).collect(),
        })
        .collect::<Vec<_>>();
    let mut allowed: Vec<PortalKeyMcpGrantDto> = Vec::new();
    for permission in snapshot.permissions.iter().filter(|permission| {
        permission.subject_kind == "user_key"
            && permission.subject_id == key_id
            && permission.operation_group.as_deref() == Some("mcp")
            && permission.allowed
    }) {
        let Some(server_id) = permission.provider_id else {
            continue;
        };
        let Some(server) = snapshot
            .mcp_servers
            .iter()
            .find(|server| server.id == server_id)
        else {
            continue;
        };
        let pattern = permission.model_pattern.clone().unwrap_or_else(|| "*".into());
        match allowed.iter_mut().find(|grant| grant.server == server.name) {
            Some(grant) => grant.patterns.push(pattern),
            None => allowed.push(PortalKeyMcpGrantDto {
                server: server.name.clone(),
                patterns: vec![pattern],
            }),
        }
    }
    PortalKeyMcpDto { available, allowed }
}

/// Build a write input. A missing token keeps the stored credential, and the
/// cached catalogue is carried over rather than cleared.
fn input_from(
    state: &impl State,
    request: &PortalMcpServerWriteRequest,
    existing: Option<&McpServerRecord>,
) -> Result<McpServerInput, AdminError> {
    let auth_kind = match request.auth_kind.as_str() {
        "none" | "bearer" | "header" => request.auth_kind.clone(),
        other => {
            return Err(AdminError::BadRequest(format!(
                "auth_kind must be none, bearer or header, not {other}"
            )));
        }
    };
    if auth_kind == "header" && request.auth_header_name.is_none() && existing.is_none() {
        return Err(AdminError::BadRequest(
            "auth_header_name is required for header auth".into(),
        ));
    }
    let envelope = match &request.token {
        Some(token) => Some(state.seal_credential(&serde_json::json!({ "token": token }))?),
        None => None,
    };
    Ok(McpServerInput {
        name: request.name.clone(),
        url: request.url.clone(),
        auth_kind,
        auth_header_name: request
            .auth_header_name
            .clone()
            .or_else(|| existing.and_then(|server| server.auth_header_name.clone())),
        envelope,
        enabled: request.enabled,
        tools: existing.map(|server| server.tools.clone()),
        tools_fetched_at: existing.and_then(|server| server.tools_fetched_at),
    })
}

fn parse<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T, AdminError> {
    serde_json::from_slice(body).map_err(|error| AdminError::BadRequest(error.to_string()))
}
