//! What DEV mode changes on screen (DEV-2, DEV-2a, DEV-2b, PANE-3, PANE-3a, PANE-3b): the
//! single-terminal view, the column's rows and folding, and the bar under each terminal.
//!
//! v0.2.15 kept these rules in `App.tsx` (`othersOf`, `hiddenPanes`, the pane footer),
//! `TabBar.tsx` (`rowTitle`), `CliIcon.tsx` (`cliName`) and `settings.ts` (the column width).

use crate::dev_mode::{DevTab, title_without_cli_mark};
use crate::layout::{FocusDirection, PaneId};
use crate::tabs::Tab;

/// The name shown for a CLI's command: its product name when known, else the command.
pub fn cli_name(cli: &str) -> &str {
    match cli {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "agy" => "Antigravity",
        "opencode" => "OpenCode",
        "pi" => "pi",
        other => other,
    }
}

/// The title of a DEV column row: without the CLI's mark, and none at all when all that is left
/// is the CLI's own name (the icon says it) and the row shows a folder; the CLI's name otherwise.
pub fn row_title(title: &str, cli: Option<&str>, has_folder: bool) -> Option<String> {
    let Some(cli) = cli else {
        return Some(title.to_owned());
    };
    let rest = title_without_cli_mark(title, Some(cli), "");
    let name_only = rest.is_empty()
        || [cli, cli_name(cli)]
            .iter()
            .any(|name| name.to_lowercase() == rest.to_lowercase());
    if !name_only {
        return Some(rest);
    }
    (!has_folder).then(|| cli_name(cli).to_owned())
}

/// The last segment of a path (the folder's name), or the path itself when it has none.
pub fn basename(path: &str) -> &str {
    match path.rsplit(['/', '\\']).next() {
        Some(name) if !name.is_empty() => name,
        _ => path,
    }
}

/// What DEV mode turned on by hand in `pane` hides (DEV-2a): the tab's panes other than `pane`
/// and its own viewer and chat.
pub fn dev_hidden_panes<T>(tab: &Tab<T>, pane: PaneId) -> Vec<PaneId> {
    tab.layout()
        .pane_ids()
        .into_iter()
        .filter(|id| tab.terminal_of(*id) != pane)
        .collect()
}

/// A terminal of a DEV tab that hides some, as the column lists it under the tab's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRow {
    pub pane: PaneId,
    /// Its place among the tab's terminals, from 1, so it keeps its number when shown.
    pub number: usize,
    /// The one on screen.
    pub shown: bool,
}

impl TerminalRow {
    /// The row's title: the terminal's title, else `Terminal {n}` in the current language.
    pub fn label(&self, title: Option<&str>, numbered: impl FnOnce(usize) -> String) -> String {
        match title {
            Some(title) if !title.is_empty() => title.to_owned(),
            _ => numbered(self.number),
        }
    }
}

/// The terminals the column lists for a tab whose DEV mode keeps `hidden` out of sight; none
/// when no terminal is hidden or every one would be (then all are shown).
pub fn terminal_rows<T>(tab: &Tab<T>, hidden: &[PaneId]) -> Vec<TerminalRow> {
    let terminals = tab.terminal_panes();
    let out = terminals.iter().filter(|id| hidden.contains(id)).count();
    if out == 0 || out == terminals.len() {
        return Vec::new();
    }
    terminals
        .iter()
        .enumerate()
        .map(|(index, pane)| TerminalRow {
            pane: *pane,
            number: index + 1,
            shown: !hidden.contains(pane),
        })
        .collect()
}

/// The kind of terminal a tab runs, as the bar under it cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetKind {
    Local,
    Ssh,
    Mosh,
    Telnet,
    Serial,
    Shared,
}

/// The **DEV** button shows only for local, SSH and Mosh terminals (`DEV_TARGETS`).
pub fn dev_button_allowed(kind: TargetKind) -> bool {
    matches!(kind, TargetKind::Local | TargetKind::Ssh | TargetKind::Mosh)
}

/// What the bar under one terminal pane needs to know besides the tab's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterInput<'a> {
    /// The tab's DEV mode, if on.
    pub dev: Option<&'a DevTab>,
    pub target: TargetKind,
    /// The `remoteMonitor` setting (SSH panes only).
    pub remote_monitor: bool,
    /// The pane's git branch, once known.
    pub branch: Option<&'a str>,
    /// The pane's folder, once known.
    pub cwd: Option<&'a str>,
    /// The `devFolderInFooter` setting.
    pub folder_in_footer: bool,
}

