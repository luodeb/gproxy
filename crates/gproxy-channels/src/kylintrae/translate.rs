use gproxy_channel_api::ChannelError;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Upstream binds a model config to the conversation id, so the id must be
/// stable per (token, config, raw) or the upstream silently falls back to the
/// tenant default model.
pub(super) fn session_id(token: &str, config: &str, raw: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [token, config, raw] {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    let digest = hasher.finalize();
    let hex = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub(super) fn request_body(
    body: &[u8],
    config: &str,
    raw: &str,
    session_id: &str,
) -> Result<Bytes, ChannelError> {
    let request: Value = serde_json::from_slice(body)
        .map_err(|error| ChannelError::Prepare(format!("chat request JSON: {error}")))?;
    let messages = request
        .get("messages")
        .ok_or_else(|| ChannelError::Prepare("chat request has no messages".into()))?;
    let native_messages = to_trae_messages(messages)?;
    let mut out = Map::new();
    out.insert("config_name".into(), json!(config));
    out.insert("conversation_id".into(), json!(session_id));
    out.insert("messages".into(), Value::Array(native_messages));
    out.insert("model_name".into(), json!(raw));
    out.insert("session_id".into(), json!(session_id));
    out.insert("stream".into(), json!(true));
    if let Some(tools) = request.get("tools") {
        if let Some(native) = to_native_tools(tools) {
            if !native.is_empty() {
                out.insert("tools".into(), Value::Array(native));
            }
        }
    }
    serde_json::to_vec(&Value::Object(out))
        .map(Bytes::from)
        .map_err(|error| ChannelError::Prepare(error.to_string()))
}

use bytes::Bytes;

fn to_trae_messages(messages: &Value) -> Result<Vec<Value>, ChannelError> {
    let Some(list) = messages.as_array() else {
        return Err(ChannelError::Prepare("messages must be an array".into()));
    };
    let mut out = Vec::new();
    for message in list {
        let Some(object) = message.as_object() else {
            continue;
        };
        let role = match object.get("role").and_then(Value::as_str) {
            Some("developer") => "system",
            Some("system") => "system",
            Some("assistant") => "assistant",
            Some("tool") => "tool",
            _ => "user",
        };
        if role == "tool" {
            let text = flatten_text(object.get("content"));
            out.push(json!({
                "role": "tool",
                "tool_call_id": object.get("tool_call_id").and_then(Value::as_str)
                    .or_else(|| object.get("name").and_then(Value::as_str))
                    .unwrap_or("unknown"),
                "content": [{"type": "text", "text": text}],
            }));
            continue;
        }
        if role == "assistant" {
            if let Some(calls) = object.get("tool_calls").and_then(Value::as_array) {
                let native: Vec<Value> = calls.iter().filter_map(native_tool_call).collect();
                if !native.is_empty() {
                    let text = flatten_text(object.get("content"));
                    out.push(json!({
                        "role": "assistant",
                        "content": [{"type": "text", "text": text}],
                        "tool_calls": native,
                    }));
                    continue;
                }
            }
        }
        let text = flatten_text(object.get("content"));
        if text.is_empty() {
            continue;
        }
        out.push(json!({
            "role": role,
            "content": [{"type": "text", "text": text}],
        }));
    }
    if out.is_empty() {
        return Err(ChannelError::Prepare("messages is empty".into()));
    }
    Ok(out)
}

fn native_tool_call(call: &Value) -> Option<Value> {
    let function = call.get("function").unwrap_or(call);
    let name = function.get("name")?.as_str()?;
    let arguments = match function.get("arguments") {
        Some(Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => "{}".into(),
    };
    Some(json!({
        "id": call.get("id").and_then(Value::as_str).unwrap_or("call"),
        "type": "function",
        "function_call": {"name": name, "arguments": arguments},
    }))
}

fn to_native_tools(tools: &Value) -> Option<Vec<Value>> {
    let list = tools.as_array()?;
    let out: Vec<Value> = list
        .iter()
        .filter_map(|tool| {
            let function = tool.get("function").unwrap_or(tool);
            let name = function.get("name")?.as_str()?;
            // Trae requires `parameters` as a JSON string; an object fails 500.
            let schema = function
                .get("parameters")
                .or_else(|| function.get("input_schema"))
                .map(|value| match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| "{}".into());
            Some(json!({
                "type": "function",
                "function": {
                    "name": name,
                    "description": function.get("description").and_then(Value::as_str).unwrap_or(""),
                    "parameters": schema,
                }
            }))
        })
        .collect();
    Some(out)
}

fn flatten_text(content: Option<&Value>) -> String {
    match content {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(block_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

fn block_text(block: &Value) -> Option<String> {
    if let Some(text) = block.as_str() {
        return Some(text.to_owned());
    }
    let object = block.as_object()?;
    let kind = object.get("type").and_then(Value::as_str).unwrap_or("text");
    match kind {
        "text" | "input_text" | "output_text" => {
            object.get("text").and_then(Value::as_str).map(str::to_owned)
        }
        "image_url" | "input_image" | "image" => Some("[image omitted]".into()),
        "tool_result" => Some(flatten_text(object.get("content"))),
        "tool_use" => Some(
            json!({"tool_call": object.get("name"), "input": object.get("input")}).to_string(),
        ),
        _ => None,
    }
}
