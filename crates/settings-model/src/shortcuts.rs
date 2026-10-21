//! The customisable keyboard shortcuts (PRD KEY-1..KEY-5), ported from v0.2.15
//! `src/features/panes/shortcuts.ts`.

use crate::capture::shortcut_from_event;

/// The subset of a key press the shortcut table needs. `key` uses the browser's key names
/// (`"ArrowLeft"`, `"F2"`, `" "`, `"t"`), which shortcut names are built from (KEY-5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyLike<'a> {
    pub key: &'a str,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    /// Side by side ("Split right").
    Row,
    /// One above the other ("Split down").
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Ctrl+= / Ctrl+- / Ctrl+0 (S15.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStep {
    Increase,
    Decrease,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarView {
    Sessions,
    Sftp,
    Explorer,
    Tunnels,
    Macros,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    KeyGen,
    PortScan,
    NetworkScan,
    WakeOnLan,
}

/// What a shortcut does (v0.2.15 `Shortcut`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NewTerminal,
    QuickConnect,
    ClosePane,
    Split(SplitDirection),
    ToggleMultiExec,
    Focus(FocusDirection),
    /// Pane-local: handled by the focused terminal itself (S15.1, S15.3).
    Find,
    Copy,
    Paste,
    SelectAll,
    Clear,
    SaveOutput,
    ToggleLogging,
    AskAi,
    /// Global font size (S15.2).
    FontSize(FontStep),
    /// Macros (S16.2, S16.4): start or stop recording, play the macro bound to Ctrl+Alt+`hotkey`.
    RecordMacro,
    PlayMacro(u8),
    /// Folds the sidebar to its tab icons (or hides it) and opens it again.
    ToggleSidebar,
    SidebarView(SidebarView),
    CloseTab,
    NextTab,
    PreviousTab,
    NewSerial,
    NewTelnet,
    NewMosh,
    Tool(Tool),
    ToggleRemoteMonitor,
    OpenSettings,
    /// The active tab's DEV mode on or off (S10).
    ToggleDevMode,
    /// Folds DEV mode's tab column to its icons (or hides it) and opens it again.
    ToggleDevColumn,
    /// Lists the folders DEV mode was last used in, to open one in a new DEV tab.
    RecentFolders,
}

/// The Settings group an action is listed under (KEY-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutGroup {
    Tabs,
    Connections,
    Terminal,
    Font,
    Sidebar,
    Tools,
    Ai,
}

macro_rules! actions {
    ($($variant:ident = $id:literal, $group:ident, $command:expr, [$($default:literal),*];)*) => {
        /// Every customisable action, in the order Settings lists them (KEY-3).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ShortcutAction {
            $($variant,)*
        }

        impl ShortcutAction {
            /// Every action, in Settings order.
            pub const ALL: [ShortcutAction; [$(stringify!($variant)),*].len()] = [$(ShortcutAction::$variant,)*];

            /// The id saved under `shortcuts` in settings.json.
            pub const fn id(self) -> &'static str {
                match self { $(ShortcutAction::$variant => $id,)* }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                match id { $($id => Some(ShortcutAction::$variant),)* _ => None }
            }

            pub const fn group(self) -> ShortcutGroup {
                match self { $(ShortcutAction::$variant => ShortcutGroup::$group,)* }
            }

            /// What the action does.
            pub const fn command(self) -> Command {
                match self { $(ShortcutAction::$variant => $command,)* }
            }

            /// Its default combinations, named as [`shortcut_from_event`] names them; empty when
            /// the action starts unbound. A default may have several (the font increase answers
            /// Ctrl+= and Ctrl++); Ctrl+Shift with a digit or a symbol is listed too, since some
            /// layouts need Shift to type them.
            pub const fn defaults(self) -> &'static [&'static str] {
                match self { $(ShortcutAction::$variant => &[$($default),*],)* }
            }
        }
    };
}

