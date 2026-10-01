mod auth;
mod model;
mod prepare;
mod routes;
mod sse;
mod translate;
mod usage;

use bytes::Bytes;
use gproxy_channel_api::{
    BoxFuture, Channel, ChannelDescriptor, ChannelError, ChannelSupport, Disposition,
    NormalizedUsage, PrepareCtx, PreparedRequest, ResponseShapeCtx, ResponseView, SimpleHttp,
    StreamCtx, StreamDecoder, UsageCtx,
};
use serde_json::Value;

pub struct KylinTraeChannel;

static DESCRIPTOR: ChannelDescriptor = ChannelDescriptor {
    id: "kylintrae",
    display_name: "Kylin TRAE",
    provider_fields: crate::metadata::KYLINTRAE,
    credential_fields: crate::metadata::REFRESH_TOKEN,
    endpoint_overrides: false,
    traffic_policy: crate::policy::KYLINTRAE,
};

impl Channel for KylinTraeChannel {
    fn routing_table(&self) -> &'static [ChannelSupport] {
        routes::ROUTES
    }

    fn descriptor(&self) -> &'static ChannelDescriptor {
        &DESCRIPTOR
    }

    fn local_models(&self, _secret: &Value) -> Option<Vec<gproxy_channel_api::ModelInfo>> {
        Some(model::local_models())
    }

    fn prepare(&self, ctx: PrepareCtx<'_>) -> Result<PreparedRequest, ChannelError> {
        prepare::request(ctx)
    }

    fn refresh_due(&self, secret: &Value) -> Option<i64> {
        auth::refresh_due(secret)
    }

    fn refresh<'a>(
        &'a self,
        secret: &'a Value,
        provider_settings: &'a Value,
        http: &'a dyn SimpleHttp,
    ) -> Option<BoxFuture<'a, Result<Value, ChannelError>>> {
        Some(Box::pin(async move {
            let base = auth::base_url(provider_settings).to_owned();
            auth::refresh(secret, &base, http).await
        }))
    }

    fn classify(&self, response: ResponseView<'_>) -> Disposition {
        crate::shared::disposition::unauthorized_only(response)
    }

    fn stream_decoder(&self, ctx: StreamCtx<'_>) -> Option<Box<dyn StreamDecoder>> {
        sse::decoder(ctx)
    }

    fn extract_usage(&self, ctx: UsageCtx<'_>) -> Option<NormalizedUsage> {
        usage::from_body(ctx)
    }

    fn shape_response(&self, ctx: ResponseShapeCtx<'_>) -> Result<Bytes, ChannelError> {
        if ctx.status.is_success() && model::is_chat(ctx.key) {
            Ok(sse::collect_completion(ctx.body))
        } else {
            Ok(ctx.body.clone())
        }
    }
}

#[cfg(test)]
mod tests;
