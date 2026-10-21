//! settings.json (PRD SET-1, SET-2, DATA-1a), ported from v0.2.15 `src/features/settings/settings.ts`.
//!
//! Deviations, both from PRD §4.1: `gpuAcceleration` is on by default everywhere and a pre-v2
//! file keeps its saved value (row 1); unknown keys are kept and written back unchanged (row 2).

use crate::capture::is_valid_shortcut;
use crate::json::Json;
use crate::shortcuts::{FontStep, ShortcutAction, ShortcutOverrides, action_using, reserved_use, shortcut_conflict};

/// Written as `"version"`; files without it or with a lower one are read the same way, then
/// written with it (SET-2).
pub const SETTINGS_VERSION: u32 = 2;

pub const MIN_FONT_SIZE: u32 = 8;
pub const MAX_FONT_SIZE: u32 = 32;
pub const MAX_FONT_FAMILY_LENGTH: usize = 200;
pub const DEFAULT_FONT_FAMILY: &str = "\"JetBrains Mono\", \"Cascadia Mono\", \"DejaVu Sans Mono\", monospace";
/// Terminal text weights offered: light, regular, medium and semibold.
pub const FONT_WEIGHTS: [u16; 4] = [300, 400, 500, 600];
/// Bold text is this much heavier than the chosen weight.
pub const BOLD_STEP: u16 = 300;
/// Terminal line heights offered, as multiples of the font size.
pub const LINE_HEIGHTS: [f64; 7] = [1.0, 1.05, 1.1, 1.15, 1.2, 1.25, 1.3];
/// The interface and the AI chat are drawn for 13 px text; their sizes scale from it.
pub const BASE_TEXT_SIZE: u32 = 13;
pub const MIN_UI_FONT_SIZE: u32 = 10;
pub const MAX_UI_FONT_SIZE: u32 = 20;
pub const MIN_CHAT_FONT_SIZE: u32 = 10;
pub const MAX_CHAT_FONT_SIZE: u32 = 24;
pub const MIN_VIEWER_FONT_SIZE: u32 = 10;
pub const MAX_VIEWER_FONT_SIZE: u32 = 24;
pub const MAX_TEXT_EDITOR_LENGTH: usize = 1024;
pub const MAX_LOCAL_SHELL_LENGTH: usize = 1024;
pub const MAX_AI_HISTORY_CONTEXT: u32 = 20;
pub const MAX_AI_MODEL_LENGTH: usize = 200;
pub const MAX_AI_BASE_URL_LENGTH: usize = 500;
pub const MAX_SHARE_URL_LENGTH: usize = 500;
pub const DEFAULT_ASSISTANT_SHORTCUT: &str = "Ctrl+Shift+Space";
pub const SIDEBAR_MIN_WIDTH: u32 = 160;
pub const SIDEBAR_MAX_WIDTH: u32 = 640;
pub const TAB_COLUMN_MIN_WIDTH: u32 = 140;
pub const TAB_COLUMN_MAX_WIDTH: u32 = 480;
pub const DEFAULT_RECENT_FOLDERS: u32 = 10;
pub const MAX_RECENT_FOLDERS: u32 = 50;
/// The commands that turn DEV mode on, as Settings → Terminal lists them by default (DEV-1).
pub const DEFAULT_DEV_COMMANDS: [&str; 6] = ["claude", "codex", "agy", "opencode", "pi", "gentle-shell"];
pub const MAX_DEV_COMMANDS: usize = 50;
const MAX_DEV_COMMAND_LENGTH: usize = 64;
/// The `language` values: `"system"` follows the operating system, then the supported locales.
pub const LANGUAGES: [&str; 11] = [
    "system", "en", "es", "de", "fr", "ja", "pt-BR", "zh-CN", "ko", "ar", "hi",
];

/// AI providers (AI-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AiProvider {
    #[default]
    Anthropic,
    Gemini,
    OpenAi,
    OpenAiCompatible,
    OpenCodeGo,
}

impl AiProvider {
    pub const ALL: [AiProvider; 5] = [
        Self::Anthropic,
        Self::Gemini,
        Self::OpenAi,
        Self::OpenAiCompatible,
        Self::OpenCodeGo,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai-compatible",
            Self::OpenCodeGo => "opencode-go",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The model a provider starts with; OpenAI, compatible endpoints and OpenCode Go leave it to the user.
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-opus-5-5",
            Self::Gemini => "gemini-3.5-flash-lite",
            Self::OpenAi | Self::OpenAiCompatible | Self::OpenCodeGo => "",
        }
    }
}

