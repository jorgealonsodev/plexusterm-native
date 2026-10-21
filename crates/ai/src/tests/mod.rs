//! Provider round trips against a local mock HTTP server (no network), ported from
//! PlexusTerm v0.2.15 `src-tauri/src/ai/tests.rs`, split per area.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test harness and assertions"
)]

mod answers;
mod conversation;
mod errors_keys;
mod model_lists;
mod opencode_go;
mod round_trips;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use super::*;
use core_secrets::{MemoryStore, NoStore, SecretStore};

const KEY: &str = "sk-test-0123456789";
const SESSION: &str = "3f2b6c1e-8d4a-4e1b-9c2f-0a1b2c3d4e5f";
const USER_AGENT: &str = concat!("PlexusTerm/", env!("CARGO_PKG_VERSION"));

/// One request as the mock server received it.
#[derive(Debug)]
struct Seen {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// Reads one HTTP/1.1 request (head and JSON body) from `socket`.
async fn read_seen(socket: &mut tokio::net::TcpStream) -> Seen {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        let n = socket.read(&mut buf).await.unwrap();
        assert!(n > 0, "client closed early");
        raw.extend_from_slice(&buf[..n]);
        if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut start = lines.next().unwrap().split(' ');
    let method = start.next().unwrap().to_string();
    let path = start.next().unwrap().to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length: usize = headers.get("content-length").map(|v| v.parse().unwrap()).unwrap_or(0);
    while raw.len() < head_end + length {
        let n = socket.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let body = serde_json::from_slice(&raw[head_end..]).unwrap_or(Value::Null);
    Seen {
        method,
        path,
        headers,
        body,
    }
}

async fn respond(socket: &mut tokio::net::TcpStream, status: u16, extra: &str, payload: &str) {
    let response = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{payload}",
        payload.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

/// Answers one request with `status` and `reply` after `delay`; returns the base URL.
async fn mock(status: u16, reply: Value, delay: Duration) -> (String, oneshot::Receiver<Seen>) {
    mock_with(status, "", reply, delay).await
}

/// Like `mock`, with extra raw response header lines (each ending in `\r\n`).
async fn mock_with(
    status: u16,
    extra: &'static str,
    reply: Value,
    delay: Duration,
) -> (String, oneshot::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let seen = read_seen(&mut socket).await;
        let _ = tx.send(seen);
        tokio::time::sleep(delay).await;
        respond(&mut socket, status, extra, &reply.to_string()).await;
    });
    (base, rx)
}

type Log = Arc<std::sync::Mutex<Vec<Seen>>>;

/// Serves any number of requests by path (`(path, status, body)`; unknown paths get 404).
async fn mock_routes(routes: Vec<(&'static str, u16, String)>) -> (String, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log: Log = Arc::default();
    let seen_log = log.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let seen = read_seen(&mut socket).await;
            let route = routes.iter().find(|(path, _, _)| *path == seen.path).cloned();
            seen_log.lock().unwrap().push(seen);
            let (status, body) = route.map_or((404, "{}".to_string()), |(_, s, b)| (s, b));
            respond(&mut socket, status, "", &body).await;
        }
    });
    (base, log)
}

fn endpoints(base: &str) -> Endpoints {
    Endpoints {
        anthropic: format!("{base}/v1/messages"),
        gemini: format!("{base}/v1beta"),
        openai: format!("{base}/v1/chat/completions"),
        opencode_go: format!("{base}/inference/go/v1"),
        models_dev: format!("{base}/api.json"),
    }
}

/// The production HTTP client (user agent, no redirects) against the mock endpoints.
fn client(base: &str) -> AiClient {
    AiClient::new(build_http().unwrap(), endpoints(base))
}

fn prompt() -> providers::Prompt<'static> {
    providers::Prompt {
        task: Task::Complete,
        system: "s",
        turns: &[],
        user: "u",
    }
}

fn store_with(provider: Provider, key: Option<&str>) -> SharedSecrets {
    let store = MemoryStore::shared();
    if let Some(key) = key {
        store.set(&provider.key_account(), "label", key).unwrap();
    }
    store
}

fn config(provider: Provider, model: &str) -> AiConfig {
    AiConfig {
        provider,
        base_url: None,
        model: model.into(),
        session_id: Some(SESSION.into()),
    }
}

fn line(text: &str) -> CompleteRequest {
    CompleteRequest {
        line: text.into(),
        os: "linux".into(),
        shell: Some("bash".into()),
        cwd: Some("/srv".into()),
        history: vec!["git status".into()],
    }
}

fn ask(prompt: &str) -> AssistRequest {
    AssistRequest {
        prompt: prompt.into(),
        os: "linux".into(),
        ..Default::default()
    }
}