actions! {
    NewTerminal = "newTerminal", Tabs, Command::NewTerminal, ["Ctrl+Shift+T"];
    ClosePane = "closePane", Tabs, Command::ClosePane, ["Ctrl+Shift+W"];
    CloseTab = "closeTab", Tabs, Command::CloseTab, [];
    NextTab = "nextTab", Tabs, Command::NextTab, [];
    PreviousTab = "previousTab", Tabs, Command::PreviousTab, [];
    SplitRight = "splitRight", Tabs, Command::Split(SplitDirection::Row), ["Ctrl+Shift+D"];
    SplitDown = "splitDown", Tabs, Command::Split(SplitDirection::Column), ["Ctrl+Shift+E"];
    FocusLeft = "focusLeft", Tabs, Command::Focus(FocusDirection::Left), ["Alt+ArrowLeft"];
    FocusRight = "focusRight", Tabs, Command::Focus(FocusDirection::Right), ["Alt+ArrowRight"];
    FocusUp = "focusUp", Tabs, Command::Focus(FocusDirection::Up), ["Alt+ArrowUp"];
    FocusDown = "focusDown", Tabs, Command::Focus(FocusDirection::Down), ["Alt+ArrowDown"];
    ToggleMultiExec = "toggleMultiExec", Tabs, Command::ToggleMultiExec, ["Ctrl+Shift+M"];
    QuickConnect = "quickConnect", Connections, Command::QuickConnect, ["Ctrl+Shift+S"];
    NewSerial = "newSerial", Connections, Command::NewSerial, [];
    NewTelnet = "newTelnet", Connections, Command::NewTelnet, [];
    NewMosh = "newMosh", Connections, Command::NewMosh, [];
    Find = "find", Terminal, Command::Find, ["Ctrl+Shift+F"];
    Copy = "copy", Terminal, Command::Copy, ["Ctrl+Shift+C"];
    Paste = "paste", Terminal, Command::Paste, ["Ctrl+Shift+V"];
    SelectAll = "selectAll", Terminal, Command::SelectAll, [];
    Clear = "clear", Terminal, Command::Clear, [];
    SaveOutput = "saveOutput", Terminal, Command::SaveOutput, [];
    ToggleLogging = "toggleLogging", Terminal, Command::ToggleLogging, [];
    ToggleDevMode = "toggleDevMode", Terminal, Command::ToggleDevMode, [];
    RecentFolders = "recentFolders", Terminal, Command::RecentFolders, [];
    AskAi = "askAi", Ai, Command::AskAi, [];
    FontIncrease = "fontIncrease", Font, Command::FontSize(FontStep::Increase),
        ["Ctrl+=", "Ctrl++", "Ctrl+Shift+=", "Ctrl+Shift++"];
    FontDecrease = "fontDecrease", Font, Command::FontSize(FontStep::Decrease), ["Ctrl+-", "Ctrl+Shift+-"];
    FontReset = "fontReset", Font, Command::FontSize(FontStep::Reset), ["Ctrl+0", "Ctrl+Shift+0"];
    ToggleSidebar = "toggleSidebar", Sidebar, Command::ToggleSidebar, ["Ctrl+Shift+B"];
    ToggleDevColumn = "toggleDevColumn", Sidebar, Command::ToggleDevColumn, [];
    ShowSessions = "showSessions", Sidebar, Command::SidebarView(SidebarView::Sessions), [];
    ShowSftp = "showSftp", Sidebar, Command::SidebarView(SidebarView::Sftp), [];
    ShowExplorer = "showExplorer", Sidebar, Command::SidebarView(SidebarView::Explorer), [];
    ShowTunnels = "showTunnels", Sidebar, Command::SidebarView(SidebarView::Tunnels), [];
    ShowMacros = "showMacros", Sidebar, Command::SidebarView(SidebarView::Macros), [];
    RecordMacro = "recordMacro", Sidebar, Command::RecordMacro, ["Ctrl+Shift+R"];
    KeyGen = "keyGen", Tools, Command::Tool(Tool::KeyGen), [];
    PortScan = "portScan", Tools, Command::Tool(Tool::PortScan), [];
    NetworkScan = "networkScan", Tools, Command::Tool(Tool::NetworkScan), [];
    WakeOnLan = "wakeOnLan", Tools, Command::Tool(Tool::WakeOnLan), [];
    ToggleRemoteMonitor = "toggleRemoteMonitor", Tools, Command::ToggleRemoteMonitor, [];
    OpenSettings = "openSettings", Tools, Command::OpenSettings, [];
}

/// The user's changes, in the order they were made: action -> its one combination, or `""` when
/// the user left it unbound. Every other action keeps its defaults, so defaults can change in
/// later versions (KEY-5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortcutOverrides {
    entries: Vec<(ShortcutAction, String)>,
}

