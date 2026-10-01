use gproxy_channel_api::{NormalizedUsage, UsageCtx};
use serde_json::Value;

use super::model;

pub(super) fn from_body(ctx: UsageCtx<'_>) -> Option<NormalizedUsage> {
    if !model::is_chat(ctx.key) {
        return None;
    }
    let usage = parse_json(&ctx.response_body).or_else(|| parse_sse(&ctx.response_body))?;
    let input = usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
    (input != 0 || output != 0).then(|| NormalizedUsage {
        input_tokens: input,
        output_tokens: output,
        cached_input_tokens: 0,
        metrics: Default::default(),
        dimensions: Default::default(),
        attempts: Vec::new(),
    })
}

fn parse_json(body: &[u8]) -> Option<Value> {
    let value: Value = serde_json::from_slice(body).ok()?;
    value.get("usage").cloned().filter(|usage| usage.is_object())
}

fn parse_sse(body: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(body);
    let mut usage = None;
    for event in text.split("\n\n") {
        let mut name = String::new();
        let mut data = String::new();
        for line in event.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                name = rest.trim().to_owned();
            } else if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(rest.trim());
            }
        }
        if name == "token_usage" {
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                let prompt = value.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
                let completion = value.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
                usage = Some(serde_json::json!({
                    "prompt_tokens": prompt,
                    "completion_tokens": completion,
                    "total_tokens": prompt + completion,
                }));
            }
        }
    }
    usage
}
