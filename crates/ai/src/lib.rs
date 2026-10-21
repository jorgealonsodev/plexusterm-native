//! AI command suggestions: inline completion of the typed line and the on-demand assistant,
//! ported from PlexusTerm v0.2.15 `src-tauri/src/ai` (PRD AI-1..AI-9, TERM-12c/12e, SEC-c).
//!
//! The caller passes the provider settings on every call; API keys live only in the system
//! keyring (account `ai:<provider>`, DATA-3), are wiped from memory after use and never go
//! back to the caller, into errors, or into `Debug` output. Requests go over rustls and never
//! follow redirects. Terminal output is never sent: the requests carry only the typed line,
//! OS, shell, folder, history commands, the question and what the user attached. Nothing
//! returned here is ever executed: the UI only shows or types it, every command text is
//! reduced to one line without control characters first, and assistant answers keep only
//! line breaks and tabs.

mod limiter;
pub mod models;
mod opencode;
mod providers;
#[cfg(test)]
mod tests;
mod text;

use std::path::{Path, PathBuf};
use std::time::Duration;

use core_secrets::{SecretError, SharedSecrets};
use zeroize::Zeroizing;

pub use limiter::{AI_MIN_INTERVAL_MS, Limiter};
pub use opencode::{GoFormat, GoModel, GoModels, MAX_AGE_SECS, NOTE_NO_FORMATS, NOTE_STALE};
pub use text::{ANSWER_CUT_NOTE, MAX_ANSWER, MAX_COMMAND, MAX_CONVERSATION, MAX_FILES, MAX_FILES_TOTAL, MAX_TURNS};

/// The AI services PlexusTerm can talk to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    Gemini,
    OpenAi,
    /// Any server implementing OpenAI Chat Completions (Ollama, LM Studio, OpenRouter...).
    OpenAiCompatible,
    /// OpenCode Go: one key, models served over several API formats (see `opencode`).
    OpenCodeGo,
}

impl Provider {
    pub fn parse(id: &str) -> Result<Self, AiError> {
        match id {
            "anthropic" => Ok(Self::Anthropic),
            "gemini" => Ok(Self::Gemini),
            "openai" => Ok(Self::OpenAi),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "opencode-go" => Ok(Self::OpenCodeGo),
            other => Err(AiError::config(format!("Unknown AI provider \"{other}\"."))),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::OpenCodeGo => "opencode-go",
        }
    }

    /// Name used in user-facing messages.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::OpenAi => "OpenAI",
            Self::OpenAiCompatible => "The OpenAI-compatible server",
            Self::OpenCodeGo => "OpenCode Go",
        }
    }

    /// Local servers (Ollama, LM Studio) usually need no key.
    pub fn requires_key(self) -> bool {
        self != Self::OpenAiCompatible
    }

    /// Keyring account holding this provider's API key.
    pub fn key_account(self) -> String {
        format!("ai:{}", self.id())
    }
}

/// Provider settings passed with every request.
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub provider: Provider,
    /// Only used by `openai-compatible` (required there), e.g. `http://localhost:11434/v1`.
    pub base_url: Option<String>,
    pub model: String,
    /// One stable ID per conversation (terminal pane or assistant opening), sent to OpenCode
    /// as `x-opencode-session`: 1-128 characters of `A-Z a-z 0-9 . _ -`.
    pub session_id: Option<String>,
}

/// Context for completing the line being typed. Terminal output is never sent.
#[derive(Debug, Clone, Default)]
pub struct CompleteRequest {
    pub line: String,
    pub os: String,
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub history: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteResponse {
    /// Text to append after the typed line; empty when there is no useful completion.
    pub completion: String,
}

/// What the user asked the assistant for, with optional terminal selection.
#[derive(Debug, Clone, Default)]
pub struct AssistRequest {
    pub prompt: String,
    pub selection: Option<String>,
    pub os: String,
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub history: Vec<String>,
    /// Earlier turns of this chat, oldest first; replayed as user/assistant messages
    /// (at most the last [`MAX_TURNS`], [`MAX_CONVERSATION`] characters in all).
    pub conversation: Vec<Turn>,
    /// Files dropped on the chat for this question (at most [`MAX_FILES`],
    /// [`MAX_FILES_TOTAL`] characters in all).
    pub files: Vec<AttachedFile>,
}

/// A file's text attached to a question.
#[derive(Debug, Clone, Default)]
pub struct AttachedFile {
    pub path: String,
    pub content: String,
}

/// One earlier question of a chat and the answer it got.
#[derive(Debug, Clone, Default)]
pub struct Turn {
    pub question: String,
    /// Terminal selection sent with that question, if any.
    pub selection: Option<String>,
    pub answer: String,
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistResponse {
    /// The answer to any question, as plain text (line breaks and tabs kept).
    pub answer: String,
    /// One single-line command when one fits the request; `None` otherwise.
    pub command: Option<String>,
}

/// The eight error kinds of PRD AI-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorKind {
    NoKey,
    Auth,
    RateLimit,
    Network,
    Timeout,
    Refused,
    BadResponse,
    Config,
}

/// A typed error whose message is meant for the user; it never carries the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiError {
    pub kind: AiErrorKind,
    pub message: String,
}

