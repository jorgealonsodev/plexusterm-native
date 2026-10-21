//! Anthropic, Gemini and OpenAI(-compatible) round trips.

use super::*;

// --- Anthropic ---------------------------------------------------------------------------

#[tokio::test]
async fn anthropic_completion_request_and_answer() {
    let (base, seen) = mock(200, anthropic_text("git commit -m \"wip\"", "end_turn"), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let got = client(&base)
        .complete(
            &store,
            &config(Provider::Anthropic, "claude-opus-5-5"),
            &line("git comm"),
        )
        .await
        .unwrap();
    assert_eq!(got.completion, "it -m \"wip\"");

    let seen = seen.await.unwrap();
    assert_eq!(seen.method, "POST");
    assert_eq!(seen.path, "/v1/messages");
    assert_eq!(seen.header("x-api-key"), Some(KEY));
    assert_eq!(seen.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(seen.header("anthropic-beta"), Some("server-side-fallback-2026-07-01"));
    assert_eq!(seen.header("content-type"), Some("application/json"));
    assert_eq!(seen.header("authorization"), None);
    let body = &seen.body;
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["max_tokens"], 2048);
    assert_eq!(body["output_config"]["effort"], "low");
    assert!(body["output_config"].get("format").is_none(), "{body}");
    assert_eq!(body["fallbacks"], "default");
    assert!(body.get("thinking").is_none(), "{body}");
    assert!(body.get("tool_choice").is_none(), "{body}");
    assert!(body.get("tools").is_none(), "{body}");
    assert_eq!(body["system"], text::COMPLETE_SYSTEM);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1, "no assistant prefill: {body}");
    assert_eq!(messages[0]["role"], "user");
    let user = messages[0]["content"].as_str().unwrap();
    assert!(user.contains(r#"Typed so far: "git comm""#), "{user}");
    assert!(user.contains("git status"), "{user}");
    assert!(user.contains("/srv"), "{user}");
}

#[tokio::test]
async fn anthropic_fallbacks_only_for_claude_opus_5_5() {
    let (base, seen) = mock(200, anthropic_text("ls -la", "end_turn"), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    client(&base)
        .complete(&store, &config(Provider::Anthropic, "claude-haiku-5-5"), &line("ls"))
        .await
        .unwrap();
    let seen = seen.await.unwrap();
    assert_eq!(seen.header("anthropic-beta"), None);
    assert!(seen.body.get("fallbacks").is_none(), "{}", seen.body);
    assert_eq!(seen.body["model"], "claude-haiku-5-5");
    assert_eq!(seen.body["output_config"]["effort"], "low");
}

#[tokio::test]
async fn anthropic_assist_request_and_answer() {
    let (base, seen) = mock(200, anthropic_text(ASSIST_JSON, "end_turn"), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let got = client(&base)
        .assist(
            &store,
            &config(Provider::Anthropic, "claude-opus-5-5"),
            &ask("free space?"),
        )
        .await
        .unwrap();
    assert_eq!(
        got,
        AssistResponse {
            answer: "Shows free disk space.".into(),
            command: Some("df -h".into()),
        }
    );
    let body = seen.await.unwrap().body;
    assert_eq!(body["max_tokens"], 16000);
    assert_eq!(body["output_config"]["effort"], "medium");
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    let schema = &body["output_config"]["format"]["schema"];
    assert_eq!(schema["required"], json!(["answer", "command"]));
    assert_eq!(schema["properties"]["answer"]["type"], "string");
    assert_eq!(
        schema["properties"]["command"]["anyOf"],
        json!([{"type": "string"}, {"type": "null"}])
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(body["system"], text::ASSIST_SYSTEM);
    assert_eq!(body["fallbacks"], "default");
}

#[tokio::test]
async fn anthropic_refusal_is_refused() {
    let reply = json!({"type": "message", "content": [], "stop_reason": "refusal",
                       "stop_details": {"category": "cyber"}});
    let (base, _seen) = mock(200, reply, Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let err = client(&base)
        .assist(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &ask("x"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Refused);
}

#[tokio::test]
async fn anthropic_max_tokens_gives_no_completion() {
    let (base, _seen) = mock(200, anthropic_text("ls -l", "max_tokens"), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let got = client(&base)
        .complete(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &line("ls"))
        .await
        .unwrap();
    assert_eq!(got.completion, "");
}

// --- Gemini ------------------------------------------------------------------------------

#[tokio::test]
async fn gemini_completion_request_and_answer() {
    let (base, seen) = mock(200, gemini_text("docker compose up -d", "STOP"), Duration::ZERO).await;
    let store = store_with(Provider::Gemini, Some(KEY));
    let got = client(&base)
        .complete(
            &store,
            &config(Provider::Gemini, "gemini-3.5-flash-lite"),
            &line("docker comp"),
        )
        .await
        .unwrap();
    assert_eq!(got.completion, "ose up -d");
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/v1beta/models/gemini-3.5-flash-lite:generateContent");
    assert_eq!(seen.header("x-goog-api-key"), Some(KEY));
    assert_eq!(seen.header("authorization"), None);
    assert!(!seen.path.contains("key="), "key must not be in the URL");
    let body = &seen.body;
    assert_eq!(body["systemInstruction"]["parts"][0]["text"], text::COMPLETE_SYSTEM);
    assert_eq!(body["contents"][0]["role"], "user");
    let user = body["contents"][0]["parts"][0]["text"].as_str().unwrap();
    assert!(user.contains(r#"Typed so far: "docker comp""#), "{user}");
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 1024);
    assert!(body["generationConfig"].get("responseMimeType").is_none(), "{body}");
}

#[tokio::test]
async fn gemini_assist_asks_for_json() {
    let (base, seen) = mock(200, gemini_text(ASSIST_JSON, "STOP"), Duration::ZERO).await;
    let store = store_with(Provider::Gemini, Some(KEY));
    let got = client(&base)
        .assist(&store, &config(Provider::Gemini, "gemini-3.5-flash-lite"), &ask("disk"))
        .await
        .unwrap();
    assert_eq!(got.command.as_deref(), Some("df -h"));
    let body = seen.await.unwrap().body;
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 8192);
    assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
    let schema = &body["generationConfig"]["responseSchema"];
    assert_eq!(schema["required"], json!(["answer"]));
    assert_eq!(schema["properties"]["answer"]["type"], "STRING");
    assert_eq!(schema["properties"]["command"]["type"], "STRING");
    assert_eq!(schema["properties"]["command"]["nullable"], true);
}

#[tokio::test]
async fn gemini_blocked_prompt_and_safety_stop_are_refused() {
    let blocked = json!({"promptFeedback": {"blockReason": "SAFETY"}});
    let (base, _seen) = mock(200, blocked, Duration::ZERO).await;
    let store = store_with(Provider::Gemini, Some(KEY));
    let err = client(&base)
        .assist(&store, &config(Provider::Gemini, "gemini-3.5-flash-lite"), &ask("x"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Refused);

    let (base, _seen) = mock(200, gemini_text("", "SAFETY"), Duration::ZERO).await;
    let err = client(&base)
        .complete(&store, &config(Provider::Gemini, "gemini-3.5-flash-lite"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Refused);
}

#[tokio::test]
async fn gemini_invalid_key_is_auth() {
    let reply = json!({"error": {"code": 400, "message": "API key not valid. Please pass a valid API key.",
        "status": "INVALID_ARGUMENT", "details": [{"reason": "API_KEY_INVALID"}]}});
    let (base, _seen) = mock(400, reply, Duration::ZERO).await;
    let store = store_with(Provider::Gemini, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::Gemini, "gemini-3.5-flash-lite"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Auth);
}

#[tokio::test]
async fn gemini_model_names_must_be_path_safe() {
    let store = store_with(Provider::Gemini, Some(KEY));
    let err = client("http://127.0.0.1:9")
        .complete(&store, &config(Provider::Gemini, "../evil?x=1"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
}

// --- OpenAI and compatible servers -------------------------------------------------------

#[tokio::test]
async fn openai_completion_request_and_answer() {
    let (base, seen) = mock(200, openai_text("kubectl get pods -A", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let got = client(&base)
        .complete(&store, &config(Provider::OpenAi, "gpt-x"), &line("kubectl get po"))
        .await
        .unwrap();
    assert_eq!(got.completion, "ds -A");
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/v1/chat/completions");
    assert_eq!(seen.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    let body = &seen.body;
    assert_eq!(body["model"], "gpt-x");
    assert_eq!(body["max_completion_tokens"], 1024);
    assert!(body.get("max_tokens").is_none(), "{body}");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], text::COMPLETE_SYSTEM);
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert!(body.get("response_format").is_none(), "{body}");
}

#[tokio::test]
async fn openai_assist_uses_a_strict_json_schema() {
    let (base, seen) = mock(200, openai_text(ASSIST_JSON, "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let got = client(&base)
        .assist(&store, &config(Provider::OpenAi, "gpt-x"), &ask("disk"))
        .await
        .unwrap();
    assert_eq!(got.command.as_deref(), Some("df -h"));
    let body = seen.await.unwrap().body;
    assert_eq!(body["max_completion_tokens"], 8192);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    let schema = &body["response_format"]["json_schema"]["schema"];
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"], json!(["answer", "command"]));
    assert_eq!(
        schema["properties"]["command"]["anyOf"],
        json!([{"type": "string"}, {"type": "null"}])
    );
}

#[tokio::test]
async fn openai_refusal_is_refused() {
    let reply = json!({"choices": [{"index": 0, "message": {"role": "assistant", "content": null,
        "refusal": "I can't help with that."}, "finish_reason": "stop"}]});
    let (base, _seen) = mock(200, reply, Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .assist(&store, &config(Provider::OpenAi, "gpt-x"), &ask("x"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Refused);
}

#[tokio::test]
async fn compatible_server_without_key_sends_no_authorization() {
    let reply = openai_text(&format!("```json\n{ASSIST_JSON}\n```"), "stop");
    let (base, seen) = mock(200, reply, Duration::ZERO).await;
    let store = store_with(Provider::OpenAiCompatible, None);
    let config = AiConfig {
        provider: Provider::OpenAiCompatible,
        base_url: Some(format!("{base}/v1/")),
        model: "llama3.2".into(),
        session_id: None,
    };
    let got = client("http://127.0.0.1:9")
        .assist(&store, &config, &ask("disk"))
        .await
        .unwrap();
    assert_eq!(got.command.as_deref(), Some("df -h"));
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/v1/chat/completions");
    assert_eq!(seen.header("authorization"), None);
    assert_eq!(seen.body["model"], "llama3.2");
    assert_eq!(seen.body["max_tokens"], 8192);
    assert!(seen.body.get("max_completion_tokens").is_none(), "{}", seen.body);
    assert!(seen.body.get("response_format").is_none(), "{}", seen.body);
}

#[tokio::test]
async fn compatible_server_with_key_sends_a_bearer_token() {
    let (base, seen) = mock(200, openai_text("ls -la", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAiCompatible, Some(KEY));
    let config = AiConfig {
        provider: Provider::OpenAiCompatible,
        base_url: Some(format!("{base}/api/v1")),
        model: "openai/gpt-x".into(),
        session_id: Some(SESSION.into()),
    };
    client("http://127.0.0.1:9")
        .complete(&store, &config, &line("ls"))
        .await
        .unwrap();
    let seen = seen.await.unwrap();
    assert_eq!(seen.path, "/api/v1/chat/completions");
    assert_eq!(seen.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(seen.body["max_tokens"], 1024);
}

#[tokio::test]
async fn compatible_base_url_is_required_and_must_be_http() {
    let store = store_with(Provider::OpenAiCompatible, None);
    for base_url in [
        None,
        Some(""),
        Some("ftp://host/v1"),
        Some("not a url"),
        Some("http://"),
    ] {
        let config = AiConfig {
            provider: Provider::OpenAiCompatible,
            base_url: base_url.map(String::from),
            model: "m".into(),
            session_id: None,
        };
        let err = client("http://127.0.0.1:9")
            .complete(&store, &config, &line("ls"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, AiErrorKind::Config, "{base_url:?}");
    }
}

#[tokio::test]
async fn an_empty_model_is_a_config_error() {
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client("http://127.0.0.1:9")
        .complete(&store, &config(Provider::OpenAi, "  "), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
}
