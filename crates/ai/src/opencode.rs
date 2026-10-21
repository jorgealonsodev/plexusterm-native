//! OpenCode Go model catalogue: which models exist and which API format each one uses.
//!
//! The model list is public (`GET <go>/models`, OpenAI list format, no key). The API format
//! of each model comes from models.dev (`"opencode-go"` provider; a model's `provider.npm`
//! overrides the provider default `@ai-sdk/openai-compatible`). Both are fetched by the
//! client, merged, and cached for 24 hours in the app config directory
//! (`core_storage::OPENCODE_GO_MODELS_FILE`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{AiError, AiErrorKind, Endpoints, USER_AGENT};

/// The API an OpenCode Go model is served with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GoFormat {
    /// OpenAI Chat Completions (`/chat/completions`).
    #[default]
    Chat,
    /// Anthropic Messages (`/messages`).
    Messages,
    /// OpenAI Responses (`/responses`).
    Responses,
    /// Google generateContent; not verified for OpenCode Go, so not supported yet.
    Google,
    /// An SDK PlexusTerm does not know.
    Unknown,
}

impl GoFormat {
    pub fn supported(self) -> bool {
        matches!(self, Self::Chat | Self::Messages | Self::Responses)
    }

    /// Maps the AI SDK package models.dev names for a model to its API format.
    fn from_npm(npm: Option<&str>) -> Self {
        match npm {
            None | Some("@ai-sdk/openai-compatible") => Self::Chat,
            Some("@ai-sdk/anthropic") => Self::Messages,
            Some("@ai-sdk/openai") => Self::Responses,
            Some("@ai-sdk/google") => Self::Google,
            Some(_) => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoModel {
    pub id: String,
    pub name: String,
    pub format: GoFormat,
    pub supported: bool,
}

/// The catalogue as the frontend (and the cache file) sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoModels {
    pub models: Vec<GoModel>,
    /// Shown under the model list, e.g. when the formats could not be read.
    #[serde(default)]
    pub note: Option<String>,
    /// Unix seconds of the fetch.
    pub fetched_at: u64,
}

/// How long a fetched catalogue is used without asking again.
pub const MAX_AGE_SECS: u64 = 24 * 60 * 60;
/// models.dev is a few megabytes; anything far larger is refused.
const MAX_MODELS_DEV_BYTES: usize = 32 * 1024 * 1024;
const MAX_LIST_BYTES: usize = 2 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ID: usize = 200;

pub const NOTE_NO_FORMATS: &str =
    "models.dev could not be reached, so every model is treated as chat completions; some models may not answer.";
pub const NOTE_STALE: &str = "Could not refresh the OpenCode Go models; showing the list saved earlier.";

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// In-memory copy of the cached catalogue, plus where the cache file lives.
#[derive(Default)]
pub struct Catalog {
    path: std::sync::Mutex<Option<PathBuf>>,
    models: tokio::sync::Mutex<Option<GoModels>>,
}

impl Catalog {
    pub fn set_path(&self, path: Option<PathBuf>) {
        if path.is_some() {
            *self.path.lock().unwrap_or_else(|p| p.into_inner()) = path;
        }
    }

    fn path(&self) -> Option<PathBuf> {
        self.path.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn load_disk(&self) -> Option<GoModels> {
        let text = core_storage::load(&self.path()?).ok()??;
        serde_json::from_str(&text).ok()
    }

    fn save_disk(&self, models: &GoModels) {
        if let (Some(path), Ok(text)) = (self.path(), serde_json::to_string(models)) {
            let _ = core_storage::save(&path, &text);
        }
    }

    /// The format of `model` from the cached catalogue (any age); chat completions when the
    /// model is unknown or nothing is cached. Never fetches, so typing never waits for it.
    pub async fn format_of(&self, model: &str) -> GoFormat {
        let mut cached = self.models.lock().await;
        if cached.is_none() {
            *cached = self.load_disk();
        }
        cached
            .as_ref()
            .and_then(|c| c.models.iter().find(|m| m.id == model))
            .map_or(GoFormat::Chat, |m| m.format)
    }

    /// The catalogue: the cached one while younger than 24 h unless `refresh`; otherwise
    /// fetched again. A failed fetch falls back to the cached list with a note.
    pub async fn list(
        &self,
        http: &reqwest::Client,
        endpoints: &Endpoints,
        refresh: bool,
    ) -> Result<GoModels, AiError> {
        let mut cached = self.models.lock().await;
        if cached.is_none() {
            *cached = self.load_disk();
        }
        let now = now();
        let fresh_enough = |c: &&GoModels| c.fetched_at <= now && now - c.fetched_at < MAX_AGE_SECS;
        if !refresh && let Some(c) = cached.as_ref().filter(fresh_enough) {
            return Ok(c.clone());
        }
        match fetch(http, endpoints, now).await {
            Ok(fresh) => {
                self.save_disk(&fresh);
                *cached = Some(fresh.clone());
                Ok(fresh)
            }
            Err(err) => match cached.as_ref() {
                Some(old) => Ok(GoModels {
                    note: Some(NOTE_STALE.into()),
                    ..old.clone()
                }),
                None => Err(err),
            },
        }
    }
}

/// GET `url` into memory, refusing bodies over `max` bytes; `None` on any failure.
async fn get_bytes(http: &reqwest::Client, url: &str, max: usize) -> Option<Vec<u8>> {
    let mut response = http
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut out = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if out.len() + chunk.len() > max {
            return None;
        }
        out.extend_from_slice(&chunk);
    }
    Some(out)
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ListedModel>,
}

#[derive(Deserialize)]
struct ListedModel {
    id: String,
}

/// Only the `"opencode-go"` entry of models.dev is kept; serde skips the other providers.
#[derive(Deserialize)]
struct ModelsDev {
    #[serde(rename = "opencode-go")]
    go: Option<DevProvider>,
}

#[derive(Deserialize)]
struct DevProvider {
    #[serde(default)]
    models: BTreeMap<String, DevModel>,
}

#[derive(Deserialize)]
struct DevModel {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    provider: Option<DevSdk>,
}

#[derive(Deserialize)]
struct DevSdk {
    #[serde(default)]
    npm: Option<String>,
}

async fn public_list(http: &reqwest::Client, url: &str) -> Option<Vec<String>> {
    let bytes = get_bytes(http, url, MAX_LIST_BYTES).await?;
    let list: ModelList = serde_json::from_slice(&bytes).ok()?;
    Some(list.data.into_iter().map(|m| m.id).collect())
}

async fn models_dev(http: &reqwest::Client, url: &str) -> Option<BTreeMap<String, DevModel>> {
    let bytes = get_bytes(http, url, MAX_MODELS_DEV_BYTES).await?;
    let catalogue: ModelsDev = serde_json::from_slice(&bytes).ok()?;
    Some(catalogue.go?.models)
}

/// Model ids end up in request bodies and the UI: short, no whitespace or control characters.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= MAX_ID && !id.chars().any(|c| c.is_control() || c.is_whitespace())
}

pub(crate) fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(MAX_ID)
        .collect::<String>()
        .trim()
        .to_string()
}

