use serde_json::json;

#[test]
fn session_id_is_stable_and_uuid_shaped() {
    let first = super::translate::session_id("trae-lt-x", "glm-5.3", "glm-5.3__dev");
    let second = super::translate::session_id("trae-lt-x", "glm-5.3", "glm-5.3__dev");
    assert_eq!(first, second);
    assert_eq!(first.len(), 36);
    assert_eq!(first.matches('-').count(), 4);
    assert_ne!(
        first,
        super::translate::session_id("trae-lt-y", "glm-5.3", "glm-5.3__dev")
    );
}

#[test]
fn request_body_is_the_six_field_native_shape() {
    let body = json!({
        "model": "glm-5.3",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{"type": "function", "function": {"name": "f", "description": "", "parameters": {"type": "object"}}}],
    });
    let out = super::translate::request_body(
        &serde_json::to_vec(&body).unwrap(),
        "glm-5.3",
        "glm-5.3__dev",
        "sid",
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["config_name"], "glm-5.3");
    assert_eq!(value["model_name"], "glm-5.3__dev");
    assert_eq!(value["stream"], true);
    assert_eq!(value["messages"][0]["role"], "user");
    // parameters must be a JSON string, not an object
    assert!(value["tools"][0]["function"]["parameters"].is_string());
}

#[test]
fn collect_completion_merges_output_and_usage() {
    let sse = concat!(
        "event: output\ndata: {\"response\":\"Hel\"}\n\n",
        "event: output\ndata: {\"response\":\"lo\"}\n\n",
        "event: token_usage\ndata: {\"prompt_tokens\":3,\"completion_tokens\":2}\n\n",
        "event: done\ndata: {\"finish_reason\":\"stop\"}\n\n",
    );
    let out = super::sse::collect_completion(sse.as_bytes());
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["choices"][0]["message"]["content"], "Hello");
    assert_eq!(value["choices"][0]["finish_reason"], "stop");
    assert_eq!(value["usage"]["prompt_tokens"], 3);
    assert_eq!(value["usage"]["completion_tokens"], 2);
}
