use gproxy_channel_api::ModelInfo;
use gproxy_protocol::{ContentGenerationKind, OperationKey, OperationKind};

/// TRAE enterprise (trae.kylinos.cn) model catalog: public config name → the raw
/// `model_name` the endpoint binds to. Mirrors the tenant catalog in trae-hub.
/// Unknown names pass through unchanged so an unregistered config still reaches
/// the upstream, which reports the real error.
const MODELS: &[(&str, &str)] = &[
    ("Doubao-Seed-Evolving", "Doubao-Seed-Evolving__dev"),
    ("Doubao-Seed-2.1-pro", "Doubao-Seed-2.1-pro__dev"),
    ("Doubao-Seed-2.1-turbo", "Doubao-Seed-2.1-turbo__dev"),
    ("Doubao-Seed-2.0-Code", "Doubao-Seed-2.0-Code__dev"),
    ("Doubao_1_6", "Doubao_1_6__dev"),
    ("glm-5.3", "glm-5.3__dev"),
    ("glm-5.3-flash", "glm-5.3-flash__dev"),
    ("glm-5.3-flashx", "glm-5.3-flashx__dev"),
    ("glm-5.2", "glm-5.2__dev"),
    ("glm-5.1", "glm-5.1__dev"),
    ("glm-5v-turbo", "glm-5v-turbo__dev"),
    ("glm-5", "glm-5__v2"),
    ("minimax-m3", "minimax-m3__dev"),
    ("minimax-m2.7", "minimax-m2.7__dev"),
    ("qwen3.8-max", "qwen3.8-max__dev"),
    ("qwen-3.7-plus", "qwen-3.7-plus__dev"),
    ("kimi-k2.7-code", "kimi-k2.7-code__dev"),
    ("kimi-k3", "kimi-k3__dev"),
    ("kimi-k2.8-preview", "kimi-k2.8-preview__dev"),
    ("DeepSeek-V4-Pro-Official", "DeepSeek-V4-Pro-Official__dev"),
    ("deepseek-V4-Pro", "deepseek-V4-Pro__dev"),
    ("DeepSeek-V4-Flash-Official", "deepseek_v4_flash_official__dev"),
    ("DeepSeek-V4-Flash", "DeepSeek-V4-Flash__dev"),
    ("DeepSeek-V4.1-Flash", "DeepSeek-V4.1-Flash__dev"),
    ("step-5-preview", "step-5-preview__dev"),
];

pub(super) fn resolve(model: &str) -> (String, String) {
    let trimmed = model.trim();
    let key = trimmed.to_lowercase();
    for (config, raw) in MODELS {
        if config.to_lowercase() == key {
            return ((*config).to_owned(), (*raw).to_owned());
        }
    }
    (trimmed.to_owned(), trimmed.to_owned())
}

pub(super) fn is_chat(key: OperationKey) -> bool {
    key.kind() == OperationKind::ContentGeneration(ContentGenerationKind::OpenAiChat)
}

pub(super) fn local_models() -> Vec<ModelInfo> {
    MODELS
        .iter()
        .map(|(config, _)| ModelInfo {
            id: (*config).to_owned(),
            display_name: Some((*config).to_owned()),
            context_window: Some(168_000),
            max_output_tokens: Some(32_000),
            thinking_supported: None,
            thinking_adaptive_supported: None,
            thinking_enabled_supported: None,
            metadata: Default::default(),
        })
        .collect()
}
