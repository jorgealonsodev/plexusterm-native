//! Request shapes and answer parsing for each provider.

use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::opencode::GoFormat;
use crate::text::Exchange;
use crate::{AiConfig, AiError, ApiKey, Endpoints, Provider, Task};

/// A header value; key headers are wiped from memory when the call is dropped.
pub type HeaderText = Zeroizing<String>;

/// The answer format of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Anthropic,
    Gemini,
    OpenAiChat,
    OpenAiResponses,
}

/// One HTTP request ready to send.
pub struct Call {
    pub url: String,
    pub headers: Vec<(&'static str, HeaderText)>,
    pub body: Value,
    pub shape: Shape,
}

/// What to ask: the feature, its system prompt, earlier turns of the chat (oldest first)
/// and the user's new message.
#[derive(Clone, Copy)]
pub struct Prompt<'a> {
    pub task: Task,
    pub system: &'a str,
    pub turns: &'a [Exchange],
    pub user: &'a str,
}

/// OpenCode's session header (one stable ID per conversation).
pub const SESSION_HEADER: &str = "x-opencode-session";
const MAX_SESSION_ID: usize = 128;

/// A session ID of 1-128 characters of `A-Z a-z 0-9 . _ -`, or a config error.
fn session_id(config: &AiConfig) -> Result<&str, AiError> {
    let id = config.session_id.as_deref().unwrap_or_default();
    let valid = !id.is_empty()
        && id.len() <= MAX_SESSION_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(id)
    } else {
        Err(AiError::config(
            "OpenCode needs a session ID of 1-128 letters, digits, '.', '_' or '-'.",
        ))
    }
}

/// Whether an OpenAI-compatible base URL points at OpenCode (`opencode.ai` or a subdomain).
fn is_opencode_host(base_url: &str) -> bool {
    reqwest::Url::parse(base_url.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "opencode.ai" || host.ends_with(".opencode.ai"))
}

fn text(value: &str) -> HeaderText {
    Zeroizing::new(value.to_owned())
}

/// `Bearer <key>`, built without an intermediate copy of the key.
pub(crate) fn bearer(key: &ApiKey) -> HeaderText {
    let mut value = Zeroizing::new(String::with_capacity(7 + key.expose().len()));
    value.push_str("Bearer ");
    value.push_str(key.expose());
    value
}

/// Sets `key` of a JSON object built with `json!({..})` (anything else is left unchanged).
fn set(object: &mut Value, key: &str, value: Value) {
    if let Some(object) = object.as_object_mut() {
        object.insert(key.to_owned(), value);
    }
}

/// What a successful (2xx) answer contained.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Text(String),
    /// The model declined (safety refusal, blocked prompt).
    Refused,
    /// The token limit cut the answer off.
    Truncated,
}

/// Anthropic Messages API version header value.
pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Beta that enables `fallbacks: "default"` (server-side retry on a recommended model).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Output budgets. Thinking counts toward Anthropic's `max_tokens`, so it gets more room
/// (assistant answers can be several paragraphs long).
fn max_tokens(provider: Provider, task: Task) -> u32 {
    match (task, provider) {
        (Task::Complete, Provider::Anthropic) => 2048,
        (Task::Complete, _) => 1024,
        (Task::Assist, Provider::Anthropic) => 16000,
        (Task::Assist, _) => 8192,
    }
}

/// Older Claude models reject `output_config` (effort and structured outputs).
fn anthropic_supports_output_config(model: &str) -> bool {
    const LEGACY: [&str; 7] = [
        "claude-3",
        "claude-haiku-4",
        "claude-sonnet-4-0",
        "claude-sonnet-4-2",
        "claude-sonnet-4-5",
        "claude-opus-4-0",
        "claude-opus-4-1",
    ];
    !LEGACY.iter().any(|prefix| model.starts_with(prefix))
}

/// `fallbacks: "default"` is sent only to the Claude Opus 5.5 family.
fn anthropic_uses_default_fallbacks(model: &str) -> bool {
    model == "claude-opus-5-5" || model.starts_with("claude-opus-5-5-")
}

