//! The tabs of a window and the panes of each (TAB-1, TAB-2, PANE-1, PANE-2, TERM-19).
//!
//! Port of v0.2.15 `src/features/tabs/tabs.ts`: the reducer's actions are methods that return
//! whether anything changed (the reducer returned the same state when nothing did).

use std::collections::BTreeMap;
use std::fmt;

use crate::layout::{FocusDirection, MAX_PANES, PaneId, PaneLayout, Side, SplitDirection, SplitId};

/// A tab of the window. Shown as `tab-<n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u32);

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab-{}", self.0)
    }
}

/// Where a viewed file is: this computer, or the SFTP session of an SSH tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathOrigin {
    Local,
    Sftp { session_id: u64 },
}

/// Where a new file viewer goes next to its terminal (`viewerPosition`, VIEW-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ViewerPosition {
    #[default]
    Left,
    Right,
    Below,
}

/// An AI chat pane and the terminal pane its commands are inserted into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLink {
    pub terminal: PaneId,
    /// The terminal selection when the chat opened, offered with its first question.
    pub selection: String,
}

/// A file viewer pane: the file it shows and the terminal pane it opened next to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerLink {
    pub terminal: PaneId,
    pub path: String,
    pub origin: PathOrigin,
    /// Counts the files viewed in it, so viewing one again reads it again.
    pub opened: u32,
}

/// What opening the AI chat does in a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    /// Open a chat below this terminal.
    Open { terminal: PaneId },
    /// Focus the chat the focused terminal already has.
    Focus { pane: PaneId },
    /// From a chat, back to its terminal.
    Terminal { pane: PaneId },
    /// The tab has `MAX_PANES` panes: nothing.
    Full,
}

/// One tab; every pane runs its own terminal for `target`, except chat and viewer panes.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab<T> {
    key: TabId,
    /// The title the shell or the user gave the tab; `None` until then.
    title: Option<String>,
    target: T,
    layout: PaneLayout,
    focused: PaneId,
    /// AI chat panes; at most one per terminal pane.
    chats: BTreeMap<PaneId, ChatLink>,
    /// File viewer panes; at most one per terminal pane.
    viewers: BTreeMap<PaneId, ViewerLink>,
    /// The colour the user gave the tab (`#rrggbb`).
    color: Option<String>,
}

impl<T> Tab<T> {
    pub fn key(&self) -> TabId {
        self.key
    }

