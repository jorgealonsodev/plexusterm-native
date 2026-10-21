//! Model lists for the chat header picker (S10).

use super::*;

// ---- S10: model lists for the chat header picker ----

fn ids(list: &models::ModelList) -> Vec<(String, String, bool)> {
    list.models
        .iter()
        .map(|m| (m.id.clone(), m.name.clone(), m.supported))
        .collect()
}

fn row(id: &str, name: &str, supported: bool) -> (String, String, bool) {
    (id.to_string(), name.to_string(), supported)
}

#[tokio::test]
async fn anthropic_models_come_from_its_model_list_with_the_key_headers() {
    let body = json!({"data": [
        {"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "type": "model"},
        {"id": "claude-haiku-5-5", "display_name": "Claude Haiku 5.5", "type": "model"}
    ], "has_more": false})
    .to_string();
    let (base, log) = mock_routes(vec![("/v1/models?limit=1000", 200, body)]).await;
    let secrets = store_with(Provider::Anthropic, Some(KEY));
    let got = client(&base).models(&secrets, Provider::Anthropic, None).await.unwrap();
    assert_eq!(
        ids(&got),
        vec![
            row("claude-opus-5-5", "Claude Opus 5.5", true),
            row("claude-haiku-5-5", "Claude Haiku 5.5", true)
        ]
    );
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].method, "GET");
    assert_eq!(log[0].header("x-api-key"), Some(KEY));
    assert_eq!(log[0].header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(log[0].header("user-agent"), Some(USER_AGENT));
}

#[tokio::test]
async fn gemini_models_keep_only_those_that_generate_content() {
    let body = json!({"models": [
        {"name": "models/gemini-3.5-flash-lite", "displayName": "Gemini 3.5 Flash-Lite",
         "supportedGenerationMethods": ["generateContent", "countTokens"]},
        {"name": "models/text-embedding-005", "displayName": "Embedding",
         "supportedGenerationMethods": ["embedContent"]}
    ]})
    .to_string();
    let (base, log) = mock_routes(vec![("/v1beta/models?pageSize=1000", 200, body)]).await;
    let secrets = store_with(Provider::Gemini, Some(KEY));
    let got = client(&base).models(&secrets, Provider::Gemini, None).await.unwrap();
    assert_eq!(
        ids(&got),
        vec![row("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", true)]
    );
    let log = log.lock().unwrap();
    assert_eq!(log[0].header("x-goog-api-key"), Some(KEY));
    assert!(!log[0].path.contains(KEY));
}

#[tokio::test]
async fn openai_models_drop_non_chat_families_and_are_sorted() {
    let body = json!({"object": "list", "data": [
        {"id": "gpt-6"}, {"id": "text-embedding-3-large"}, {"id": "tts-1"},
        {"id": "whisper-1"}, {"id": "dall-e-3"}, {"id": "omni-moderation-latest"},
        {"id": "gpt-5-mini"}, {"id": "bad id"}
    ]})
    .to_string();
    let (base, log) = mock_routes(vec![("/v1/models", 200, body)]).await;
    let secrets = store_with(Provider::OpenAi, Some(KEY));
    let got = client(&base).models(&secrets, Provider::OpenAi, None).await.unwrap();
    assert_eq!(
        ids(&got),
        vec![row("gpt-5-mini", "gpt-5-mini", true), row("gpt-6", "gpt-6", true)]
    );
    assert_eq!(
        log.lock().unwrap()[0].header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
}

#[tokio::test]
async fn openai_compatible_models_use_the_base_url_and_need_no_key() {
    let body = json!({"data": [{"id": "llama4"}, {"id": "qwen3"}]}).to_string();
    let (base, log) = mock_routes(vec![("/v1/models", 200, body)]).await;
    let secrets = store_with(Provider::OpenAiCompatible, None);
    let ai = client("http://127.0.0.1:9");
    let got = ai
        .models(&secrets, Provider::OpenAiCompatible, Some(&format!("{base}/v1/")))
        .await
        .unwrap();
    assert_eq!(
        ids(&got),
        vec![row("llama4", "llama4", true), row("qwen3", "qwen3", true)]
    );
    assert_eq!(log.lock().unwrap()[0].header("authorization"), None);
    let err = ai.models(&secrets, Provider::OpenAiCompatible, None).await.unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
}

#[tokio::test]
async fn listing_models_without_a_key_asks_for_one_and_sends_nothing() {
    let (base, log) = mock_routes(vec![]).await;
    let secrets = store_with(Provider::Anthropic, None);
    let err = client(&base)
        .models(&secrets, Provider::Anthropic, None)
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::NoKey);
    assert!(log.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_rejected_key_while_listing_models_is_an_auth_error() {
    let (base, _log) = mock_routes(vec![("/v1/models", 401, "{}".into())]).await;
    let secrets = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .models(&secrets, Provider::OpenAi, None)
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Auth);
}

#[tokio::test]
async fn opencode_go_models_come_from_its_catalogue() {
    let (base, _log) = mock_routes(go_routes((200, GO_LIST), (200, MODELS_DEV))).await;
    let secrets = store_with(Provider::OpenCodeGo, Some(KEY));
    let got = client(&base)
        .models(&secrets, Provider::OpenCodeGo, None)
        .await
        .unwrap();
    assert_eq!(got.models.len(), 5);
    assert_eq!(
        ids(&got)[0],
        row("gemini-4-argon-contributor", "Gemini 4 Argon Contributor", false)
    );
}

#[tokio::test]
async fn a_rejected_key_message_names_each_provider_once() {
    for provider in [Provider::OpenAiCompatible, Provider::OpenAi] {
        let err = status_error(provider, 401, "{}", None);
        assert_eq!(err.kind, AiErrorKind::Auth);
        assert!(!err.message.contains("The The"), "{}", err.message);
    }
    assert_eq!(
        status_error(Provider::OpenAiCompatible, 401, "{}", None).message,
        "The OpenAI-compatible server rejected the request: its API key is missing or wrong. Check it in Settings > AI."
    );
    assert_eq!(
        status_error(Provider::OpenAi, 403, "{}", None).message,
        "The OpenAI API key was rejected. Check it in Settings > AI."
    );
}