impl AiError {
    pub fn new(kind: AiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::new(AiErrorKind::Config, message)
    }

    pub fn bad_response(provider: Provider) -> Self {
        Self::new(
            AiErrorKind::BadResponse,
            format!("{} sent an answer PlexusTerm could not read.", provider.display_name()),
        )
    }

    fn refused(provider: Provider) -> Self {
        Self::new(
            AiErrorKind::Refused,
            format!("{} declined to answer this request.", provider.display_name()),
        )
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AiError {}

fn keyring_error(err: SecretError) -> AiError {
    AiError::config(format!("Could not use the system keyring: {err}."))
}

/// An API key read from the keyring; its bytes are wiped when it is dropped and never show
/// up in `Debug` output.
#[derive(Clone)]
pub struct ApiKey(Zeroizing<String>);

impl ApiKey {
    #[cfg(test)]
    pub fn new(key: impl Into<String>) -> Self {
        Self(Zeroizing::new(key.into()))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(***)")
    }
}

/// Endpoints of the hosted providers; tests point them at a local mock server.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// Full Messages API URL.
    pub anthropic: String,
    /// API root; `/models/{model}:generateContent` is appended.
    pub gemini: String,
    /// Full Chat Completions URL.
    pub openai: String,
    /// OpenCode Go API root; `/chat/completions`, `/messages`, `/responses` and `/models` hang off it.
    pub opencode_go: String,
    /// models.dev catalogue, which says which API format each OpenCode Go model uses.
    pub models_dev: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            anthropic: "https://api.anthropic.com/v1/messages".into(),
            gemini: "https://generativelanguage.googleapis.com/v1beta".into(),
            openai: "https://api.openai.com/v1/chat/completions".into(),
            opencode_go: "https://opencode.ai/inference/go/v1".into(),
            models_dev: "https://models.dev/api.json".into(),
        }
    }
}

/// Shared HTTP client and limits (one per app).
pub struct AiClient {
    http: reqwest::Client,
    endpoints: Endpoints,
    complete_timeout: Duration,
    assist_timeout: Duration,
    /// OpenCode Go model list (memory copy of the cache file).
    go_models: opencode::Catalog,
}

impl Default for AiClient {
    fn default() -> Self {
        let http = build_http().unwrap_or_else(|_| reqwest::Client::new());
        let client = Self::new(http, Endpoints::default());
        client.set_go_cache(core_storage::config_dir().as_deref().map(go_cache_path));
        client
    }
}

/// How PlexusTerm identifies itself to every provider (OpenCode asks clients to).
pub(crate) const USER_AGENT: &str = concat!("PlexusTerm/", env!("CARGO_PKG_VERSION"));

/// Redirects are never followed, so key headers are never re-sent to another host.
fn build_http() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .build()
}

/// The file in `config_dir` that caches the OpenCode Go model list.
pub fn go_cache_path(config_dir: &Path) -> PathBuf {
    config_dir.join(core_storage::OPENCODE_GO_MODELS_FILE)
}

/// Adds a header whose value is marked sensitive (kept out of reqwest's `Debug` output). An
/// invalid value is passed on as text so the request fails when it is sent.
pub(crate) fn with_header(
    builder: reqwest::RequestBuilder,
    name: &'static str,
    value: &str,
) -> reqwest::RequestBuilder {
    match reqwest::header::HeaderValue::from_str(value) {
        Ok(mut value) => {
            value.set_sensitive(true);
            builder.header(name, value)
        }
        Err(_) => builder.header(name, value),
    }
}

/// Which of the two features a request serves; it picks budgets and timeouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Task {
    Complete,
    Assist,
}

