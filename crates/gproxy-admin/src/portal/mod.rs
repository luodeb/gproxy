mod auth;
mod context;
mod keys;
mod models;
mod oauth_sessions;
mod recent;
mod usage;

use bytes::Bytes;
use gproxy_store::records::SettingRecord;
use http::request::Parts;
use http::{Method, Response, StatusCode};

use crate::control;
use crate::dto::{ModelDiscoverRequest, ModelTestRequest, PortalModelDto};
use crate::route::Entity;
use crate::{AdminError, State, response};

pub(crate) const RECENT_REQUESTS_SETTING: &str = "portal_recent_requests_enabled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalIdentity {
    pub user_id: i64,
    pub org_id: Option<i64>,
    pub team_id: Option<i64>,
    pub user_name: String,
}

enum Route {
    Context,
    Models,
    Channels,
    Usage,
    UsageOverview,
    RecentRequests,
}

pub async fn dispatch(state: &impl State, parts: &Parts, body: Bytes) -> Option<Response<Bytes>> {
    let path = parts.uri.path();
    if path != "/portal/api" && !path.starts_with("/portal/api/") {
        return None;
    }
    let result = async {
        match (&parts.method, path) {
            (&Method::POST, "/portal/api/login") => return auth::login(state, parts, &body).await,
            (&Method::GET, "/portal/api/session") => return auth::status(state, parts).await,
            (&Method::POST, "/portal/api/logout") => return auth::logout(state, parts).await,
            (&Method::POST, "/portal/api/password") => {
                return auth::change_password(state, parts, &body).await;
            }
            _ => {}
        }
        let identity = auth::identity(state, parts).await?;
        if path == "/portal/api/oauth-sessions" && parts.method == Method::GET {
            return oauth_sessions::list(state, &identity, parts).await;
        }
        if let Some(id) = path
            .strip_prefix("/portal/api/oauth-sessions/")
            .and_then(|id| id.parse::<i64>().ok())
            && parts.method == Method::DELETE
        {
            return oauth_sessions::revoke(state, &identity, parts, id).await;
        }
        if path == "/portal/api/keys" {
            return match parts.method {
                Method::GET => keys::list(state, &identity).await,
                Method::POST => keys::create(state, parts, &identity, &body).await,
                _ => Err(AdminError::NotFound),
            };
        }
        if let Some(id) = path
            .strip_prefix("/portal/api/keys/")
            .and_then(|value| value.strip_suffix("/models"))
            .and_then(|value| value.parse::<i64>().ok())
        {
            return match parts.method {
                Method::GET => keys::models(state, &identity, id).await,
                Method::PUT => keys::set_models(state, &identity, parts, id, &body).await,
                _ => Err(AdminError::NotFound),
            };
        }
        if let Some(id) = path
            .strip_prefix("/portal/api/keys/")
            .and_then(|value| value.strip_suffix("/reveal"))
            .and_then(|value| value.parse::<i64>().ok())
            && parts.method == Method::POST
        {
            return keys::reveal(state, parts, &identity, id).await;
        }
        if let Some(id) = path
            .strip_prefix("/portal/api/keys/")
            .and_then(|value| value.parse::<i64>().ok())
        {
            return match parts.method {
                Method::PATCH => keys::update(state, parts, &identity, id, &body).await,
                Method::DELETE => keys::delete(state, parts, &identity, id).await,
                _ => Err(AdminError::NotFound),
            };
        }
        if let Some(value) = management(state, parts, &body, &identity).await? {
            return Ok(value);
        }
        match route(&parts.method, path)? {
            Route::Context => context::get(state, &identity).await,
            Route::Models => response::json(StatusCode::OK, &models(state, &identity)),
            Route::Channels => response::json(StatusCode::OK, &state.channel_catalogue()),
            Route::Usage => usage::get(state, &identity, parts).await,
            Route::UsageOverview => usage::overview(state, &identity, parts).await,
            Route::RecentRequests => recent::get(state, &identity, parts).await,
        }
    }
    .await;
    Some(response::render(result, "portal"))
}

