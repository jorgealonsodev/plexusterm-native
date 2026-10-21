//! Ported from PlexusTerm v0.2.15 `src/features/settings/settings.test.ts` (PRD SET-1, SET-2, KEY-5).
#![recursion_limit = "256"]

use serde_json::{Value, json};
use settings_model::settings::{
    AiProvider, DEFAULT_FONT_FAMILY, FONT_WEIGHTS, LINE_HEIGHTS, Settings, StartupState, ViewerPosition,
    clean_share_url, step_chat_font_size, step_font_size,
};
use settings_model::shortcuts::{FontStep, ShortcutAction};

#[allow(clippy::unwrap_used, reason = "test helper")]
fn as_json(settings: &Settings) -> Value {
    serde_json::from_str(&settings.to_json()).unwrap()
}
fn parse(json: &str) -> Settings {
    Settings::parse(Some(json))
}
fn parse_value(value: &Value) -> Settings {
    Settings::parse(Some(&value.to_string()))
}
fn defaults() -> Settings {
    Settings::default()
}
fn shortcut_entries(settings: &Settings) -> Vec<(&'static str, String)> {
    settings.shortcuts.iter().map(|(a, s)| (a.id(), s.to_owned())).collect()
}
fn entries(list: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    list.iter().map(|(a, s)| (*a, (*s).to_owned())).collect()
}

fn default_json() -> Value {
    json!({
        "version": 2,
        "fontSize": 14,
        "copyOnSelect": true,
        "rightClickPastes": true,
        "confirmMultilinePaste": true,
        "errorHighlight": true,
        "theme": "catppuccin-mocha",
        "fontFamily": DEFAULT_FONT_FAMILY,
        "remoteMonitor": true,
        "textEditor": "",
        "inlineSuggestions": true,
        "commandHistory": true,
        "aiSuggestions": false,
        "aiProvider": "anthropic",
        "aiBaseUrl": "",
        "aiModel": "claude-opus-5-5",
        "aiHistoryContext": 5,
        "assistantShortcut": "Ctrl+Shift+Space",
        "sidebarCollapsed": false,
        "sidebarHideWhenCollapsed": false,
        "localShell": "",
        "localShellArgs": "",
        "localCwd": "",
        "sidebarWidth": null,
        "checkUpdates": true,
        "aiSaveChats": true,
        "fileDetails": false,
        "terminalFollowsBrowser": true,
        "sidebarAtStartup": "last",
        "language": "system",
        "shortcuts": {},
        "shiftEnterNewline": true,
        "attentionNotifications": true,
        "gpuAcceleration": true,
        "gitFileStatus": true,
        "viewerAutoSave": false,
        "viewerPosition": "left",
        "viewerChangedLines": true,
        "fontWeight": 400,
        "lineHeight": 1,
        "uiFontSize": 13,
        "chatFontSize": 13,
        "viewerFontSize": 13,
        "devCommands": ["claude", "codex", "agy", "opencode", "pi", "gentle-shell"],
        "tabColumnWidth": null,
        "devColumnCollapsed": false,
        "devColumnHideWhenCollapsed": false,
        "devColumnAtStartup": "last",
        "recentFolderLimit": 10,
        "devFolderInFooter": true,
        "shareRelayUrl": "",
        "shareViewerOrigin": "",
        "shareViewNeedsApproval": true,
        "shareSudoReset": true,
        "shareBlockLid": false,
        "shareRecordInput": false,
    })
}

/// `default_json()` with `changes` applied.
fn default_json_with(changes: Value) -> Value {
    let mut value = default_json();
    if let (Some(target), Some(changes)) = (value.as_object_mut(), changes.as_object()) {
        for (k, v) in changes {
            target.insert(k.clone(), v.clone());
        }
    }
    value
}

// describe("parseSettings")

// it("drops OpenCode's own opencode-go/ prefix from OpenCode Go model ids")
#[test]
fn drops_opencodes_own_opencode_go_prefix_from_opencode_go_model_ids() {
    let s = parse_value(&json!({ "aiProvider": "opencode-go", "aiModel": "opencode-go/glm-5.3-flash" }));
    assert_eq!(s.ai_model, "glm-5.3-flash");
    // Other providers keep slashes (e.g. OpenRouter ids).
    let s = parse_value(&json!({ "aiProvider": "openai-compatible", "aiModel": "meta/llama-4" }));
    assert_eq!(s.ai_model, "meta/llama-4");
}

// it("returns the defaults when there is no settings file")
#[test]
fn returns_the_defaults_when_there_is_no_settings_file() {
    let s = Settings::parse(None);
    assert_eq!(s, defaults());
    let written = as_json(&s);
    assert_eq!(written, default_json());
    // SET-1: 55 keys plus "version".
    assert_eq!(written.as_object().map(serde_json::Map::len), Some(56));
    assert_eq!(defaults().font_size, 14);
}