/// The bar under a terminal, left to right: folder, branch, remote monitor, **DEV** button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFooter {
    /// Another terminal is below it on screen: only the DEV option, without the bar's look.
    pub bare: bool,
    /// The folder's full path (the tooltip); the bar shows its [`basename`].
    pub folder: Option<String>,
    pub branch: Option<String>,
    pub monitor: bool,
    pub dev_button: bool,
    /// The DEV button is highlighted (**DEV mode: on**).
    pub dev_on: bool,
}

/// The bar under `pane`, or `None` when none of branch, monitor and DEV button applies.
pub fn pane_footer<T>(tab: &Tab<T>, pane: PaneId, input: &FooterInput<'_>) -> Option<PaneFooter> {
    // Below it on screen: the panes DEV mode hides do not count.
    let hidden = input.dev.map_or(&[][..], |dev| dev.hidden.as_slice());
    let below = tab.layout().shown(hidden).neighbour(pane, FocusDirection::Down);
    let bare = below.is_some_and(|below| tab.is_terminal_pane(below));
    let branch = match input.dev {
        Some(_) if !bare => input.branch.map(str::to_owned),
        _ => None,
    };
    let monitor = input.target == TargetKind::Ssh && input.remote_monitor && !bare;
    let dev_button = dev_button_allowed(input.target);
    // DEV mode names each terminal's folder, to tell a tab's terminals apart.
    let folder = match input.dev {
        Some(dev) if input.folder_in_footer => input
            .cwd
            .map(str::to_owned)
            .or_else(|| (dev.pane == pane).then(|| dev.folder.clone()).flatten()),
        _ => None,
    };
    if branch.is_none() && !monitor && !dev_button {
        return None;
    }
    Some(PaneFooter {
        bare,
        folder,
        branch,
        monitor,
        dev_button,
        dev_on: input.dev.is_some(),
    })
}

pub const TAB_COLUMN_MIN_WIDTH: u32 = 140;
pub const TAB_COLUMN_MAX_WIDTH: u32 = 480;
pub const TAB_COLUMN_DEFAULT_WIDTH: u32 = 220;
pub const TAB_COLUMN_FOLDED_WIDTH: u32 = 44;

/// A dragged column width, rounded and clamped to 140-480 px; a non-finite one is the default.
pub fn clamp_tab_column_width(width: f64) -> u32 {
    if !width.is_finite() {
        return TAB_COLUMN_DEFAULT_WIDTH;
    }
    let clamped = width
        .round()
        .clamp(f64::from(TAB_COLUMN_MIN_WIDTH), f64::from(TAB_COLUMN_MAX_WIDTH));
    // Clamped to 140..=480 above, so the conversion is exact.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to 140..=480"
    )]
    let width = clamped as u32;
    width
}

/// `devColumnAtStartup`: the column's state when PlexusTerm opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColumnAtStartup {
    /// As it was when the window last closed (`devColumnCollapsed`).
    #[default]
    Last,
    Expanded,
    Collapsed,
}

/// The DEV column's folding (DEV-2b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DevColumn {
    collapsed: bool,
    /// `devColumnHideWhenCollapsed`: folded, the column hides instead of showing a strip.
    hide_when_collapsed: bool,
}

impl DevColumn {
    /// The state at startup from `devColumnAtStartup`, the saved `devColumnCollapsed` and
    /// `devColumnHideWhenCollapsed`.
    pub fn at_startup(at: ColumnAtStartup, saved_collapsed: bool, hide_when_collapsed: bool) -> Self {
        let collapsed = match at {
            ColumnAtStartup::Last => saved_collapsed,
            ColumnAtStartup::Expanded => false,
            ColumnAtStartup::Collapsed => true,
        };
        Self {
            collapsed,
            hide_when_collapsed,
        }
    }

    /// The toolbar button and the `toggleDevColumn` shortcut; the new state is what
    /// `devColumnCollapsed` saves.
    pub fn toggle(&mut self) -> bool {
        self.collapsed = !self.collapsed;
        self.collapsed
    }

    pub fn set_hide_when_collapsed(&mut self, hide: bool) {
        self.hide_when_collapsed = hide;
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Whether the column takes room at all.
    pub fn is_visible(&self) -> bool {
        !self.collapsed || !self.hide_when_collapsed
    }

    /// Folded to a narrow strip of the rows' icons.
    pub fn is_folded_strip(&self) -> bool {
        self.collapsed && !self.hide_when_collapsed
    }

    /// The toolbar button shows only while the active tab is in DEV mode.
    pub fn toggle_button_shown(active_tab_in_dev: bool) -> bool {
        active_tab_in_dev
    }
}
