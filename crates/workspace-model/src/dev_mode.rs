//! DEV mode (DEV-1, DEV-1a): a per-tab mode for working with AI command-line tools. It turns on
//! when the command line runs one of them (or, as a fallback, when a terminal takes one's title)
//! and off when the shell's prompt comes back after it, unless it was turned on by hand.
//!
//! Port of v0.2.15 `src/features/devmode/devMode.ts` ([`DevState`], the reducer) and
//! `useDevMode.ts` ([`DevMode`], the per-pane memory around it).

use std::collections::{BTreeMap, BTreeSet};

use crate::layout::PaneId;
use crate::tabs::TabId;

/// The commands that turn DEV mode on, as Settings → Terminal lists them by default.
pub const DEFAULT_DEV_COMMANDS: [&str; 6] = ["claude", "codex", "agy", "opencode", "pi", "gentle-shell"];
pub const MAX_DEV_COMMANDS: usize = 50;
pub const MAX_DEV_COMMAND_LENGTH: usize = 64;

/// Titles some CLIs give their terminal, by command.
const TITLE_NAMES: [(&str, &str); 3] = [("claude code", "claude"), ("antigravity", "agy"), ("π", "pi")];

pub fn default_dev_commands() -> Vec<String> {
    DEFAULT_DEV_COMMANDS.iter().map(|c| (*c).to_owned()).collect()
}

fn contains<S: AsRef<str>>(commands: &[S], name: &str) -> bool {
    commands.iter().any(|c| c.as_ref() == name)
}

/// The saved list: names without spaces or commas, at most 64 characters, no duplicates, at
/// most 50; the defaults when the value is missing.
pub fn clean_dev_commands<I, S>(value: Option<I>) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let Some(items) = value else {
        return default_dev_commands();
    };
    let mut names: Vec<String> = Vec::new();
    for item in items {
        let name = item.as_ref().trim();
        let valid = !name.is_empty()
            && name.chars().count() <= MAX_DEV_COMMAND_LENGTH
            && !name.chars().any(|c| c.is_whitespace() || c == ',');
        if !valid || contains(&names, name) {
            continue;
        }
        names.push(name.to_owned());
        if names.len() == MAX_DEV_COMMANDS {
            break;
        }
    }
    names
}

/// The list as typed in Settings: names separated by commas or spaces.
pub fn parse_dev_commands(text: &str) -> Vec<String> {
    clean_dev_commands(Some(text.split(|c: char| c.is_whitespace() || c == ',')))
}

pub fn dev_commands_text<S: AsRef<str>>(commands: &[S]) -> String {
    commands.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(", ")
}

/// The CLIs a DEV mode already on recognises: the list and the default ones, so a CLI left out
/// of the list still shows its icon; only the list turns DEV mode on by itself.
pub fn with_default_commands<S: AsRef<str>>(commands: &[S]) -> Vec<String> {
    let mut all: Vec<String> = commands.iter().map(|c| c.as_ref().to_owned()).collect();
    for default in DEFAULT_DEV_COMMANDS {
        if !contains(&all, default) {
            all.push(default.to_owned());
        }
    }
    all
}

/// The program of a path (`~/bin/claude`, `C:\tools\codex.exe`).
fn program_name(word: &str) -> &str {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    let cut = name.len().saturating_sub(4);
    match name.get(cut..) {
        Some(suffix)
            if [".exe", ".cmd", ".bat", ".ps1"]
                .iter()
                .any(|s| suffix.eq_ignore_ascii_case(s)) =>
        {
            name.get(..cut).unwrap_or(name)
        }
        _ => name,
    }
}

/// The listed CLI a command line runs (its first word), or `None`.
pub fn cli_of_command<S: AsRef<str>>(command: &str, commands: &[S]) -> Option<String> {
    let first = command.split_whitespace().next()?;
    let name = program_name(first);
    contains(commands, name).then(|| name.to_owned())
}

/// Whether `text` is `name` or starts with it followed by a space.
fn starts_with_word(text: &str, name: &str) -> bool {
    text.strip_prefix(name)
        .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
}