/// How the sidebar (or DEV mode's tab column) opens with PlexusTerm (L17, DEV-2b).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StartupState {
    /// As it was left.
    #[default]
    Last,
    Expanded,
    Collapsed,
}

impl StartupState {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Last => "last",
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Last, Self::Expanded, Self::Collapsed]
            .into_iter()
            .find(|s| s.id() == id)
    }
}

/// Where a file viewer opens next to its terminal (VIEW-7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewerPosition {
    #[default]
    Left,
    Right,
    Below,
}

impl ViewerPosition {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Below => "below",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Left, Self::Right, Self::Below]
            .into_iter()
            .find(|p| p.id() == id)
    }
}

/// Keys of settings.json this version does not know, kept in their order and written back
/// unchanged (PRD §4.1 deviation 2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnknownKeys(Vec<(String, Json)>);

impl UnknownKeys {
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Every setting of settings.json (the 55 keys of v0.2.15, SET-1); the JSON key of each field is
/// its camelCase name.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Terminal font size in pixels, 8..32.
    pub font_size: u32,
    /// Selecting text with the mouse copies it to the clipboard.
    pub copy_on_select: bool,
    /// Right click pastes; when off it opens the terminal context menu.
    pub right_click_pastes: bool,
    /// Pasting more than one line asks first.
    pub confirm_multiline_paste: bool,
    /// Output lines that look like errors are shown in the colour scheme's red.
    pub error_highlight: bool,
    /// Colour scheme id (`ui_theme::schemes`).
    pub theme: String,
    /// Font family: one font name, or a font list.
    pub font_family: String,
    /// One of [`FONT_WEIGHTS`]; bold text is [`BOLD_STEP`] heavier.
    pub font_weight: u16,
    /// One of [`LINE_HEIGHTS`].
    pub line_height: f64,
    /// SSH tabs show the remote monitor bar.
    pub remote_monitor: bool,
    /// Program that "Open with default text editor" uses; empty uses the system's default.
    pub text_editor: String,
    /// File lists (SFTP, Explorer) show each entry's size and date (SET-4).
    pub file_details: bool,
    /// Opening a folder in the SFTP browser or the Explorer runs `cd` to it in the terminal.
    pub terminal_follows_browser: bool,
    pub sidebar_at_startup: StartupState,
    /// Grey inline suggestions of the rest of the command while typing.
    pub inline_suggestions: bool,
    /// Commands sent with Enter are recorded in history.json and suggested.
    pub command_history: bool,
    /// Ask the AI provider when the history has no suggestion.
    pub ai_suggestions: bool,
    pub ai_provider: AiProvider,
    /// Base URL of an OpenAI-compatible endpoint; empty for the others.
    pub ai_base_url: String,
    /// Model name; empty leaves it for the user to fill.
    pub ai_model: String,
    /// How many of the host's last commands are sent as AI context, 0..20.
    pub ai_history_context: u32,
    /// Opens the on-demand assistant; its own key, not under `shortcuts` (KEY-5).
    pub assistant_shortcut: String,
    pub sidebar_collapsed: bool,
    /// Folding hides the sidebar completely instead of leaving its tab icons.
    pub sidebar_hide_when_collapsed: bool,
    /// Shell of new local terminals: a path or a name on PATH; empty runs the system default.
    pub local_shell: String,
    /// Its arguments as typed.
    pub local_shell_args: String,
    /// Folder new local terminals start in (`~` is the home folder); empty is the home folder.
    pub local_cwd: String,
    /// Width in px dragged by the user; `None` keeps each sidebar tab's default width.
    pub sidebar_width: Option<u32>,
    /// Ask GitHub for a newer release at startup.
    pub check_updates: bool,
    /// Keep AI conversations in ai-chats.json after closing PlexusTerm.
    pub ai_save_chats: bool,
    /// One of [`LANGUAGES`].
    pub language: String,
    /// The user's shortcut changes only (KEY-5).
    pub shortcuts: ShortcutOverrides,
    /// Shift+Enter sends a line feed (TERM-8).
    pub shift_enter_newline: bool,
    /// A desktop notification when a background terminal asks for attention (TERM-9).
    pub attention_notifications: bool,
    /// Terminals draw on the GPU; off, the software renderer draws them (SET-2).
    pub gpu_acceleration: bool,
    /// File lists colour entries by their git status.
    pub git_file_status: bool,
    /// The file viewer saves an edited file by itself a moment after typing stops.
    pub viewer_auto_save: bool,
    pub viewer_position: ViewerPosition,
    /// The file viewer marks the lines changed since the file's last commit.
    pub viewer_changed_lines: bool,
    /// Text size of the interface in pixels, 10..20.
    pub ui_font_size: u32,
    /// Text size of the AI chat in pixels, 10..24.
    pub chat_font_size: u32,
    /// Text size of the file viewer's code and Markdown in pixels, 10..24.
    pub viewer_font_size: u32,
    /// Commands that turn a tab's DEV mode on (DEV-1).
    pub dev_commands: Vec<String>,
    /// Width in px of DEV mode's tab column; `None` keeps the default width.
    pub tab_column_width: Option<u32>,
    pub dev_column_collapsed: bool,
    pub dev_column_hide_when_collapsed: bool,
    pub dev_column_at_startup: StartupState,
    /// How many recent DEV mode folders the Explorer keeps, 1..50.
    pub recent_folder_limit: u32,
    /// In DEV mode the bar under each terminal names its folder.
    pub dev_folder_in_footer: bool,
    /// Terminal sharing: the relay's `wss://` URL (empty until set).
    pub share_relay_url: String,
    /// Terminal sharing: the `https://` origin that serves the viewer page (empty until set).
    pub share_viewer_origin: String,
    pub share_view_needs_approval: bool,
    pub share_sudo_reset: bool,
    pub share_block_lid: bool,
    pub share_record_input: bool,
    /// Keys this version does not know.
    pub unknown: UnknownKeys,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 14,
            copy_on_select: true,
            right_click_pastes: true,
            confirm_multiline_paste: true,
            error_highlight: true,
            theme: ui_theme::DEFAULT_SCHEME_ID.to_owned(),
            font_family: DEFAULT_FONT_FAMILY.to_owned(),
            font_weight: 400,
            line_height: 1.0,
            remote_monitor: true,
            text_editor: String::new(),
            file_details: false,
            terminal_follows_browser: true,
            sidebar_at_startup: StartupState::Last,
            inline_suggestions: true,
            command_history: true,
            ai_suggestions: false,
            ai_provider: AiProvider::Anthropic,
            ai_base_url: String::new(),
            ai_model: AiProvider::Anthropic.default_model().to_owned(),
            ai_history_context: 5,
            assistant_shortcut: DEFAULT_ASSISTANT_SHORTCUT.to_owned(),
            sidebar_collapsed: false,
            sidebar_hide_when_collapsed: false,
            local_shell: String::new(),
            local_shell_args: String::new(),
            local_cwd: String::new(),
            sidebar_width: None,
            check_updates: true,
            ai_save_chats: true,
            language: "system".to_owned(),
            shortcuts: ShortcutOverrides::new(),
            shift_enter_newline: true,
            attention_notifications: true,
            gpu_acceleration: true,
            git_file_status: true,
            viewer_auto_save: false,
            viewer_position: ViewerPosition::Left,
            viewer_changed_lines: true,
            ui_font_size: 13,
            chat_font_size: 13,
            viewer_font_size: 13,
            dev_commands: DEFAULT_DEV_COMMANDS.map(str::to_owned).to_vec(),
            tab_column_width: None,
            dev_column_collapsed: false,
            dev_column_hide_when_collapsed: false,
            dev_column_at_startup: StartupState::Last,
            recent_folder_limit: DEFAULT_RECENT_FOLDERS,
            dev_folder_in_footer: true,
            share_relay_url: String::new(),
            share_viewer_origin: String::new(),
            share_view_needs_approval: true,
            share_sudo_reset: true,
            share_block_lid: false,
            share_record_input: false,
            unknown: UnknownKeys::default(),
        }
    }
}