async fn fetch(http: &reqwest::Client, endpoints: &Endpoints, now: u64) -> Result<GoModels, AiError> {
    let list_url = format!("{}/models", endpoints.opencode_go.trim_end_matches('/'));
    let (listed, dev) = tokio::join!(public_list(http, &list_url), models_dev(http, &endpoints.models_dev));
    merge(listed, dev, now)
}

/// The public list says which models exist; models.dev gives names and formats.
fn merge(listed: Option<Vec<String>>, dev: Option<BTreeMap<String, DevModel>>, now: u64) -> Result<GoModels, AiError> {
    let (ids, note) = match (listed, &dev) {
        (Some(ids), Some(_)) => (ids, None),
        (Some(ids), None) => (ids, Some(NOTE_NO_FORMATS.to_string())),
        (None, Some(dev)) => (dev.keys().cloned().collect(), None),
        (None, None) => {
            return Err(AiError::new(
                AiErrorKind::Network,
                "Could not fetch the OpenCode Go model list.",
            ));
        }
    };
    let mut seen = std::collections::HashSet::new();
    let mut models: Vec<GoModel> = ids
        .into_iter()
        .filter(|id| valid_id(id) && seen.insert(id.clone()))
        .map(|id| {
            let known = dev.as_ref().and_then(|d| d.get(&id));
            let format = match &dev {
                Some(_) => GoFormat::from_npm(known.and_then(|m| m.provider.as_ref()).and_then(|p| p.npm.as_deref())),
                None => GoFormat::Chat,
            };
            let name = known
                .and_then(|m| m.name.as_deref())
                .map(clean_name)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| id.clone());
            GoModel {
                id,
                name,
                format,
                supported: format.supported(),
            }
        })
        .collect();
    models.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(GoModels {
        models,
        note,
        fetched_at: now,
    })
}
