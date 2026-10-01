use std::collections::BTreeMap;

use bytes::Bytes;
use gproxy_channel_api::{ChannelError, Frame, StreamCtx, StreamDecoder, StreamEnd, StreamTail};
use serde_json::{json, Value};

use super::{auth, model};

pub(super) fn decoder(ctx: StreamCtx<'_>) -> Option<Box<dyn StreamDecoder>> {
    model::is_chat(ctx.key)
        .then(|| Box::new(ChatDecoder::new(ctx.request_body.clone())) as Box<dyn StreamDecoder>)
}

#[derive(Default)]
struct ToolAcc {
    id: String,
    name: String,
    arguments: String,
}

pub(super) struct ChatDecoder {
    buffer: Vec<u8>,
    model: String,
    id: String,
    created: i64,
    first: bool,
    finish: Option<String>,
    usage: Option<Value>,
    tools: BTreeMap<i64, ToolAcc>,
    done: bool,
}

impl ChatDecoder {
    fn new(request_body: Bytes) -> Self {
        let model = serde_json::from_slice::<Value>(&request_body)
            .ok()
            .and_then(|value| value.get("config_name").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        let id = format!("chatcmpl-{}", super::prepare::token());
        let created = auth::now_secs();
        Self {
            buffer: Vec::new(),
            model,
            id,
            created,
            first: true,
            finish: None,
            usage: None,
            tools: BTreeMap::new(),
            done: false,
        }
    }

    fn chunk(&self, delta: Value, finish: Option<&str>, usage: Option<&Value>) -> Frame {
        let mut payload = json!({
            "id": self.id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
        });
        if let Some(usage) = usage {
            payload["usage"] = usage.clone();
        }
        Frame(Bytes::from(format!("data: {payload}\n\n")))
    }

    fn drain(&mut self) -> Vec<Frame> {
        let mut frames = Vec::new();
        while let Some(pos) = find_event_end(&self.buffer) {
            let raw: Vec<u8> = self.buffer.drain(..pos).collect();
            // trailing delimiter already removed by drain(..pos.0); drop blank lines
            let text = String::from_utf8_lossy(&raw).into_owned();
            frames.extend(self.handle(&text));
        }
        frames
    }

    fn handle(&mut self, text: &str) -> Vec<Frame> {
        let (mut event, mut data) = (String::new(), String::new());
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                event = rest.trim().to_owned();
            } else if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(rest.trim());
            }
        }
        let payload: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
        let mut frames = Vec::new();
        match event.as_str() {
            "output" => {
                let mut delta = serde_json::Map::new();
                if let Some(reasoning) = payload.get("reasoning_content").and_then(Value::as_str) {
                    if !reasoning.is_empty() {
                        delta.insert("reasoning_content".into(), json!(reasoning));
                    }
                }
                if let Some(content) = payload.get("response").and_then(Value::as_str) {
                    if !content.is_empty() {
                        delta.insert("content".into(), json!(content));
                    }
                }
                if let Some(calls) = payload.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        accumulate(&mut self.tools, call);
                    }
                }
                if !delta.is_empty() {
                    if self.first {
                        delta.insert("role".into(), json!("assistant"));
                        self.first = false;
                    }
                    frames.push(self.chunk(Value::Object(delta), None, None));
                }
            }
            "token_usage" => self.usage = Some(openai_usage(&payload)),
            "done" => {
                if let Some(reason) = payload.get("finish_reason").and_then(Value::as_str) {
                    self.finish = Some(reason.to_owned());
                }
            }
            "error" => {
                frames.push(error_frame(&data));
                self.done = true;
            }
            "message" | "data" => {
                let code = payload.get("code").and_then(Value::as_i64);
                if code.is_some() && code != Some(0) {
                    let message = payload
                        .get("error")
                        .or_else(|| payload.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or("upstream error");
                    frames.push(error_frame(message));
                    self.done = true;
                }
            }
            _ => {}
        }
        frames
    }

    fn tail_frames(&mut self) -> Vec<Frame> {
        let mut frames = Vec::new();
        if self.done {
            return frames;
        }
        for (index, entry) in std::mem::take(&mut self.tools) {
            if entry.name.is_empty() {
                continue;
            }
            if self.first {
                frames.push(self.chunk(json!({"role": "assistant"}), None, None));
                self.first = false;
            }
            frames.push(self.chunk(
                json!({"tool_calls": [{
                    "index": index,
                    "id": entry.id,
                    "type": "function",
                    "function": {"name": entry.name, "arguments": if entry.arguments.is_empty() { "{}".to_owned() } else { entry.arguments }},
                }]}),
                None,
                None,
            ));
        }
        let finish = self
            .finish
            .clone()
            .unwrap_or_else(|| if self.tools.is_empty() { "stop".into() } else { "tool_calls".into() });
        let usage = self.usage.clone();
        frames.push(self.chunk(json!({}), Some(&finish), usage.as_ref()));
        frames.push(Frame(Bytes::from_static(b"data: [DONE]\n\n")));
        frames
    }
}

