use bytes::Bytes;
use gproxy_channel_api::{ChannelError, SimpleHttp};
use serde_json::{json, Value};

pub(super) const DEFAULT_BASE_URL: &str = "https://trae.kylinos.cn";

pub(super) fn refresh_token(secret: &Value) -> Result<&str, ChannelError> {
    secret
        .get("refresh_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ChannelError::Secret("refresh_token missing".into()))
}

pub(super) fn access_token(secret: &Value) -> Result<&str, ChannelError> {
    secret
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ChannelError::Secret("access_token missing (refresh pending)".into()))
}

fn expires_at(secret: &Value) -> Option<i64> {
    secret.get("expires_at").and_then(Value::as_i64)
}

pub(super) fn now_secs() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// A JWT lives ~14 days; refresh an hour before it lapses.
pub(super) fn refresh_due(secret: &Value) -> Option<i64> {
    let Some(exp) = expires_at(secret) else {
        return Some(0);
    };
    (exp - now_secs() < 3_600).then_some((exp - 3_600).max(0))
}

pub(super) async fn refresh(
    secret: &Value,
    base_url: &str,
    http: &dyn SimpleHttp,
) -> Result<Value, ChannelError> {
    let token = refresh_token(secret)?.to_owned();
    let url = format!(
        "{}/cloudide/api/v3/trae/oauth/ExchangeToken",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::to_vec(&json!({ "RefreshToken": token }))
        .map_err(|error| ChannelError::Refresh(error.to_string()))?;
    let request = http::Request::builder()
        .method(http::Method::POST)
        .uri(url)
        .header(http::header::CONTENT_TYPE, "application/json")
        .header("x-cloudide-token", &token)
        .header(http::header::USER_AGENT, "Go-http-client/1.1")
        .body(Bytes::from(body))
        .map_err(|error| ChannelError::Refresh(error.to_string()))?;
    let response = http.send(request).await?;
    let value: Value = serde_json::from_slice(&response.into_body())
        .map_err(|error| ChannelError::Refresh(format!("exchange response: {error}")))?;
    if value.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(ChannelError::Refresh(format!(
            "token exchange rejected: {value}"
        )));
    }
    let data = value.get("Data").cloned().unwrap_or(Value::Null);
    let jwt = data
        .get("Token")
        .and_then(Value::as_str)
        .ok_or_else(|| ChannelError::Refresh("exchange returned no token".into()))?;
    let expires_ms = data
        .get("TokenExpireAt")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let mut updated = secret.clone();
    if let Some(object) = updated.as_object_mut() {
        object.insert("access_token".into(), json!(jwt));
        object.insert("expires_at".into(), json!(expires_ms / 1_000));
    }
    Ok(updated)
}

pub(super) fn base_url(settings: &Value) -> &str {
    settings
        .get("base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(DEFAULT_BASE_URL)
}

pub(super) fn app_id(settings: &Value) -> &str {
    settings
        .get("app_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("7b3f9dc2-8a4e-5c6d-2f1b-9e4a3c5b7df0")
}