    /// The title the shell or the user gave it; `None` while the target's title is shown.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// What the tab bar shows: the given title, or the target's title (in the current language).
    pub fn display_title(&self, target_title: impl FnOnce(&T) -> String) -> String {
        self.title.clone().unwrap_or_else(|| target_title(&self.target))
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn layout(&self) -> &PaneLayout {
        &self.layout
    }

    pub fn focused_pane(&self) -> PaneId {
        self.focused
    }

    pub fn chats(&self) -> &BTreeMap<PaneId, ChatLink> {
        &self.chats
    }

    pub fn viewers(&self) -> &BTreeMap<PaneId, ViewerLink> {
        &self.viewers
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// A pane that runs a terminal (not an AI chat or a file viewer).
    pub fn is_terminal_pane(&self, pane: PaneId) -> bool {
        !self.chats.contains_key(&pane) && !self.viewers.contains_key(&pane)
    }

    /// The terminal of a pane: itself, or the terminal its chat or viewer belongs to.
    pub fn terminal_of(&self, pane: PaneId) -> PaneId {
        self.chats
            .get(&pane)
            .map(|link| link.terminal)
            .or_else(|| self.viewers.get(&pane).map(|link| link.terminal))
            .unwrap_or(pane)
    }

    /// The terminal of the focused pane.
    pub fn terminal_pane(&self) -> PaneId {
        self.terminal_of(self.focused)
    }

    /// The terminal panes in visual order.
    pub fn terminal_panes(&self) -> Vec<PaneId> {
        self.layout
            .pane_ids()
            .into_iter()
            .filter(|pane| self.is_terminal_pane(*pane))
            .collect()
    }

    pub fn chat_action(&self) -> ChatAction {
        // From a file viewer, the chat is its terminal's.
        let focused = self
            .viewers
            .get(&self.focused)
            .map_or(self.focused, |link| link.terminal);
        if let Some(link) = self.chats.get(&focused) {
            return ChatAction::Terminal { pane: link.terminal };
        }
        if let Some((pane, _)) = self.chats.iter().find(|(_, link)| link.terminal == focused) {
            return ChatAction::Focus { pane: *pane };
        }
        if self.layout.pane_count() >= MAX_PANES {
            return ChatAction::Full;
        }
        ChatAction::Open { terminal: focused }
    }

    fn close_panes(&mut self, pane: PaneId) -> Closing {
        // A terminal's chat and viewer panes go with it.
        let mut closing = vec![pane];
        closing.extend(
            self.chats
                .iter()
                .filter(|(_, link)| link.terminal == pane)
                .map(|(id, _)| *id),
        );
        closing.extend(
            self.viewers
                .iter()
                .filter(|(_, link)| link.terminal == pane)
                .map(|(id, _)| *id),
        );
        let Some(layout) = closing
            .iter()
            .try_fold(self.layout.clone(), |layout, id| layout.closed(*id))
        else {
            return Closing::Tab;
        };
        if layout == self.layout {
            return Closing::Nothing;
        }
        let link_terminal = self
            .chats
            .get(&pane)
            .map(|link| link.terminal)
            .or_else(|| self.viewers.get(&pane).map(|link| link.terminal));
        match link_terminal {
            // A closed chat or viewer gives the focus back to its terminal.
            Some(terminal) if self.focused == pane => self.focused = terminal,
            _ if closing.contains(&self.focused) => {
                // Focus the pane that now sits where the closed one was in visual order.
                let index = self.layout.pane_ids().iter().position(|id| *id == pane).unwrap_or(0);
                let remaining = layout.pane_ids();
                if let Some(next) = remaining.get(index).or_else(|| remaining.last()) {
                    self.focused = *next;
                }
            }
            _ => {}
        }
        self.layout = layout;
        self.chats.retain(|id, _| !closing.contains(id));
        self.viewers.retain(|id, _| !closing.contains(id));
        Closing::Panes
    }
}

enum Closing {
    Nothing,
    Panes,
    Tab,
}

/// The tabs of a window, the active one, and the counters that keep keys and pane ids unique.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabs<T> {
    tabs: Vec<Tab<T>>,
    active: Option<TabId>,
    next_seq: u32,
    next_pane_seq: u32,
}

impl<T> Default for Tabs<T> {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            next_seq: 1,
            next_pane_seq: 1,
        }
    }
}