/// The assistant's answer (Anthropic structured outputs and OpenAI strict mode): every
/// property required, so the optional command is a nullable string.
fn assist_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string", "description": "The answer, in the user's language."},
            "command": {
                "anyOf": [{"type": "string"}, {"type": "null"}],
                "description": "One shell command line that does what was asked, or null."
            }
        },
        "required": ["answer", "command"],
        "additionalProperties": false
    })
}

/// `base_url` of an OpenAI-compatible server (`http(s)://host[:port]/path`), without trailing `/`.
pub(crate) fn compatible_url(base_url: Option<&str>) -> Result<String, AiError> {
    let base = base_url.map(str::trim).unwrap_or_default();
    if base.is_empty() {
        return Err(AiError::config(
            "Enter the base URL of the OpenAI-compatible server (for example http://localhost:11434/v1).",
        ));
    }
    let invalid = || AiError::config(format!("\"{base}\" is not a valid http(s) base URL."));
    let url = reqwest::Url::parse(base).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return Err(invalid());
    }
    Ok(format!("{}/chat/completions", base.trim_end_matches('/')))
}

/// Gemini puts the model in the URL path, so only plain model codes are accepted.
fn gemini_model(model: &str) -> Result<&str, AiError> {
    let model = model.strip_prefix("models/").unwrap_or(model);
    let valid = model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if valid && !model.starts_with('.') {
        Ok(model)
    } else {
        Err(AiError::config(format!(
            "\"{model}\" is not a valid Gemini model name."
        )))
    }
}

/// The chat as `{role, content}` messages: earlier turns (user, then the assistant under
/// `assistant_role`) and the new user message last.
fn chat_messages(prompt: &Prompt<'_>, assistant_role: &str) -> Vec<Value> {
    let mut messages = Vec::with_capacity(prompt.turns.len() * 2 + 1);
    for turn in prompt.turns {
        messages.push(json!({"role": "user", "content": turn.user}));
        messages.push(json!({"role": assistant_role, "content": turn.assistant}));
    }
    messages.push(json!({"role": "user", "content": prompt.user}));
    messages
}

/// Chat Completions messages: the system prompt first, then the chat.
fn openai_messages(prompt: &Prompt<'_>) -> Vec<Value> {
    let mut messages = vec![json!({"role": "system", "content": prompt.system})];
    messages.extend(chat_messages(prompt, "assistant"));
    messages
}

/// Gemini `contents`: the chat with roles `user` and `model`, text in `parts`.
fn gemini_contents(prompt: &Prompt<'_>) -> Vec<Value> {
    chat_messages(prompt, "model")
        .into_iter()
        .map(|m| json!({"role": m.get("role"), "parts": [{"text": m.get("content")}]}))
        .collect()
}

