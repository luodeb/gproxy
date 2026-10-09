//! Live activity for the portal overview.
//!
//! One read-only snapshot: what this account's keys have done in the last few
//! minutes, which models that traffic used, and how many requests are in the
//! gateway right now. It aggregates usage rows — nothing here needs request
//! logging to be enabled, because aggregates are recorded regardless.

use bytes::Bytes;
use gproxy_store::records::{UsageAggregateQuery, UsageGroupBy};
use http::{Response, StatusCode};

use super::PortalIdentity;
use crate::dto::{
    PortalActivityDto, PortalActivityModelDto, PortalActivityRecentDto, PortalActivityRequestDto,
    PortalActivitySessionDto,
};
use crate::{AdminError, State, response};

/// How far back "active" reaches. Long enough to see a session between turns,
/// short enough that the panel reads as "now".
const WINDOW_SECONDS: i64 = 300;

pub(super) async fn get(state: &impl State, identity: &PortalIdentity) -> Result<Response<Bytes>, AdminError> {
    let now = unix_now();
    let scope = UsageAggregateQuery {
        from: now - WINDOW_SECONDS,
        to: now + 1,
        group_by: UsageGroupBy::Dimensions,
        user_key_id: None,
        user_id: Some(identity.user_id),
        provider_id: None,
        credential_id: None,
        model: None,
    };
    let rows = state.store().usage_aggregate(&scope).await?;

    // One pass rolls the per-(key, provider, model) rows up into both views.
    let snapshot = state.store().control_snapshot().await?;
    let key_name = |key_id: i64| {
        snapshot
            .user_keys
            .iter()
            .find(|key| key.id == key_id)
            .map(|key| (key.prefix.clone(), key.label.clone()))
    };
    let provider_name = |provider_id: i64| {
        snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.label.clone().unwrap_or_else(|| provider.name.clone()))
    };

    let mut sessions: Vec<PortalActivitySessionDto> = Vec::new();
    let mut models: Vec<PortalActivityModelDto> = Vec::new();
    for row in rows {
        if let Some(key_id) = row.user_key_id {
            let (prefix, label) = key_name(key_id).unwrap_or((None, None));
            let session = match sessions.iter_mut().find(|session| session.key_id == key_id) {
                Some(session) => session,
                None => {
                    sessions.push(PortalActivitySessionDto {
                        key_id,
                        prefix,
                        label,
                        requests: 0,
                        input_tokens: 0,
                        output_tokens: 0,
                        last_at: 0,
                        models: Vec::new(),
                        tokens_per_second: 0.0,
                    });
                    sessions.last_mut().expect("just pushed")
                }
            };
            session.requests += row.requests;
            session.input_tokens += row.input_tokens;
            session.output_tokens += row.output_tokens;
            session.last_at = session.last_at.max(row.last_at);
            if !session.models.contains(&row.model) {
                session.models.push(row.model.clone());
            }
        }
        let provider = provider_name(row.provider_id);
        let model = match models
            .iter_mut()
            .find(|model| model.model == row.model && model.provider == provider)
        {
            Some(model) => model,
            None => {
                models.push(PortalActivityModelDto {
                    model: row.model.clone(),
                    provider,
                    requests: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    last_at: 0,
                });
                models.last_mut().expect("just pushed")
            }
        };
        model.requests += row.requests;
        model.input_tokens += row.input_tokens;
        model.output_tokens += row.output_tokens;
        model.last_at = model.last_at.max(row.last_at);
    }
    sessions.sort_by(|left, right| right.last_at.cmp(&left.last_at));
    models.sort_by(|left, right| right.last_at.cmp(&left.last_at));

    // The last minute reads from the same aggregates so one query answers all:
    // the headline numbers, and each key's tokens over 60s for its speed.
    let minute_scope = UsageAggregateQuery {
        from: now - 60,
        group_by: UsageGroupBy::Dimensions,
        ..scope
    };
    let minute_rows = state.store().usage_aggregate(&minute_scope).await?;
    let mut minute_requests = 0_u64;
    let mut minute_tokens = 0_u64;
    let mut key_minute_tokens: std::collections::HashMap<i64, u64> = std::collections::HashMap::new();
    for row in &minute_rows {
        minute_requests += row.requests;
        minute_tokens += row.input_tokens + row.output_tokens;
        if let Some(key_id) = row.user_key_id {
            *key_minute_tokens.entry(key_id).or_default() += row.input_tokens + row.output_tokens;
        }
    }
    for session in &mut sessions {
        let tokens = key_minute_tokens.get(&session.key_id).copied().unwrap_or_default();
        session.tokens_per_second = tokens as f64 / 60.0;
    }

    // Requests in flight right now, oldest first.
    let requests = state
        .in_flight_requests()
        .into_iter()
        .map(|request| {
            let (prefix, label) = request
                .user_key_id
                .and_then(key_name)
                .unwrap_or((None, None));
            PortalActivityRequestDto {
                request_id: request.request_id,
                key_id: request.user_key_id.unwrap_or_default(),
                prefix,
                label,
                model: request.model,
                started_at: request.started_at_ms,
            }
        })
        .collect::<Vec<_>>();

    // Just-finished requests, so the list has content between polls.
    let recent = state
        .store()
        .recent_usage_for_user(identity.user_id, 10)
        .await?
        .into_iter()
        .map(|record| {
            let (prefix, label) = record
                .user_key_id
                .and_then(key_name)
                .unwrap_or((None, None));
            PortalActivityRecentDto {
                request_id: record.request_id,
                key_id: record.user_key_id.unwrap_or_default(),
                prefix,
                label,
                model: record.upstream_model,
                provider: provider_name(record.provider_id),
                input_tokens: record.input_tokens,
                output_tokens: record.output_tokens,
                at: record.at,
                latency_ms: record.latency_ms,
            }
        })
        .collect::<Vec<_>>();

    response::json(
        StatusCode::OK,
        &PortalActivityDto {
            now,
            in_flight: state.in_flight(),
            window_seconds: WINDOW_SECONDS,
            last_minute_requests: minute_requests,
            last_minute_tokens: minute_tokens,
            requests,
            recent,
            sessions,
            models,
        },
    )
}

fn unix_now() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().try_into().unwrap_or(i64::MAX))
        .unwrap_or_default()
}
