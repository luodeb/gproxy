use gproxy_channel_api::{ChannelError, PrepareCtx, PreparedRequest};
use gproxy_protocol::StreamFraming;
use http::header::{HeaderName, HeaderValue};
use serde_json::{json, Value};

use super::{auth, model, translate};

pub(super) fn request(ctx: PrepareCtx<'_>) -> Result<PreparedRequest, ChannelError> {
    if !model::is_chat(ctx.key) {
        return Err(ChannelError::Prepare(
            "kylintrae supports OpenAI Chat Completions only".into(),
        ));
    }
    let jwt = auth::access_token(ctx.secret)?.to_owned();
    let login = auth::refresh_token(ctx.secret)?.to_owned();
    let base = auth::base_url(ctx.provider_settings).to_owned();
    let (config, raw) = model::resolve(ctx.upstream_model);
    let session_id = translate::session_id(&login, &config, &raw);
    let body = translate::request_body(ctx.body, &config, &raw, &session_id)?;

    let uri = crate::shared::http::join(&base, "/api/ide/v2/llm_raw_chat", None)?;
    let mut request = http::Request::builder()
        .method(http::Method::POST)
        .uri(crate::shared::http::strip_userinfo(uri)?)
        .body(body)
        .map_err(|error| ChannelError::Prepare(error.to_string()))?;
    *request.headers_mut() = headers(ctx.provider_settings, &jwt, &session_id, &config, &raw)?;
    Ok(PreparedRequest {
        request,
        framing: Some(StreamFraming::Sse),
        websocket: false,
        profile: None,
    })
}

fn headers(
    settings: &Value,
    jwt: &str,
    session_id: &str,
    config: &str,
    raw: &str,
) -> Result<http::HeaderMap, ChannelError> {
    let base = auth::base_url(settings);
    let extra = json!({
        "agent_loop_id": session_id,
        "api_host": base,
        "api_key": jwt,
        "base_url": format!("{}/trae-cli/api/v1/llm/proxy", base.trim_end_matches('/')),
        "config_name": config,
        "config_source": 1,
        "display_name": config,
        "model_name": raw,
        "real_api_key": "",
        "real_base_url": "",
        "session_id": session_id,
        "user_prompt_submit_id": session_id,
    })
    .to_string();

    let mut headers = http::HeaderMap::new();
    let pairs = [
        (http::header::CONTENT_TYPE, "application/json".to_owned()),
        (http::header::ACCEPT, "text/event-stream".to_owned()),
        (http::header::CONNECTION, "keep-alive".to_owned()),
        (
            HeaderName::from_static("authorization"),
            format!("Cloud-IDE-JWT {jwt}"),
        ),
        (HeaderName::from_static("x-cloudide-token"), jwt.to_owned()),
        (HeaderName::from_static("x-ide-token"), jwt.to_owned()),
        (HeaderName::from_static("x-app-id"), auth::app_id(settings).to_owned()),
        (
            HeaderName::from_static("x-ide-version"),
            setting(settings, "ide_version", "3.4.0").to_owned(),
        ),
        (
            HeaderName::from_static("x-ide-version-code"),
            setting(settings, "ide_version_code", "20991231").to_owned(),
        ),
        (
            HeaderName::from_static("x-ide-function"),
            setting(settings, "ide_function", "chat").to_owned(),
        ),
        (
            HeaderName::from_static("x-request-id"),
            uuid_like(),
        ),
        (HeaderName::from_static("extra"), extra),
    ];
    for (name, value) in pairs {
        let value = HeaderValue::from_str(&value)
            .map_err(|error| ChannelError::Prepare(format!("header {name}: {error}")))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn setting<'a>(settings: &'a Value, key: &str, default: &'a str) -> &'a str {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
}

fn uuid_like() -> String {
    token()
}

pub(super) fn token() -> String {
    use sha2::{Digest, Sha256};
    let seed = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
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