impl AiClient {
    pub fn new(http: reqwest::Client, endpoints: Endpoints) -> Self {
        Self {
            http,
            endpoints,
            complete_timeout: Duration::from_secs(8),
            assist_timeout: Duration::from_secs(120),
            go_models: opencode::Catalog::default(),
        }
    }

    /// Where the OpenCode Go model list is cached (set once the app paths are known).
    pub fn set_go_cache(&self, path: Option<PathBuf>) {
        self.go_models.set_path(path);
    }

    /// OpenCode Go models with their names and API formats, from the 24 h cache unless
    /// `refresh` (or the cache is missing or old).
    pub async fn opencode_models(&self, refresh: bool) -> Result<opencode::GoModels, AiError> {
        self.go_models.list(&self.http, &self.endpoints, refresh).await
    }

    #[cfg(test)]
    pub(crate) fn with_timeouts(mut self, complete: Duration, assist: Duration) -> Self {
        self.complete_timeout = complete;
        self.assist_timeout = assist;
        self
    }

    pub async fn complete(
        &self,
        secrets: &SharedSecrets,
        config: &AiConfig,
        request: &CompleteRequest,
    ) -> Result<CompleteResponse, AiError> {
        if request.line.trim().is_empty() {
            return Ok(CompleteResponse {
                completion: String::new(),
            });
        }
        let user = text::complete_prompt(request);
        let completion = match self
            .call(
                secrets,
                config,
                providers::Prompt {
                    task: Task::Complete,
                    system: text::COMPLETE_SYSTEM,
                    turns: &[],
                    user: &user,
                },
            )
            .await?
        {
            providers::Reply::Text(reply) => text::clean_completion(&request.line, &reply),
            providers::Reply::Truncated => String::new(),
            providers::Reply::Refused => return Err(AiError::refused(config.provider)),
        };
        Ok(CompleteResponse { completion })
    }

    pub async fn assist(
        &self,
        secrets: &SharedSecrets,
        config: &AiConfig,
        request: &AssistRequest,
    ) -> Result<AssistResponse, AiError> {
        if request.prompt.trim().is_empty() {
            return Err(AiError::config("Describe what you want to do."));
        }
        let user = text::assist_prompt(request);
        let turns = text::conversation(&request.conversation);
        let prompt = providers::Prompt {
            task: Task::Assist,
            system: text::ASSIST_SYSTEM,
            turns: &turns,
            user: &user,
        };
        match self.call(secrets, config, prompt).await? {
            providers::Reply::Text(reply) => text::parse_assist(&reply)
                .map(|(answer, command)| AssistResponse { answer, command })
                .ok_or_else(|| AiError::bad_response(config.provider)),
            providers::Reply::Truncated => Err(AiError::new(
                AiErrorKind::BadResponse,
                format!("The answer from {} was cut off.", config.provider.display_name()),
            )),
            providers::Reply::Refused => Err(AiError::refused(config.provider)),
        }
    }

    async fn call(
        &self,
        secrets: &SharedSecrets,
        config: &AiConfig,
        prompt: providers::Prompt<'_>,
    ) -> Result<providers::Reply, AiError> {
        let provider = config.provider;
        let task = prompt.task;
        let key = load_key(secrets, provider).await?;
        if provider.requires_key() && key.is_none() {
            return Err(AiError::new(
                AiErrorKind::NoKey,
                format!(
                    "No {} API key is saved. Add one in Settings > AI.",
                    provider.display_name()
                ),
            ));
        }
        let format = match provider {
            Provider::OpenCodeGo => {
                let format = self.go_models.format_of(config.model.trim()).await;
                if !format.supported() {
                    return Err(AiError::config(format!(
                        "PlexusTerm does not support the API of the OpenCode Go model \"{}\" yet; choose another model in Settings > AI.",
                        config.model.trim()
                    )));
                }
                format
            }
            _ => opencode::GoFormat::Chat,
        };
        let call = providers::build(&self.endpoints, config, key.as_ref(), prompt, format)?;
        let timeout = match task {
            Task::Complete => self.complete_timeout,
            Task::Assist => self.assist_timeout,
        };
        let mut builder = self
            .http
            .post(&call.url)
            .timeout(timeout)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .json(&call.body);
        for (name, value) in &call.headers {
            builder = with_header(builder, name, value);
        }
        let response = builder.send().await.map_err(|err| transport_error(provider, err))?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|err| transport_error(provider, err))?;
        if !(200..300).contains(&status) {
            return Err(status_error(provider, status, &body, key.as_ref()));
        }
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|_| AiError::bad_response(provider))?;
        providers::parse(call.shape, provider, &value)
    }
}

