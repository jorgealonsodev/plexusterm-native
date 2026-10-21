//! OpenCode Go: session ids, formats per model and the model catalogue.

use super::*;

// --- OpenCode Go and session IDs ------------------------------------------------------

#[tokio::test]
async fn opencode_go_uses_chat_completions_with_a_bearer_key_and_the_session() {
    let (base, seen) = mock(200, openai_text("git status", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let got = client(&base)
        .complete(&store, &config(Provider::OpenCodeGo, "kimi-k2.6"), &line("git sta"))
        .await
        .unwrap();
    assert_eq!(got.completion, "tus");
    let seen = seen.await.unwrap();
    assert_eq!(seen.method, "POST");
    assert_eq!(seen.path, "/inference/go/v1/chat/completions");
    assert_eq!(seen.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(seen.header("x-opencode-session"), Some(SESSION));
    assert_eq!(seen.header("user-agent"), Some(USER_AGENT));
    let body = &seen.body;
    assert_eq!(body["model"], "kimi-k2.6");
    assert_eq!(body["max_tokens"], 1024);
    assert!(body.get("max_completion_tokens").is_none(), "{body}");
    assert!(body.get("response_format").is_none(), "{body}");
    assert_eq!(body["messages"][0]["role"], "system");
}

#[tokio::test]
async fn opencode_go_assist_reads_the_json_answer() {
    let (base, seen) = mock(200, openai_text(ASSIST_JSON, "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let got = client(&base)
        .assist(&store, &config(Provider::OpenCodeGo, "glm-5.3"), &ask("disk"))
        .await
        .unwrap();
    assert_eq!(got.command.as_deref(), Some("df -h"));
    let seen = seen.await.unwrap();
    assert_eq!(seen.header("x-opencode-session"), Some(SESSION));
    assert_eq!(seen.body["max_tokens"], 8192);
}

#[test]
fn the_default_opencode_endpoints_are_the_go_api_and_models_dev() {
    let endpoints = Endpoints::default();
    assert_eq!(endpoints.opencode_go, "https://opencode.ai/inference/go/v1");
    assert_eq!(endpoints.models_dev, "https://models.dev/api.json");
}

#[tokio::test]
async fn opencode_go_refuses_a_missing_or_invalid_session_id_before_any_request() {
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let too_long = "a".repeat(129);
    for session in [
        None,
        Some(""),
        Some("has space"),
        Some("a/b"),
        Some("ñ"),
        Some(too_long.as_str()),
    ] {
        let config = AiConfig {
            session_id: session.map(String::from),
            ..config(Provider::OpenCodeGo, "kimi-k2.6")
        };
        let err = client("http://127.0.0.1:9")
            .complete(&store, &config, &line("ls"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, AiErrorKind::Config, "{session:?}");
        assert!(err.message.contains("session"), "{}", err.message);
    }
}

#[test]
fn session_ids_of_1_to_128_safe_characters_are_accepted() {
    let longest = "Az09._-".repeat(18) + "ab";
    assert_eq!(longest.len(), 128);
    for session in ["x", longest.as_str()] {
        let config = AiConfig {
            session_id: Some(session.into()),
            ..config(Provider::OpenCodeGo, "kimi-k2.6")
        };
        let call = providers::build(
            &Endpoints::default(),
            &config,
            Some(&ApiKey::new(KEY)),
            prompt(),
            opencode::GoFormat::Chat,
        )
        .unwrap();
        assert!(
            call.headers
                .iter()
                .any(|(name, value)| *name == "x-opencode-session" && value.as_str() == session)
        );
    }
}

fn compatible_at(base_url: &str) -> AiConfig {
    AiConfig {
        provider: Provider::OpenAiCompatible,
        base_url: Some(base_url.into()),
        model: "kimi-k2.6".into(),
        session_id: Some(SESSION.into()),
    }
}

#[test]
fn a_compatible_server_at_opencode_ai_gets_the_session_header() {
    for base in ["https://opencode.ai/inference/go/v1", "https://OpenCode.ai/zen/v1/"] {
        let call = providers::build(
            &Endpoints::default(),
            &compatible_at(base),
            Some(&ApiKey::new(KEY)),
            prompt(),
            opencode::GoFormat::Chat,
        )
        .unwrap();
        assert!(
            call.headers
                .iter()
                .any(|(name, value)| *name == "x-opencode-session" && value.as_str() == SESSION),
            "{base}"
        );
    }
    let missing = AiConfig {
        session_id: None,
        ..compatible_at("https://opencode.ai/inference/go/v1")
    };
    let err = providers::build(
        &Endpoints::default(),
        &missing,
        None,
        prompt(),
        opencode::GoFormat::Chat,
    )
    .err()
    .unwrap();
    assert_eq!(err.kind, AiErrorKind::Config);
}

#[tokio::test]
async fn a_compatible_server_elsewhere_gets_no_session_header() {
    let (base, seen) = mock(200, openai_text("ls -la", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAiCompatible, None);
    client("http://127.0.0.1:9")
        .complete(&store, &compatible_at(&format!("{base}/v1")), &line("ls"))
        .await
        .unwrap();
    let seen = seen.await.unwrap();
    assert_eq!(seen.header("x-opencode-session"), None);
    for base in ["https://opencode.ai.example.com/v1", "https://notopencode.ai/v1"] {
        let call = providers::build(
            &Endpoints::default(),
            &compatible_at(base),
            None,
            prompt(),
            opencode::GoFormat::Chat,
        )
        .unwrap();
        assert!(
            call.headers.iter().all(|(name, _)| *name != "x-opencode-session"),
            "{base}"
        );
    }
}

#[tokio::test]
async fn every_provider_identifies_itself_as_plexusterm() {
    let replies = [
        (Provider::Anthropic, anthropic_text("ls -l", "end_turn")),
        (Provider::Gemini, gemini_text("ls -l", "STOP")),
        (Provider::OpenAi, openai_text("ls -l", "stop")),
        (Provider::OpenAiCompatible, openai_text("ls -l", "stop")),
        (Provider::OpenCodeGo, openai_text("ls -l", "stop")),
    ];
    for (provider, reply) in replies {
        let (base, seen) = mock(200, reply, Duration::ZERO).await;
        let store = store_with(provider, Some(KEY));
        let config = AiConfig {
            base_url: Some(format!("{base}/v1")),
            ..config(provider, "m")
        };
        client(&base).complete(&store, &config, &line("ls")).await.unwrap();
        let seen = seen.await.unwrap();
        assert_eq!(seen.header("user-agent"), Some(USER_AGENT), "{provider:?}");
    }
}

#[test]
fn opencode_go_parses_with_its_id_and_name() {
    assert_eq!(Provider::parse("opencode-go").unwrap(), Provider::OpenCodeGo);
    assert_eq!(Provider::OpenCodeGo.id(), "opencode-go");
    assert_eq!(Provider::OpenCodeGo.key_account(), "ai:opencode-go");
    assert_eq!(Provider::OpenCodeGo.display_name(), "OpenCode Go");
    assert!(Provider::OpenCodeGo.requires_key());
}

// --- OpenCode Go: formats per model and the model catalogue ------------------------------

#[tokio::test]
async fn opencode_go_messages_models_get_the_anthropic_shape_with_both_key_headers() {
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("minimax-m3", "messages")], 0);
    let (base, seen) = mock(200, anthropic_text("git status", "end_turn"), Duration::ZERO).await;
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let ai = client(&base);
    ai.set_go_cache(Some(path));
    let got = ai
        .complete(&store, &config(Provider::OpenCodeGo, "minimax-m3"), &line("git sta"))
        .await
        .unwrap();
    assert_eq!(got.completion, "tus");
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/inference/go/v1/messages");
    assert_eq!(seen.header("x-api-key"), Some(KEY));
    assert_eq!(seen.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(seen.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(seen.header("x-opencode-session"), Some(SESSION));
    assert_eq!(seen.header("user-agent"), Some(USER_AGENT));
    assert_eq!(seen.header("anthropic-beta"), None);
    let body = &seen.body;
    assert_eq!(body["model"], "minimax-m3");
    assert_eq!(body["max_tokens"], 1024);
    assert_eq!(body["system"], text::COMPLETE_SYSTEM);
    assert_eq!(body["messages"][0]["role"], "user");
    assert!(body.get("output_config").is_none(), "{body}");
    assert!(body.get("fallbacks").is_none(), "{body}");
}

#[tokio::test]
async fn opencode_go_responses_models_post_instructions_and_input_to_responses() {
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("gpt-6-luna", "responses")], 0);
    let (base, seen) = mock(200, responses_text(ASSIST_JSON), Duration::ZERO).await;
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let ai = client(&base);
    ai.set_go_cache(Some(path));
    let got = ai
        .assist(&store, &config(Provider::OpenCodeGo, "gpt-6-luna"), &ask("disk"))
        .await
        .unwrap();
    assert_eq!(got.command.as_deref(), Some("df -h"));
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/inference/go/v1/responses");
    assert_eq!(seen.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(seen.header("x-api-key"), None);
    assert_eq!(seen.header("x-opencode-session"), Some(SESSION));
    let body = &seen.body;
    assert_eq!(body["model"], "gpt-6-luna");
    assert_eq!(body["instructions"], text::ASSIST_SYSTEM);
    assert!(body["input"].as_str().unwrap().contains("Request: disk"), "{body}");
    assert_eq!(body["max_output_tokens"], 8192);
    assert_eq!(body["store"], false);
    assert!(body.get("messages").is_none(), "{body}");
}

#[tokio::test]
async fn responses_answers_cut_off_or_refused_are_reported() {
    let cases = [
        (
            json!({"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}, "output": []}),
            AiErrorKind::BadResponse,
        ),
        (
            json!({"status": "completed", "output": [{"type": "message", "content": [
                {"type": "refusal", "refusal": "No."}]}]}),
            AiErrorKind::Refused,
        ),
    ];
    for (reply, kind) in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = go_cache(&dir, &[("gpt-6-luna", "responses")], 0);
        let (base, _seen) = mock(200, reply, Duration::ZERO).await;
        let store = store_with(Provider::OpenCodeGo, Some(KEY));
        let ai = client(&base);
        ai.set_go_cache(Some(path));
        let err = ai
            .assist(&store, &config(Provider::OpenCodeGo, "gpt-6-luna"), &ask("x"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, kind);
    }
}

#[tokio::test]
async fn opencode_go_google_models_are_not_supported_yet_and_send_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("gemini-4-argon-contributor", "google")], 0);
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let ai = client("http://127.0.0.1:9");
    ai.set_go_cache(Some(path));
    let err = ai
        .complete(
            &store,
            &config(Provider::OpenCodeGo, "gemini-4-argon-contributor"),
            &line("ls"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    assert!(err.message.contains("does not support"), "{}", err.message);
}

fn summary(models: &opencode::GoModels) -> Vec<(String, String, opencode::GoFormat, bool)> {
    models
        .models
        .iter()
        .map(|m| (m.id.clone(), m.name.clone(), m.format, m.supported))
        .collect()
}

#[tokio::test]
async fn the_opencode_go_catalogue_joins_the_public_list_with_models_dev_formats() {
    use opencode::GoFormat::*;
    let (base, log) = mock_routes(go_routes((200, GO_LIST), (200, MODELS_DEV))).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opencode-go-models.json");
    let ai = client(&base);
    ai.set_go_cache(Some(path.clone()));
    let got = ai.opencode_models(false).await.unwrap();
    let s = |v: &str| v.to_string();
    assert_eq!(
        summary(&got),
        vec![
            (
                s("gemini-4-argon-contributor"),
                s("Gemini 4 Argon Contributor"),
                Google,
                false
            ),
            (s("gpt-6-luna"), s("GPT-6 Luna"), Responses, true),
            (s("kimi-k3"), s("Kimi K3"), Chat, true),
            (s("minimax-m3"), s("MiniMax-M3"), Messages, true),
            (s("omen-alpha"), s("omen-alpha"), Chat, true),
        ]
    );
    assert_eq!(got.note, None);
    assert!(got.fetched_at >= now_secs() - 5);
    {
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        for seen in log.iter() {
            assert_eq!(seen.method, "GET");
            assert_eq!(seen.header("user-agent"), Some(USER_AGENT));
            assert_eq!(seen.header("authorization"), None);
        }
    }
    // Cached on disk for the next start, and in memory for this one.
    let saved: opencode::GoModels = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved, got);
    assert_eq!(ai.opencode_models(false).await.unwrap(), got);
    assert_eq!(log.lock().unwrap().len(), 2);
    // The cache file keeps v0.2.15's camelCase fields.
    let json = serde_json::to_value(&got).unwrap();
    assert_eq!(json["models"][1]["format"], "responses");
    assert!(json["fetchedAt"].is_u64());
}

#[tokio::test]
async fn a_fresh_cache_file_is_used_without_asking_and_refresh_or_age_fetches_again() {
    let (base, log) = mock_routes(go_routes((200, GO_LIST), (200, MODELS_DEV))).await;
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("cached-model", "chat")], 60);
    let ai = client(&base);
    ai.set_go_cache(Some(path.clone()));
    let got = ai.opencode_models(false).await.unwrap();
    assert_eq!(got.models[0].id, "cached-model");
    assert!(log.lock().unwrap().is_empty());
    let refreshed = ai.opencode_models(true).await.unwrap();
    assert_eq!(refreshed.models.len(), 5);
    assert_eq!(log.lock().unwrap().len(), 2);

    let old = go_cache(&dir, &[("cached-model", "chat")], opencode::MAX_AGE_SECS + 1);
    let ai = client(&base);
    ai.set_go_cache(Some(old));
    assert_eq!(ai.opencode_models(false).await.unwrap().models.len(), 5);
    assert_eq!(log.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn without_models_dev_every_listed_model_is_chat_completions_with_a_note() {
    let (base, _log) = mock_routes(go_routes((200, GO_LIST), (503, "{}"))).await;
    let got = client(&base).opencode_models(true).await.unwrap();
    assert_eq!(got.models.len(), 5);
    assert!(
        got.models
            .iter()
            .all(|m| m.format == opencode::GoFormat::Chat && m.supported && m.name == m.id)
    );
    assert_eq!(got.note.as_deref(), Some(opencode::NOTE_NO_FORMATS));
}

#[tokio::test]
async fn without_the_public_list_models_dev_alone_gives_the_models() {
    let (base, _log) = mock_routes(go_routes((500, "{}"), (200, MODELS_DEV))).await;
    let got = client(&base).opencode_models(true).await.unwrap();
    let ids: Vec<&str> = got.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "gemini-4-argon-contributor",
            "gpt-6-luna",
            "kimi-k3",
            "minimax-m3",
            "retired"
        ]
    );
}

#[tokio::test]
async fn when_both_sources_fail_the_saved_list_is_kept_with_a_note_or_it_is_a_network_error() {
    let (base, _log) = mock_routes(vec![]).await;
    let err = client(&base).opencode_models(true).await.unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Network);
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("cached-model", "messages")], 10);
    let ai = client(&base);
    ai.set_go_cache(Some(path));
    let got = ai.opencode_models(true).await.unwrap();
    assert_eq!(got.models[0].id, "cached-model");
    assert_eq!(got.note.as_deref(), Some(opencode::NOTE_STALE));
}

#[tokio::test]
async fn models_missing_from_the_catalogue_fall_back_to_chat_completions() {
    let dir = tempfile::tempdir().unwrap();
    let path = go_cache(&dir, &[("minimax-m3", "messages")], 0);
    let (base, seen) = mock(200, openai_text("ls -la", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenCodeGo, Some(KEY));
    let ai = client(&base);
    ai.set_go_cache(Some(path));
    ai.complete(&store, &config(Provider::OpenCodeGo, "new-model"), &line("ls"))
        .await
        .unwrap();
    assert_eq!(seen.await.unwrap().path, "/inference/go/v1/chat/completions");
}
