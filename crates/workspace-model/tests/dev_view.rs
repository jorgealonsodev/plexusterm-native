//! The DEV view rules v0.2.15 kept in `App.tsx`, `TabBar.tsx` and `PaneGrid.tsx` (DEV-2,
//! DEV-2a, DEV-2b, PANE-3, PANE-3a, PANE-3b). v0.2.15 checked them only through rendered
//! components (`App.devMode.test.tsx`, `TabBar.test.tsx`); the test names say which rule.

use workspace_model::{
    ColumnAtStartup, DevColumn, DevState, FooterInput, PaneFooter, PaneId, PathOrigin, SplitDirection,
    TAB_COLUMN_DEFAULT_WIDTH, TAB_COLUMN_MAX_WIDTH, TAB_COLUMN_MIN_WIDTH, TabId, Tabs, TargetKind, ViewerPosition,
    basename, clamp_tab_column_width, cli_name, dev_button_allowed, dev_hidden_panes, pane_footer, row_title,
    terminal_rows, title_without_cli_mark,
};

const T1: TabId = TabId(1);

const fn p(n: u32) -> PaneId {
    PaneId(n)
}

/// tab-1: pane-1 | pane-2, a viewer (pane-3) left of pane-2 and a chat (pane-4) under pane-1.
fn split_tab() -> Tabs<()> {
    let mut tabs = Tabs::new();
    tabs.open(());
    tabs.split(T1, SplitDirection::Row);
    tabs.open_viewer(T1, "/p/a.rs", PathOrigin::Local, ViewerPosition::Left);
    tabs.focus_pane(T1, p(1));
    tabs.open_chat(T1, "");
    tabs
}

fn input<'a>(dev: Option<&'a workspace_model::DevTab>) -> FooterInput<'a> {
    FooterInput {
        dev,
        target: TargetKind::Local,
        remote_monitor: false,
        branch: Some("main"),
        cwd: Some("/work/app"),
        folder_in_footer: true,
    }
}

// DEV-2: the CLIs' display names.
#[test]
fn names_the_known_clis_by_their_product_name() {
    assert_eq!(cli_name("claude"), "Claude Code");
    assert_eq!(cli_name("agy"), "Antigravity");
    assert_eq!(cli_name("opencode"), "OpenCode");
    assert_eq!(cli_name("aider"), "aider");
}

// DEV-2: a row's title drops the CLI's mark (`titleWithoutCliMark`).
#[test]
fn a_rows_title_drops_the_clis_mark() {
    assert_eq!(title_without_cli_mark("π - daily-work", Some("pi"), "pi"), "daily-work");
    assert_eq!(title_without_cli_mark("π: notes", Some("pi"), "pi"), "notes");
    assert_eq!(title_without_cli_mark("π", Some("pi"), "pi"), "pi");
    assert_eq!(
        title_without_cli_mark("✳ Fix the build", Some("claude"), "claude"),
        "Fix the build"
    );
    assert_eq!(title_without_cli_mark("πr² notes", Some("pi"), ""), "πr² notes");
    // Tabs without a CLI keep their title.
    assert_eq!(title_without_cli_mark("✳ x", None, ""), "✳ x");
}

// DEV-2: when what is left only names the CLI, no title next to a folder, else its name.
#[test]
fn a_rows_title_is_hidden_next_to_a_folder_when_it_only_names_the_cli() {
    assert_eq!(row_title("✳ Claude Code", Some("claude"), true), None);
    assert_eq!(
        row_title("✳ Claude Code", Some("claude"), false).as_deref(),
        Some("Claude Code")
    );
    assert_eq!(row_title("π", Some("pi"), false).as_deref(), Some("pi"));
    assert_eq!(row_title("✳ Fix it", Some("claude"), true).as_deref(), Some("Fix it"));
    assert_eq!(row_title("vim", None, true).as_deref(), Some("vim"));
}

// DEV-2a: turned on in one terminal, DEV mode hides the others but not its viewer and chat.
#[test]
fn dev_mode_on_one_terminal_hides_the_other_panes_but_its_viewer_and_chat() {
    let tabs = split_tab();
    let tab = tabs.tab(T1).unwrap();
    assert_eq!(tab.layout().pane_ids(), vec![p(1), p(4), p(3), p(2)]);
    assert_eq!(dev_hidden_panes(tab, p(1)), vec![p(3), p(2)]);
    assert_eq!(dev_hidden_panes(tab, p(2)), vec![p(1), p(4)]);
}

