//! Aggregated model management for the portal. A portal "model" is an exposed
//! name plus the sources that serve it; the route that binds them is created,
//! updated, and deleted here so the UI never has to reason about routes.

use bytes::Bytes;
use gproxy_store::records::{
    ExposedModelInput, RouteInput, RouteMemberInput, RouteStrategy,
};
use http::Response;

use crate::control;
use crate::dto::{
    PortalManagedModelDto, PortalManagedModelWriteRequest, PortalModelSourceDto,
    PortalModelSourceWriteRequest,
};
use crate::handlers::util;
use crate::{AdminError, State, response};

const MAX_ATTEMPTS: u32 = 3;

fn route_name(model_name: &str) -> String {
    format!("portal-model:{model_name}")
}

pub(super) async fn list(state: &impl State) -> Result<Response<Bytes>, AdminError> {
    let snapshot = state.store().control_snapshot().await?;
    let models = snapshot
        .exposed_models
        .iter()
        .map(|model| PortalManagedModelDto {
            id: model.id,
            name: model.name.clone(),
            display_name: model.display_name.clone(),
            enabled: model.enabled,
            sources: snapshot
                .route_members
                .iter()
                .filter(|member| member.route_id == model.route_id)
                .map(source_dto)
                .collect(),
        })
        .collect::<Vec<_>>();
    response::json(http::StatusCode::OK, &models)
}

pub(super) async fn create(
    state: &impl State,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    let request: PortalManagedModelWriteRequest = util::parse(body)?;
    let name = require_name(&request.name)?;
    ensure_unique_name(state, &name, None).await?;
    let route_id = state
        .store()
        .insert_route(&RouteInput {
            name: route_name(&name),
            strategy: RouteStrategy::Weighted,
            max_attempts: MAX_ATTEMPTS,
            enabled: request.enabled,
        })
        .await?;
    let id = state
        .store()
        .insert_exposed_model(&ExposedModelInput {
            name,
            route_id,
            display_name: normalize_optional(request.display_name),
            enabled: request.enabled,
        })
        .await?;
    util::created(state, id).await
}

pub(super) async fn update(
    state: &impl State,
    id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    let request: PortalManagedModelWriteRequest = util::parse(body)?;
    let name = require_name(&request.name)?;
    let model = find_model(state, id).await?;
    ensure_unique_name(state, &name, Some(id)).await?;
    state
        .store()
        .update_route(
            model.route_id,
            &RouteInput {
                name: route_name(&name),
                strategy: RouteStrategy::Weighted,
                max_attempts: MAX_ATTEMPTS,
                enabled: request.enabled,
            },
        )
        .await?;
    let applied = state
        .store()
        .update_exposed_model(
            id,
            &ExposedModelInput {
                name,
                route_id: model.route_id,
                display_name: normalize_optional(request.display_name),
                enabled: request.enabled,
            },
        )
        .await?;
    util::updated(state, applied).await
}

pub(super) async fn delete(state: &impl State, id: i64) -> Result<Response<Bytes>, AdminError> {
    let model = find_model(state, id).await?;
    let applied = state.store().delete_exposed_model(id).await?;
    state.store().delete_route(model.route_id).await?;
    util::updated(state, applied).await
}

pub(super) async fn create_source(
    state: &impl State,
    model_id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    let model = find_model(state, model_id).await?;
    let request: PortalModelSourceWriteRequest = util::parse(body)?;
    validate_source(state, &request).await?;
    let id = state
        .store()
        .insert_route_member(&RouteMemberInput {
            route_id: model.route_id,
            provider_id: request.provider_id,
            upstream_model: request.upstream_model.trim().to_owned(),
            tier: request.tier,
            weight: request.weight,
            enabled: request.enabled,
        })
        .await?;
    util::created(state, id).await
}

pub(super) async fn update_source(
    state: &impl State,
    model_id: i64,
    member_id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    let model = find_model(state, model_id).await?;
    let request: PortalModelSourceWriteRequest = util::parse(body)?;
    validate_source(state, &request).await?;
    let member = find_member(state, model.route_id, member_id).await?;
    let applied = state
        .store()
        .update_route_member(
            member.id,
            &RouteMemberInput {
                route_id: model.route_id,
                provider_id: request.provider_id,
                upstream_model: request.upstream_model.trim().to_owned(),
                tier: request.tier,
                weight: request.weight,
                enabled: request.enabled,
            },
        )
        .await?;
    util::updated(state, applied).await
}

pub(super) async fn delete_source(
    state: &impl State,
    model_id: i64,
    member_id: i64,
) -> Result<Response<Bytes>, AdminError> {
    let model = find_model(state, model_id).await?;
    let member = find_member(state, model.route_id, member_id).await?;
    let applied = state.store().delete_route_member(member.id).await?;
    util::updated(state, applied).await
}

async fn ensure_unique_name(
    state: &impl State,
    name: &str,
    current: Option<i64>,
) -> Result<(), AdminError> {
    let snapshot = state.store().control_snapshot().await?;
    if snapshot
        .exposed_models
        .iter()
        .any(|model| model.name == name && Some(model.id) != current)
    {
        return Err(AdminError::Conflict(format!(
            "model name already exists: {name}"
        )));
    }
    Ok(())
}

async fn find_model(
    state: &impl State,
    id: i64,
) -> Result<gproxy_store::records::ExposedModelRecord, AdminError> {
    state
        .store()
        .control_snapshot()
        .await?
        .exposed_models
        .into_iter()
        .find(|model| model.id == id)
        .ok_or(AdminError::NotFound)
}

async fn find_member(
    state: &impl State,
    route_id: i64,
    member_id: i64,
) -> Result<gproxy_store::records::RouteMemberRecord, AdminError> {
    state
        .store()
        .control_snapshot()
        .await?
        .route_members
        .into_iter()
        .find(|member| member.id == member_id && member.route_id == route_id)
        .ok_or(AdminError::NotFound)
}

async fn validate_source(
    state: &impl State,
    request: &PortalModelSourceWriteRequest,
) -> Result<(), AdminError> {
    if request.upstream_model.trim().is_empty() {
        return Err(AdminError::BadRequest(
            "upstream_model must not be blank".into(),
        ));
    }
    if request.weight == 0 {
        return Err(AdminError::BadRequest("source weight must be positive".into()));
    }
    control::validators::provider(state, request.provider_id).await
}

fn require_name(value: &str) -> Result<String, AdminError> {
    let name = value.trim();
    if name.is_empty() {
        return Err(AdminError::BadRequest("model name must not be blank".into()));
    }
    Ok(name.to_owned())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn source_dto(member: &gproxy_store::records::RouteMemberRecord) -> PortalModelSourceDto {
    PortalModelSourceDto {
        id: member.id,
        provider_id: member.provider_id,
        upstream_model: member.upstream_model.clone(),
        tier: member.tier,
        weight: member.weight,
        enabled: member.enabled,
    }
}
