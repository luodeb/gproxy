use gproxy_protocol::openai;

use crate::TransformError;

mod parts;
mod tool_output;

use parts::response_part_to_chat;

pub(super) fn output_to_chat(
    output: openai::ResponseOutput,
) -> Result<openai::ChatTextContent, TransformError> {
    tool_output::output_to_chat(output)
}

pub(super) fn user_text(text: String) -> openai::ChatCompletionMessageParam {
    openai::ChatCompletionMessageParam::User(crate::wire!(openai::ChatUserMessageParam {
        role: openai::ChatUserRole::User,
        content: openai::ChatContent::Text(text),
        name: None,
        rest: Default::default(),
    }))
}

pub(super) fn user_content(
    content: openai::ResponseEasyInputContent,
) -> Result<openai::ChatCompletionMessageParam, TransformError> {
    Ok(openai::ChatCompletionMessageParam::User(crate::wire!(
        openai::ChatUserMessageParam {
            role: openai::ChatUserRole::User,
            content: easy_user(content)?,
            name: None,
            rest: Default::default(),
        }
    )))
}

pub(super) fn text_message(
    content: openai::ResponseEasyInputContent,
    role: openai::ResponseEasyInputMessageRole,
) -> Result<openai::ChatCompletionMessageParam, TransformError> {
    // Developer maps to `system`, not the OpenAI-modern `developer` role: the
    // classic set is what every OpenAI-compatible upstream accepts, and a
    // mid-conversation developer message is semantically a system note.
    match role {
        openai::ResponseEasyInputMessageRole::User => return user_content(content),
        openai::ResponseEasyInputMessageRole::System
        | openai::ResponseEasyInputMessageRole::Developer => {
            let content = easy_text(content)?;
            Ok(openai::ChatCompletionMessageParam::System(
                openai::ChatSystemMessageParam {
                    role: openai::ChatSystemRole::System,
                    content,
                    name: None,
                    rest: Default::default(),
                },
            ))
        }
        openai::ResponseEasyInputMessageRole::Assistant => {
            let content = easy_text(content)?;
            let content = match content {
                openai::ChatTextContent::Text(text) => openai::ChatAssistantContent::Text(text),
                openai::ChatTextContent::Parts(parts) => openai::ChatAssistantContent::Text(
                    parts
                        .into_iter()
                        .filter_map(|part| match part {
                            openai::ChatTextContentPart::Text(part) => Some(part.text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                other => openai::ChatAssistantContent::Unknown(serde_json::Value::Null),
            };
            #[allow(unreachable_patterns)]
            let _ = &content;
            Ok(openai::ChatCompletionMessageParam::Assistant(crate::wire!(
                openai::ChatAssistantMessageParam {
                    role: openai::ChatAssistantRole::Assistant,
                    content: Some(content),
                    audio: None,
                    function_call: None,
                    name: None,
                    reasoning_content: None,
                    refusal: None,
                    tool_calls: None,
                    rest: Default::default(),
                }
            )))
        }
        #[cfg(not(feature = "exhaustive"))]
        _ => Err(crate::TransformError::unsupported(
            "protocol enum",
            "unrecognized external variant",
        )),
    }
}

pub(super) fn easy_text(
    content: openai::ResponseEasyInputContent,
) -> Result<openai::ChatTextContent, TransformError> {
    match content {
        openai::ResponseEasyInputContent::Text(text) => Ok(openai::ChatTextContent::Text(text)),
        openai::ResponseEasyInputContent::Parts(parts) => Ok(openai::ChatTextContent::Parts(
            parts
                .into_iter()
                .map(|part| match part {
                    openai::ResponseInputContentPart::InputText(part) => Ok(
                        openai::ChatTextContentPart::Text(crate::wire!(openai::ChatTextPart {
                            type_: openai::ChatTextPartType::Text,
                            text: part.text,
                            prompt_cache_breakpoint: part.prompt_cache_breakpoint,
                            rest: Default::default(),
                        })),
                    ),
                    unsupported @ (openai::ResponseInputContentPart::InputImage(_)
                    | openai::ResponseInputContentPart::InputFile(_)
                    | openai::ResponseInputContentPart::InputAudio(_)) => {
                        Err(TransformError::unsupported(
                            "Responses text content",
                            serde_json::to_string(&unsupported)?,
                        ))
                    }
                    #[cfg(not(feature = "exhaustive"))]
                    _ => {
                        return Err(crate::TransformError::unsupported(
                            "protocol enum",
                            "unrecognized external variant",
                        ));
                    }
                })
                .collect::<Result<_, _>>()?,
        )),
        openai::ResponseEasyInputContent::Unknown(raw) => Err(TransformError::unsupported(
            "OpenAI Responses text content",
            raw.to_string(),
        )),
        openai::ResponseEasyInputContent::OutputParts(parts) => {
            Ok(openai::ChatTextContent::Parts(output_text_parts(parts)))
        }
        #[cfg(not(feature = "exhaustive"))]
        _ => {
            return Err(crate::TransformError::unsupported(
                "protocol enum",
                "unrecognized external variant",
            ));
        }
    }
}

pub(super) fn easy_user(
    content: openai::ResponseEasyInputContent,
) -> Result<openai::ChatContent, TransformError> {
    match content {
        openai::ResponseEasyInputContent::Text(text) => Ok(openai::ChatContent::Text(text)),
        openai::ResponseEasyInputContent::Parts(parts) => Ok(openai::ChatContent::Parts(
            parts
                .into_iter()
                .map(response_part_to_chat)
                .collect::<Result<_, _>>()?,
        )),
        openai::ResponseEasyInputContent::Unknown(raw) => Err(TransformError::unsupported(
            "OpenAI Responses user content",
            raw.to_string(),
        )),
        openai::ResponseEasyInputContent::OutputParts(parts) => Ok(openai::ChatContent::Parts(
            output_text_parts(parts)
                .into_iter()
                .filter_map(|part| match part {
                    openai::ChatTextContentPart::Text(part) => {
                        Some(openai::ChatContentPart::Text(part))
                    }
                    openai::ChatTextContentPart::Unknown(_) => None,
                    #[cfg(not(feature = "exhaustive"))]
                    _ => None,
                })
                .collect(),
        )),
        #[cfg(not(feature = "exhaustive"))]
        _ => {
            return Err(crate::TransformError::unsupported(
                "protocol enum",
                "unrecognized external variant",
            ));
        }
    }
}

fn output_text_parts(
    parts: Vec<openai::ResponseMessageOutputContentPart>,
) -> Vec<openai::ChatTextContentPart> {
    parts
        .into_iter()
        .filter_map(|part| match part {
            openai::ResponseMessageOutputContentPart::OutputText(part) => Some(
                openai::ChatTextContentPart::Text(crate::wire!(openai::ChatTextPart {
                    type_: openai::ChatTextPartType::Text,
                    text: part.text,
                    prompt_cache_breakpoint: None,
                    rest: Default::default(),
                })),
            ),
            openai::ResponseMessageOutputContentPart::Refusal(part) => Some(
                openai::ChatTextContentPart::Text(crate::wire!(openai::ChatTextPart {
                    type_: openai::ChatTextPartType::Text,
                    text: part.refusal,
                    prompt_cache_breakpoint: None,
                    rest: Default::default(),
                })),
            ),
            openai::ResponseMessageOutputContentPart::Unknown(_) => None,
            #[cfg(not(feature = "exhaustive"))]
            _ => None,
        })
        .collect()
}

pub(super) fn easy_assistant(
    content: openai::ResponseEasyInputContent,
) -> Result<openai::ChatAssistantContent, TransformError> {
    match content {
        openai::ResponseEasyInputContent::Text(text) => {
            Ok(openai::ChatAssistantContent::Text(text))
        }
        openai::ResponseEasyInputContent::OutputParts(parts) => Ok(output_content(parts)),
        openai::ResponseEasyInputContent::Unknown(raw) => Err(TransformError::unsupported(
            "OpenAI Responses assistant content",
            raw.to_string(),
        )),
        unsupported @ openai::ResponseEasyInputContent::Parts(_) => {
            Err(TransformError::unsupported(
                "Responses assistant content",
                serde_json::to_string(&unsupported)?,
            ))
        }
        #[cfg(not(feature = "exhaustive"))]
        _ => {
            return Err(crate::TransformError::unsupported(
                "protocol enum",
                "unrecognized external variant",
            ));
        }
    }
}

pub(super) fn output_content(
    parts: Vec<openai::ResponseMessageOutputContentPart>,
) -> openai::ChatAssistantContent {
    openai::ChatAssistantContent::Parts(
        parts
            .into_iter()
            .filter_map(|part| match part {
                openai::ResponseMessageOutputContentPart::OutputText(part) => Some(
                    openai::ChatAssistantContentPart::Text(crate::wire!(openai::ChatTextPart {
                        type_: openai::ChatTextPartType::Text,
                        text: part.text,
                        prompt_cache_breakpoint: None,
                        rest: Default::default(),
                    })),
                ),
                openai::ResponseMessageOutputContentPart::Refusal(part) => {
                    Some(openai::ChatAssistantContentPart::Refusal(crate::wire!(
                        openai::ChatRefusalPart {
                            type_: openai::ChatRefusalPartType::Refusal,
                            refusal: part.refusal,
                            prompt_cache_breakpoint: None,
                            rest: Default::default(),
                        }
                    )))
                }
                openai::ResponseMessageOutputContentPart::Unknown(_) => None,
                #[cfg(not(feature = "exhaustive"))]
                _ => None,
            })
            .collect(),
    )
}