impl ShortcutOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    /// The override of `action`, if it has one.
    pub fn get(&self, action: ShortcutAction) -> Option<&str> {
        self.entries.iter().find(|(a, _)| *a == action).map(|(_, s)| s.as_str())
    }

    /// Sets `action`'s combination (`""` = unbound); a changed override keeps its place.
    pub fn set(&mut self, action: ShortcutAction, shortcut: impl Into<String>) {
        let shortcut = shortcut.into();
        match self.entries.iter_mut().find(|(a, _)| *a == action) {
            Some(entry) => entry.1 = shortcut,
            None => self.entries.push((action, shortcut)),
        }
    }

    /// Brings back `action`'s defaults.
    pub fn remove(&mut self, action: ShortcutAction) {
        self.entries.retain(|(a, _)| *a != action);
    }

    pub fn iter(&self) -> impl Iterator<Item = (ShortcutAction, &str)> {
        self.entries.iter().map(|(a, s)| (*a, s.as_str()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Fixed keys that no action may take, by what they do (KEY-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservedUse {
    /// Ctrl+Alt+1..9 play macros (see [`shortcut_for`]).
    PlayMacro,
    /// Ctrl+Right accepts the next word of an inline suggestion.
    AcceptWord,
}

/// Ctrl+Alt+1..9, as shortcut names.
pub const MACRO_HOTKEYS: [&str; 9] = [
    "Ctrl+Alt+1",
    "Ctrl+Alt+2",
    "Ctrl+Alt+3",
    "Ctrl+Alt+4",
    "Ctrl+Alt+5",
    "Ctrl+Alt+6",
    "Ctrl+Alt+7",
    "Ctrl+Alt+8",
    "Ctrl+Alt+9",
];

/// The fixed use of `shortcut`, if it is a fixed key.
pub fn reserved_use(shortcut: &str) -> Option<ReservedUse> {
    if MACRO_HOTKEYS.contains(&shortcut) {
        Some(ReservedUse::PlayMacro)
    } else if shortcut == "Ctrl+ArrowRight" {
        Some(ReservedUse::AcceptWord)
    } else {
        None
    }
}

/// The combinations an action answers to: its override when there is one (`""` = none), else
/// its defaults.
pub fn bindings_of(action: ShortcutAction, overrides: &ShortcutOverrides) -> Vec<&str> {
    match overrides.get(action) {
        None => action.defaults().to_vec(),
        Some("") => Vec::new(),
        Some(own) => vec![own],
    }
}

/// The combination shown for an action (its first one), or `""` when it is unbound.
pub fn shortcut_label(action: ShortcutAction, overrides: &ShortcutOverrides) -> &str {
    bindings_of(action, overrides).first().copied().unwrap_or("")
}

/// The action that answers to `shortcut` with these overrides, leaving out `except`.
pub fn action_using(
    shortcut: &str,
    overrides: &ShortcutOverrides,
    except: Option<ShortcutAction>,
) -> Option<ShortcutAction> {
    ShortcutAction::ALL
        .into_iter()
        .find(|&id| Some(id) != except && bindings_of(id, overrides).contains(&shortcut))
}

/// What already uses a combination: an action, a fixed key, or the AI assistant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutConflict {
    Action(ShortcutAction),
    Reserved(ReservedUse),
    Assistant,
}

/// Why `shortcut` cannot be given to `action` (`None` when it is free), checked in this order: a
/// fixed key, the assistant shortcut, another action's effective binding (KEY-5). Pass `action`
/// `None` when checking the assistant itself; the assistant shortcut only counts for an action.
pub fn shortcut_conflict(
    shortcut: &str,
    overrides: &ShortcutOverrides,
    action: Option<ShortcutAction>,
    assistant_shortcut: Option<&str>,
) -> Option<ShortcutConflict> {
    if let Some(used) = reserved_use(shortcut) {
        return Some(ShortcutConflict::Reserved(used));
    }
    if action.is_some() && assistant_shortcut == Some(shortcut) {
        return Some(ShortcutConflict::Assistant);
    }
    action_using(shortcut, overrides, action).map(ShortcutConflict::Action)
}

/// The application command for a key press with the given overrides, or `None` when the key
/// belongs to the terminal. Terminals must not see these keys: they would otherwise send e.g.
/// Ctrl+D (EOF) for Ctrl+Shift+D.
pub fn shortcut_for(key: &KeyLike<'_>, overrides: &ShortcutOverrides) -> Option<Command> {
    let name = shortcut_from_event(key)?;
    if let Some(action) = action_using(&name, overrides, None) {
        return Some(action.command());
    }
    // Only the digit itself: a character typed with AltGr (Ctrl+Alt on some systems) stays the terminal's.
    match key.key.as_bytes() {
        [digit @ b'1'..=b'9'] if key.ctrl && key.alt && !key.shift && !key.meta => {
            Some(Command::PlayMacro(digit - b'0'))
        }
        _ => None,
    }
}