fn anthropic_text(text: &str, stop: &str) -> Value {
    json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
        "content": [{"type": "thinking", "thinking": "", "signature": "x"}, {"type": "text", "text": text}],
        "stop_reason": stop, "usage": {"input_tokens": 10, "output_tokens": 5}
    })
}

fn gemini_text(text: &str, finish: &str) -> Value {
    json!({
        "candidates": [{"content": {"role": "model", "parts": [
            {"text": "thinking...", "thought": true}, {"text": text}
        ]}, "finishReason": finish, "index": 0}],
        "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 4, "totalTokenCount": 14},
        "modelVersion": "gemini-3.5-flash-lite"
    })
}

fn openai_text(text: &str, finish: &str) -> Value {
    json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "gpt-x",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": text, "refusal": null},
                     "logprobs": null, "finish_reason": finish}],
        "usage": {"prompt_tokens": 9, "completion_tokens": 3, "total_tokens": 12}
    })
}

const ASSIST_JSON: &str = r#"{"answer": "Shows free disk space.", "command": "df -h"}"#;
/// A general answer with no command (S8).
const ANSWER_ONLY_JSON: &str = r#"{"answer": "¡Hola! ¿En qué te ayudo?", "command": null}"#;

// --- Shared by several areas --------------------------------------------------------------

fn responses_text(text: &str) -> Value {
    json!({
        "id": "resp_1", "object": "response", "status": "completed",
        "output": [
            {"type": "reasoning", "id": "rs_1", "summary": []},
            {"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": text, "annotations": []}]}
        ]
    })
}

/// A cache file holding `models` as `(id, format)`, fetched `age` seconds ago.
fn go_cache(dir: &tempfile::TempDir, models: &[(&str, &str)], age: u64) -> std::path::PathBuf {
    let path = dir.path().join("opencode-go-models.json");
    let models: Vec<Value> = models
        .iter()
        .map(|(id, format)| json!({"id": id, "name": id, "format": format, "supported": *format != "google"}))
        .collect();
    let cache = json!({"models": models, "note": null, "fetchedAt": now_secs() - age});
    std::fs::write(&path, cache.to_string()).unwrap();
    path
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn go_routes(list: (u16, &str), dev: (u16, &str)) -> Vec<(&'static str, u16, String)> {
    vec![
        ("/inference/go/v1/models", list.0, list.1.to_string()),
        ("/api.json", dev.0, dev.1.to_string()),
    ]
}

const GO_LIST: &str = r#"{"object":"list","data":[
    {"id":"kimi-k3","object":"model","owned_by":"opencode"},
    {"id":"minimax-m3","object":"model","owned_by":"opencode"},
    {"id":"gpt-6-luna","object":"model","owned_by":"opencode"},
    {"id":"gemini-4-argon-contributor","object":"model","owned_by":"opencode"},
    {"id":"omen-alpha","object":"model","owned_by":"opencode"},
    {"id":"kimi-k3","object":"model","owned_by":"opencode"},
    {"id":"bad id","object":"model","owned_by":"opencode"}
]}"#;

const MODELS_DEV: &str = r#"{
  "openai": {"id": "openai", "models": {"gpt-x": {"id": "gpt-x", "name": "GPT X", "provider": {"npm": "@ai-sdk/anthropic"}}}},
  "opencode-go": {"id": "opencode-go", "npm": "@ai-sdk/openai-compatible", "api": "https://opencode.ai/inference/go/v1",
    "models": {
      "kimi-k3": {"id": "kimi-k3", "name": "Kimi K3", "limit": {"context": 1, "output": 1}},
      "minimax-m3": {"id": "minimax-m3", "name": "MiniMax-M3", "provider": {"npm": "@ai-sdk/anthropic"}},
      "gpt-6-luna": {"id": "gpt-6-luna", "name": "GPT-6 Luna", "provider": {"npm": "@ai-sdk/openai"}},
      "gemini-4-argon-contributor": {"id": "gemini-4-argon-contributor", "name": "Gemini 4 Argon Contributor", "provider": {"npm": "@ai-sdk/google"}},
      "retired": {"id": "retired", "name": "Retired"}
    }}
}"#;

/// The provider's own success body around `text`, for each assistant API shape.
fn reply_for(provider: Provider, text: &str) -> Value {
    match provider {
        Provider::Anthropic => anthropic_text(text, "end_turn"),
        Provider::Gemini => gemini_text(text, "STOP"),
        _ => openai_text(text, "stop"),
    }
}

fn compatible_config(base: &str) -> AiConfig {
    AiConfig {
        provider: Provider::OpenAiCompatible,
        base_url: Some(format!("{base}/v1")),
        model: "llama3.2".into(),
        session_id: None,
    }
}