/// The listed CLI a terminal title names, or `None`: the title starts with the command (after a
/// spinner or symbol, as Claude Code's "✳ Claude Code") followed by nothing or a space.
pub fn cli_of_title<S: AsRef<str>>(title: &str, commands: &[S]) -> Option<String> {
    let text = title.trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    if text.is_empty() {
        return None;
    }
    for (name, command) in TITLE_NAMES {
        if contains(commands, command) && starts_with_word(&text, name) {
            return Some(command.to_owned());
        }
    }
    commands
        .iter()
        .map(AsRef::as_ref)
        .find(|command| starts_with_word(&text, &command.to_lowercase()))
        .map(str::to_owned)
}

/// A character that may border a whole-word CLI name.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// A title that names the running CLI as a word anywhere, as cmd.exe titles its window while a
/// program runs (`C:\WINDOWS\system32\cmd.exe - claude`). `claude-project` is another word.
fn mentions_cli(title: &str, cli: &str) -> bool {
    let text = title.to_lowercase();
    let aliases = TITLE_NAMES
        .iter()
        .filter(|(_, command)| *command == cli)
        .map(|(name, _)| *name);
    std::iter::once(cli).chain(aliases).any(|name| {
        !name.is_empty()
            && text.char_indices().any(|(i, _)| {
                let (Some(before), Some(after)) = (text.get(..i), text.get(i..)) else {
                    return false;
                };
                let Some(rest) = after.strip_prefix(name) else {
                    return false;
                };
                !before.chars().next_back().is_some_and(is_word_char) && !rest.chars().next().is_some_and(is_word_char)
            })
    })
}

/// A non-ASCII symbol (a spinner or mark such as Claude Code's ✳ or ⠂).
fn is_symbol(c: char) -> bool {
    !c.is_alphanumeric() && !c.is_whitespace() && !c.is_ascii()
}

/// Whether a title still belongs to `cli` running: while a CLI runs its titles name it or start
/// with a non-ASCII symbol; any other title is the shell's again. ASCII punctuation is the
/// shell's (`~/proj`, `[me@box]`).
pub fn title_keeps_cli<S: AsRef<str>>(title: &str, cli: &str, commands: &[S]) -> bool {
    cli_of_title(title, commands).is_some()
        || title.trim_start().chars().next().is_some_and(is_symbol)
        || mentions_cli(title, cli)
}

/// A tab's title without the mark its CLI puts in front (pi and gentle-shell's "π - ", Claude
/// Code's spinner), for rows that already show the CLI's icon; `name` when nothing else is left.
/// Titles of tabs without a CLI are returned unchanged.
pub fn title_without_cli_mark(title: &str, cli: Option<&str>, name: &str) -> String {
    if cli.is_none() {
        return title.to_owned();
    }
    let mut text = title;
    if let Some(rest) = text.strip_prefix('π') {
        let spaced = rest.trim_start();
        if let Some(after) = spaced.strip_prefix(['-', '–', '—', ':']) {
            text = after.trim_start();
        } else if spaced.len() < rest.len() || rest.is_empty() {
            text = spaced;
        }
    }
    let unmarked = text.trim_start_matches(is_symbol);
    if unmarked.len() < text.len() {
        text = unmarked.trim_start();
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        name.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Prompts compared loosely: spaces and changing numbers (a clock, a duration) do not count.
fn prompt_shape(text: &str) -> String {
    let mut shape = String::with_capacity(text.len());
    let mut last = None;
    for c in text.chars() {
        let mapped = if c.is_ascii_digit() {
            '0'
        } else if c.is_whitespace() {
            ' '
        } else {
            c
        };
        // A run of digits is one 0, a run of spaces one space.
        let class = if mapped == '0' && c.is_ascii_digit() {
            Some('0')
        } else if mapped == ' ' {
            Some(' ')
        } else {
            None
        };
        if class.is_none() || class != last {
            shape.push(mapped);
        }
        last = class;
    }
    shape.trim().to_owned()
}

/// A tab in DEV mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevTab {
    /// Identifies this DEV mode, so a slow folder lookup cannot anchor a later one.
    pub id: u64,
    /// Turned on by hand: only a hand (button or shortcut) turns it off.
    pub manual: bool,
    /// The CLI running in it (its icon), or `None`.
    pub cli: Option<String>,
    /// The pane that ran the CLI (or turned it on by hand); its prompt coming back turns an
    /// automatic DEV mode off.
    pub pane: PaneId,
    /// The text before the cursor when the CLI was started (the shell's prompt).
    pub prompt: Option<String>,
    /// The folder the command ran in, once known.
    pub folder: Option<String>,
    /// The project root (the git repository holding `folder`, else `folder`), once known.
    pub root: Option<String>,
    /// The panes kept out of sight while it is on (DEV-2a).
    pub hidden: Vec<PaneId>,
}

impl DevTab {
    fn new(id: u64, manual: bool, pane: PaneId) -> Self {
        Self {
            id,
            manual,
            cli: None,
            pane,
            prompt: None,
            folder: None,
            root: None,
            hidden: Vec::new(),
        }
    }
}

/// DEV mode by tab; a tab not listed is not in DEV mode. Each method returns whether it changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DevState {
    tabs: BTreeMap<TabId, DevTab>,
}