/// The top-level object of a settings file.
struct Raw<'a>(&'a [(String, Json)]);

impl Raw<'_> {
    fn get(&self, key: &str) -> Option<&Json> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn bool(&self, key: &str, fallback: bool) -> bool {
        self.get(key).and_then(Json::as_bool).unwrap_or(fallback)
    }

    fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Json::as_str)
    }

    fn num(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Json::as_f64)
    }

    fn startup(&self, key: &str) -> StartupState {
        self.str(key).and_then(StartupState::from_id).unwrap_or_default()
    }

    /// The saved folded state, or the one the startup choice forces (L17).
    fn collapsed(&self, startup_key: &str, key: &str) -> bool {
        match self.startup(startup_key) {
            StartupState::Last => self.bool(key, false),
            StartupState::Expanded => false,
            StartupState::Collapsed => true,
        }
    }
}

impl Settings {
    /// Reads settings.json. No file, unreadable JSON or anything but an object gives the
    /// defaults; every invalid value falls back to its default (sizes are rounded and clamped);
    /// unknown keys are kept for [`Settings::to_json`]. Files before version 2 read the same.
    pub fn parse(json: Option<&str>) -> Self {
        let d = Self::default();
        let Some(Json::Object(entries)) = json.and_then(Json::parse) else {
            return d;
        };
        let raw = Raw(&entries);
        let ai_provider = raw
            .str("aiProvider")
            .and_then(AiProvider::from_id)
            .unwrap_or(d.ai_provider);
        // The assistant and the overrides must not share a combination: the assistant keeps its own
        // unless an override makes it clash, and then the overrides are checked against the default.
        let wanted = raw
            .str("assistantShortcut")
            .filter(|s| is_valid_shortcut(s) && reserved_use(s).is_none())
            .unwrap_or(DEFAULT_ASSISTANT_SHORTCUT);
        let candidates = override_candidates(raw.get("shortcuts"));
        let mut shortcuts = clean_shortcut_overrides(candidates, wanted);
        let assistant_shortcut = clean_assistant_shortcut(Some(wanted), &shortcuts);
        if assistant_shortcut != wanted {
            let kept = shortcuts.iter().map(|(a, s)| (a, s.to_owned())).collect();
            shortcuts = clean_shortcut_overrides(kept, &assistant_shortcut);
        }
        let known = d.known_entries();
        let unknown = entries
            .iter()
            .filter(|(key, _)| key != "version" && !known.iter().any(|(k, _)| k == key))
            .cloned()
            .collect();
        Self {
            font_size: raw.num("fontSize").map_or(d.font_size, clamp_font_size),
            copy_on_select: raw.bool("copyOnSelect", d.copy_on_select),
            right_click_pastes: raw.bool("rightClickPastes", d.right_click_pastes),
            confirm_multiline_paste: raw.bool("confirmMultilinePaste", d.confirm_multiline_paste),
            error_highlight: raw.bool("errorHighlight", d.error_highlight),
            theme: raw
                .str("theme")
                .filter(|id| is_theme_id(id))
                .map_or(d.theme, str::to_owned),
            font_family: clean_font_family(raw.str("fontFamily")),
            font_weight: raw
                .num("fontWeight")
                .and_then(|w| FONT_WEIGHTS.into_iter().find(|&offered| f64::from(offered) == w))
                .unwrap_or(d.font_weight),
            line_height: raw
                .num("lineHeight")
                .and_then(|h| LINE_HEIGHTS.into_iter().find(|&offered| offered == h))
                .unwrap_or(d.line_height),
            remote_monitor: raw.bool("remoteMonitor", d.remote_monitor),
            text_editor: clean_text_editor(raw.str("textEditor")),
            file_details: raw.bool("fileDetails", d.file_details),
            terminal_follows_browser: raw.bool("terminalFollowsBrowser", d.terminal_follows_browser),
            sidebar_at_startup: raw.startup("sidebarAtStartup"),
            inline_suggestions: raw.bool("inlineSuggestions", d.inline_suggestions),
            command_history: raw.bool("commandHistory", d.command_history),
            ai_suggestions: raw.bool("aiSuggestions", d.ai_suggestions),
            ai_provider,
            ai_base_url: clean_ai_base_url(raw.str("aiBaseUrl")),
            ai_model: clean_ai_model(raw.str("aiModel"), ai_provider),
            ai_history_context: raw
                .num("aiHistoryContext")
                .map_or(d.ai_history_context, clamp_ai_history_context),
            assistant_shortcut,
            sidebar_collapsed: raw.collapsed("sidebarAtStartup", "sidebarCollapsed"),
            sidebar_hide_when_collapsed: raw.bool("sidebarHideWhenCollapsed", d.sidebar_hide_when_collapsed),
            dev_column_collapsed: raw.collapsed("devColumnAtStartup", "devColumnCollapsed"),
            dev_column_hide_when_collapsed: raw.bool("devColumnHideWhenCollapsed", d.dev_column_hide_when_collapsed),
            dev_column_at_startup: raw.startup("devColumnAtStartup"),
            recent_folder_limit: raw
                .num("recentFolderLimit")
                .map_or(d.recent_folder_limit, clamp_recent_folder_limit),
            dev_folder_in_footer: raw.bool("devFolderInFooter", d.dev_folder_in_footer),
            local_shell: clean_local_shell(raw.str("localShell")),
            local_shell_args: clean_local_shell(raw.str("localShellArgs")),
            local_cwd: clean_local_shell(raw.str("localCwd")),
            sidebar_width: raw.num("sidebarWidth").map(clamp_sidebar_width),
            check_updates: raw.bool("checkUpdates", d.check_updates),
            ai_save_chats: raw.bool("aiSaveChats", d.ai_save_chats),
            language: raw
                .str("language")
                .filter(|l| LANGUAGES.contains(l))
                .map_or(d.language, str::to_owned),
            shortcuts,
            shift_enter_newline: raw.bool("shiftEnterNewline", d.shift_enter_newline),
            attention_notifications: raw.bool("attentionNotifications", d.attention_notifications),
            // PRD §4.1 deviation 1: no pre-v2 migration of this value.
            gpu_acceleration: raw.bool("gpuAcceleration", d.gpu_acceleration),
            git_file_status: raw.bool("gitFileStatus", d.git_file_status),
            viewer_auto_save: raw.bool("viewerAutoSave", d.viewer_auto_save),
            viewer_position: raw
                .str("viewerPosition")
                .and_then(ViewerPosition::from_id)
                .unwrap_or_default(),
            viewer_changed_lines: raw.bool("viewerChangedLines", d.viewer_changed_lines),
            ui_font_size: raw.num("uiFontSize").map_or(d.ui_font_size, clamp_ui_font_size),
            chat_font_size: raw.num("chatFontSize").map_or(d.chat_font_size, clamp_chat_font_size),
            viewer_font_size: raw
                .num("viewerFontSize")
                .map_or(d.viewer_font_size, clamp_viewer_font_size),
            dev_commands: clean_dev_commands(raw.get("devCommands")),
            tab_column_width: raw.num("tabColumnWidth").map(clamp_tab_column_width),
            share_relay_url: clean_share_url(raw.str("shareRelayUrl"), &["wss", "ws"]),
            share_viewer_origin: clean_share_url(raw.str("shareViewerOrigin"), &["https", "http"]),
            share_view_needs_approval: raw.bool("shareViewNeedsApproval", d.share_view_needs_approval),
            share_sudo_reset: raw.bool("shareSudoReset", d.share_sudo_reset),
            share_block_lid: raw.bool("shareBlockLid", d.share_block_lid),
            share_record_input: raw.bool("shareRecordInput", d.share_record_input),
            unknown: UnknownKeys(unknown),
        }
    }

