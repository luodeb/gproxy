use base64::Engine as _;
use bytes::Bytes;
use gproxy_store::records::{PermissionInput, UserKeyInput, UserKeyUpdateInput};
use http::request::Parts;
use http::{Response, StatusCode};

use super::PortalIdentity;
use crate::auth::{now, verify_same_origin};
use crate::dto::{
    PortalKeyCreateRequest, PortalKeyModelsDto, PortalKeyModelsRequest, UserKeyCreateResponse,
    UserKeyDto, UserKeyPrefix, UserKeyRevealResponse, UserKeyUpdateRequest,
};
use crate::{AdminError, State, response};

pub(super) async fn list(
    state: &impl State,
    identity: &PortalIdentity,
) -> Result<Response<Bytes>, AdminError> {
    let snapshot = state.store().control_snapshot().await?;
    let oauth_keys = state.store().oauth_user_key_ids().await?;
    let keys = snapshot
        .user_keys
        .iter()
        .filter(|key| key.user_id == identity.user_id)
        .filter(|key| !oauth_keys.contains(&key.id))
        .map(|key| UserKeyDto {
            id: key.id,
            user_id: key.user_id,
            prefix: key.prefix.clone(),
            label: key.label.clone(),
            revealable: key.revealable,
            expires_at: key.expires_at,
            enabled: key.enabled,
        })
        .collect::<Vec<_>>();
    response::json(StatusCode::OK, &keys)
}

pub(super) async fn reveal(
    state: &impl State,
    parts: &Parts,
    identity: &PortalIdentity,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    own_key(state, identity, id).await?;
    let api_key = state.reveal_user_key(id).await?;
    response::json(
        StatusCode::OK,
        &UserKeyRevealResponse {
            id,
            api_key,
            revealed_at: now()?,
        },
    )
}

pub(super) async fn create(
    state: &impl State,
    parts: &Parts,
    identity: &PortalIdentity,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    let request: PortalKeyCreateRequest = parse(body)?;
    let current = now()?;
    if request.expires_at.is_some_and(|expiry| expiry <= current) {
        return Err(AdminError::BadRequest(
            "user key expiry must be in the future".into(),
        ));
    }
    let prefix = match request.prefix {
        UserKeyPrefix::Sk => "sk",
        UserKeyPrefix::At => "at",
    };
    let api_key = generate(prefix)?;
    let (digest_version, digest) = state.digest_user_key(&api_key);
    let id = state
        .store()
        .insert_user_key(&UserKeyInput {
            user_id: identity.user_id,
            digest,
            digest_version,
            prefix: api_key.chars().take(12).collect(),
            envelope: state.seal_user_key(&api_key)?,
            label: request.label,
            expires_at: request.expires_at,
            enabled: true,
        })
        .await?;
    state.reload().await?;
    response::json(
        StatusCode::CREATED,
        &UserKeyCreateResponse {
            id,
            api_key,
            prefix: prefix.into(),
        },
    )
}

pub(super) async fn update(
    state: &impl State,
    parts: &Parts,
    identity: &PortalIdentity,
    id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    own_key(state, identity, id).await?;
    let request: UserKeyUpdateRequest = parse(body)?;
    let current = now()?;
    if request.expires_at.is_some_and(|expiry| expiry <= current) {
        return Err(AdminError::BadRequest(
            "user key expiry must be in the future".into(),
        ));
    }
    let applied = state
        .store()
        .update_user_key(
            id,
            &UserKeyUpdateInput {
                label: request.label,
                expires_at: request.expires_at,
                enabled: request.enabled,
            },
        )
        .await?;
    if !applied {
        return Err(AdminError::NotFound);
    }
    state.reload().await?;
    Ok(response::empty(StatusCode::NO_CONTENT))
}

pub(super) async fn delete(
    state: &impl State,
    parts: &Parts,
    identity: &PortalIdentity,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    own_key(state, identity, id).await?;
    if !state.store().delete_user_key(id).await? {
        return Err(AdminError::NotFound);
    }
    state.reload().await?;
    Ok(response::empty(StatusCode::NO_CONTENT))
}

async fn own_key(state: &impl State, identity: &PortalIdentity, id: i64) -> Result<(), AdminError> {
    if state.store().is_oauth_user_key(id).await? {
        return Err(AdminError::NotFound);
    }
    state
        .store()
        .control_snapshot()
        .await?
        .user_keys
        .iter()
        .any(|key| key.id == id && key.user_id == identity.user_id)
        .then_some(())
        .ok_or(AdminError::NotFound)
}

fn generate(prefix: &str) -> Result<String, AdminError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| AdminError::Internal("secure randomness unavailable".into()))?;
    Ok(format!(
        "{prefix}-gp-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn parse<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T, AdminError> {
    serde_json::from_slice(body).map_err(|error| AdminError::BadRequest(error.to_string()))
}

/// The models one key may reach, and the full catalogue to choose from.
pub(super) async fn models(
    state: &impl State,
    identity: &PortalIdentity,
    id: i64,
) -> Result<Response<Bytes>, AdminError> {
    ensure_key(state, identity, id).await?;
    let snapshot = state.store().control_snapshot().await?;
    response::json(
        StatusCode::OK,
        &PortalKeyModelsDto {
            available: available(state, identity).await,
            allowed: key_patterns(&snapshot, id),
        },
    )
}

/// Replace one key's model allow-list. An empty list clears the key's own rows,
/// so it falls back to the account's access.
pub(super) async fn set_models(
    state: &impl State,
    identity: &PortalIdentity,
    parts: &Parts,
    id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    ensure_key(state, identity, id).await?;
    let request: PortalKeyModelsRequest =
        serde_json::from_slice(body).map_err(|error| AdminError::BadRequest(error.to_string()))?;
    for permission in state
        .store()
        .control_snapshot()
        .await?
        .permissions
        .iter()
        .filter(|permission| permission.subject_kind == "user_key" && permission.subject_id == id)
    {
        state.store().delete_permission(permission.id).await?;
    }
    for model in request.models {
        state
            .store()
            .insert_permission(&PermissionInput {
                subject_kind: "user_key".into(),
                subject_id: id,
                provider_id: None,
                operation_group: None,
                model_pattern: Some(model),
                allowed: true,
            })
            .await?;
    }
    state.reload().await?;
    let snapshot = state.store().control_snapshot().await?;
    response::json(
        StatusCode::OK,
        &PortalKeyModelsDto {
            available: available(state, identity).await,
            allowed: key_patterns(&snapshot, id),
        },
    )
}

async fn available(state: &impl State, identity: &PortalIdentity) -> Vec<String> {
    state
        .portal_models(identity)
        .into_iter()
        .map(|model| model.name)
        .collect()
}

pub(super) async fn ensure_key(
    state: &impl State,
    identity: &PortalIdentity,
    id: i64,
) -> Result<(), AdminError> {
    let keys = state.store().control_snapshot().await?.user_keys;
    if keys
        .iter()
        .any(|key| key.id == id && key.user_id == identity.user_id)
    {
        Ok(())
    } else {
        Err(AdminError::NotFound)
    }
}

fn key_patterns(snapshot: &gproxy_store::records::ControlSnapshot, id: i64) -> Vec<String> {
    snapshot
        .permissions
        .iter()
        .filter(|permission| {
            permission.subject_kind == "user_key"
                && permission.subject_id == id
                && permission.allowed
                // MCP grants share this table; they are not model names.
                && permission.operation_group.is_none()
        })
        .filter_map(|permission| permission.model_pattern.clone())
        .collect()
}
