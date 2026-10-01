use bytes::Bytes;
use gproxy_store::records::{UsageAggregateQuery, UsageGroupBy, UsageSeriesQuery};
use http::request::Parts;
use http::{Response, StatusCode};

use super::PortalIdentity;
use crate::dto::{
    PortalUsageDto, PortalUsageModelDto, PortalUsageOverviewDto, PortalUsageOverviewQueryDto,
    PortalUsagePointDto, PortalUsageQueryDto,
};
use crate::{AdminError, State, handlers, response};

pub(super) async fn get(
    state: &impl State,
    identity: &PortalIdentity,
    parts: &Parts,
) -> Result<Response<Bytes>, AdminError> {
    let query =
        serde_urlencoded::from_str::<PortalUsageQueryDto>(parts.uri.query().unwrap_or_default())
            .map_err(|error| AdminError::BadRequest(error.to_string()))?;
    let (from, to) = handlers::observability::range(query.from, query.to)?;
    let record = state
        .store()
        .usage_aggregate(&UsageAggregateQuery {
            from,
            to,
            group_by: UsageGroupBy::User,
            user_key_id: None,
            user_id: Some(identity.user_id),
            provider_id: None,
            credential_id: None,
            model: None,
        })
        .await?
        .into_iter()
        .next();
    response::json(
        StatusCode::OK,
        &PortalUsageDto {
            from,
            to,
            requests: record.as_ref().map_or(0, |record| record.requests),
            input_tokens: record.as_ref().map_or(0, |record| record.input_tokens),
            output_tokens: record.as_ref().map_or(0, |record| record.output_tokens),
            cached_input_tokens: record
                .as_ref()
                .map_or(0, |record| record.cached_input_tokens),
            cost: record.map_or_else(|| "0".into(), |record| record.cost.normalize().to_string()),
        },
    )
}

pub(super) async fn overview(
    state: &impl State,
    identity: &PortalIdentity,
    parts: &Parts,
) -> Result<Response<Bytes>, AdminError> {
    let query = serde_urlencoded::from_str::<PortalUsageOverviewQueryDto>(
        parts.uri.query().unwrap_or_default(),
    )
    .map_err(|error| AdminError::BadRequest(error.to_string()))?;
    let (from, to) = handlers::observability::range(query.from, query.to)?;
    let scope = UsageAggregateQuery {
        from,
        to,
        group_by: UsageGroupBy::User,
        user_key_id: query.user_key_id,
        user_id: Some(identity.user_id),
        provider_id: None,
        credential_id: None,
        model: query.model.clone(),
    };
    let totals = state.store().usage_aggregate(&scope).await?.into_iter().next();

    let mut by_model = scope.clone();
    by_model.group_by = UsageGroupBy::Model;
    let models = state
        .store()
        .usage_aggregate(&by_model)
        .await?
        .into_iter()
        .map(|record| PortalUsageModelDto {
            model: record.group,
            requests: record.requests,
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens,
            cached_input_tokens: record.cached_input_tokens,
            cost: record.cost.normalize().to_string(),
        })
        .collect::<Vec<_>>();

    let series = state
        .store()
        .usage_series(&UsageSeriesQuery {
            from,
            to,
            user_id: Some(identity.user_id),
            user_key_id: query.user_key_id,
            model: query.model,
        })
        .await?
        .into_iter()
        .map(|point| PortalUsagePointDto {
            bucket_start: point.bucket_start,
            requests: point.requests,
            input_tokens: point.input_tokens,
            output_tokens: point.output_tokens,
            cached_input_tokens: point.cached_input_tokens,
            cost: point.cost.normalize().to_string(),
        })
        .collect::<Vec<_>>();

    response::json(
        StatusCode::OK,
        &PortalUsageOverviewDto {
            from,
            to,
            requests: totals.as_ref().map_or(0, |record| record.requests),
            input_tokens: totals.as_ref().map_or(0, |record| record.input_tokens),
            output_tokens: totals.as_ref().map_or(0, |record| record.output_tokens),
            cached_input_tokens: totals
                .as_ref()
                .map_or(0, |record| record.cached_input_tokens),
            cost: totals.map_or_else(|| "0".into(), |record| record.cost.normalize().to_string()),
            series,
            models,
        },
    )
}