    /// settings.json as v0.2.15 writes it (two-space indent, `"version": 2` first), followed by
    /// the unknown keys.
    pub fn to_json(&self) -> String {
        let mut entries = vec![("version".to_owned(), Json::number(f64::from(SETTINGS_VERSION)))];
        entries.extend(self.known_entries().into_iter().map(|(k, v)| (k.to_owned(), v)));
        entries.extend(self.unknown.0.iter().cloned());
        Json::Object(entries).to_pretty_string()
    }

    /// The same settings without the unknown keys.
    pub fn without_unknown_keys(&self) -> Self {
        Self {
            unknown: UnknownKeys::default(),
            ..self.clone()
        }
    }

    pub fn unknown_keys(&self) -> impl Iterator<Item = &str> {
        self.unknown.keys()
    }

    /// Sets the terminal font size, rounded and clamped; a non-finite size keeps the current one.
    pub fn set_font_size(&mut self, size: f64) {
        if size.is_finite() {
            self.font_size = clamp_font_size(size);
        }
    }

    /// The 55 keys, in the order v0.2.15 writes them.
    fn known_entries(&self) -> Vec<(&'static str, Json)> {
        let b = Json::Bool;
        let s = |v: &str| Json::String(v.to_owned());
        let n = |v: u32| Json::number(f64::from(v));
        let width = |v: Option<u32>| v.map_or(Json::Null, n);
        vec![
            ("fontSize", n(self.font_size)),
            ("copyOnSelect", b(self.copy_on_select)),
            ("rightClickPastes", b(self.right_click_pastes)),
            ("confirmMultilinePaste", b(self.confirm_multiline_paste)),
            ("errorHighlight", b(self.error_highlight)),
            ("theme", s(&self.theme)),
            ("fontFamily", s(&self.font_family)),
            ("fontWeight", n(u32::from(self.font_weight))),
            ("lineHeight", Json::number(self.line_height)),
            ("remoteMonitor", b(self.remote_monitor)),
            ("textEditor", s(&self.text_editor)),
            ("fileDetails", b(self.file_details)),
            ("terminalFollowsBrowser", b(self.terminal_follows_browser)),
            ("sidebarAtStartup", s(self.sidebar_at_startup.id())),
            ("inlineSuggestions", b(self.inline_suggestions)),
            ("commandHistory", b(self.command_history)),
            ("aiSuggestions", b(self.ai_suggestions)),
            ("aiProvider", s(self.ai_provider.id())),
            ("aiBaseUrl", s(&self.ai_base_url)),
            ("aiModel", s(&self.ai_model)),
            ("aiHistoryContext", n(self.ai_history_context)),
            ("assistantShortcut", s(&self.assistant_shortcut)),
            ("sidebarCollapsed", b(self.sidebar_collapsed)),
            ("sidebarHideWhenCollapsed", b(self.sidebar_hide_when_collapsed)),
            ("devColumnCollapsed", b(self.dev_column_collapsed)),
            ("devColumnHideWhenCollapsed", b(self.dev_column_hide_when_collapsed)),
            ("devColumnAtStartup", s(self.dev_column_at_startup.id())),
            ("recentFolderLimit", n(self.recent_folder_limit)),
            ("devFolderInFooter", b(self.dev_folder_in_footer)),
            ("localShell", s(&self.local_shell)),
            ("localShellArgs", s(&self.local_shell_args)),
            ("localCwd", s(&self.local_cwd)),
            ("sidebarWidth", width(self.sidebar_width)),
            ("checkUpdates", b(self.check_updates)),
            ("aiSaveChats", b(self.ai_save_chats)),
            ("language", s(&self.language)),
            (
                "shortcuts",
                Json::Object(self.shortcuts.iter().map(|(a, v)| (a.id().to_owned(), s(v))).collect()),
            ),
            ("shiftEnterNewline", b(self.shift_enter_newline)),
            ("attentionNotifications", b(self.attention_notifications)),
            ("gpuAcceleration", b(self.gpu_acceleration)),
            ("gitFileStatus", b(self.git_file_status)),
            ("viewerAutoSave", b(self.viewer_auto_save)),
            ("viewerPosition", s(self.viewer_position.id())),
            ("viewerChangedLines", b(self.viewer_changed_lines)),
            ("uiFontSize", n(self.ui_font_size)),
            ("chatFontSize", n(self.chat_font_size)),
            ("viewerFontSize", n(self.viewer_font_size)),
            (
                "devCommands",
                Json::Array(self.dev_commands.iter().map(|c| s(c)).collect()),
            ),
            ("tabColumnWidth", width(self.tab_column_width)),
            ("shareRelayUrl", s(&self.share_relay_url)),
            ("shareViewerOrigin", s(&self.share_viewer_origin)),
            ("shareViewNeedsApproval", b(self.share_view_needs_approval)),
            ("shareSudoReset", b(self.share_sudo_reset)),
            ("shareBlockLid", b(self.share_block_lid)),
            ("shareRecordInput", b(self.share_record_input)),
        ]
    }
}