// it("opens the sidebar as Settings choose at startup, or as it was left (L17)")
#[test]
fn opens_the_sidebar_as_settings_choose_at_startup_or_as_it_was_left() {
    let at = |startup: &str, collapsed: bool| {
        parse_value(&json!({ "sidebarAtStartup": startup, "sidebarCollapsed": collapsed })).sidebar_collapsed
    };
    assert!(at("last", true));
    assert!(!at("last", false));
    assert!(!at("expanded", true));
    assert!(at("collapsed", false));
    assert_eq!(
        parse(r#"{"sidebarAtStartup":"sideways"}"#).sidebar_at_startup,
        StartupState::Last
    );
}

// it("opens the DEV tab column as Settings choose at startup, or as it was left")
#[test]
fn opens_the_dev_tab_column_as_settings_choose_at_startup_or_as_it_was_left() {
    let at = |startup: &str, collapsed: bool| {
        parse_value(&json!({ "devColumnAtStartup": startup, "devColumnCollapsed": collapsed })).dev_column_collapsed
    };
    assert!(at("last", true));
    assert!(!at("last", false));
    assert!(!at("expanded", true));
    assert!(at("collapsed", false));
    assert_eq!(
        parse(r#"{"devColumnAtStartup":"sideways"}"#).dev_column_at_startup,
        StartupState::Last
    );
    assert!(parse(r#"{"devColumnHideWhenCollapsed":true}"#).dev_column_hide_when_collapsed);
}

// it("keeps between 1 and 50 recent DEV mode folders, 10 by default")
#[test]
fn keeps_between_1_and_50_recent_dev_mode_folders_10_by_default() {
    let of = |v: Value| parse_value(&json!({ "recentFolderLimit": v })).recent_folder_limit;
    assert_eq!(of(json!(20)), 20);
    assert_eq!(of(json!(0)), 1);
    assert_eq!(of(json!(99)), 50);
    assert_eq!(of(json!(7.6)), 8);
    assert_eq!(of(json!("many")), 10);
    assert!(!parse(r#"{"devFolderInFooter":false}"#).dev_folder_in_footer);
}

// it("reads where the file viewer opens, the left when unknown")
#[test]
fn reads_where_the_file_viewer_opens_the_left_when_unknown() {
    let of = |v: &str| parse_value(&json!({ "viewerPosition": v })).viewer_position;
    assert_eq!(of("right"), ViewerPosition::Right);
    assert_eq!(of("below"), ViewerPosition::Below);
    assert_eq!(of("above"), ViewerPosition::Left);
    assert_eq!(Settings::parse(None).viewer_position, ViewerPosition::Left);
}

// it("returns the defaults for a file that is not a JSON object")
#[test]
fn returns_the_defaults_for_a_file_that_is_not_a_json_object() {
    assert_eq!(parse("{not json"), defaults());
    assert_eq!(parse("[1,2]"), defaults());
    assert_eq!(parse("null"), defaults());
    assert_eq!(parse(""), defaults());
}

// it("reads saved values")
#[test]
fn reads_saved_values() {
    let file = concat!(
        r#"{"version":1,"fontSize":18,"copyOnSelect":false,"rightClickPastes":false,"confirmMultilinePaste":false,"errorHighlight":false,"theme":"nord","fontFamily":"Fira Code","remoteMonitor":false,"textEditor":"code --wait","#,
        r#""inlineSuggestions":false,"commandHistory":false,"aiSuggestions":true,"aiProvider":"openai-compatible","#,
        r#""aiBaseUrl":"http://localhost:11434/v1","aiModel":"llama3.1","aiHistoryContext":0,"assistantShortcut":"Ctrl+Alt+K","#,
        r#""sidebarCollapsed":true,"sidebarHideWhenCollapsed":true,"sidebarWidth":320,"localShell":" /usr/bin/fish ","localShellArgs":" --login ","localCwd":" ~/projects ","checkUpdates":false,"aiSaveChats":false,"fileDetails":true,"terminalFollowsBrowser":false,"sidebarAtStartup":"collapsed","language":"ja"}"#
    );
    let s = parse(file);
    assert_eq!(s.ai_provider, AiProvider::OpenAiCompatible);
    assert_eq!(
        as_json(&s),
        default_json_with(json!({
            "fontSize": 18,
            "copyOnSelect": false,
            "rightClickPastes": false,
            "confirmMultilinePaste": false,
            "errorHighlight": false,
            "theme": "nord",
            "fontFamily": "Fira Code",
            "remoteMonitor": false,
            "textEditor": "code --wait",
            "inlineSuggestions": false,
            "commandHistory": false,
            "aiSuggestions": true,
            "aiProvider": "openai-compatible",
            "aiBaseUrl": "http://localhost:11434/v1",
            "aiModel": "llama3.1",
            "aiHistoryContext": 0,
            "assistantShortcut": "Ctrl+Alt+K",
            "sidebarCollapsed": true,
            "sidebarHideWhenCollapsed": true,
            "localShell": "/usr/bin/fish",
            "localShellArgs": "--login",
            "localCwd": "~/projects",
            "sidebarWidth": 320,
            "checkUpdates": false,
            "aiSaveChats": false,
            "fileDetails": true,
            "terminalFollowsBrowser": false,
            "sidebarAtStartup": "collapsed",
            "language": "ja",
            // A file from before customisable shortcuts loads with the defaults; so do the
            // settings added for AI command-line tools (every other key keeps its default).
        }))
    );
}

// it("clamps the font size to 8..32 and rounds it")
#[test]
fn clamps_the_font_size_to_8_to_32_and_rounds_it() {
    assert_eq!(parse(r#"{"fontSize":2}"#).font_size, 8);
    assert_eq!(parse(r#"{"fontSize":99}"#).font_size, 32);
    assert_eq!(parse(r#"{"fontSize":15.6}"#).font_size, 16);
    // JavaScript's Math.round: halves go up.
    assert_eq!(parse(r#"{"fontSize":14.5}"#).font_size, 15);
}

// it("defaults invalid values and ignores unknown keys")
// Native: unknown keys are kept and written back unchanged (PRD §4.1 deviation 2).
#[test]
fn defaults_invalid_values_and_keeps_unknown_keys_apart() {
    let parsed = parse(r#"{"fontSize":"big","copyOnSelect":"yes","rightClickPastes":0,"colour":"dark"}"#);
    assert_eq!(parsed.without_unknown_keys(), defaults());
    assert_eq!(parsed.unknown_keys().collect::<Vec<_>>(), ["colour"]);
    assert_eq!(as_json(&parsed), default_json_with(json!({ "colour": "dark" })));
    assert_eq!(parse(r#"{"fontSize":null}"#).font_size, 14);
    // 1e999 is Infinity in JavaScript; serde_json refuses it, so the whole file reads as defaults.
    assert_eq!(parse(r#"{"fontSize":1e999}"#).font_size, 14);
}

// Native (§4.1 deviation 2): unknown keys survive a round trip in their order, nested values included.
#[test]
fn writes_unknown_keys_back_unchanged() {
    let file = r#"{"zeta":{"b":1,"a":[true,null,"x"]},"fontSize":20,"alpha":1.5,"version":2}"#;
    let s = parse(file);
    assert_eq!(s.unknown_keys().collect::<Vec<_>>(), ["zeta", "alpha"]);
    let written = s.to_json();
    assert_eq!(
        as_json(&s),
        default_json_with(json!({ "fontSize": 20, "zeta": {"b":1,"a":[true,null,"x"]}, "alpha": 1.5 }))
    );
    // Nested order is kept as written: "b" before "a".
    assert!(written.find("\"b\"") < written.find("\"a\""));
    assert_eq!(parse(&written), s);
}

// it("uses the default theme for unknown or invalid theme ids")
#[test]
fn uses_the_default_theme_for_unknown_or_invalid_theme_ids() {
    assert_eq!(parse(r#"{"theme":"dracula-pro"}"#).theme, "catppuccin-mocha");
    assert_eq!(parse(r#"{"theme":7}"#).theme, "catppuccin-mocha");
}

// it("uses the default font for an empty, overlong or invalid font family and trims it")
#[test]
fn uses_the_default_font_for_an_empty_overlong_or_invalid_font_family_and_trims_it() {
    assert_eq!(parse(r#"{"fontFamily":"   "}"#).font_family, DEFAULT_FONT_FAMILY);
    assert_eq!(
        parse_value(&json!({ "fontFamily": "x".repeat(201) })).font_family,
        DEFAULT_FONT_FAMILY
    );
    assert_eq!(
        parse_value(&json!({ "fontFamily": "x".repeat(200) })).font_family,
        "x".repeat(200)
    );
    assert_eq!(parse(r#"{"fontFamily":false}"#).font_family, DEFAULT_FONT_FAMILY);
    assert_eq!(
        parse(r#"{"fontFamily":"  Hack Nerd Font  "}"#).font_family,
        "Hack Nerd Font"
    );
}

// it("uses the default for an invalid remote monitor value")
#[test]
fn uses_the_default_for_an_invalid_remote_monitor_value() {
    assert!(parse(r#"{"remoteMonitor":"off"}"#).remote_monitor);
    assert!(!parse(r#"{"remoteMonitor":false}"#).remote_monitor);
}

// it("uses the system default text editor for an invalid or overlong value and trims it")
#[test]
fn uses_the_system_default_text_editor_for_an_invalid_or_overlong_value_and_trims_it() {
    assert_eq!(parse(r#"{"textEditor":7}"#).text_editor, "");
    assert_eq!(parse(r#"{"textEditor":"   "}"#).text_editor, "");
    assert_eq!(parse_value(&json!({ "textEditor": "x".repeat(1025) })).text_editor, "");
    assert_eq!(
        parse_value(&json!({ "textEditor": "x".repeat(1024) })).text_editor,
        "x".repeat(1024)
    );
    assert_eq!(
        parse(r#"{"textEditor":"  /usr/bin/kate  "}"#).text_editor,
        "/usr/bin/kate"
    );
}

// it("defaults invalid suggestion and AI switches")
#[test]
fn defaults_invalid_suggestion_and_ai_switches() {
    let s = parse(r#"{"inlineSuggestions":"no","commandHistory":1,"aiSuggestions":"yes"}"#);
    assert_eq!(
        [s.inline_suggestions, s.command_history, s.ai_suggestions],
        [true, true, false]
    );
}

// it("knows Anthropic, Gemini, OpenAI and OpenAI-compatible providers, each with its default model")
#[test]
fn knows_anthropic_gemini_openai_and_openai_compatible_providers_each_with_its_default_model() {
    let s = parse(r#"{"aiProvider":"gemini"}"#);
    assert_eq!(
        (s.ai_provider, s.ai_model.as_str()),
        (AiProvider::Gemini, "gemini-3.5-flash-lite")
    );
    let s = parse(r#"{"aiProvider":"openai"}"#);
    assert_eq!((s.ai_provider, s.ai_model.as_str()), (AiProvider::OpenAi, ""));
    let s = parse(r#"{"aiProvider":"opencode"}"#);
    assert_eq!(
        (s.ai_provider, s.ai_model.as_str()),
        (AiProvider::Anthropic, "claude-opus-5-5")
    );
    assert_eq!(
        parse(r#"{"aiProvider":"gemini","aiModel":"  gemini-2.5-pro "}"#).ai_model,
        "gemini-2.5-pro"
    );
    assert_eq!(
        parse_value(&json!({ "aiModel": "x".repeat(201) })).ai_model,
        "claude-opus-5-5"
    );
    assert_eq!(parse(r#"{"aiProvider":"openai","aiModel":7}"#).ai_model, "");
}

// it("knows OpenCode Go, whose model is left for the user to pick")
#[test]
fn knows_opencode_go_whose_model_is_left_for_the_user_to_pick() {
    let s = parse(r#"{"aiProvider":"opencode-go"}"#);
    assert_eq!((s.ai_provider, s.ai_model.as_str()), (AiProvider::OpenCodeGo, ""));
    assert_eq!(
        parse(r#"{"aiProvider":"opencode-go","aiModel":" kimi-k3 "}"#).ai_model,
        "kimi-k3"
    );
    assert_eq!(
        parse(r#"{"aiProvider":"opencode-go","aiModel":"OpenCode-Go/kimi-k3"}"#).ai_model,
        "kimi-k3"
    );
}

// it("accepts only http(s) base URLs")
#[test]
fn accepts_only_http_s_base_urls() {
    assert_eq!(
        parse(r#"{"aiBaseUrl":" https://openrouter.ai/api/v1 "}"#).ai_base_url,
        "https://openrouter.ai/api/v1"
    );
    assert_eq!(
        parse(r#"{"aiBaseUrl":"HTTP://localhost:11434/v1"}"#).ai_base_url,
        "HTTP://localhost:11434/v1"
    );
    assert_eq!(parse(r#"{"aiBaseUrl":"file:///etc/passwd"}"#).ai_base_url, "");
    assert_eq!(parse(r#"{"aiBaseUrl":"https://a b"}"#).ai_base_url, "");
    assert_eq!(parse(r#"{"aiBaseUrl":"https://"}"#).ai_base_url, "");
    assert_eq!(parse(r#"{"aiBaseUrl":3}"#).ai_base_url, "");
}

// it("keeps 0..20 history commands as AI context, rounding and clamping")
#[test]
fn keeps_0_to_20_history_commands_as_ai_context_rounding_and_clamping() {
    assert_eq!(parse(r#"{"aiHistoryContext":25}"#).ai_history_context, 20);
    assert_eq!(parse(r#"{"aiHistoryContext":-3}"#).ai_history_context, 0);
    assert_eq!(parse(r#"{"aiHistoryContext":2.6}"#).ai_history_context, 3);
    assert_eq!(parse(r#"{"aiHistoryContext":"many"}"#).ai_history_context, 5);
}

// it("uses the default assistant shortcut for an invalid one")
#[test]
fn uses_the_default_assistant_shortcut_for_an_invalid_one() {
    assert_eq!(
        parse(r#"{"assistantShortcut":"Space"}"#).assistant_shortcut,
        "Ctrl+Shift+Space"
    );
    assert_eq!(
        parse(r#"{"assistantShortcut":"Ctrl+Shift+T"}"#).assistant_shortcut,
        "Ctrl+Shift+Space"
    );
    assert_eq!(
        parse(r#"{"assistantShortcut":42}"#).assistant_shortcut,
        "Ctrl+Shift+Space"
    );
    assert_eq!(
        parse(r#"{"assistantShortcut":"Alt+Shift+A"}"#).assistant_shortcut,
        "Alt+Shift+A"
    );
    assert_eq!(
        parse(r#"{"assistantShortcut":"Ctrl+Alt+5"}"#).assistant_shortcut,
        "Ctrl+Shift+Space"
    );
}

// it("keeps only usable shortcut overrides")
#[test]
fn keeps_only_usable_shortcut_overrides() {
    // Raw JSON text: `json!` would sort the keys, and their order matters here.
    let shortcuts = |value: &str, extra: &str| shortcut_entries(&parse(&format!(r#"{{"shortcuts":{value}{extra}}}"#)));
    assert_eq!(
        shortcuts(
            r#"{"newTerminal":"Ctrl+Alt+N","toggleSidebar":"","keyGen":"Alt+K"}"#,
            ""
        ),
        entries(&[
            ("newTerminal", "Ctrl+Alt+N"),
            ("toggleSidebar", ""),
            ("keyGen", "Alt+K")
        ])
    );
    // Unknown actions, invalid shortcuts and values that are not text.
    assert_eq!(
        shortcuts(
            r#"{"launchRocket":"Alt+R","closeTab":"Shift+A","nextTab":3,"previousTab":"Ctrl+"}"#,
            ""
        ),
        entries(&[])
    );
    // The fixed keys and the assistant shortcut.
    assert_eq!(
        shortcuts(
            r#"{"closeTab":"Ctrl+Alt+2","nextTab":"Ctrl+ArrowRight","previousTab":"Ctrl+Shift+Space"}"#,
            ""
        ),
        entries(&[])
    );
    assert_eq!(
        shortcuts(r#"{"previousTab":"Alt+A"}"#, r#","assistantShortcut":"Alt+A""#),
        entries(&[])
    );
    // Taken by another action's default, or by another override.
    assert_eq!(shortcuts(r#"{"closeTab":"Ctrl+Shift+T"}"#, ""), entries(&[]));
    assert_eq!(
        shortcuts(r#"{"closeTab":"Alt+X","nextTab":"Alt+X"}"#, ""),
        entries(&[("closeTab", "Alt+X")])
    );
    // Freed by another override: a swap is kept.
    assert_eq!(
        shortcuts(r#"{"newTerminal":"Ctrl+Shift+S","quickConnect":"Ctrl+Shift+T"}"#, ""),
        entries(&[("newTerminal", "Ctrl+Shift+S"), ("quickConnect", "Ctrl+Shift+T")])
    );
    assert_eq!(shortcuts(r#""Ctrl+Shift+T""#, ""), entries(&[]));
    assert_eq!(shortcuts(r#"["Alt+X"]"#, ""), entries(&[]));
}

// Native: "the later clash dropped" depends on the order written in the file, which serde_json's
// sorted maps would lose: "nextTab" is written before "closeTab" here, so "closeTab" is dropped.
#[test]
fn drops_the_later_of_two_clashing_overrides_in_file_order() {
    let s = parse(r#"{"shortcuts":{"nextTab":"Alt+X","closeTab":"Alt+X"}}"#);
    assert_eq!(shortcut_entries(&s), entries(&[("nextTab", "Alt+X")]));
    assert!(s.to_json().contains("\"nextTab\": \"Alt+X\""));
}

// it("lets the assistant take a combination an override freed")
#[test]
fn lets_the_assistant_take_a_combination_an_override_freed() {
    let s = parse_value(&json!({ "shortcuts": { "newTerminal": "Alt+N" }, "assistantShortcut": "Ctrl+Shift+T" }));
    assert_eq!(s.assistant_shortcut, "Ctrl+Shift+T");
    assert_eq!(shortcut_entries(&s), entries(&[("newTerminal", "Alt+N")]));
}

// Native: an override that takes the assistant's combination drops instead of the assistant.
#[test]
fn an_override_that_takes_the_assistant_combination_is_dropped() {
    let s = parse_value(&json!({ "shortcuts": { "keyGen": "Ctrl+Shift+Space" } }));
    assert_eq!(s.assistant_shortcut, "Ctrl+Shift+Space");
    assert_eq!(shortcut_entries(&s), entries(&[]));
}

// it("keeps valid values next to invalid ones")
#[test]
fn keeps_valid_values_next_to_invalid_ones() {
    let expected = Settings {
        font_size: 20,
        ..defaults()
    };
    assert_eq!(parse(r#"{"fontSize":20,"copyOnSelect":"no"}"#), expected);
}

// describe("the removed gitBranch setting")
// it("is ignored in older settings files: the branch only depends on DEV mode")
// Native: it is not a setting any more, so it is kept as an unknown key (§4.1 deviation 2).
#[test]
fn the_removed_git_branch_setting_is_ignored_in_older_settings_files() {
    let s = parse(r#"{"version":1,"gitBranch":false,"fontSize":15}"#);
    assert_eq!(s.font_size, 15);
    assert_eq!(s.unknown_keys().collect::<Vec<_>>(), ["gitBranch"]);
    assert!(!default_json().as_object().is_some_and(|o| o.contains_key("gitBranch")));
}

// describe("serializeSettings") it("round-trips through parseSettings with a version")
#[test]
fn serialize_round_trips_through_parse_with_a_version() {
    let mut s = Settings {
        font_size: 11,
        copy_on_select: false,
        right_click_pastes: true,
        confirm_multiline_paste: false,
        theme: "gruvbox-dark".into(),
        font_family: "Iosevka".into(),
        remote_monitor: false,
        text_editor: "/opt/My Editor/editor".into(),
        inline_suggestions: false,
        command_history: true,
        ai_suggestions: true,
        ai_provider: AiProvider::Gemini,
        ai_base_url: String::new(),
        ai_model: "gemini-2.5-pro".into(),
        ai_history_context: 12,
        assistant_shortcut: "Ctrl+Alt+Space".into(),
        sidebar_collapsed: true,
        sidebar_hide_when_collapsed: true,
        local_shell: "C:\\Program Files\\Git\\bin\\bash.exe".into(),
        local_shell_args: "--login -i".into(),
        local_cwd: "~/work".into(),
        ai_save_chats: false,
        file_details: true,
        terminal_follows_browser: false,
        sidebar_at_startup: StartupState::Last,
        sidebar_width: Some(480),
        check_updates: false,
        error_highlight: false,
        language: "pt-BR".into(),
        shift_enter_newline: false,
        attention_notifications: false,
        gpu_acceleration: false,
        git_file_status: false,
        viewer_auto_save: true,
        viewer_position: ViewerPosition::Left,
        viewer_changed_lines: false,
        font_weight: 500,
        line_height: 1.0,
        ui_font_size: 15,
        chat_font_size: 17,
        viewer_font_size: 13,
        dev_commands: vec!["aider".into()],
        tab_column_width: Some(300),
        dev_column_collapsed: true,
        dev_column_hide_when_collapsed: true,
        dev_column_at_startup: StartupState::Last,
        recent_folder_limit: 25,
        dev_folder_in_footer: false,
        share_relay_url: "wss://relay.example.com".into(),
        share_viewer_origin: "https://view.example.com".into(),
        share_view_needs_approval: false,
        share_sudo_reset: false,
        share_block_lid: true,
        share_record_input: true,
        ..defaults()
    };
    s.shortcuts.set(ShortcutAction::NewTerminal, "Ctrl+Alt+N");
    s.shortcuts.set(ShortcutAction::ToggleSidebar, "");
    s.shortcuts.set(ShortcutAction::NextTab, "Ctrl+PageDown");
    let written = s.to_json();
    let value = as_json(&s);
    assert_eq!(value.get("version"), Some(&json!(2)));
    assert_eq!(value.get("lineHeight"), Some(&json!(1)));
    assert_eq!(
        value.get("shortcuts"),
        Some(&json!({ "newTerminal": "Ctrl+Alt+N", "toggleSidebar": "", "nextTab": "Ctrl+PageDown" }))
    );
    assert!(written.starts_with("{\n  \"version\": 2,\n  \"fontSize\": 11,"));
    assert!(written.contains("\"lineHeight\": 1,"));
    assert_eq!(parse(&written), s);
}

// describe("stepFontSize") it("grows, shrinks and resets the font size within 8..32")
#[test]
fn grows_shrinks_and_resets_the_font_size_within_8_to_32() {
    assert_eq!(step_font_size(14, FontStep::Increase), 15);
    assert_eq!(step_font_size(14, FontStep::Decrease), 13);
    assert_eq!(step_font_size(20, FontStep::Reset), 14);
    assert_eq!(step_font_size(32, FontStep::Increase), 32);
    assert_eq!(step_font_size(8, FontStep::Decrease), 8);
}

// describe("sidebar settings")
// it("default to an open sidebar that folds to icons, and keep saved booleans only")
#[test]
fn sidebar_defaults_to_open_folding_to_icons_and_keeps_saved_booleans_only() {
    assert!(!defaults().sidebar_collapsed);
    assert!(!defaults().sidebar_hide_when_collapsed);
    let s = parse(r#"{"sidebarCollapsed":true,"sidebarHideWhenCollapsed":"yes"}"#);
    assert!(s.sidebar_collapsed);
    assert!(!s.sidebar_hide_when_collapsed);
}

// it("keeps a dragged width within 160..640 px and forgets anything else")
#[test]
fn keeps_a_dragged_width_within_160_to_640_px_and_forgets_anything_else() {
    assert_eq!(defaults().sidebar_width, None);
    let of = |v: Value| parse_value(&json!({ "sidebarWidth": v })).sidebar_width;
    assert_eq!(of(json!(300)), Some(300));
    assert_eq!(of(json!(300.6)), Some(301));
    assert_eq!(of(json!(20)), Some(160));
    assert_eq!(of(json!(5000)), Some(640));
    for bad in [json!("300"), Value::Null, json!(true)] {
        assert_eq!(of(bad), None);
    }
}

// describe("local shell settings")
// it("default to the system shell and keep only text of at most 1024 characters")
#[test]
fn local_shell_defaults_to_the_system_shell_and_keeps_only_text_of_at_most_1024_characters() {
    assert_eq!(defaults().local_shell, "");
    assert_eq!(defaults().local_shell_args, "");
    let s = parse_value(&json!({ "localShell": 7, "localShellArgs": "x".repeat(1025) }));
    assert_eq!(s.local_shell, "");
    assert_eq!(s.local_shell_args, "");
}

// describe("update check setting") it("is on by default and keeps only a saved boolean")
#[test]
fn update_check_is_on_by_default_and_keeps_only_a_saved_boolean() {
    assert!(defaults().check_updates);
    assert!(!parse(r#"{"checkUpdates":false}"#).check_updates);
    assert!(parse(r#"{"checkUpdates":"no"}"#).check_updates);
}

// describe("language setting") it("defaults to the system language and keeps only supported values")
#[test]
fn language_defaults_to_the_system_language_and_keeps_only_supported_values() {
    assert_eq!(parse(r#"{"language":"zh-CN"}"#).language, "zh-CN");
    assert_eq!(parse(r#"{"language":"klingon"}"#).language, "system");
}

// describe("DEV mode commands setting (S10)")
// it("lists the known AI CLIs by default and keeps an edited list, even an empty one")
#[test]
fn lists_the_known_ai_clis_by_default_and_keeps_an_edited_list_even_an_empty_one() {
    assert_eq!(
        Settings::parse(None).dev_commands,
        ["claude", "codex", "agy", "opencode", "pi", "gentle-shell"]
    );
    assert_eq!(
        parse(r#"{"devCommands":["aider"," claude ","bad name"]}"#).dev_commands,
        ["aider", "claude"]
    );
    assert!(parse(r#"{"devCommands":[]}"#).dev_commands.is_empty());
    assert_eq!(
        parse(r#"{"devCommands":"claude"}"#).dev_commands,
        defaults().dev_commands
    );
    // No duplicates, no commas, at most 64 characters, at most 50 names.
    assert_eq!(parse(r#"{"devCommands":["pi","pi","a,b",7]}"#).dev_commands, ["pi"]);
    let long: Vec<String> = (0..60).map(|i| format!("cli{i}")).collect();
    assert_eq!(parse_value(&json!({ "devCommands": long })).dev_commands.len(), 50);
    assert!(
        parse_value(&json!({ "devCommands": ["x".repeat(65)] }))
            .dev_commands
            .is_empty()
    );
}

// describe("DEV tab column width (S13)") it("is the default width until dragged, kept within 140..480 px")
#[test]
fn dev_tab_column_is_the_default_width_until_dragged_kept_within_140_to_480_px() {
    assert_eq!(Settings::parse(None).tab_column_width, None);
    assert_eq!(parse(r#"{"tabColumnWidth":300.4}"#).tab_column_width, Some(300));
    assert_eq!(parse(r#"{"tabColumnWidth":10}"#).tab_column_width, Some(140));
    assert_eq!(parse(r#"{"tabColumnWidth":9999}"#).tab_column_width, Some(480));
    assert_eq!(parse(r#"{"tabColumnWidth":"wide"}"#).tab_column_width, None);
}

// describe("interface and chat text sizes")
// it("give the file viewer its own text size, 13 px by default, between 10 and 24")
#[test]
fn give_the_file_viewer_its_own_text_size_13_px_by_default_between_10_and_24() {
    assert_eq!(defaults().viewer_font_size, 13);
    assert_eq!(parse(r#"{"viewerFontSize":99}"#).viewer_font_size, 24);
    assert_eq!(parse(r#"{"viewerFontSize":1}"#).viewer_font_size, 10);
    assert_eq!(parse(r#"{"viewerFontSize":16.4}"#).viewer_font_size, 16);
    assert_eq!(parse(r#"{"viewerFontSize":"big"}"#).viewer_font_size, 13);
}

// it("default to 13 px and stay within their ranges")
#[test]
fn interface_and_chat_text_sizes_default_to_13_px_and_stay_within_their_ranges() {
    assert_eq!((defaults().ui_font_size, defaults().chat_font_size), (13, 13));
    let s = parse(r#"{"uiFontSize":99,"chatFontSize":1}"#);
    assert_eq!((s.ui_font_size, s.chat_font_size), (20, 10));
    let s = parse(r#"{"uiFontSize":"big","chatFontSize":15.6}"#);
    assert_eq!((s.ui_font_size, s.chat_font_size), (13, 16));
}

// it("steps the chat text with Ctrl+= / Ctrl+- / Ctrl+0 within 10..24")
#[test]
fn steps_the_chat_text_within_10_to_24() {
    assert_eq!(step_chat_font_size(13, FontStep::Increase), 14);
    assert_eq!(step_chat_font_size(13, FontStep::Decrease), 12);
    assert_eq!(step_chat_font_size(24, FontStep::Increase), 24);
    assert_eq!(step_chat_font_size(10, FontStep::Decrease), 10);
    assert_eq!(step_chat_font_size(20, FontStep::Reset), 13);
}

// describe("GPU acceleration on Linux"): both v0.2.15 tests are dropped (WebKitGTK only); PRD §4.1
// deviation 1 replaces them: GPU on by default everywhere, and pre-v2 files keep their saved value.
#[test]
fn gpu_acceleration_is_on_by_default_and_pre_v2_files_keep_their_saved_value() {
    assert!(Settings::parse(None).gpu_acceleration);
    let old = parse(r#"{"version":1,"gpuAcceleration":true,"fontSize":16}"#);
    assert!(old.gpu_acceleration);
    assert_eq!(old.font_size, 16);
    assert!(!parse(r#"{"gpuAcceleration":false}"#).gpu_acceleration);
    // Migrated files are written as version 2.
    assert_eq!(as_json(&old).get("version"), Some(&json!(2)));
}

// describe("terminal font weight") it("is 400 by default and only one of the offered weights")
#[test]
fn terminal_font_weight_is_400_by_default_and_only_one_of_the_offered_weights() {
    assert_eq!(defaults().font_weight, 400);
    assert_eq!(parse(r#"{"fontWeight":600}"#).font_weight, 600);
    assert_eq!(parse(r#"{"fontWeight":450}"#).font_weight, 400);
    assert_eq!(parse(r#"{"fontWeight":"bold"}"#).font_weight, 400);
    assert_eq!(FONT_WEIGHTS, [300, 400, 500, 600]);
    assert_eq!(settings_model::settings::bold_weight(400), 700);
    assert_eq!(settings_model::settings::bold_weight(600), 900);
}

// describe("terminal line height") it("is 1 by default and only one of the offered heights")
#[test]
#[allow(clippy::float_cmp, reason = "the offered heights are exact literals")]
fn terminal_line_height_is_1_by_default_and_only_one_of_the_offered_heights() {
    assert_eq!(defaults().line_height, 1.0);
    assert_eq!(parse(r#"{"lineHeight":1}"#).line_height, 1.0);
    assert_eq!(parse(r#"{"lineHeight":1.3}"#).line_height, 1.3);
    assert_eq!(parse(r#"{"lineHeight":1.12}"#).line_height, 1.0);
    // A height saved earlier is kept.
    assert_eq!(parse(r#"{"lineHeight":1.15}"#).line_height, 1.15);
    assert_eq!(parse(r#"{"lineHeight":"tall"}"#).line_height, 1.0);
    assert_eq!(LINE_HEIGHTS, [1.0, 1.05, 1.1, 1.15, 1.2, 1.25, 1.3]);
}

// describe("sharing settings")
// it("are off the relay until set, ask for View only joins and reset sudo by default")
#[test]
fn sharing_is_off_the_relay_until_set_asks_for_view_only_joins_and_resets_sudo_by_default() {
    let d = Settings::parse(None);
    assert_eq!([d.share_relay_url.as_str(), d.share_viewer_origin.as_str()], ["", ""]);
    assert_eq!(
        [
            d.share_view_needs_approval,
            d.share_sudo_reset,
            d.share_block_lid,
            d.share_record_input
        ],
        [true, true, false, false]
    );
}

// it("keep only relay and viewer URLs of their schemes")
#[test]
fn sharing_keeps_only_relay_and_viewer_urls_of_their_schemes() {
    assert_eq!(
        clean_share_url(Some(" wss://relay.example.com "), &["wss", "ws"]),
        "wss://relay.example.com"
    );
    assert_eq!(clean_share_url(Some("https://relay.example.com"), &["wss", "ws"]), "");
    assert_eq!(
        clean_share_url(Some("https://view.example.com"), &["https", "http"]),
        "https://view.example.com"
    );
    assert_eq!(clean_share_url(Some("javascript://x"), &["https", "http"]), "");
    assert_eq!(clean_share_url(None, &["https"]), "");
    let s = parse(r#"{"shareRelayUrl":"http://x","shareViewerOrigin":"https://v.example"}"#);
    assert_eq!(
        [s.share_relay_url.as_str(), s.share_viewer_origin.as_str()],
        ["", "https://v.example"]
    );
}
