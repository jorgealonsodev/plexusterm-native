//! Error kinds and the keyring.

use super::*;

// --- Errors ------------------------------------------------------------------------------

#[tokio::test]
async fn http_401_is_auth_and_never_carries_the_key() {
    let reply = json!({"error": {"message": format!("Incorrect API key provided: {KEY}")}});
    let (base, _seen) = mock(401, reply, Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Auth);
    assert!(!err.to_string().contains(KEY), "{err}");
    assert!(!format!("{err:?}").contains(KEY));
}

#[tokio::test]
async fn http_429_is_rate_limit() {
    let reply = json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}});
    let (base, _seen) = mock(429, reply, Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::RateLimit);
}

#[tokio::test]
async fn http_400_keeps_the_provider_message_without_the_key() {
    let reply = json!({"error": {"message": format!("model not found for key {KEY}")}});
    let (base, _seen) = mock(400, reply, Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    assert!(err.message.contains("model not found for key ***"), "{}", err.message);
    assert!(!err.message.contains(KEY));
}

#[tokio::test]
async fn http_529_overloaded_is_network() {
    let reply = json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}});
    let (base, _seen) = mock(529, reply, Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Network);
}

#[tokio::test]
async fn a_slow_provider_times_out() {
    let (base, _seen) = mock(200, anthropic_text("ls -l", "end_turn"), Duration::from_secs(5)).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let err = client(&base)
        .with_timeouts(Duration::from_millis(200), Duration::from_millis(200))
        .complete(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Timeout);
}

#[tokio::test]
async fn an_unreachable_server_is_network() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Network);
}

#[tokio::test]
async fn a_non_json_answer_is_bad_response() {
    let (base, _seen) = mock(200, json!("<html>"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::BadResponse);
}

#[tokio::test]
async fn hosted_providers_without_a_key_fail_before_any_request() {
    let store = store_with(Provider::Anthropic, None);
    for provider in [
        Provider::Anthropic,
        Provider::Gemini,
        Provider::OpenAi,
        Provider::OpenCodeGo,
    ] {
        let err = client("http://127.0.0.1:9")
            .complete(&store, &config(provider, "m"), &line("ls"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, AiErrorKind::NoKey, "{provider:?}");
    }
    let unavailable: SharedSecrets = Arc::new(NoStore);
    let err = client("http://127.0.0.1:9")
        .assist(&unavailable, &config(Provider::Gemini, "m"), &ask("x"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::NoKey);
}

#[tokio::test]
async fn an_empty_line_is_not_sent() {
    let store = store_with(Provider::OpenAi, Some(KEY));
    let got = client("http://127.0.0.1:9")
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("   "))
        .await
        .unwrap();
    assert_eq!(got.completion, "");
}

#[test]
fn the_default_client_builds_with_tls() {
    assert!(build_http().is_ok());
}

// --- Keys --------------------------------------------------------------------------------

#[tokio::test]
async fn keys_are_saved_reported_and_deleted_in_the_keyring() {
    let store = MemoryStore::shared();
    let shared: SharedSecrets = store.clone();
    assert!(!key_status(&shared, "anthropic").await.unwrap());
    key_save(&shared, "anthropic", format!("  {KEY}\n")).await.unwrap();
    assert!(key_status(&shared, "anthropic").await.unwrap());
    assert_eq!(store.get("ai:anthropic").unwrap().as_deref(), Some(KEY));
    assert_eq!(
        store.label("ai:anthropic").as_deref(),
        Some("PlexusTerm AI key (Anthropic)")
    );
    assert!(!key_status(&shared, "gemini").await.unwrap());
    key_delete(&shared, "anthropic").await.unwrap();
    assert!(!key_status(&shared, "anthropic").await.unwrap());
    key_delete(&shared, "anthropic").await.unwrap();
}

#[tokio::test]
async fn key_accounts_follow_the_provider_id() {
    let store = MemoryStore::shared();
    let shared: SharedSecrets = store.clone();
    for id in ["anthropic", "gemini", "openai", "openai-compatible", "opencode-go"] {
        key_save(&shared, id, "k".into()).await.unwrap();
        assert_eq!(store.get(&format!("ai:{id}")).unwrap().as_deref(), Some("k"));
    }
}

#[tokio::test]
async fn empty_keys_and_unknown_providers_are_rejected() {
    let shared: SharedSecrets = MemoryStore::shared();
    let err = key_save(&shared, "anthropic", "   ".into()).await.unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    let err = key_save(&shared, "opencode", "k".into()).await.unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    assert_eq!(
        key_status(&shared, "opencode").await.unwrap_err().kind,
        AiErrorKind::Config
    );
}

#[tokio::test]
async fn an_unavailable_keyring_reports_no_key_and_refuses_to_save() {
    let shared: SharedSecrets = Arc::new(NoStore);
    assert!(!key_status(&shared, "openai").await.unwrap());
    let err = key_save(&shared, "openai", KEY.to_string()).await.unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    assert!(!err.message.contains(KEY), "{}", err.message);
}

#[test]
fn api_keys_are_redacted_in_debug_output() {
    let key = ApiKey::new(KEY);
    assert_eq!(format!("{key:?}"), "ApiKey(***)");
}

#[test]
fn the_opencode_go_cache_lives_in_the_config_dir_under_its_v0_2_15_name() {
    let dir = std::path::Path::new("/home/ana/.config/dev.plexusterm.app");
    assert_eq!(go_cache_path(dir), dir.join("opencode-go-models.json"));
}

#[test]
fn key_headers_are_marked_sensitive() {
    let http = build_http().unwrap();
    let request = with_header(http.get("http://127.0.0.1:9/"), "x-api-key", KEY)
        .build()
        .unwrap();
    let value = request.headers().get("x-api-key").unwrap();
    assert!(value.is_sensitive());
    assert!(!format!("{request:?}").contains(KEY));
}
