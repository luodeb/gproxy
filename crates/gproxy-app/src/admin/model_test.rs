use bytes::Bytes;
use futures_util::StreamExt;
use gproxy_admin::AdminError;
use gproxy_admin::dto::{ModelTestRequest, ModelTestResponse};
use http::{HeaderMap, HeaderValue, Method};
use serde_json::json;
use web_time::Instant;

use crate::AppHandle;

/// One small completion, sent the way a client would send it.
///
/// This is real inference: it authenticates with the operator's own key, passes
/// admission and quota, and settles like any other request. There is no cheaper
/// path on purpose — a probe that skipped the funnel would be traffic nobody paid
/// for, which is the hole v2 opened with its service bypass.
pub(super) async fn run(
    app: &AppHandle,
    actor_user_id: i64,
    request: &ModelTestRequest,
) -> Result<ModelTestResponse, AdminError> {
    let snapshot = app.inner.host.services.control.current();
    let provider = snapshot
        .providers
        .iter()
        .find(|provider| provider.id == request.provider_id)
        .ok_or_else(|| AdminError::BadRequest("unknown provider".into()))?;

    let (key_prefix, secret) = super::operator_key(app, actor_user_id, &snapshot).await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        http::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {secret}"))
            .map_err(|_| AdminError::Internal("malformed test key".into()))?,
    );
    let body = json!({
        "model": request.model_id,
        "max_tokens": 64,
        "stream": true,
        "messages": [{ "role": "user", "content": "hi" }],
    });
    let ctx = gproxy_core::RequestCtx {
        request_id: format!("model-test:{}:{}", request.provider_id, request.model_id),
        client_ip: None,
        method: Method::POST,
        // The provider prefix lives in the routing mode, not the path: ingress patterns
        // are matched after a host has stripped it.
        path: "/v1/chat/completions".into(),
        query: None,
        headers,
        body: Bytes::from(serde_json::to_vec(&body).expect("static json")),
        upgrade: false,
        force_model_refresh: true,
        mode: gproxy_core::RoutingMode::Scoped {
            provider: provider.name.clone(),
        },
    };

    let started = Instant::now();
    let outcome = app.execute(ctx).await;
    let mut ttft: Option<u64> = None;
    let mut content = String::new();
    let mut output_tokens: u64 = 0;
    let mut error_message: Option<String> = None;
    let (status, mut ok);

    match outcome {
        Ok(outcome) => {
            status = outcome.status.as_u16();
            ok = outcome.status.is_success();
            match outcome.body {
                gproxy_core::ResponseBody::Full(bytes) => {
                    if ok {
                        content = first_text(&bytes).unwrap_or_default();
                        output_tokens = output_tokens_of(&bytes).unwrap_or(0);
                    } else {
                        error_message = upstream_error(&bytes);
                    }
                }
                gproxy_core::ResponseBody::Stream(mut stream) => {
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let elapsed = elapsed_ms(started);
                                let sample = scan(&String::from_utf8_lossy(&bytes));
                                if ttft.is_none() && sample.first_token {
                                    ttft = Some(elapsed);
                                }
                                content.push_str(&sample.content);
                                if let Some(tokens) = sample.output_tokens {
                                    output_tokens = tokens;
                                }
                                if sample.error.is_some() {
                                    error_message = sample.error;
                                }
                            }
                            Err(error) => {
                                ok = false;
                                error_message = Some(error.to_string());
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Err(error) => {
            status = 0;
            ok = false;
            error_message = Some(error.to_string());
        }
    }

    let latency_ms = elapsed_ms(started);
    if error_message.is_some() {
        ok = false;
    }
    let tokens_per_second = ttft.and_then(|first| {
        let span = latency_ms.saturating_sub(first);
        (span > 0 && output_tokens > 0).then(|| output_tokens as f64 * 1_000.0 / span as f64)
    });

    Ok(ModelTestResponse {
        ok,
        status,
        latency_ms,
        ttft_ms: ttft,
        output_tokens,
        tokens_per_second,
        key_prefix,
        reply: ok
            .then(|| content.trim().to_owned())
            .filter(|value| !value.is_empty()),
        message: error_message,
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

#[derive(Default)]
struct Scan {
    first_token: bool,
    content: String,
    output_tokens: Option<u64>,
    error: Option<String>,
}

/// Read one upstream chunk's SSE `data:` frames for the first-token signal,
/// streamed text, the final usage, and any upstream error.
fn scan(text: &str) -> Scan {
    let mut out = Scan::default();
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
        {
            out.error = Some(message.to_owned());
        }
        if let Some(tokens) = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(serde_json::Value::as_u64)
        {
            out.output_tokens = Some(tokens);
        }
        let delta = value
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"));
        if let Some(delta) = delta {
            if let Some(text) = delta.get("content").and_then(serde_json::Value::as_str) {
                if !text.is_empty() {
                    out.first_token = true;
                    out.content.push_str(text);
                }
            }
            if delta
                .get("reasoning_content")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| !text.is_empty())
            {
                out.first_token = true;
            }
        }
    }
    out
}

fn output_tokens_of(body: &[u8]) -> Option<u64> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()?
        .get("usage")?
        .get("completion_tokens")?
        .as_u64()
}

/// The first assistant text in an OpenAI-shaped reply, trimmed for display.
fn first_text(body: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let text = value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    Some(text.chars().take(200).collect())
}

/// An upstream refusal, in whatever shape the vendor sent it: OpenAI and Claude both
/// nest a message under `error`, and anything else falls back to the raw body.
fn upstream_error(body: &[u8]) -> Option<String> {
    let text = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            let error = value.get("error")?;
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or_else(|| Some(error.to_string()))
        })
        .or_else(|| String::from_utf8(body.to_vec()).ok())?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.chars().take(300).collect())
}
