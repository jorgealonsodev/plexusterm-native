//! Chat follow-ups (S9).

use super::*;

// --- Chat conversation (S9) --------------------------------------------------------------

fn follow_up() -> AssistRequest {
    AssistRequest {
        conversation: vec![Turn {
            question: "what uses the space?".into(),
            selection: None,
            answer: "Sizes, smallest first.".into(),
            command: Some("du -sh *".into()),
        }],
        ..ask("and sorted?")
    }
}

const FIRST_ANSWER: &str = r#"{"answer":"Sizes, smallest first.","command":"du -sh *"}"#;

/// `(role, text)` of each message, whatever the provider's shape.
fn roles_and_texts(messages: &Value, text_of: impl Fn(&Value) -> String) -> Vec<(String, String)> {
    messages
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["role"].as_str().unwrap().to_string(), text_of(m)))
        .collect()
}

fn assert_replayed(got: &[(String, String)], assistant_role: &str, what: &str) {
    assert_eq!(got.len(), 3, "{what}: {got:?}");
    assert_eq!(
        got[0],
        ("user".into(), "Request: what uses the space?".into()),
        "{what}"
    );
    assert_eq!(got[1].0, assistant_role, "{what}");
    let replayed: Value = serde_json::from_str(&got[1].1).unwrap();
    assert_eq!(replayed, serde_json::from_str::<Value>(FIRST_ANSWER).unwrap(), "{what}");
    assert_eq!(got[2].0, "user", "{what}");
    assert!(got[2].1.contains("Request: and sorted?"), "{what}: {}", got[2].1);
    assert!(!got[2].1.contains("what uses the space"), "{what}");
}

#[tokio::test]
async fn follow_ups_replay_earlier_turns_as_alternating_messages_for_every_provider() {
    for (provider, model) in [
        (Provider::Anthropic, "claude-opus-5-5"),
        (Provider::Gemini, "gemini-3.5-flash-lite"),
        (Provider::OpenAi, "gpt-x"),
        (Provider::OpenAiCompatible, "llama3.2"),
    ] {
        let (base, seen) = mock(200, reply_for(provider, ASSIST_JSON), Duration::ZERO).await;
        let store = store_with(provider, Some(KEY));
        let config = if provider == Provider::OpenAiCompatible {
            compatible_config(&base)
        } else {
            config(provider, model)
        };
        client(&base).assist(&store, &config, &follow_up()).await.unwrap();
        let body = seen.await.unwrap().body;
        let what = format!("{provider:?}");
        match provider {
            Provider::Anthropic => {
                assert_eq!(body["system"], text::ASSIST_SYSTEM);
                let got = roles_and_texts(&body["messages"], |m| m["content"].as_str().unwrap().into());
                assert_replayed(&got, "assistant", &what);
            }
            Provider::Gemini => {
                assert_eq!(body["systemInstruction"]["parts"][0]["text"], text::ASSIST_SYSTEM);
                let got = roles_and_texts(&body["contents"], |m| m["parts"][0]["text"].as_str().unwrap().into());
                assert_replayed(&got, "model", &what);
            }
            _ => {
                let messages = body["messages"].as_array().unwrap();
                assert_eq!(messages[0]["role"], "system", "{what}");
                assert_eq!(messages[0]["content"], text::ASSIST_SYSTEM, "{what}");
                let rest = Value::Array(messages[1..].to_vec());
                let got = roles_and_texts(&rest, |m| m["content"].as_str().unwrap().into());
                assert_replayed(&got, "assistant", &what);
            }
        }
    }
}

#[tokio::test]
async fn opencode_go_follow_ups_replay_turns_in_each_model_format() {
    for (model, format) in [
        ("glm-5.3", "chat"),
        ("minimax-m3", "messages"),
        ("gpt-6-luna", "responses"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = go_cache(&dir, &[(model, format)], 0);
        let reply = match format {
            "messages" => anthropic_text(ASSIST_JSON, "end_turn"),
            "responses" => responses_text(ASSIST_JSON),
            _ => openai_text(ASSIST_JSON, "stop"),
        };
        let (base, seen) = mock(200, reply, Duration::ZERO).await;
        let store = store_with(Provider::OpenCodeGo, Some(KEY));
        let ai = client(&base);
        ai.set_go_cache(Some(path));
        ai.assist(&store, &config(Provider::OpenCodeGo, model), &follow_up())
            .await
            .unwrap();
        let seen = seen.await.unwrap();
        assert_eq!(seen.header("x-opencode-session"), Some(SESSION), "{format}");
        let body = &seen.body;
        let content = |m: &Value| m["content"].as_str().unwrap().to_string();
        match format {
            "messages" => {
                assert_eq!(body["system"], text::ASSIST_SYSTEM);
                assert_replayed(&roles_and_texts(&body["messages"], content), "assistant", format);
            }
            "responses" => {
                assert_eq!(body["instructions"], text::ASSIST_SYSTEM);
                assert_replayed(&roles_and_texts(&body["input"], content), "assistant", format);
            }
            _ => {
                let messages = body["messages"].as_array().unwrap();
                assert_eq!(messages[0]["content"], text::ASSIST_SYSTEM);
                let rest = Value::Array(messages[1..].to_vec());
                assert_replayed(&roles_and_texts(&rest, content), "assistant", format);
            }
        }
    }
}

#[tokio::test]
async fn a_first_question_sends_no_earlier_turns() {
    let (base, seen) = mock(200, anthropic_text(ASSIST_JSON, "end_turn"), Duration::ZERO).await;
    let store = store_with(Provider::Anthropic, Some(KEY));
    client(&base)
        .assist(&store, &config(Provider::Anthropic, "claude-opus-5-5"), &ask("disk"))
        .await
        .unwrap();
    let body = seen.await.unwrap().body;
    assert_eq!(body["messages"].as_array().unwrap().len(), 1, "{body}");
}