impl DevState {
    pub fn get(&self, tab: TabId) -> Option<&DevTab> {
        self.tabs.get(&tab)
    }

    pub fn is_on(&self, tab: TabId) -> bool {
        self.tabs.contains_key(&tab)
    }

    pub fn iter(&self) -> impl Iterator<Item = (TabId, &DevTab)> {
        self.tabs.iter().map(|(tab, dev)| (*tab, dev))
    }

    /// A command line ran in `pane` with `prompt` before the cursor.
    pub fn command<S: AsRef<str>>(
        &mut self,
        tab: TabId,
        pane: PaneId,
        command: &str,
        prompt: &str,
        commands: &[S],
        id: u64,
    ) -> bool {
        let cli = match self.tabs.get(&tab) {
            Some(_) => cli_of_command(command, &with_default_commands(commands)),
            None => cli_of_command(command, commands),
        };
        let Some(cli) = cli else {
            return false;
        };
        let dev = match self.tabs.get_mut(&tab) {
            // Typed inside a running CLI: not a shell command.
            Some(dev) if !dev.manual || dev.cli.is_some() => return false,
            Some(dev) => dev,
            None => self.tabs.entry(tab).or_insert(DevTab::new(id, false, pane)),
        };
        dev.cli = Some(cli);
        dev.pane = pane;
        dev.prompt = Some(prompt.to_owned());
        true
    }

    /// A pane's terminal took `title`.
    pub fn title<S: AsRef<str>>(&mut self, tab: TabId, pane: PaneId, title: &str, commands: &[S], id: u64) -> bool {
        let on = self.tabs.get(&tab);
        if let Some(dev) = on
            && let Some(cli) = &dev.cli
        {
            // Another title in its pane is the shell's again: the CLI has ended.
            let still = title_keeps_cli(title, cli, &with_default_commands(commands));
            return !still && dev.pane == pane && self.prompt_back(tab, pane);
        }
        let cli = match on {
            Some(_) => cli_of_title(title, &with_default_commands(commands)),
            None => cli_of_title(title, commands),
        };
        let Some(cli) = cli else {
            return false;
        };
        // A hand-made DEV mode learns its CLI from the title.
        let dev = self.tabs.entry(tab).or_insert(DevTab::new(id, false, pane));
        dev.cli = Some(cli);
        dev.pane = pane;
        true
    }

    /// The shell marked its prompt (OSC 7 or OSC 133).
    pub fn prompt_mark(&mut self, tab: TabId, pane: PaneId) -> bool {
        self.prompt_back(tab, pane)
    }

    /// A pane's output went quiet with `row` before the cursor.
    pub fn quiet(&mut self, tab: TabId, pane: PaneId, row: &str) -> bool {
        let prompt = self
            .tabs
            .get(&tab)
            .and_then(|dev| dev.prompt.as_deref())
            .map(prompt_shape)
            .unwrap_or_default();
        if prompt.is_empty() || prompt_shape(row) != prompt {
            return false;
        }
        self.prompt_back(tab, pane)
    }