impl<T> Tabs<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tabs in tab-bar order.
    pub fn tabs(&self) -> &[Tab<T>] {
        &self.tabs
    }

    pub fn active_key(&self) -> Option<TabId> {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab<T>> {
        self.active.and_then(|key| self.tab(key))
    }

    pub fn tab(&self, key: TabId) -> Option<&Tab<T>> {
        self.tabs.iter().find(|tab| tab.key == key)
    }

    fn tab_mut(&mut self, key: TabId) -> Option<&mut Tab<T>> {
        self.tabs.iter_mut().find(|tab| tab.key == key)
    }

    fn index_of(&self, key: TabId) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.key == key)
    }

    fn next_pane(&mut self) -> PaneId {
        let pane = PaneId(self.next_pane_seq);
        self.next_pane_seq += 1;
        pane
    }

    /// The tab that holds `pane`.
    pub fn tab_of_pane(&self, pane: PaneId) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|tab| tab.layout.contains(pane))
            .map(|tab| tab.key)
    }

    /// Every tab's panes (what DEV mode's `forget` needs).
    pub fn pane_map(&self) -> BTreeMap<TabId, Vec<PaneId>> {
        self.tabs.iter().map(|tab| (tab.key, tab.layout.pane_ids())).collect()
    }

    /// The terminal panes of every tab, chat and viewer panes left out (multi-execution).
    pub fn terminal_panes(&self) -> Vec<PaneId> {
        self.tabs.iter().flat_map(Tab::terminal_panes).collect()
    }

    /// Opens a tab with one pane and makes it active.
    pub fn open(&mut self, target: T) -> TabId {
        let key = TabId(self.next_seq);
        self.next_seq += 1;
        let pane = self.next_pane();
        self.tabs.push(Tab {
            key,
            title: None,
            target,
            layout: PaneLayout::leaf(pane),
            focused: pane,
            chats: BTreeMap::new(),
            viewers: BTreeMap::new(),
            color: None,
        });
        self.active = Some(key);
        key
    }

    /// Closes a tab; closing the active one activates the tab that took its index, else the one
    /// before it (TERM-19b).
    pub fn close(&mut self, key: TabId) -> bool {
        let Some(index) = self.index_of(key) else {
            return false;
        };
        self.tabs.remove(index);
        if self.active == Some(key) {
            self.active = self
                .tabs
                .get(index)
                .or_else(|| index.checked_sub(1).and_then(|i| self.tabs.get(i)))
                .map(|tab| tab.key);
        }
        true
    }

    /// **Close other tabs**, keeping `key` active.
    pub fn close_others(&mut self, key: TabId) -> bool {
        if self.index_of(key).is_none() {
            return false;
        }
        let changed = self.tabs.len() > 1 || self.active != Some(key);
        self.tabs.retain(|tab| tab.key == key);
        self.active = Some(key);
        changed
    }

    /// **Close tabs to the right** (**Close tabs below** in the DEV column); `key` becomes active
    /// when the active tab closed.
    pub fn close_right(&mut self, key: TabId) -> bool {
        let Some(index) = self.index_of(key) else {
            return false;
        };
        if index + 1 >= self.tabs.len() {
            return false;
        }
        self.tabs.truncate(index + 1);
        if !self.active.is_some_and(|active| self.index_of(active).is_some()) {
            self.active = Some(key);
        }
        true
    }

    pub fn close_all(&mut self) -> bool {
        let changed = !self.tabs.is_empty() || self.active.is_some();
        self.tabs.clear();
        self.active = None;
        changed
    }

    /// **Close other tabs** is disabled with fewer than 2 tabs.
    pub fn can_close_others(&self) -> bool {
        self.tabs.len() >= 2
    }

    /// **Close tabs to the right** is disabled on the last tab.
    pub fn can_close_right(&self, key: TabId) -> bool {
        self.index_of(key).is_some_and(|index| index + 1 < self.tabs.len())
    }

    pub fn activate(&mut self, key: TabId) -> bool {
        if self.active == Some(key) || self.index_of(key).is_none() {
            return false;
        }
        self.active = Some(key);
        true
    }

    /// Puts a tab at index `to` of the list (clamped), dragged in the tab bar or the DEV column.
    pub fn move_tab(&mut self, key: TabId, to: usize) -> bool {
        let Some(from) = self.index_of(key) else {
            return false;
        };
        let to = to.min(self.tabs.len().saturating_sub(1));
        if from == to {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        true
    }

    /// The shell or the user named the tab. Programs set the same title again and again: false.
    pub fn rename(&mut self, key: TabId, title: &str) -> bool {
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        if tab.title.as_deref() == Some(title) {
            return false;
        }
        tab.title = Some(title.to_owned());
        true
    }

    /// Gives a tab a colour (`None`: none).
    pub fn set_color(&mut self, key: TabId, color: Option<&str>) -> bool {
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        if tab.color.as_deref() == color {
            return false;
        }
        tab.color = color.map(str::to_owned);
        true
    }

    /// Splits the focused pane (PANE-1): the new pane goes right of or below it and takes the
    /// focus. `None` when refused (unknown tab, `MAX_PANES` panes).
    pub fn split(&mut self, key: TabId, direction: SplitDirection) -> Option<PaneId> {
        let pane = PaneId(self.next_pane_seq);
        let tab = self.tab_mut(key)?;
        if !tab.layout.split_pane(tab.focused, direction, pane, Side::After) {
            return None;
        }
        tab.focused = pane;
        self.next_pane_seq += 1;
        Some(pane)
    }

    /// The AI chat key: see [`Tab::chat_action`].
    pub fn open_chat(&mut self, key: TabId, selection: &str) -> bool {
        let pane = PaneId(self.next_pane_seq);
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        match tab.chat_action() {
            ChatAction::Full => false,
            ChatAction::Focus { pane: target } | ChatAction::Terminal { pane: target } => self.focus_pane(key, target),
            ChatAction::Open { terminal } => {
                if !tab
                    .layout
                    .split_pane(terminal, SplitDirection::Column, pane, Side::After)
                {
                    return false;
                }
                tab.focused = pane;
                tab.chats.insert(
                    pane,
                    ChatLink {
                        terminal,
                        selection: selection.to_owned(),
                    },
                );
                self.next_pane_seq += 1;
                true
            }
        }
    }

    /// Shows a file in the viewer of the focused terminal, opening one at `position` if it has
    /// none (PANE-2).
    pub fn open_viewer(&mut self, key: TabId, path: &str, origin: PathOrigin, position: ViewerPosition) -> bool {
        let pane = PaneId(self.next_pane_seq);
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        let terminal = tab.terminal_pane();
        let existing = tab.viewers.iter_mut().find(|(_, link)| link.terminal == terminal);
        if let Some((id, link)) = existing {
            let id = *id;
            link.path = path.to_owned();
            link.origin = origin;
            link.opened += 1;
            tab.focused = id;
            return true;
        }
        let (direction, side) = match position {
            ViewerPosition::Left => (SplitDirection::Row, Side::Before),
            ViewerPosition::Right => (SplitDirection::Row, Side::After),
            ViewerPosition::Below => (SplitDirection::Column, Side::After),
        };
        if !tab.layout.split_pane(terminal, direction, pane, side) {
            return false;
        }
        tab.focused = pane;
        tab.viewers.insert(
            pane,
            ViewerLink {
                terminal,
                path: path.to_owned(),
                origin,
                opened: 1,
            },
        );
        self.next_pane_seq += 1;
        true
    }

    /// Closes a pane with its chat and viewer; the tab closes with its last pane (TERM-19b).
    pub fn close_pane(&mut self, key: TabId, pane: PaneId) -> bool {
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        match tab.close_panes(pane) {
            Closing::Nothing => false,
            Closing::Panes => true,
            Closing::Tab => self.close(key),
        }
    }

    /// A shell exited: only its pane closes (TERM-19).
    pub fn shell_exited(&mut self, pane: PaneId) -> bool {
        self.tab_of_pane(pane).is_some_and(|key| self.close_pane(key, pane))
    }

    pub fn focus_pane(&mut self, key: TabId, pane: PaneId) -> bool {
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        if tab.focused == pane || !tab.layout.contains(pane) {
            return false;
        }
        tab.focused = pane;
        true
    }

    /// Alt+Arrow (PANE-1).
    pub fn focus_direction(&mut self, key: TabId, direction: FocusDirection) -> bool {
        let Some(tab) = self.tab_mut(key) else {
            return false;
        };
        match tab.layout.neighbour(tab.focused, direction) {
            Some(target) => {
                tab.focused = target;
                true
            }
            None => false,
        }
    }

    /// A divider dragged: the split's ratio, clamped.
    pub fn resize_split(&mut self, key: TabId, split: SplitId, ratio: f64) -> bool {
        self.tab_mut(key).is_some_and(|tab| tab.layout.set_ratio(split, ratio))
    }
}

impl<T: Clone> Tabs<T> {
    /// **Duplicate tab**: a new tab for the same target, with the same colour.
    pub fn duplicate(&mut self, key: TabId) -> Option<TabId> {
        let (target, color) = self.tab(key).map(|tab| (tab.target.clone(), tab.color.clone()))?;
        let copy = self.open(target);
        if let Some(tab) = self.tab_mut(copy) {
            tab.color = color;
        }
        Some(copy)
    }
}
