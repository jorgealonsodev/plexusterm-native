//! Prompts sent to the providers and cleaning of what they answer.
//!
//! Everything a provider returns is untrusted: commands come back as one line with no
//! control characters, so typing them into a terminal can never press Enter or send an
//! escape sequence; assistant answers keep only their line breaks and tabs.

use crate::{AssistRequest, CompleteRequest, Turn};

pub const COMPLETE_SYSTEM: &str = "You complete shell commands while the user types them in a terminal. \
Reply with only the complete command line, starting with exactly the text already typed, on a single line. \
No explanation, no quotes, no markdown. If you have no useful completion, reply with nothing.";

pub const ASSIST_SYSTEM: &str = "You are the assistant inside PlexusTerm, a terminal app. \
Answer any question or request: explanations, steps, diagnosing an error or selected text, general help, any topic. \
Reply in the language the user wrote in. Keep the answer clear and reasonably short, in plain text \
(no markdown except fenced code blocks for code). \
When, and only when, a single shell command for the given OS and shell directly does what the user asks, \
also give that command as one line (combine steps with && or pipes); otherwise give no command. \
You never run anything and must not claim to have run anything: the user reviews any command before running it. \
Reply with only a JSON object {\"answer\": \"...\", \"command\": \"...\"}, \
or {\"answer\": \"...\", \"command\": null} when no command fits.";

/// History commands sent as context at most (the settings allow 0-20).
const MAX_HISTORY: usize = 20;
/// Longest single field (line, history entry, folder) sent.
const MAX_FIELD: usize = 500;
const MAX_PROMPT: usize = 4000;
const MAX_SELECTION: usize = 8000;
/// Files attached to one question, and the characters of their text in all.
pub const MAX_FILES: usize = 5;
pub const MAX_FILES_TOTAL: usize = 100_000;
const MAX_DETAIL: usize = 200;
const MAX_COMPLETION: usize = 300;
/// Longest assistant command accepted; a longer one is dropped (never cut), the answer kept.
pub const MAX_COMMAND: usize = 2000;
/// Longest assistant answer shown; a longer one is cut and ends with `ANSWER_CUT_NOTE`.
pub const MAX_ANSWER: usize = 8000;
pub const ANSWER_CUT_NOTE: &str = "\n… (answer shortened)";

/// Earlier chat turns replayed with a question at most (the oldest go first).
pub const MAX_TURNS: usize = 10;
/// Characters of replayed turns at most, questions and answers together (the oldest go first).
pub const MAX_CONVERSATION: usize = 16_000;

/// One earlier turn as it is replayed: the user's message and the assistant's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exchange {
    pub user: String,
    pub assistant: String,
}

/// Cuts `s` to at most `max` characters.
fn cap(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((at, _)) => s.get(..at).unwrap_or(s),
        None => s,
    }
}

fn context(os: &str, shell: Option<&str>, cwd: Option<&str>, history: &[String]) -> String {
    let mut out = format!("OS: {}\n", cap(os, MAX_FIELD));
    if let Some(shell) = shell.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("Shell: {}\n", cap(shell, MAX_FIELD)));
    }
    if let Some(cwd) = cwd.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("Current folder: {}\n", cap(cwd, MAX_FIELD)));
    }
    let recent = history
        .get(history.len().saturating_sub(MAX_HISTORY)..)
        .unwrap_or_default();
    if !recent.is_empty() {
        out.push_str("Recent commands (oldest first):\n");
        for command in recent {
            out.push_str(&single_line(cap(command, MAX_FIELD)));
            out.push('\n');
        }
    }
    out
}

pub fn complete_prompt(request: &CompleteRequest) -> String {
    let mut out = context(
        &request.os,
        request.shell.as_deref(),
        request.cwd.as_deref(),
        &request.history,
    );
    // JSON string quoting keeps trailing spaces visible.
    let typed = serde_json::to_string(cap(&request.line, MAX_FIELD)).unwrap_or_default();
    out.push_str(&format!("Typed so far: {typed}"));
    out
}