    /// The shell's prompt is back in `pane`: an automatic DEV mode ends, a hand-made one
    /// forgets its CLI.
    fn prompt_back(&mut self, tab: TabId, pane: PaneId) -> bool {
        let Some(dev) = self.tabs.get_mut(&tab) else {
            return false;
        };
        if dev.pane != pane {
            return false;
        }
        if !dev.manual {
            self.tabs.remove(&tab);
            return true;
        }
        dev.cli.take().is_some()
    }

    /// Shows another terminal of a DEV tab, keeping `hidden` (the others) out of sight.
    pub fn show(&mut self, tab: TabId, pane: PaneId, hidden: Vec<PaneId>) -> bool {
        let Some(dev) = self.tabs.get_mut(&tab) else {
            return false;
        };
        if dev.pane == pane && dev.hidden == hidden {
            return false;
        }
        dev.pane = pane;
        dev.hidden = hidden;
        true
    }

    /// The **DEV** button or shortcut: off when on; else on by hand in `pane`, hiding `hidden`.
    pub fn toggle(&mut self, tab: TabId, pane: PaneId, id: u64, hidden: Vec<PaneId>) -> bool {
        if self.tabs.remove(&tab).is_none() {
            let mut dev = DevTab::new(id, true, pane);
            dev.hidden = hidden;
            self.tabs.insert(tab, dev);
        }
        true
    }

    /// The project folder looked up for the DEV mode `id`; ignored for a later one.
    pub fn anchor(&mut self, tab: TabId, id: u64, folder: &str, root: &str) -> bool {
        let Some(dev) = self.tabs.get_mut(&tab).filter(|dev| dev.id == id) else {
            return false;
        };
        if dev.folder.as_deref() == Some(folder) && dev.root.as_deref() == Some(root) {
            return false;
        }
        dev.folder = Some(folder.to_owned());
        dev.root = Some(root.to_owned());
        true
    }

    /// Given the open tabs and their panes: forgets closed tabs and automatic DEV modes whose
    /// pane closed; a manual DEV mode survives its pane.
    pub fn forget(&mut self, panes: &BTreeMap<TabId, Vec<PaneId>>) -> bool {
        let before = self.tabs.len();
        self.tabs.retain(|tab, dev| {
            panes
                .get(tab)
                .is_some_and(|open| dev.manual || open.contains(&dev.pane))
        });
        self.tabs.len() != before
    }
}

/// A tab just turned DEV mode on: the window looks up the folder `pane` is in and its project
/// root, then calls [`DevMode::anchor`] with the same `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocateRequest {
    pub tab: TabId,
    pub id: u64,
    pub pane: PaneId,
}