// DEV-2a: the column lists the tab's terminals numbered by place, marking the one on screen;
// nothing when none or every terminal is hidden.
#[test]
fn the_column_lists_a_dev_tabs_terminals_marking_the_one_on_screen() {
    let tabs = split_tab();
    let tab = tabs.tab(T1).unwrap();
    let rows = terminal_rows(tab, &[p(3), p(2)]);
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].pane, rows[0].number, rows[0].shown), (p(1), 1, true));
    assert_eq!((rows[1].pane, rows[1].number, rows[1].shown), (p(2), 2, false));
    assert_eq!(rows[1].label(Some(""), |n| format!("Terminal {n}")), "Terminal 2");
    assert_eq!(rows[1].label(Some("vim"), |n| format!("Terminal {n}")), "vim");
    assert!(terminal_rows(tab, &[]).is_empty());
    assert!(terminal_rows(tab, &[p(1), p(2)]).is_empty());
}

// DEV-2a: splitting while in DEV mode shows the new terminal; turned off, the split comes back.
#[test]
fn splitting_in_dev_mode_shows_the_new_terminal_and_turning_it_off_shows_all() {
    let mut tabs = split_tab();
    let mut dev = DevState::default();
    let hidden = dev_hidden_panes(tabs.tab(T1).unwrap(), p(1));
    dev.toggle(T1, p(1), 1, hidden);
    tabs.focus_pane(T1, p(1));
    // The tab is full (4 panes); close the chat to make room for a split.
    tabs.close_pane(T1, p(4));
    let new = tabs.split(T1, SplitDirection::Column).unwrap();
    let hidden = &dev.get(T1).unwrap().hidden;
    let shown = tabs.tab(T1).unwrap().layout().shown(hidden);
    assert_eq!(shown.pane_ids(), vec![p(1), new]);
    dev.toggle(T1, p(1), 2, vec![]);
    let hidden = dev.get(T1).map(|d| d.hidden.clone()).unwrap_or_default();
    assert_eq!(tabs.tab(T1).unwrap().layout().shown(&hidden).pane_count(), 4);
}

// PANE-3: the DEV button only for local, SSH and Mosh terminals.
#[test]
fn the_dev_button_shows_for_local_ssh_and_mosh_terminals_only() {
    for (kind, allowed) in [
        (TargetKind::Local, true),
        (TargetKind::Ssh, true),
        (TargetKind::Mosh, true),
        (TargetKind::Telnet, false),
        (TargetKind::Serial, false),
        (TargetKind::Shared, false),
    ] {
        assert_eq!(dev_button_allowed(kind), allowed, "{kind:?}");
    }
}

// PANE-3, PANE-3b: in DEV mode the bar shows the folder and the branch; outside, no folder or
// branch; no bar when nothing applies.
#[test]
fn the_bar_shows_folder_and_branch_in_dev_mode_only() {
    let mut tabs = Tabs::new();
    tabs.open(());
    let tab = tabs.tab(T1).unwrap();
    let mut dev = DevState::default();
    dev.toggle(T1, p(1), 1, vec![]);
    assert_eq!(
        pane_footer(tab, p(1), &input(dev.get(T1))),
        Some(PaneFooter {
            bare: false,
            folder: Some("/work/app".to_owned()),
            branch: Some("main".to_owned()),
            monitor: false,
            dev_button: true,
            dev_on: true,
        })
    );
    let off = pane_footer(tab, p(1), &input(None)).unwrap();
    assert_eq!((off.folder, off.branch, off.dev_button), (None, None, true));
    // devFolderInFooter off: no folder.
    let no_folder = FooterInput {
        folder_in_footer: false,
        ..input(dev.get(T1))
    };
    assert_eq!(pane_footer(tab, p(1), &no_folder).unwrap().folder, None);
    // A serial terminal outside DEV mode, without a monitor: no bar.
    let serial = FooterInput {
        target: TargetKind::Serial,
        ..input(None)
    };
    assert_eq!(pane_footer(tab, p(1), &serial), None);
    // An SSH terminal with the remote monitor on shows it.
    let ssh = FooterInput {
        target: TargetKind::Ssh,
        remote_monitor: true,
        ..input(None)
    };
    assert!(pane_footer(tab, p(1), &ssh).unwrap().monitor);
}

