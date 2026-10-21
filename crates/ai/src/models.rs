//! Model lists for the chat header picker (S10): each provider's own `GET /models`, and the
//! OpenCode Go catalogue (S7).

use serde::Deserialize;
use serde_json::Value;

use core_secrets::SharedSecrets;

use crate::opencode::{clean_name, valid_id};
use crate::{
    AiClient, AiError, AiErrorKind, ApiKey, Provider, USER_AGENT, load_key, status_error, transport_error, with_header,
};

/// One model as the picker shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// False for models PlexusTerm cannot talk to yet (shown disabled).
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelList {
    pub models: Vec<ModelInfo>,
    pub note: Option<String>,
}

/// Lists are a few hundred kilobytes at most; anything far larger is refused.
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_MODELS: usize = 1000;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// OpenAI model families that do not answer chat requests.
const OPENAI_NOT_CHAT: [&str; 10] = [
    "embedding",
    "tts",
    "whisper",
    "dall-e",
    "moderation",
    "transcribe",
    "realtime",
    "audio",
    "image",
    "davinci",
];

impl AiClient {
    /// The models `provider` offers, read with its saved key (the OpenAI-compatible server
    /// at `base_url`, key optional).
    pub async fn models(
        &self,
        secrets: &SharedSecrets,
        provider: Provider,
        base_url: Option<&str>,
    ) -> Result<ModelList, AiError> {
        let url = match provider {
            Provider::OpenCodeGo => return self.opencode_model_list().await,
            Provider::Anthropic => format!(
                "{}/models?limit=1000",
                strip_suffix(&self.endpoints.anthropic, "/messages")
            ),
            Provider::Gemini => format!("{}/models?pageSize=1000", self.endpoints.gemini.trim_end_matches('/')),
            Provider::OpenAi => format!("{}/models", strip_suffix(&self.endpoints.openai, "/chat/completions")),
            Provider::OpenAiCompatible => format!(
                "{}/models",
                strip_suffix(&crate::providers::compatible_url(base_url)?, "/chat/completions")
            ),
        };
        let key = load_key(secrets, provider).await?;
        if provider.requires_key() && key.is_none() {
            return Err(AiError::new(
                AiErrorKind::NoKey,
                format!("No {} API key is saved.", provider.display_name()),
            ));
        }
        let value = self.get_json(provider, &url, key.as_ref()).await?;
        let mut models = parse(provider, &value).ok_or_else(|| AiError::bad_response(provider))?;
        let mut seen = std::collections::HashSet::new();
        models.retain(|m| valid_id(&m.id) && seen.insert(m.id.clone()));
        if matches!(provider, Provider::OpenAi | Provider::OpenAiCompatible) {
            models.sort_by(|a, b| a.id.cmp(&b.id));
        }
        models.truncate(MAX_MODELS);
        Ok(ModelList { models, note: None })
    }

    /// The OpenCode Go catalogue (24 h cache) as a picker list.
    async fn opencode_model_list(&self) -> Result<ModelList, AiError> {
        let go = self.opencode_models(false).await?;
        Ok(ModelList {
            models: go
                .models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.id,
                    name: m.name,
                    supported: m.supported,
                })
                .collect(),
            note: go.note,
        })
    }

    async fn get_json(&self, provider: Provider, url: &str, key: Option<&ApiKey>) -> Result<Value, AiError> {
        let mut request = self
            .http
            .get(url)
            .timeout(TIMEOUT)
            .header(reqwest::header::USER_AGENT, USER_AGENT);
        if let Some(key) = key {
            request = match provider {
                Provider::Anthropic => with_header(request, "x-api-key", key.expose())
                    .header("anthropic-version", crate::providers::ANTHROPIC_VERSION),
                Provider::Gemini => with_header(request, "x-goog-api-key", key.expose()),
                _ => with_header(request, "authorization", &crate::providers::bearer(key)),
            };
        }
        let mut response = request.send().await.map_err(|err| transport_error(provider, err))?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| transport_error(provider, err))? {
            if body.len() + chunk.len() > MAX_BYTES {
                return Err(AiError::bad_response(provider));
            }
            body.extend_from_slice(&chunk);
        }
        if !(200..300).contains(&status) {
            return Err(status_error(provider, status, &String::from_utf8_lossy(&body), key));
        }
        serde_json::from_slice(&body).map_err(|_| AiError::bad_response(provider))
    }
}

fn strip_suffix<'a>(url: &'a str, suffix: &str) -> &'a str {
    let url = url.trim_end_matches('/');
    url.strip_suffix(suffix).unwrap_or(url)
}

#[derive(Deserialize)]
struct Listed {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModel {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
}

fn info(id: String, name: Option<&str>) -> ModelInfo {
    let name = name
        .map(clean_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    ModelInfo {
        id,
        name,
        supported: true,
    }
}

fn parse(provider: Provider, value: &Value) -> Option<Vec<ModelInfo>> {
    if provider == Provider::Gemini {
        let models: Vec<GeminiModel> = serde_json::from_value(value.get("models")?.clone()).ok()?;
        return Some(
            models
                .into_iter()
                .filter(|m| m.supported_generation_methods.iter().any(|g| g == "generateContent"))
                .map(|m| {
                    let id = m.name.strip_prefix("models/").unwrap_or(&m.name).to_string();
                    info(id, m.display_name.as_deref())
                })
                .collect(),
        );
    }
    let listed: Vec<Listed> = serde_json::from_value(value.get("data")?.clone()).ok()?;
    Some(
        listed
            .into_iter()
            .filter(|m| provider != Provider::OpenAi || !OPENAI_NOT_CHAT.iter().any(|f| m.id.contains(f)))
            .map(|m| info(m.id, m.display_name.as_deref()))
            .collect(),
    )
}