/// What first named the CLI running in a pane, replayed for a DEV mode turned on later by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Command {
        command: String,
        prompt: String,
        commands: Vec<String>,
    },
    Title {
        title: String,
        commands: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Running {
    cli: String,
    seen: Seen,
}

/// The DEV mode of every tab with the memory `useDevMode.ts` keeps per pane: which panes last
/// showed a CLI title, and which CLI runs in each pane until its shell is back (DEV-1a).
#[derive(Debug, Clone, Default)]
pub struct DevMode {
    state: DevState,
    next_id: u64,
    /// Whether each pane's last title named a CLI: only a new CLI title turns DEV mode on.
    titled_cli: BTreeMap<PaneId, bool>,
    running: BTreeMap<PaneId, Running>,
    locate: Vec<LocateRequest>,
}

impl DevMode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &DevState {
        &self.state
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// The folder lookups asked for since the last call.
    pub fn take_locate_requests(&mut self) -> Vec<LocateRequest> {
        std::mem::take(&mut self.locate)
    }

    /// Records a lookup when `tab` was off before a change and is on after it.
    fn applied(&mut self, tab: TabId, was_on: bool, changed: bool) -> bool {
        if changed
            && !was_on
            && let Some(dev) = self.state.get(tab)
        {
            self.locate.push(LocateRequest {
                tab,
                id: dev.id,
                pane: dev.pane,
            });
        }
        changed
    }

    pub fn command<S: AsRef<str>>(
        &mut self,
        tab: TabId,
        pane: PaneId,
        command: &str,
        prompt: &str,
        commands: &[S],
    ) -> bool {
        // Typed inside a running CLI, a command is not the start of one.
        if let Some(cli) = cli_of_command(command, &with_default_commands(commands))
            && !self.running.contains_key(&pane)
        {
            let seen = Seen::Command {
                command: command.to_owned(),
                prompt: prompt.to_owned(),
                commands: commands.iter().map(|c| c.as_ref().to_owned()).collect(),
            };
            self.running.insert(pane, Running { cli, seen });
        }
        let id = self.id();
        let was_on = self.state.is_on(tab);
        let changed = self.state.command(tab, pane, command, prompt, commands, id);
        self.applied(tab, was_on, changed)
    }

    /// A CLI retitles its terminal while it runs: only the first such title counts, so a DEV
    /// mode turned off by hand stays off until the CLI starts again.
    pub fn title<S: AsRef<str>>(&mut self, tab: TabId, pane: PaneId, title: &str, commands: &[S]) -> bool {
        let defaults = with_default_commands(commands);
        // A DEV mode already on also knows the default CLIs (their icon), listed or not.
        let is_cli = if self.state.is_on(tab) {
            cli_of_title(title, &defaults).is_some()
        } else {
            cli_of_title(title, commands).is_some()
        };
        let was = self.titled_cli.insert(pane, is_cli).unwrap_or(false);
        match self.running.get(&pane) {
            Some(running) if !title_keeps_cli(title, &running.cli, &defaults) => {
                self.running.remove(&pane);
            }
            Some(_) => {}
            None => {
                if let Some(cli) = cli_of_title(title, &defaults) {
                    let seen = Seen::Title {
                        title: title.to_owned(),
                        commands: commands.iter().map(|c| c.as_ref().to_owned()).collect(),
                    };
                    self.running.insert(pane, Running { cli, seen });
                }
            }
        }
        // A DEV mode on without a CLI (turned on by hand while one ran) learns it from any CLI title.
        let unknown = self.state.get(tab).is_some_and(|dev| dev.cli.is_none());
        if !(is_cli && (!was || unknown)) {
            return false;
        }
        let id = self.id();
        let was_on = self.state.is_on(tab);
        let changed = self.state.title(tab, pane, title, commands, id);
        self.applied(tab, was_on, changed)
    }

    pub fn prompt_mark(&mut self, tab: TabId, pane: PaneId) -> bool {
        self.running.remove(&pane);
        self.state.prompt_mark(tab, pane)
    }

    pub fn quiet(&mut self, tab: TabId, pane: PaneId, row: &str) -> bool {
        self.state.quiet(tab, pane, row)
    }

    pub fn show(&mut self, tab: TabId, pane: PaneId, hidden: Vec<PaneId>) -> bool {
        self.state.show(tab, pane, hidden)
    }

    /// Turns DEV mode on or off by hand from `pane`; `hidden` are the panes to keep out of sight
    /// (see `dev_view::dev_hidden_panes`). Turned on while a CLI already runs in the
    /// pane, it is known now, not at its next title.
    pub fn toggle(&mut self, tab: TabId, pane: PaneId, hidden: Vec<PaneId>) -> bool {
        let id = self.id();
        let was_on = self.state.is_on(tab);
        let changed = self.state.toggle(tab, pane, id, hidden);
        self.applied(tab, was_on, changed);
        if let Some(running) = self.running.get(&pane).cloned()
            && self.state.is_on(tab)
        {
            let id = self.id();
            match running.seen {
                Seen::Command {
                    command,
                    prompt,
                    commands,
                } => self.state.command(tab, pane, &command, &prompt, &commands, id),
                Seen::Title { title, commands } => self.state.title(tab, pane, &title, &commands, id),
            };
        }
        changed
    }

    pub fn anchor(&mut self, tab: TabId, id: u64, folder: &str, root: &str) -> bool {
        self.state.anchor(tab, id, folder, root)
    }

    /// Given the open tabs and their panes: forgets what closed panes reported.
    pub fn forget(&mut self, panes: &BTreeMap<TabId, Vec<PaneId>>) -> bool {
        let open: BTreeSet<PaneId> = panes.values().flatten().copied().collect();
        self.running.retain(|pane, _| open.contains(pane));
        self.titled_cli.retain(|pane, _| open.contains(pane));
        self.state.forget(panes)
    }
}