/// Reads the provider's key; a missing keyring only matters for providers that need a key.
async fn load_key(secrets: &SharedSecrets, provider: Provider) -> Result<Option<ApiKey>, AiError> {
    let account = provider.key_account();
    let found = core_secrets::run(secrets, move |s| {
        if s.available() {
            Ok(s.get(&account)?.map(Zeroizing::new))
        } else {
            Ok(None)
        }
    })
    .await;
    match found {
        Ok(key) => Ok(key.filter(|k| !k.is_empty()).map(ApiKey)),
        Err(_) if !provider.requires_key() => Ok(None),
        Err(err) => Err(keyring_error(err)),
    }
}

fn transport_error(provider: Provider, err: reqwest::Error) -> AiError {
    if err.is_timeout() {
        return AiError::new(
            AiErrorKind::Timeout,
            format!("{} did not answer in time.", provider.display_name()),
        );
    }
    AiError::new(
        AiErrorKind::Network,
        format!("Could not reach {}: {}.", provider.display_name(), err.without_url()),
    )
}

/// Maps a non-2xx answer; the provider's own message is kept only when it cannot carry the key.
fn status_error(provider: Provider, status: u16, body: &str, key: Option<&ApiKey>) -> AiError {
    let name = provider.display_name();
    let detail = text::error_detail(body, key.map(ApiKey::expose));
    let gemini_bad_key = provider == Provider::Gemini
        && status == 400
        && (body.contains("API_KEY_INVALID") || body.contains("API key not valid"));
    match status {
        401 | 403 => auth_error(provider),
        300..=399 => AiError::config(format!(
            "{name} redirected the request (HTTP {status}); PlexusTerm does not follow redirects. Check the base URL."
        )),
        _ if gemini_bad_key => auth_error(provider),
        429 => AiError::new(
            AiErrorKind::RateLimit,
            format!("{name} is limiting requests; try again in a moment."),
        ),
        500..=599 => AiError::new(
            AiErrorKind::Network,
            format!("{name} is unavailable right now (HTTP {status})."),
        ),
        400 | 404 | 422 => AiError::config(match detail {
            Some(detail) => format!("{name} rejected the request (HTTP {status}): {detail}"),
            None => format!("{name} rejected the request (HTTP {status}). Check the model name."),
        }),
        _ => AiError::new(AiErrorKind::BadResponse, format!("{name} answered with HTTP {status}.")),
    }
}

fn auth_error(provider: Provider) -> AiError {
    let message = match provider {
        // Its display name already reads "The OpenAI-compatible server", and a key is optional there.
        Provider::OpenAiCompatible => format!(
            "{} rejected the request: its API key is missing or wrong. Check it in Settings > AI.",
            provider.display_name()
        ),
        _ => format!(
            "The {} API key was rejected. Check it in Settings > AI.",
            provider.display_name()
        ),
    };
    AiError::new(AiErrorKind::Auth, message)
}

/// Whether an API key is saved for `provider`; the key itself never leaves this crate.
pub async fn key_status(secrets: &SharedSecrets, provider: &str) -> Result<bool, AiError> {
    let account = Provider::parse(provider)?.key_account();
    core_secrets::run(secrets, move |s| {
        Ok(s.available() && s.get(&account)?.map(Zeroizing::new).is_some_and(|k| !k.is_empty()))
    })
    .await
    .map_err(keyring_error)
}

/// Saves `key` (trimmed) as the provider's API key; every copy of it is wiped afterwards.
pub async fn key_save(secrets: &SharedSecrets, provider: &str, key: String) -> Result<(), AiError> {
    let key = Zeroizing::new(key);
    let provider = Provider::parse(provider)?;
    let key = Zeroizing::new(key.trim().to_string());
    if key.is_empty() {
        return Err(AiError::config("Enter an API key."));
    }
    let account = provider.key_account();
    let label = format!("PlexusTerm AI key ({})", provider.display_name());
    core_secrets::run(secrets, move |s| s.set(&account, &label, &key))
        .await
        .map_err(keyring_error)
}

pub async fn key_delete(secrets: &SharedSecrets, provider: &str) -> Result<(), AiError> {
    let account = Provider::parse(provider)?.key_account();
    core_secrets::run(secrets, move |s| s.delete(&account))
        .await
        .map_err(keyring_error)
}