/// Portal-owned write surface. Providers, credentials, and upstream models are
/// delegated to the shared `control` service; exposed models are handled by the
/// aggregated `models` module so the portal never manipulates routes directly.
/// The portal is a single-user surface, so no per-user scoping is applied.
async fn management(
    state: &impl State,
    parts: &Parts,
    body: &Bytes,
    identity: &PortalIdentity,
) -> Result<Option<Response<Bytes>>, AdminError> {
    let path = parts.uri.path();
    if let Some(id) = path
        .strip_prefix("/portal/api/providers/")
        .and_then(|value| value.strip_suffix("/model-test"))
        .and_then(|value| value.parse::<i64>().ok())
        && parts.method == Method::POST
    {
        let request: ModelTestRequest = serde_json::from_slice(body)
            .map_err(|error| AdminError::BadRequest(format!("model-test body: {error}")))?;
        let response = state
            .test_model(identity.user_id, &ModelTestRequest { provider_id: id, ..request })
            .await?;
        return response::json(StatusCode::OK, &response).map(Some);
    }
    if let Some(id) = path
        .strip_prefix("/portal/api/providers/")
        .and_then(|value| value.strip_suffix("/discover"))
        .and_then(|value| value.parse::<i64>().ok())
        && parts.method == Method::POST
    {
        let response = state
            .discover_models(identity.user_id, &ModelDiscoverRequest { provider_id: id })
            .await?;
        return response::json(StatusCode::OK, &response).map(Some);
    }
    if let Some(id) = path
        .strip_prefix("/portal/api/credentials/")
        .and_then(|value| value.strip_suffix("/reveal"))
        .and_then(|value| value.parse::<i64>().ok())
        && parts.method == Method::POST
    {
        return control::credential_secret(state, id).await.map(Some);
    }
    if let Some(id) = path
        .strip_prefix("/portal/api/credentials/")
        .and_then(|value| value.strip_suffix("/health-reset"))
        .and_then(|value| value.parse::<i64>().ok())
        && parts.method == Method::POST
    {
        return control::credential_health_reset(state, id).await.map(Some);
    }
    if let Some(value) = managed_models(state, parts, body).await? {
        return Ok(Some(value));
    }
    let (entity, id) = match management_entity(path) {
        Some(value) => value,
        None => return Ok(None),
    };
    let value = match id {
        None => match parts.method {
            Method::GET => control::list(state, entity).await,
            Method::POST => control::create(state, entity, body).await,
            _ => return Ok(None),
        },
        Some(id) => match parts.method {
            Method::PATCH => control::update(state, entity, id, body).await,
            Method::DELETE => control::delete(state, entity, id).await,
            _ => return Ok(None),
        },
    };
    Ok(Some(value?))
}

async fn managed_models(
    state: &impl State,
    parts: &Parts,
    body: &Bytes,
) -> Result<Option<Response<Bytes>>, AdminError> {
    let path = parts.uri.path();
    let Some(rest) = path.strip_prefix("/portal/api/managed-models") else {
        return Ok(None);
    };
    let trimmed = rest.trim_matches('/');
    let segments: Vec<&str> = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split('/').collect()
    };
    let value = match segments.as_slice() {
        [] => match parts.method {
            Method::GET => models::list(state).await,
            Method::POST => models::create(state, body).await,
            _ => return Ok(None),
        },
        [id] => match parts.method {
            Method::PATCH => models::update(state, parse_id(id)?, body).await,
            Method::DELETE => models::delete(state, parse_id(id)?).await,
            _ => return Ok(None),
        },
        [id, "sources"] => match parts.method {
            Method::POST => models::create_source(state, parse_id(id)?, body).await,
            _ => return Ok(None),
        },
        [id, "sources", source_id] => match parts.method {
            Method::PATCH => {
                models::update_source(state, parse_id(id)?, parse_id(source_id)?, body).await
            }
            Method::DELETE => models::delete_source(state, parse_id(id)?, parse_id(source_id)?).await,
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(Some(value?))
}

fn parse_id(value: &str) -> Result<i64, AdminError> {
    value.parse::<i64>().map_err(|_| AdminError::NotFound)
}

fn management_entity(path: &str) -> Option<(Entity, Option<i64>)> {
    const ENTITIES: &[(&str, Entity)] = &[
        ("/portal/api/providers", Entity::Providers),
        ("/portal/api/credentials", Entity::Credentials),
        ("/portal/api/provider-models", Entity::ProviderModels),
    ];
    for (prefix, entity) in ENTITIES {
        if let Some(rest) = path.strip_prefix(prefix) {
            if rest.is_empty() {
                return Some((*entity, None));
            }
            if let Ok(id) = rest.trim_start_matches('/').parse::<i64>() {
                return Some((*entity, Some(id)));
            }
        }
    }
    None
}
pub(crate) fn recent_requests_enabled(settings: &[SettingRecord]) -> bool {
    settings.iter().any(|setting| {
        setting.key == RECENT_REQUESTS_SETTING && setting.value == serde_json::Value::Bool(true)
    })
}

fn models(state: &impl State, identity: &PortalIdentity) -> Vec<PortalModelDto> {
    state.portal_models(identity)
}

fn route(method: &Method, path: &str) -> Result<Route, AdminError> {
    if method != Method::GET {
        return Err(AdminError::NotFound);
    }
    match path {
        "/portal/api/context" => Ok(Route::Context),
        "/portal/api/models" => Ok(Route::Models),
        "/portal/api/channels" => Ok(Route::Channels),
        "/portal/api/usage" => Ok(Route::Usage),
        "/portal/api/usage/overview" => Ok(Route::UsageOverview),
        "/portal/api/recent-requests" => Ok(Route::RecentRequests),
        _ => Err(AdminError::NotFound),
    }
}
