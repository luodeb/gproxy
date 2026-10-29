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

    // Grouped by dimensions, then folded per (provider, model): grouping by
    // model name alone merges the same model served by different upstreams.
    let mut by_model = scope.clone();
    by_model.group_by = UsageGroupBy::Dimensions;
    let snapshot = state.store().control_snapshot().await?;
    let provider_name = |provider_id: i64| {
        snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.label.clone().unwrap_or_else(|| provider.name.clone()))
    };
    let mut models: Vec<PortalUsageModelDto> = Vec::new();
    for record in state.store().usage_aggregate(&by_model).await? {
        let provider = provider_name(record.provider_id);
        let entry = match models
            .iter_mut()
            .find(|entry| entry.model == record.model && entry.provider == provider)
        {
            Some(entry) => entry,
            None => {
                models.push(PortalUsageModelDto {
                    model: record.model.clone(),
                    provider,
                    requests: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_input_tokens: 0,
                    cost: "0".into(),
                });
                models.last_mut().expect("just pushed")
            }
        };
        entry.requests += record.requests;
        entry.input_tokens += record.input_tokens;
        entry.output_tokens += record.output_tokens;
        entry.cached_input_tokens += record.cached_input_tokens;
        let cost: rust_decimal::Decimal = entry.cost.parse().unwrap_or_default();
        entry.cost = (cost + record.cost).normalize().to_string();
    }
    models.sort_by(|left, right| right.requests.cmp(&left.requests));

    let series = state
        .store()
        .usage_series(&UsageSeriesQuery {
            from,
            to,
            user_id: Some(identity.user_id),
            user_key_id: query.user_key_id,
            model: query.model.clone(),
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

    // Per-model hourly lines, capped to the models the by-model card already
    // shows so a long tail cannot blow up the payload.
    let visible: std::collections::BTreeSet<String> = models
        .iter()
        .take(12)
        .map(|model| model.model.clone())
        .collect();
    let mut model_series = Vec::new();
    if visible.len() > 1 {
        for trend in state
            .store()
            .usage_series_by_model(&UsageSeriesQuery {
                from,
                to,
                user_id: Some(identity.user_id),
                user_key_id: query.user_key_id,
                model: query.model.clone(),
            })
            .await?
        {
            if !visible.contains(&trend.model) {
                continue;
            }
            model_series.push(crate::dto::PortalUsageModelSeriesDto {
                model: trend.model,
                series: trend
                    .points
                    .into_iter()
                    .map(|point| PortalUsagePointDto {
                        bucket_start: point.bucket_start,
                        requests: point.requests,
                        input_tokens: point.input_tokens,
                        output_tokens: point.output_tokens,
                        cached_input_tokens: point.cached_input_tokens,
                        cost: point.cost.normalize().to_string(),
                    })
                    .collect(),
            });
        }
    }

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
            model_series,
        },
    )
}