// PANE-3b: before the terminal reports its folder, the bar of the pane that ran the CLI names
// the folder DEV mode found.
#[test]
fn the_bar_falls_back_to_the_folder_dev_mode_found_for_its_pane() {
    let mut tabs = Tabs::new();
    tabs.open(());
    let mut dev = DevState::default();
    dev.toggle(T1, p(1), 1, vec![]);
    dev.anchor(T1, 1, "/p/src", "/p");
    let unknown = FooterInput {
        cwd: None,
        ..input(dev.get(T1))
    };
    let footer = pane_footer(tabs.tab(T1).unwrap(), p(1), &unknown).unwrap();
    assert_eq!(footer.folder.as_deref(), Some("/p/src"));
    assert_eq!(basename("/p/src"), "src");
    assert_eq!(basename("C:\\work\\api"), "api");
    assert_eq!(basename("/"), "/");
}

// PANE-3a: a terminal with another terminal below it keeps only the DEV option; when DEV mode
// hides the terminal below, it keeps its whole bar.
#[test]
fn a_terminal_with_a_terminal_below_keeps_only_the_dev_option() {
    let mut tabs = Tabs::new();
    tabs.open(());
    tabs.split(T1, SplitDirection::Column); // pane-2 below pane-1
    let tab = tabs.tab(T1).unwrap();
    let mut dev = DevState::default();
    dev.toggle(T1, p(2), 1, vec![]);
    let ssh = |dev| FooterInput {
        target: TargetKind::Ssh,
        remote_monitor: true,
        ..input(dev)
    };
    let top = pane_footer(tab, p(1), &ssh(dev.get(T1))).unwrap();
    assert!(top.bare);
    assert_eq!((top.branch, top.monitor, top.dev_button), (None, false, true));
    let below = pane_footer(tab, p(2), &ssh(dev.get(T1))).unwrap();
    assert!(!below.bare);
    assert_eq!(below.branch.as_deref(), Some("main"));
    let mut hiding = DevState::default();
    hiding.toggle(T1, p(1), 1, vec![p(2)]);
    let alone = pane_footer(tab, p(1), &ssh(hiding.get(T1))).unwrap();
    assert!(!alone.bare);
    assert_eq!(alone.branch.as_deref(), Some("main"));
}

// PANE-3a: a chat below a terminal does not make its bar bare.
#[test]
fn a_chat_below_a_terminal_does_not_make_its_bar_bare() {
    let mut tabs = Tabs::new();
    tabs.open(());
    tabs.open_chat(T1, "");
    let footer = pane_footer(tabs.tab(T1).unwrap(), p(1), &input(None)).unwrap();
    assert!(!footer.bare);
}

// DEV-2: the column width is clamped to 140-480 px and rounded; null means the default.
#[test]
fn the_tab_column_width_is_clamped_and_rounded() {
    assert_eq!(TAB_COLUMN_DEFAULT_WIDTH, 220);
    assert_eq!(clamp_tab_column_width(10.0), TAB_COLUMN_MIN_WIDTH);
    assert_eq!(clamp_tab_column_width(900.0), TAB_COLUMN_MAX_WIDTH);
    assert_eq!(clamp_tab_column_width(250.6), 251);
    assert_eq!(clamp_tab_column_width(f64::NAN), TAB_COLUMN_DEFAULT_WIDTH);
}

// DEV-2b: the column folds to a strip, or hides when `devColumnHideWhenCollapsed`; the startup
// state follows `devColumnAtStartup`.
#[test]
fn the_dev_column_folds_and_starts_as_settings_say() {
    let mut column = DevColumn::at_startup(ColumnAtStartup::Last, false, false);
    assert!(!column.is_collapsed());
    assert!(column.is_visible());
    assert!(!column.is_folded_strip());
    column.toggle();
    assert!(column.is_collapsed());
    assert!(column.is_folded_strip());
    assert!(DevColumn::at_startup(ColumnAtStartup::Last, true, false).is_collapsed());
    assert!(!DevColumn::at_startup(ColumnAtStartup::Expanded, true, false).is_collapsed());
    assert!(DevColumn::at_startup(ColumnAtStartup::Collapsed, false, false).is_collapsed());
    let hidden = DevColumn::at_startup(ColumnAtStartup::Collapsed, false, true);
    assert!(!hidden.is_visible());
    assert!(!hidden.is_folded_strip());
    // The toolbar button shows only while the active tab is in DEV mode.
    assert!(DevColumn::toggle_button_shown(true));
    assert!(!DevColumn::toggle_button_shown(false));
}