pub fn assist_prompt(request: &AssistRequest) -> String {
    let mut out = context(
        &request.os,
        request.shell.as_deref(),
        request.cwd.as_deref(),
        &request.history,
    );
    if let Some(selection) = request.selection.as_deref().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("Selected terminal text:\n{}\n", cap(selection, MAX_SELECTION)));
    }
    let mut budget = MAX_FILES_TOTAL;
    for file in request.files.iter().take(MAX_FILES) {
        if budget == 0 {
            break;
        }
        let content = cap(&file.content, budget);
        budget -= content.chars().count();
        out.push_str(&format!(
            "Attached file {}:\n{}\n\n",
            single_line(cap(&file.path, MAX_FIELD)),
            content
        ));
    }
    out.push_str(&format!("Request: {}", cap(request.prompt.trim(), MAX_PROMPT)));
    out
}

/// Drops `<think>...</think>` blocks some local models put in their answer.
fn strip_reasoning(reply: &str) -> String {
    let mut out = String::new();
    let mut rest = reply;
    while let Some((before, after)) = rest.split_once("<think>") {
        out.push_str(before);
        match after.split_once("</think>") {
            Some((_, tail)) => rest = tail,
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Splits off the first fenced code block: (its content, the text around it).
fn split_fence(reply: &str) -> Option<(String, String)> {
    let (before, after) = reply.split_once("```")?;
    let (_, body) = after.split_once('\n')?;
    let (code, tail) = body.split_once("```")?;
    let outside = [before.trim(), tail.trim()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Some((code.to_string(), outside))
}

/// Unicode format characters (category Cf: bidi overrides, zero-width marks, BOM...) and
/// the line/paragraph separators (Zl, Zp); they could hide or reorder what a command shows.
fn is_format_or_separator(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Characters never kept in a provider's answer: controls (Cc), format (Cf) and separators (Zl, Zp).
fn is_unsafe(c: char) -> bool {
    c.is_control() || is_format_or_separator(c)
}

/// Removes control, format and separator characters (tabs become spaces).
fn strip_controls(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\t' => Some(' '),
            c if is_unsafe(c) => None,
            c => Some(c),
        })
        .collect()
}

/// Joins a multi-line command into one line: `\\`-continuations with a space, others with `&&`.
fn single_line(command: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut continued = false;
    for raw in command.replace("\r\n", "\n").split(['\n', '\r']) {
        let line = strip_controls(raw);
        let line = if continued { line.trim_start() } else { line.as_str() }.trim_end();
        let (line, continues) = match line.strip_suffix('\\') {
            Some(head) => (head.trim_end(), true),
            None => (line, false),
        };
        match lines.last_mut() {
            Some(last) if continued => {
                if !line.is_empty() {
                    last.push(' ');
                    last.push_str(line);
                }
            }
            _ if line.trim().is_empty() => {}
            _ => lines.push(line.trim_start().to_string()),
        }
        continued = continues && !lines.is_empty();
    }
    lines.join(" && ")
}

fn unwrap_quotes(s: &str) -> &str {
    for q in ['`', '"', '\''] {
        if let Some(inner) = s.strip_prefix(q).and_then(|rest| rest.strip_suffix(q))
            && !inner.contains(q)
        {
            return inner;
        }
    }
    s
}

/// The text to append after `line`, from a reply that should repeat the whole line.
pub fn clean_completion(line: &str, reply: &str) -> String {
    let reply = strip_reasoning(reply);
    let text = split_fence(&reply).map_or(reply, |(code, _)| code);
    let Some(first) = text.lines().map(str::trim).find(|l| !l.is_empty()) else {
        return String::new();
    };
    let mut candidate = unwrap_quotes(first);
    for marker in ["$ ", "> ", "# "] {
        candidate = candidate.strip_prefix(marker).unwrap_or(candidate);
    }
    let candidate = strip_controls(candidate);
    let typed = line.trim_start();
    let rest = if let Some(rest) = candidate.strip_prefix(typed) {
        rest
    } else if typed.ends_with(char::is_whitespace) {
        // The model dropped the trailing space: accept it only at a word boundary.
        match candidate.strip_prefix(typed.trim_end()) {
            Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim_start(),
            _ => "",
        }
    } else {
        ""
    };
    cap(rest.trim_end(), MAX_COMPLETION).to_string()
}

#[derive(serde::Deserialize)]
struct Suggestion {
    #[serde(default)]
    answer: Option<String>,
    /// The older `{command, explanation}` shape, read as the answer.
    #[serde(default)]
    explanation: Option<String>,
    #[serde(default)]
    command: Option<String>,
}

/// The JSON answer in `text`, if any; an object without any of its fields (such as the
/// `{}` of `find -exec`) is ordinary text.
fn json_object(text: &str) -> Option<Suggestion> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str::<Suggestion>(text.get(start..=end)?)
        .ok()
        .filter(|s| s.answer.is_some() || s.explanation.is_some() || s.command.is_some())
}

/// Reads `(answer, command)` from the assistant's reply: JSON (bare, fenced or inside
/// text), else the whole reply as the answer with no command. `None` when nothing is left.
/// A command that is empty, or longer than `MAX_COMMAND` once on one line, is dropped.
pub fn parse_assist(reply: &str) -> Option<(String, Option<String>)> {
    let reply = strip_reasoning(reply);
    let parsed = json_object(&reply).or_else(|| split_fence(&reply).and_then(|(code, _)| json_object(&code)));
    let (answer, command) = match parsed {
        Some(s) => (s.answer.or(s.explanation).unwrap_or_default(), s.command),
        None => (reply, None),
    };
    let command = command
        .map(|c| single_line(&c))
        .filter(|c| !c.is_empty() && c.chars().count() <= MAX_COMMAND);
    let answer = clean_answer(&answer);
    if answer.is_empty() && command.is_none() {
        return None;
    }
    Some((answer, command))
}

/// An answer for display: CRLF/CR become LF; newlines and tabs stay, every other control,
/// format or separator character goes; at most `MAX_ANSWER` characters plus a note.
fn clean_answer(s: &str) -> String {
    let kept = clean_text(s);
    let kept = kept.as_str();
    if kept.chars().count() > MAX_ANSWER {
        format!("{}{ANSWER_CUT_NOTE}", cap(kept, MAX_ANSWER).trim_end())
    } else {
        kept.to_string()
    }
}

/// Earlier turns as user/assistant messages, oldest first: control characters stripped
/// (newlines and tabs kept), the assistant's reply in the JSON format it was asked for, at
/// most `MAX_TURNS` turns and `MAX_CONVERSATION` characters, dropping the oldest first.
pub fn conversation(turns: &[Turn]) -> Vec<Exchange> {
    let recent = turns.get(turns.len().saturating_sub(MAX_TURNS)..).unwrap_or_default();
    let mut out: Vec<Exchange> = recent.iter().filter_map(replay).collect();
    let size = |e: &Exchange| e.user.chars().count() + e.assistant.chars().count();
    let mut total: usize = out.iter().map(size).sum();
    let mut drop = 0;
    for exchange in &out {
        if total <= MAX_CONVERSATION {
            break;
        }
        total -= size(exchange);
        drop += 1;
    }
    out.drain(..drop);
    out
}

/// One turn as messages; `None` for an empty question.
fn replay(turn: &Turn) -> Option<Exchange> {
    let question = clean_text(&turn.question);
    let question = cap(&question, MAX_PROMPT);
    if question.is_empty() {
        return None;
    }
    let mut user = String::new();
    if let Some(selection) = turn.selection.as_deref().map(clean_text).filter(|s| !s.is_empty()) {
        user.push_str(&format!(
            "Selected terminal text:\n{}\n",
            cap(&selection, MAX_SELECTION)
        ));
    }
    user.push_str(&format!("Request: {question}"));
    let command = turn
        .command
        .as_deref()
        .map(single_line)
        .filter(|c| !c.is_empty() && c.chars().count() <= MAX_COMMAND);
    let assistant = serde_json::json!({"answer": clean_answer(&turn.answer), "command": command});
    Some(Exchange {
        user,
        assistant: assistant.to_string(),
    })
}

/// Text replayed to a provider: newlines and tabs kept, other controls, format and separator
/// characters removed, trimmed.
fn clean_text(s: &str) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter_map(|c| match c {
            '\n' | '\t' => Some(c),
            '\r' => Some('\n'),
            c if is_unsafe(c) => None,
            c => Some(c),
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The provider's own error message (`error.message`), with the key masked and shortened.
pub fn error_detail(body: &str, key: Option<&str>) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = value.pointer("/error/message")?.as_str()?;
    let mut message = strip_controls(message.trim());
    if let Some(key) = key.filter(|k| !k.is_empty()) {
        message = message.replace(key, "***");
    }
    if message.is_empty() {
        return None;
    }
    if message.chars().count() > MAX_DETAIL {
        message = format!("{}…", cap(&message, MAX_DETAIL));
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "test assertions")]

    use super::*;

    fn complete(line: &str) -> CompleteRequest {
        CompleteRequest {
            line: line.into(),
            os: "linux".into(),
            shell: Some("bash".into()),
            cwd: Some("/home/ana/src".into()),
            history: vec!["git status".into(), "cargo test".into()],
        }
    }

    #[test]
    fn complete_prompt_carries_only_the_given_fields() {
        let prompt = complete_prompt(&complete("git comm"));
        assert!(prompt.contains("OS: linux"), "{prompt}");
        assert!(prompt.contains("Shell: bash"), "{prompt}");
        assert!(prompt.contains("Current folder: /home/ana/src"), "{prompt}");
        assert!(prompt.contains("git status\ncargo test"), "{prompt}");
        assert!(prompt.contains(r#"Typed so far: "git comm""#), "{prompt}");

        let bare = CompleteRequest {
            line: "ls ".into(),
            os: "linux".into(),
            ..Default::default()
        };
        let prompt = complete_prompt(&bare);
        assert!(!prompt.contains("Shell:"), "{prompt}");
        assert!(!prompt.contains("Current folder:"), "{prompt}");
        assert!(!prompt.contains("Recent commands"), "{prompt}");
        assert!(prompt.contains(r#"Typed so far: "ls ""#), "{prompt}");
    }

    #[test]
    fn prompts_keep_only_the_last_twenty_history_commands() {
        let mut request = complete("ls");
        request.history = (0..30).map(|i| format!("cmd{i}")).collect();
        let prompt = complete_prompt(&request);
        assert!(!prompt.contains("cmd9\n"), "{prompt}");
        assert!(prompt.contains("cmd10\n"), "{prompt}");
        assert!(prompt.contains("cmd29"), "{prompt}");
    }

    #[test]
    fn assist_prompt_includes_the_request_and_the_selection_only_when_given() {
        let request = AssistRequest {
            prompt: "find big files".into(),
            selection: Some("du: cannot read".into()),
            os: "linux".into(),
            ..Default::default()
        };
        let prompt = assist_prompt(&request);
        assert!(prompt.contains("Request: find big files"), "{prompt}");
        assert!(prompt.contains("Selected terminal text:\ndu: cannot read"), "{prompt}");
        let prompt = assist_prompt(&AssistRequest {
            selection: None,
            ..request
        });
        assert!(!prompt.contains("Selected terminal text"), "{prompt}");
    }

    #[test]
    fn assist_prompt_includes_attached_files_within_their_limits() {
        use crate::AttachedFile;
        let file = |path: &str, content: String| AttachedFile {
            path: path.into(),
            content,
        };
        let request = AssistRequest {
            prompt: "why does it fail?".into(),
            os: "linux".into(),
            files: vec![file("/etc/app.conf", "port = 80".into())],
            ..Default::default()
        };
        let prompt = assist_prompt(&request);
        assert!(
            prompt.contains("Attached file /etc/app.conf:\nport = 80\n\nRequest: why does it fail?"),
            "{prompt}"
        );
        assert!(
            !assist_prompt(&AssistRequest {
                files: vec![],
                ..request.clone()
            })
            .contains("Attached file")
        );

        let many: Vec<AttachedFile> = (0..7).map(|i| file(&format!("/f{i}"), "x".into())).collect();
        let prompt = assist_prompt(&AssistRequest {
            files: many,
            ..request.clone()
        });
        assert_eq!(prompt.matches("Attached file").count(), MAX_FILES);

        let big = vec![
            file("/a", "a".repeat(MAX_FILES_TOTAL - 10)),
            file("/b", "b".repeat(50)),
            file("/c", "c".repeat(5)),
        ];
        let prompt = assist_prompt(&AssistRequest { files: big, ..request });
        assert!(
            prompt.contains(&format!("Attached file /b:\n{}\n\nRequest", "b".repeat(10))),
            "the last file gets what is left of the budget"
        );
        assert!(!prompt.contains("Attached file /c"), "the budget is spent");
    }

    #[test]
    fn completion_strips_the_echoed_line() {
        assert_eq!(clean_completion("git comm", "git commit -m \"wip\""), "it -m \"wip\"");
        assert_eq!(clean_completion("ls ", "ls -la"), "-la");
        assert_eq!(clean_completion("  docker ps", "docker ps -a"), " -a");
    }

    #[test]
    fn completion_strips_fences_backticks_quotes_and_prompt_markers() {
        assert_eq!(
            clean_completion("git pu", "```bash\ngit push origin main\n```"),
            "sh origin main"
        );
        assert_eq!(clean_completion("git pu", "`git push`"), "sh");
        assert_eq!(clean_completion("git pu", "\"git push\""), "sh");
        assert_eq!(clean_completion("git pu", "$ git push"), "sh");
    }

    #[test]
    fn completion_keeps_only_the_first_line() {
        assert_eq!(clean_completion("ls", "\nls -la\nls -R\n"), " -la");
    }

    #[test]
    fn completion_is_empty_when_the_reply_does_not_continue_the_line() {
        assert_eq!(clean_completion("ls ", "lsof -i"), "");
        assert_eq!(clean_completion("git", "cargo build"), "");
        assert_eq!(clean_completion("git push", "git push"), "");
        assert_eq!(clean_completion("git", ""), "");
    }

    #[test]
    fn completion_never_carries_control_characters_or_reasoning() {
        assert_eq!(clean_completion("echo", "echo hi\u{1b}[2J\u{7}"), " hi[2J");
        assert_eq!(clean_completion("ls", "<think>user wants ls</think>ls -l"), " -l");
    }

    fn some(answer: &str, command: Option<&str>) -> Option<(String, Option<String>)> {
        Some((answer.into(), command.map(Into::into)))
    }

    #[test]
    fn assist_parses_an_answer_with_a_command_and_one_without() {
        let reply = r#"{"answer": "Sizes, sorted.", "command": "du -sh * | sort -h"}"#;
        assert_eq!(parse_assist(reply), some("Sizes, sorted.", Some("du -sh * | sort -h")));
        let reply = r#"{"answer": "Hola, ¿en qué te ayudo?", "command": null}"#;
        assert_eq!(parse_assist(reply), some("Hola, ¿en qué te ayudo?", None));
        let reply = r#"{"answer": "Hi.", "command": ""}"#;
        assert_eq!(parse_assist(reply), some("Hi.", None));
        let reply = r#"{"answer": "Hi."}"#;
        assert_eq!(parse_assist(reply), some("Hi.", None));
    }

    #[test]
    fn assist_parses_json_wrapped_in_fences_or_text() {
        let fenced = "```json\n{\"answer\": \"Lists all.\", \"command\": \"ls -la\"}\n```";
        assert_eq!(parse_assist(fenced), some("Lists all.", Some("ls -la")));
        let chatty = "Sure! {\"answer\": \"Prints the folder.\", \"command\": \"pwd\"} Hope it helps.";
        assert_eq!(parse_assist(chatty), some("Prints the folder.", Some("pwd")));
    }

    #[test]
    fn assist_reads_the_older_explanation_field_as_the_answer() {
        let reply = r#"{"command": "pwd", "explanation": "Prints the folder."}"#;
        assert_eq!(parse_assist(reply), some("Prints the folder.", Some("pwd")));
    }

    #[test]
    fn a_plain_text_reply_is_the_answer_without_a_command() {
        assert_eq!(
            parse_assist("Hola. Puedo ayudarte con lo que necesites."),
            some("Hola. Puedo ayudarte con lo que necesites.", None)
        );
        let reply = "Use this:\n```sh\nfree -h\n```\nIt shows memory.";
        assert_eq!(parse_assist(reply), some(reply, None));
        let braces = "Run find . -exec rm {} \\; with care.";
        assert_eq!(parse_assist(braces), some(braces, None));
    }

    #[test]
    fn assist_commands_are_one_line_without_control_characters() {
        let reply = r#"{"answer": "x", "command": "cd /tmp\nls \\\n  -la\u001b[A\r"}"#;
        assert_eq!(parse_assist(reply), some("x", Some("cd /tmp && ls -la[A")));
    }

    #[test]
    fn answers_keep_lines_and_tabs_but_drop_other_control_and_format_characters() {
        let reply = "{\"answer\": \"Step 1:\\tcd\\r\\nStep 2\\u001b[2J\\u0007\u{202E}\u{200B}\u{FEFF} done\u{2028}\", \"command\": null}";
        assert_eq!(parse_assist(reply), some("Step 1:\tcd\nStep 2[2J done", None));
        let plain = "a\r\nb\u{1b}]0;title\u{7}c";
        assert_eq!(parse_assist(plain), some("a\nb]0;titlec", None));
    }

    #[test]
    fn long_answers_are_cut_with_a_note() {
        let at_cap = "a".repeat(MAX_ANSWER);
        assert_eq!(parse_assist(&at_cap), some(&at_cap, None));
        let over = format!("{at_cap}bcd");
        let (answer, command) = parse_assist(&over).unwrap();
        assert_eq!(command, None);
        assert!(answer.starts_with(&at_cap), "{}", &answer[MAX_ANSWER - 5..]);
        assert!(answer.ends_with(ANSWER_CUT_NOTE), "{}", &answer[MAX_ANSWER - 5..]);
        assert!(!answer.contains('b'));
    }

    #[test]
    fn assist_rejects_empty_answers() {
        assert_eq!(parse_assist(""), None);
        assert_eq!(parse_assist("  \n "), None);
        assert_eq!(parse_assist(r#"{"answer": "", "command": null}"#), None);
        assert_eq!(parse_assist(r#"{"answer": "\u001b", "command": "\u0007"}"#), None);
    }

    #[test]
    fn a_command_without_an_answer_is_kept() {
        assert_eq!(parse_assist(r#"{"answer": "", "command": "ls"}"#), some("", Some("ls")));
    }

    #[test]
    fn error_detail_reads_the_provider_message_and_redacts_the_key() {
        let body =
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad key sk-secret-123 here"}}"#;
        assert_eq!(
            error_detail(body, Some("sk-secret-123")).as_deref(),
            Some("bad key *** here")
        );
        assert_eq!(error_detail("<html>oops</html>", None), None);
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(500));
        assert_eq!(error_detail(&long, None).unwrap().chars().count(), 201);
    }

    #[test]
    fn completions_drop_format_and_separator_characters() {
        let reply = "git status\u{202E}\u{200B} --short\u{2028}\u{2029}\u{FEFF}";
        assert_eq!(clean_completion("git st", reply), "atus --short");
    }

    #[test]
    fn assist_drops_format_and_separator_characters_from_command_and_answer() {
        let reply = "{\"command\": \"rm\u{202E}\u{2066} -i a\u{2028}b\", \"answer\": \"Asks\u{200F} first.\u{2029}\\nOk\u{00AD}.\"}";
        assert_eq!(parse_assist(reply), some("Asks first.\nOk.", Some("rm -i ab")));
    }

    #[test]
    fn commands_longer_than_the_cap_are_dropped_and_the_answer_kept() {
        let at_cap = "a".repeat(MAX_COMMAND);
        let reply = format!("{{\"answer\": \"x\", \"command\": \"{at_cap}\"}}");
        assert_eq!(parse_assist(&reply), some("x", Some(&at_cap)));
        let over = format!("{{\"answer\": \"x\", \"command\": \"{at_cap}b\"}}");
        assert_eq!(parse_assist(&over), some("x", None));
        let only = format!("{{\"answer\": \"\", \"command\": \"{at_cap}b\"}}");
        assert_eq!(parse_assist(&only), None);
    }

    fn turn(question: &str, answer: &str, command: Option<&str>) -> Turn {
        Turn {
            question: question.into(),
            selection: None,
            answer: answer.into(),
            command: command.map(Into::into),
        }
    }

    #[test]
    fn turns_are_replayed_as_the_question_and_the_json_answer() {
        let mut first = turn(
            "what is using space?",
            "Sizes, smallest first.",
            Some("du -sh * | sort -h"),
        );
        first.selection = Some("df: 99% full".into());
        let got = conversation(&[first, turn("and in /var?", "Same there.", None)]);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0].user,
            "Selected terminal text:\ndf: 99% full\nRequest: what is using space?"
        );
        let answer: serde_json::Value = serde_json::from_str(&got[0].assistant).unwrap();
        assert_eq!(
            answer,
            serde_json::json!({"answer": "Sizes, smallest first.", "command": "du -sh * | sort -h"})
        );
        assert_eq!(got[1].user, "Request: and in /var?");
        let answer: serde_json::Value = serde_json::from_str(&got[1].assistant).unwrap();
        assert_eq!(answer, serde_json::json!({"answer": "Same there.", "command": null}));
    }

    #[test]
    fn replayed_turns_lose_control_characters_but_keep_line_breaks() {
        let got = conversation(&[turn(
            "fix\u{1b}[31m this\nplease\u{202E}",
            "Step 1\n\tStep 2\u{7}\u{2028}",
            Some("ls\r\nrm -i a"),
        )]);
        assert_eq!(got[0].user, "Request: fix[31m this\nplease");
        let answer: serde_json::Value = serde_json::from_str(&got[0].assistant).unwrap();
        assert_eq!(answer["answer"], "Step 1\n\tStep 2");
        assert_eq!(answer["command"], "ls && rm -i a");
    }

    #[test]
    fn only_the_last_ten_turns_are_replayed() {
        let turns: Vec<Turn> = (1..=12)
            .map(|i| turn(&format!("q{i}"), &format!("a{i}"), None))
            .collect();
        let got = conversation(&turns);
        assert_eq!(got.len(), MAX_TURNS);
        assert_eq!(got[0].user, "Request: q3");
        assert_eq!(got[9].user, "Request: q12");
    }

    #[test]
    fn the_oldest_turns_go_until_the_conversation_fits_16000_characters() {
        let long = "x".repeat(3_000);
        let turns: Vec<Turn> = (1..=6).map(|i| turn(&format!("q{i}"), &long, None)).collect();
        let got = conversation(&turns);
        let total: usize = got
            .iter()
            .map(|e| e.user.chars().count() + e.assistant.chars().count())
            .sum();
        assert!(total <= MAX_CONVERSATION, "{total}");
        assert_eq!(got.len(), 5);
        assert_eq!(got[0].user, "Request: q2");
        assert_eq!(got[4].user, "Request: q6");
        // A single turn over the budget is not replayed at all.
        let huge = turn("q", &"y".repeat(MAX_ANSWER), Some(&"z".repeat(MAX_COMMAND)));
        let mut big = huge.clone();
        big.question = "w".repeat(MAX_PROMPT);
        big.selection = Some("s".repeat(MAX_SELECTION));
        assert!(conversation(&[big]).is_empty());
        assert_eq!(conversation(&[huge]).len(), 1);
    }

    #[test]
    fn empty_questions_are_not_replayed() {
        assert!(conversation(&[turn("  ", "a", None)]).is_empty());
    }
}