/// JavaScript's `Math.round`: halves round up.
fn js_round(value: f64) -> f64 {
    let floor = value.floor();
    if value - floor >= 0.5 { floor + 1.0 } else { floor }
}

/// `value` rounded and kept within `min..=max`.
fn clamp_round(value: f64, min: u32, max: u32) -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to min..=max first"
    )]
    let clamped = js_round(value).clamp(f64::from(min), f64::from(max)) as u32;
    clamped
}

pub fn clamp_font_size(size: f64) -> u32 {
    clamp_round(size, MIN_FONT_SIZE, MAX_FONT_SIZE)
}
pub fn clamp_ui_font_size(size: f64) -> u32 {
    clamp_round(size, MIN_UI_FONT_SIZE, MAX_UI_FONT_SIZE)
}
pub fn clamp_chat_font_size(size: f64) -> u32 {
    clamp_round(size, MIN_CHAT_FONT_SIZE, MAX_CHAT_FONT_SIZE)
}
pub fn clamp_viewer_font_size(size: f64) -> u32 {
    clamp_round(size, MIN_VIEWER_FONT_SIZE, MAX_VIEWER_FONT_SIZE)
}
pub fn clamp_recent_folder_limit(count: f64) -> u32 {
    clamp_round(count, 1, MAX_RECENT_FOLDERS)
}
pub fn clamp_ai_history_context(count: f64) -> u32 {
    clamp_round(count, 0, MAX_AI_HISTORY_CONTEXT)
}
/// Rounds a dragged sidebar width and keeps it within the sidebar's limits.
pub fn clamp_sidebar_width(width: f64) -> u32 {
    clamp_round(width, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}
/// Rounds a dragged width of DEV mode's tab column and keeps it within its limits.
pub fn clamp_tab_column_width(width: f64) -> u32 {
    clamp_round(width, TAB_COLUMN_MIN_WIDTH, TAB_COLUMN_MAX_WIDTH)
}

/// The weight of bold text for a chosen weight.
pub fn bold_weight(weight: u16) -> u16 {
    weight.saturating_add(BOLD_STEP).min(900)
}

/// Length as JavaScript counts it (UTF-16 code units), which the v0.2.15 limits use.
fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// JavaScript's `String.prototype.trim`.
fn js_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

fn is_theme_id(id: &str) -> bool {
    ui_theme::schemes().iter().any(|scheme| scheme.id == id)
}

/// A trimmed, non-empty font family of at most 200 characters, or the default one.
pub fn clean_font_family(value: Option<&str>) -> String {
    let family = value.map(js_trim).unwrap_or_default();
    if family.is_empty() || js_len(family) > MAX_FONT_FAMILY_LENGTH {
        DEFAULT_FONT_FAMILY
    } else {
        family
    }
    .to_owned()
}

/// A trimmed text editor command of at most 1024 characters, or `""` (the system default).
pub fn clean_text_editor(value: Option<&str>) -> String {
    let editor = value.map(js_trim).unwrap_or_default();
    if js_len(editor) <= MAX_TEXT_EDITOR_LENGTH {
        editor.to_owned()
    } else {
        String::new()
    }
}

/// A trimmed text of at most 1024 characters (the local shell, its arguments, its folder), or `""`.
pub fn clean_local_shell(value: Option<&str>) -> String {
    let text = value.map(js_trim).unwrap_or_default();
    if js_len(text) <= MAX_LOCAL_SHELL_LENGTH {
        text.to_owned()
    } else {
        String::new()
    }
}

/// A trimmed model name of at most 200 characters, or the provider's default model.
pub fn clean_ai_model(value: Option<&str>, provider: AiProvider) -> String {
    let Some(mut model) = value.map(js_trim) else {
        return provider.default_model().to_owned();
    };
    // OpenCode's own config names Go models "opencode-go/<id>"; the API wants just the id.
    const PREFIX: &str = "opencode-go/";
    if provider == AiProvider::OpenCodeGo
        && model
            .get(..PREFIX.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(PREFIX))
    {
        model = model.get(PREFIX.len()..).unwrap_or_default();
    }
    if js_len(model) <= MAX_AI_MODEL_LENGTH {
        model.to_owned()
    } else {
        provider.default_model().to_owned()
    }
}

/// `scheme://` followed by at least one character and no white space.
fn is_url_of(url: &str, scheme: &str) -> bool {
    url.split_once("://").is_some_and(|(s, rest)| {
        s.eq_ignore_ascii_case(scheme) && !rest.is_empty() && !rest.chars().any(char::is_whitespace)
    })
}

/// A trimmed http(s) URL of at most 500 characters, or `""`.
pub fn clean_ai_base_url(value: Option<&str>) -> String {
    let url = value.map(js_trim).unwrap_or_default();
    let ok = js_len(url) <= MAX_AI_BASE_URL_LENGTH && (is_url_of(url, "http") || is_url_of(url, "https"));
    if ok { url.to_owned() } else { String::new() }
}

/// A sharing URL of one of `schemes` (`wss`/`ws` for the relay, `https`/`http` for the viewer
/// page), or `""`.
pub fn clean_share_url(value: Option<&str>, schemes: &[&str]) -> String {
    let url = value.map(js_trim).unwrap_or_default();
    let scheme = url.split("://").next().unwrap_or_default().to_lowercase();
    let ok = js_len(url) <= MAX_SHARE_URL_LENGTH
        && schemes.contains(&scheme.as_str())
        && !scheme.is_empty()
        && scheme.bytes().all(|c| c.is_ascii_lowercase())
        && is_url_of(url, &scheme);
    if ok { url.to_owned() } else { String::new() }
}

/// The saved DEV mode commands: names without spaces or commas, at most 64 characters, no
/// duplicates, at most 50; the defaults when the value is not a list.
fn clean_dev_commands(value: Option<&Json>) -> Vec<String> {
    let Some(Json::Array(items)) = value else {
        return DEFAULT_DEV_COMMANDS.map(str::to_owned).to_vec();
    };
    let mut names: Vec<String> = Vec::new();
    for name in items.iter().filter_map(Json::as_str).map(js_trim) {
        let valid = !name.is_empty()
            && js_len(name) <= MAX_DEV_COMMAND_LENGTH
            && !name.chars().any(|c| c.is_whitespace() || c == ',');
        if valid && !names.iter().any(|n| n == name) {
            names.push(name.to_owned());
            if names.len() == MAX_DEV_COMMANDS {
                break;
            }
        }
    }
    names
}

/// A valid shortcut that PlexusTerm does not already use with these overrides, or the default
/// assistant shortcut.
pub fn clean_assistant_shortcut(value: Option<&str>, overrides: &ShortcutOverrides) -> String {
    value
        .filter(|s| is_valid_shortcut(s) && shortcut_conflict(s, overrides, None, None).is_none())
        .unwrap_or(DEFAULT_ASSISTANT_SHORTCUT)
        .to_owned()
}

/// The text values of known actions under `shortcuts`, in file order.
fn override_candidates(value: Option<&Json>) -> Vec<(ShortcutAction, String)> {
    let Some(Json::Object(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(id, v)| Some((ShortcutAction::from_id(id)?, v.as_str()?.to_owned())))
        .collect()
}

/// The usable overrides among `candidates`: `""` (unbound) or a valid combination that is not a
/// fixed key, not the assistant's, and not one another action answers to. When two overrides
/// clash, the later one is dropped (bringing its defaults back), until none clashes (KEY-5).
pub fn clean_shortcut_overrides(
    candidates: Vec<(ShortcutAction, String)>,
    assistant_shortcut: &str,
) -> ShortcutOverrides {
    let mut kept = ShortcutOverrides::new();
    for (action, shortcut) in candidates {
        let usable = shortcut.is_empty()
            || (is_valid_shortcut(&shortcut) && reserved_use(&shortcut).is_none() && shortcut != assistant_shortcut);
        if usable {
            kept.set(action, shortcut);
        }
    }
    loop {
        let clash = kept
            .iter()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find(|(id, shortcut)| !shortcut.is_empty() && action_using(shortcut, &kept, Some(*id)).is_some())
            .map(|(id, _)| id);
        match clash {
            Some(id) => kept.remove(id),
            None => return kept,
        }
    }
}

/// Ctrl+= / Ctrl+- / Ctrl+0 on the terminal font size (TERM-18).
pub fn step_font_size(size: u32, step: FontStep) -> u32 {
    match step {
        FontStep::Reset => Settings::default().font_size,
        FontStep::Increase => clamp_font_size(f64::from(size) + 1.0),
        FontStep::Decrease => clamp_font_size(f64::from(size) - 1.0),
    }
}

/// The same keys while the AI chat has the focus change its text only (AI-8).
pub fn step_chat_font_size(size: u32, step: FontStep) -> u32 {
    match step {
        FontStep::Reset => Settings::default().chat_font_size,
        FontStep::Increase => clamp_chat_font_size(f64::from(size) + 1.0),
        FontStep::Decrease => clamp_chat_font_size(f64::from(size) - 1.0),
    }
}