impl StreamDecoder for ChatDecoder {
    fn push(&mut self, chunk: Bytes) -> Result<Vec<Frame>, ChannelError> {
        self.buffer.extend_from_slice(&chunk);
        if self.buffer.len() > 100 * 1024 * 1024 {
            return Err(ChannelError::Decode("kylintrae SSE event exceeds 100 MiB".into()));
        }
        Ok(self.drain())
    }

    fn finish(&mut self, end: StreamEnd) -> Result<StreamTail, ChannelError> {
        let mut frames = Vec::new();
        if end == StreamEnd::Complete {
            if !self.buffer.is_empty() {
                let raw = std::mem::take(&mut self.buffer);
                frames.extend(self.handle(&String::from_utf8_lossy(&raw)));
            }
            frames.extend(self.tail_frames());
        }
        Ok(StreamTail {
            frames,
            usage: self.usage.as_ref().and_then(usage_of),
            actual_service_tier: None,
        })
    }
}

/// Return (event_byte_len_including_the_delimiter,). SSE events end on a blank
/// line; tolerate both LF and CRLF.
fn find_event_end(buffer: &[u8]) -> Option<usize> {
    let mut index = 0;
    while index + 1 < buffer.len() {
        if buffer[index] == b'\n' && buffer[index + 1] == b'\n' {
            return Some(index + 2);
        }
        if index + 3 < buffer.len()
            && &buffer[index..index + 4] == b"\r\n\r\n"
        {
            return Some(index + 4);
        }
        index += 1;
    }
    None
}

fn accumulate(tools: &mut BTreeMap<i64, ToolAcc>, call: &Value) {
    let function = call.get("function_call").cloned().unwrap_or(Value::Null);
    let index = call.get("index").and_then(Value::as_i64).unwrap_or(0);
    let name = function.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = match function.get("arguments") {
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    let entry = tools.entry(index).or_default();
    if let Some(id) = call.get("id").and_then(Value::as_str) {
        entry.id = id.to_owned();
    }
    if !name.is_empty() {
        entry.name = name.to_owned();
    }
    entry.arguments.push_str(&arguments);
    if entry.id.is_empty() {
        entry.id = format!("call_{index}");
    }
}

fn openai_usage(usage: &Value) -> Value {
    let prompt = usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
    let completion = usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
    let total = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(prompt + completion);
    json!({"prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": total})
}

fn usage_of(usage: &Value) -> Option<gproxy_channel_api::NormalizedUsage> {
    let input = usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
    (input != 0 || output != 0).then(|| gproxy_channel_api::NormalizedUsage {
        input_tokens: input,
        output_tokens: output,
        cached_input_tokens: 0,
        metrics: Default::default(),
        dimensions: Default::default(),
        attempts: Vec::new(),
    })
}

fn error_frame(message: &str) -> Frame {
    let body = json!({"error": {"message": message, "type": "upstream_error"}});
    Frame(Bytes::from(format!("data: {body}\n\n")))
}

/// Collapse a buffered upstream SSE body into one OpenAI chat.completion object,
/// used when the client asked for a non-streaming response.
pub(super) fn collect_completion(body: &[u8]) -> Bytes {
    let text = String::from_utf8_lossy(body);
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut finish = String::from("stop");
    let mut usage: Option<Value> = None;
    let mut tools: BTreeMap<i64, ToolAcc> = BTreeMap::new();
    for event in text.split("\n\n") {
        let (mut name, mut data) = (String::new(), String::new());
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
        let payload: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        match name.as_str() {
            "output" => {
                if let Some(part) = payload.get("response").and_then(Value::as_str) {
                    content.push_str(part);
                }
                if let Some(part) = payload.get("reasoning_content").and_then(Value::as_str) {
                    reasoning.push_str(part);
                }
                if let Some(calls) = payload.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        accumulate(&mut tools, call);
                    }
                }
            }
            "token_usage" => usage = Some(openai_usage(&payload)),
            "done" => {
                if let Some(reason) = payload.get("finish_reason").and_then(Value::as_str) {
                    finish = reason.to_owned();
                }
            }
            _ => {}
        }
    }
    let mut message = serde_json::Map::new();
    message.insert("role".into(), json!("assistant"));
    message.insert("content".into(), json!(content));
    if !reasoning.is_empty() {
        message.insert("reasoning_content".into(), json!(reasoning));
    }
    let calls: Vec<Value> = tools
        .into_iter()
        .filter(|(_, entry)| !entry.name.is_empty())
        .map(|(_, entry)| {
            json!({
                "id": entry.id,
                "type": "function",
                "function": {"name": entry.name, "arguments": if entry.arguments.is_empty() { "{}".to_owned() } else { entry.arguments }},
            })
        })
        .collect();
    if !calls.is_empty() {
        message.insert("tool_calls".into(), Value::Array(calls));
    }
    let mut completion = json!({
        "id": format!("chatcmpl-{}", super::prepare::token()),
        "object": "chat.completion",
        "created": auth::now_secs(),
        "model": "",
        "choices": [{"index": 0, "message": Value::Object(message), "finish_reason": finish}],
    });
    completion["usage"] = usage.unwrap_or_else(|| json!({"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}));
    Bytes::from(completion.to_string())
}