/// Builds the request; `go_format` only matters to OpenCode Go.
pub fn build(
    endpoints: &Endpoints,
    config: &AiConfig,
    key: Option<&ApiKey>,
    prompt: Prompt<'_>,
    go_format: GoFormat,
) -> Result<Call, AiError> {
    let Prompt { task, system, .. } = prompt;
    let model = config.model.trim();
    if model.is_empty() {
        return Err(AiError::config("Choose a model in Settings > AI."));
    }
    let provider = config.provider;
    let limit = max_tokens(provider, task);
    let mut headers: Vec<(&'static str, HeaderText)> = Vec::new();
    let call = match provider {
        Provider::OpenCodeGo => return opencode_go(endpoints, config, key, prompt, go_format),
        Provider::Anthropic => {
            if let Some(key) = key {
                headers.push(("x-api-key", text(key.expose())));
            }
            headers.push(("anthropic-version", text(ANTHROPIC_VERSION)));
            let mut body = json!({
                "model": model,
                "max_tokens": limit,
                "system": system,
                "messages": chat_messages(&prompt, "assistant"),
            });
            if anthropic_supports_output_config(model) {
                let mut output = json!({
                    "effort": match task { Task::Complete => "low", Task::Assist => "medium" },
                });
                if task == Task::Assist {
                    set(
                        &mut output,
                        "format",
                        json!({"type": "json_schema", "schema": assist_schema()}),
                    );
                }
                set(&mut body, "output_config", output);
            }
            if anthropic_uses_default_fallbacks(model) {
                set(&mut body, "fallbacks", json!("default"));
                headers.push(("anthropic-beta", text(FALLBACK_BETA)));
            }
            Call {
                url: endpoints.anthropic.clone(),
                headers,
                body,
                shape: Shape::Anthropic,
            }
        }
        Provider::Gemini => {
            let model = gemini_model(model)?;
            if let Some(key) = key {
                headers.push(("x-goog-api-key", text(key.expose())));
            }
            let mut generation = json!({"maxOutputTokens": limit});
            if task == Task::Assist {
                set(&mut generation, "responseMimeType", json!("application/json"));
                set(
                    &mut generation,
                    "responseSchema",
                    json!({
                        "type": "OBJECT",
                        "properties": {
                            "answer": {"type": "STRING"},
                            "command": {"type": "STRING", "nullable": true}
                        },
                        "required": ["answer"]
                    }),
                );
            }
            Call {
                url: format!(
                    "{}/models/{model}:generateContent",
                    endpoints.gemini.trim_end_matches('/')
                ),
                headers,
                body: json!({
                    "systemInstruction": {"parts": [{"text": system}]},
                    "contents": gemini_contents(&prompt),
                    "generationConfig": generation,
                }),
                shape: Shape::Gemini,
            }
        }
        Provider::OpenAi | Provider::OpenAiCompatible => {
            let url = if provider == Provider::OpenAi {
                endpoints.openai.clone()
            } else {
                compatible_url(config.base_url.as_deref())?
            };
            if let Some(key) = key {
                headers.push(("authorization", bearer(key)));
            }
            let base = config.base_url.as_deref().unwrap_or_default();
            if provider == Provider::OpenAiCompatible && is_opencode_host(base) {
                headers.push((SESSION_HEADER, text(session_id(config)?)));
            }
            let mut body = json!({
                "model": model,
                "messages": openai_messages(&prompt),
            });
            if provider == Provider::OpenAi {
                // OpenAI deprecated `max_tokens` (reasoning models reject it).
                set(&mut body, "max_completion_tokens", json!(limit));
                if task == Task::Assist {
                    set(
                        &mut body,
                        "response_format",
                        json!({
                            "type": "json_schema",
                            "json_schema": {"name": "assistant_answer", "strict": true, "schema": assist_schema()}
                        }),
                    );
                }
            } else {
                // Ollama and LM Studio implement `max_tokens`; JSON is asked for in the prompt.
                set(&mut body, "max_tokens", json!(limit));
            }
            Call {
                url,
                headers,
                body,
                shape: Shape::OpenAiChat,
            }
        }
    };
    Ok(call)
}

/// OpenCode Go: one Bearer key and a session header, the body in the model's own API format.
/// Only portable fields are sent (no Claude-only `output_config`/`fallbacks`, no JSON schema).
fn opencode_go(
    endpoints: &Endpoints,
    config: &AiConfig,
    key: Option<&ApiKey>,
    prompt: Prompt<'_>,
    format: GoFormat,
) -> Result<Call, AiError> {
    let Prompt { task, system, user, .. } = prompt;
    let model = config.model.trim();
    let limit = max_tokens(Provider::OpenCodeGo, task);
    let base = endpoints.opencode_go.trim_end_matches('/');
    let mut headers: Vec<(&'static str, HeaderText)> = vec![(SESSION_HEADER, text(session_id(config)?))];
    if let Some(key) = key {
        headers.push(("authorization", bearer(key)));
    }
    let call = match format {
        GoFormat::Chat => Call {
            url: format!("{base}/chat/completions"),
            headers,
            body: json!({
                "model": model,
                "messages": openai_messages(&prompt),
                "max_tokens": limit,
            }),
            shape: Shape::OpenAiChat,
        },
        GoFormat::Messages => {
            // OpenCode's own Anthropic client sends the key as `x-api-key` on `/messages`.
            if let Some(key) = key {
                headers.push(("x-api-key", text(key.expose())));
            }
            headers.push(("anthropic-version", text(ANTHROPIC_VERSION)));
            Call {
                url: format!("{base}/messages"),
                headers,
                body: json!({
                    "model": model,
                    "max_tokens": limit,
                    "system": system,
                    "messages": chat_messages(&prompt, "assistant"),
                }),
                shape: Shape::Anthropic,
            }
        }
        GoFormat::Responses => Call {
            url: format!("{base}/responses"),
            headers,
            body: json!({
                "model": model,
                "instructions": system,
                // A plain string for a first question, the chat as message items after it.
                "input": if prompt.turns.is_empty() {
                    json!(user)
                } else {
                    json!(chat_messages(&prompt, "assistant"))
                },
                "max_output_tokens": limit,
                "store": false,
            }),
            shape: Shape::OpenAiResponses,
        },
        GoFormat::Google | GoFormat::Unknown => {
            return Err(AiError::config(format!(
                "PlexusTerm does not support the API of the OpenCode Go model \"{model}\" yet."
            )));
        }
    };
    Ok(call)
}

fn anthropic_reply(body: &Value) -> Option<Reply> {
    match body["stop_reason"].as_str() {
        Some("refusal") => return Some(Reply::Refused),
        Some("max_tokens") => return Some(Reply::Truncated),
        _ => {}
    }
    let text: String = body["content"]
        .as_array()?
        .iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();
    Some(Reply::Text(text))
}

fn gemini_reply(body: &Value) -> Option<Reply> {
    const BLOCKED: [&str; 6] = [
        "SAFETY",
        "RECITATION",
        "BLOCKLIST",
        "PROHIBITED_CONTENT",
        "SPII",
        "IMAGE_SAFETY",
    ];
    let candidate = body["candidates"].as_array().and_then(|c| c.first());
    let Some(candidate) = candidate else {
        return body
            .pointer("/promptFeedback/blockReason")
            .is_some_and(Value::is_string)
            .then_some(Reply::Refused);
    };
    let finish = candidate["finishReason"].as_str().unwrap_or_default();
    if BLOCKED.contains(&finish) {
        return Some(Reply::Refused);
    }
    if finish == "MAX_TOKENS" {
        return Some(Reply::Truncated);
    }
    let text: String = candidate
        .pointer("/content/parts")?
        .as_array()?
        .iter()
        .filter(|part| part["thought"] != true)
        .filter_map(|part| part["text"].as_str())
        .collect();
    Some(Reply::Text(text))
}

fn openai_reply(body: &Value) -> Option<Reply> {
    let choice = body["choices"].as_array()?.first()?;
    let message = &choice["message"];
    let refused = message["refusal"].as_str().is_some_and(|r| !r.is_empty());
    match choice["finish_reason"].as_str() {
        _ if refused => return Some(Reply::Refused),
        Some("content_filter") => return Some(Reply::Refused),
        Some("length") => return Some(Reply::Truncated),
        _ => {}
    }
    let text = match &message["content"] {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect(),
        _ => return None,
    };
    Some(Reply::Text(text))
}

/// OpenAI Responses: `output[]` message items with `output_text` / `refusal` parts.
fn responses_reply(body: &Value) -> Option<Reply> {
    if body["status"] == "incomplete" {
        return Some(
            match body.pointer("/incomplete_details/reason").and_then(Value::as_str) {
                Some("content_filter") => Reply::Refused,
                _ => Reply::Truncated,
            },
        );
    }
    let Some(items) = body["output"].as_array() else {
        return body["output_text"].as_str().map(|t| Reply::Text(t.to_string()));
    };
    let mut text = String::new();
    for item in items.iter().filter(|item| item["type"] == "message") {
        for part in item["content"].as_array()? {
            match part["type"].as_str() {
                Some("refusal") => return Some(Reply::Refused),
                Some("output_text") => text.push_str(part["text"].as_str().unwrap_or_default()),
                _ => {}
            }
        }
    }
    Some(Reply::Text(text))
}

pub fn parse(shape: Shape, provider: Provider, body: &Value) -> Result<Reply, AiError> {
    let reply = match shape {
        Shape::Anthropic => anthropic_reply(body),
        Shape::Gemini => gemini_reply(body),
        Shape::OpenAiChat => openai_reply(body),
        Shape::OpenAiResponses => responses_reply(body),
    };
    reply.ok_or_else(|| AiError::bad_response(provider))
}
