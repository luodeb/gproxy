//! Per-provider wire-protocol handling for the Responses API.
//!
//! Every provider is seeded with one routing rule per (operation, protocol
//! kind), and the Responses rows default to passthrough — right for upstreams
//! that serve `/v1/responses` themselves, wrong for the many
//! OpenAI-compatible upstreams that only speak Chat Completions, where a
//! passthrough pull fails upstream with a bare 404. The toggle flips the two
//! Responses rows between passthrough and transform-to-chat, in place: the
//! rows are unique per (provider, operation, kind), so the switch owns them
//! and never inserts or deletes.

use bytes::Bytes;
use gproxy_store::records::{RoutingRuleInput, RoutingRuleRecord};
use http::request::Parts;
use http::{Response, StatusCode};

use crate::auth::verify_same_origin;
use crate::dto::{PortalProviderProtocolDto, PortalProviderProtocolRequest};
use crate::{AdminError, State, response};

/// The operations whose Responses rows this toggle flips.
const OPERATIONS: [&str; 2] = ["generate_content", "stream_generate_content"];

fn is_responses_row(rule: &RoutingRuleRecord) -> bool {
    rule.kind == "openai_responses" && OPERATIONS.contains(&rule.operation.as_str())
}

fn input_from(rule: &RoutingRuleRecord, translate: bool) -> RoutingRuleInput {
    RoutingRuleInput {
        provider_id: rule.provider_id,
        operation: rule.operation.clone(),
        kind: rule.kind.clone(),
        implementation: if translate { "transform_to".into() } else { "passthrough".into() },
        dest_operation: translate.then(|| rule.operation.clone()),
        dest_kind: translate.then(|| "openai_chat".into()),
        sort_order: rule.sort_order,
        enabled: true,
    }
}

pub(super) async fn get(
    state: &impl State,
    provider_id: i64,
) -> Result<Response<Bytes>, AdminError> {
    ensure_provider(state, provider_id).await?;
    response::json(
        StatusCode::OK,
        &PortalProviderProtocolDto {
            translate_responses: translated(state, provider_id).await?,
        },
    )
}

pub(super) async fn set(
    state: &impl State,
    parts: &Parts,
    provider_id: i64,
    body: &Bytes,
) -> Result<Response<Bytes>, AdminError> {
    verify_same_origin(parts)?;
    ensure_provider(state, provider_id).await?;
    let request: PortalProviderProtocolRequest =
        serde_json::from_slice(body).map_err(|error| AdminError::BadRequest(error.to_string()))?;
    let translate = request.translate_responses;
    if translated(state, provider_id).await? == translate {
        return response::json(StatusCode::OK, &PortalProviderProtocolDto { translate_responses: translate });
    }
    let snapshot = state.store().control_snapshot().await?;
    let rows: Vec<_> = snapshot
        .routing_rules
        .iter()
        .filter(|rule| rule.provider_id == provider_id && is_responses_row(rule))
        .collect();
    if !translate {
        // Back to passthrough; rows the seeding never created can stay absent,
        // which means the same thing.
        for rule in &rows {
            state
                .store()
                .update_routing_rule(rule.id, &input_from(rule, false))
                .await?;
        }
    } else {
        // Flip the seeded rows; a provider without them (store-level creation)
        // gets fresh transform rows.
        for operation in OPERATIONS {
            match rows.iter().find(|rule| rule.operation == operation) {
                Some(rule) => {
                    state
                        .store()
                        .update_routing_rule(rule.id, &input_from(rule, true))
                        .await?;
                }
                None => {
                    state
                        .store()
                        .insert_routing_rule(&RoutingRuleInput {
                            provider_id,
                            operation: operation.into(),
                            kind: "openai_responses".into(),
                            implementation: "transform_to".into(),
                            dest_operation: Some(operation.into()),
                            dest_kind: Some("openai_chat".into()),
                            sort_order: 0,
                            enabled: true,
                        })
                        .await?;
                }
            }
        }
    }
    state.reload().await?;
    response::json(StatusCode::OK, &PortalProviderProtocolDto { translate_responses: translate })
}

async fn translated(state: &impl State, provider_id: i64) -> Result<bool, AdminError> {
    Ok(state
        .store()
        .control_snapshot()
        .await?
        .routing_rules
        .iter()
        .filter(|rule| rule.provider_id == provider_id && is_responses_row(rule))
        .any(|rule| rule.enabled && rule.implementation == "transform_to"))
}

async fn ensure_provider(state: &impl State, provider_id: i64) -> Result<(), AdminError> {
    if state
        .store()
        .control_snapshot()
        .await?
        .providers
        .iter()
        .any(|provider| provider.id == provider_id)
    {
        Ok(())
    } else {
        Err(AdminError::NotFound)
    }
}
