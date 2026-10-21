//! Verifier advisories and general answers (S8).

use super::*;

// --- Verifier advisories -----------------------------------------------------------------

#[tokio::test]
async fn redirects_are_not_followed_and_are_a_config_error() {
    let (elsewhere, other_log) = mock_routes(vec![]).await;
    let location: &'static str = Box::leak(format!("Location: {elsewhere}/steal\r\n").into_boxed_str());
    let (base, seen) = mock_with(302, location, json!({}), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    let err = client(&base)
        .complete(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &line("ls"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::Config);
    assert!(err.message.contains("redirect"), "{}", err.message);
    assert_eq!(seen.await.unwrap().header("x-api-key"), Some(KEY));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(other_log.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_assistant_command_over_2000_characters_is_dropped_and_the_answer_kept() {
    let long = "a".repeat(text::MAX_COMMAND + 1);
    let answer = format!("{{\"answer\": \"x\", \"command\": \"{long}\"}}");
    let (base, _seen) = mock(200, openai_text(&answer, "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let got = client(&base)
        .assist(&store, &config(Provider::OpenAi, "gpt-x"), &ask("x"))
        .await
        .unwrap();
    assert_eq!(
        got,
        AssistResponse {
            answer: "x".into(),
            command: None
        }
    );
}

// --- General answers (S8) ----------------------------------------------------------------

#[tokio::test]
async fn every_provider_reads_an_answer_without_a_command() {
    for (provider, model) in [
        (Provider::Anthropic, "claude-opus-5-5"),
        (Provider::Gemini, "gemini-3.5-flash-lite"),
        (Provider::OpenAi, "gpt-x"),
        (Provider::OpenCodeGo, "glm-5.3"),
        (Provider::OpenAiCompatible, "llama3.2"),
    ] {
        let (base, _seen) = mock(200, reply_for(provider, ANSWER_ONLY_JSON), Duration::ZERO).await;
        let store = store_with(provider, Some(KEY));
        let config = if provider == Provider::OpenAiCompatible {
            compatible_config(&base)
        } else {
            config(provider, model)
        };
        let got = client(&base).assist(&store, &config, &ask("hola")).await.unwrap();
        assert_eq!(
            got,
            AssistResponse {
                answer: "¡Hola! ¿En qué te ayudo?".into(),
                command: None
            },
            "{provider:?}"
        );
    }
}

#[tokio::test]
async fn every_provider_sends_the_general_assistant_prompt() {
    for (provider, model) in [
        (Provider::Anthropic, "claude-opus-5-5"),
        (Provider::Gemini, "gemini-3.5-flash-lite"),
        (Provider::OpenAi, "gpt-x"),
        (Provider::OpenCodeGo, "glm-5.3"),
        (Provider::OpenAiCompatible, "llama3.2"),
    ] {
        let (base, seen) = mock(200, reply_for(provider, ASSIST_JSON), Duration::ZERO).await;
        let store = store_with(provider, Some(KEY));
        let config = if provider == Provider::OpenAiCompatible {
            compatible_config(&base)
        } else {
            config(provider, model)
        };
        client(&base)
            .assist(&store, &config, &ask("¿qué es un inodo?"))
            .await
            .unwrap();
        let body = seen.await.unwrap().body;
        let system = match provider {
            Provider::Anthropic => &body["system"],
            Provider::Gemini => &body["systemInstruction"]["parts"][0]["text"],
            _ => &body["messages"][0]["content"],
        };
        assert_eq!(system, text::ASSIST_SYSTEM, "{provider:?}");
    }
    let prompt = text::ASSIST_SYSTEM;
    for needle in [
        "any question",
        "language the user wrote",
        "\"command\": null",
        "never run",
    ] {
        assert!(prompt.contains(needle), "{needle}: {prompt}");
    }
}

#[tokio::test]
async fn plain_text_replies_become_the_answer_with_no_command() {
    let reply = "Un inodo guarda los datos de un archivo.\n```sh\nls -i\n```";
    // OpenAI-compatible (chat completions).
    let (base, _seen) = mock(200, openai_text(reply, "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAiCompatible, None);
    let got = client(&base)
        .assist(&store, &compatible_config(&base), &ask("inodo"))
        .await
        .unwrap();
    assert_eq!(got.answer, reply);
    assert_eq!(got.command, None);
    // OpenCode Go messages and responses models.
    for (model, format, body) in [
        ("minimax-m3", "messages", anthropic_text(reply, "end_turn")),
        ("gpt-6-luna", "responses", responses_text(reply)),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = go_cache(&dir, &[(model, format)], 0);
        let (base, _seen) = mock(200, body, Duration::ZERO).await;
        let store = store_with(Provider::OpenCodeGo, Some(KEY));
        let ai = client(&base);
        ai.set_go_cache(Some(path));
        let got = ai
            .assist(&store, &config(Provider::OpenCodeGo, model), &ask("inodo"))
            .await
            .unwrap();
        assert_eq!(
            got,
            AssistResponse {
                answer: reply.into(),
                command: None
            },
            "{format}"
        );
    }
}

#[tokio::test]
async fn an_empty_assistant_reply_is_a_bad_response() {
    let (base, _seen) = mock(200, openai_text("  \n", "stop"), Duration::ZERO).await;
    let store = store_with(Provider::OpenAi, Some(KEY));
    let err = client(&base)
        .assist(&store, &config(Provider::OpenAi, "gpt-x"), &ask("x"))
        .await
        .unwrap_err();
    assert_eq!(err.kind, AiErrorKind::BadResponse);
}
