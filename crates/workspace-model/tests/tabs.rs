//! Port of v0.2.15 `src/features/tabs/tabs.test.ts` (TAB-1, TAB-2 `move`, PANE-1, PANE-2,
//! TERM-19/19b); the original `it(...)` text is quoted above each test. "The same state" (`toBe`)
//! becomes a method returning false and leaving the state equal.

use workspace_model::{
    ChatAction, ChatLink, FocusDirection, MAX_PANES, PaneId, PaneLayout, PathOrigin, SplitDirection, SplitId, TabId,
    Tabs, ViewerLink, ViewerPosition,
};

#[derive(Debug, Clone, PartialEq)]
enum Target {
    Local,
    Ssh { host: &'static str, port: u16 },
}

fn target_title(target: &Target) -> String {
    match target {
        Target::Local => "Local shell".to_owned(),
        Target::Ssh { host, port } => format!("{host}:{port}"),
    }
}

const fn p(n: u32) -> PaneId {
    PaneId(n)
}

const fn t(n: u32) -> TabId {
    TabId(n)
}

fn with_tabs(count: usize) -> Tabs<Target> {
    let mut tabs = Tabs::new();
    for _ in 0..count {
        tabs.open(Target::Local);
    }
    tabs
}

fn keys(tabs: &Tabs<Target>) -> Vec<TabId> {
    tabs.tabs().iter().map(|tab| tab.key()).collect()
}

fn ids(tabs: &Tabs<Target>, index: usize) -> Vec<PaneId> {
    tabs.tabs()
        .get(index)
        .map(|tab| tab.layout().pane_ids())
        .unwrap_or_default()
}

fn focused(tabs: &Tabs<Target>, index: usize) -> Option<PaneId> {
    tabs.tabs().get(index).map(|tab| tab.focused_pane())
}

fn split_first(tabs: &mut Tabs<Target>, direction: SplitDirection) -> Option<PaneId> {
    let key = tabs.tabs().first().map(|tab| tab.key())?;
    tabs.split(key, direction)
}

fn full_tab() -> Tabs<Target> {
    let mut tabs = with_tabs(1);
    for _ in 1..MAX_PANES {
        assert!(tabs.split(t(1), SplitDirection::Row).is_some());
    }
    tabs
}

const LOCAL: PathOrigin = PathOrigin::Local;

fn view(tabs: &mut Tabs<Target>, path: &str) -> bool {
    tabs.open_viewer(t(1), path, LOCAL, ViewerPosition::default())
}

// "opens a tab and makes it active"
#[test]
fn opens_a_tab_and_makes_it_active() {
    let tabs = with_tabs(1);
    assert_eq!(tabs.tabs().len(), 1);
    let tab = &tabs.tabs()[0];
    // Until the shell or the user names it, the tab shows its target's title.
    assert_eq!(tab.title(), None);
    assert_eq!(tab.display_title(target_title), "Local shell");
    assert_eq!(tabs.active_key(), Some(tab.key()));
}

// "titles SSH tabs user@host, adding the port only when it is not 22": the title text belongs
// to the session targets; here only the target a tab keeps and shows is checked.
#[test]
fn a_tab_keeps_its_target_and_shows_its_title() {
    let mut tabs = Tabs::new();
    tabs.open(Target::Ssh {
        host: "srv",
        port: 2222,
    });
    let tab = &tabs.tabs()[0];
    assert_eq!(
        *tab.target(),
        Target::Ssh {
            host: "srv",
            port: 2222
        }
    );
    assert_eq!(tab.display_title(target_title), "srv:2222");
}

// "gives every tab a unique key"
#[test]
fn gives_every_tab_a_unique_key() {
    let tabs = with_tabs(2);
    assert_ne!(tabs.tabs()[0].key(), tabs.tabs()[1].key());
}

// "activates a tab by key"
#[test]
fn activates_a_tab_by_key() {
    let mut tabs = with_tabs(2);
    assert!(tabs.activate(t(1)));
    assert_eq!(tabs.active_key(), Some(t(1)));
    assert!(!tabs.activate(t(99)));
}

// "closing the active tab activates its right neighbour, else the left one"
#[test]
fn closing_the_active_tab_activates_its_right_neighbour_else_the_left_one() {
    let mut tabs = with_tabs(3);
    let [a, b, c] = [t(1), t(2), t(3)];
    tabs.activate(b);
    assert!(tabs.close(b));
    assert_eq!(tabs.active_key(), Some(c));
    assert!(tabs.close(c));
    assert_eq!(tabs.active_key(), Some(a));
}

// "closing an inactive tab keeps the active one"
#[test]
fn closing_an_inactive_tab_keeps_the_active_one() {
    let mut tabs = with_tabs(2);
    assert!(tabs.close(t(1)));
    assert_eq!(tabs.active_key(), Some(t(2)));
}

// "closing the last tab leaves no active tab"
#[test]
fn closing_the_last_tab_leaves_no_active_tab() {
    let mut tabs = with_tabs(1);
    assert!(tabs.close(t(1)));
    assert!(tabs.tabs().is_empty());
    assert_eq!(tabs.active_key(), None);
    assert!(!tabs.close(t(1)));
}

// "keeps the same state when a tab is renamed to the title it already has (no re-render)"
#[test]
fn keeps_the_same_state_when_a_tab_is_renamed_to_the_title_it_already_has() {
    let mut tabs = with_tabs(1);
    assert!(tabs.rename(t(1), "vim"));
    let before = tabs.clone();
    assert!(!tabs.rename(t(1), "vim"));
    assert_eq!(tabs, before);
}

// "renames a tab"
#[test]
fn renames_a_tab() {
    let mut tabs = with_tabs(1);
    tabs.rename(t(1), "vim");
    assert_eq!(tabs.tabs()[0].title(), Some("vim"));
    assert_eq!(tabs.tabs()[0].display_title(target_title), "vim");
}

// "opens a tab with one focused pane; pane ids are unique across tabs"
#[test]
fn opens_a_tab_with_one_focused_pane_pane_ids_are_unique_across_tabs() {
    let tabs = with_tabs(2);
    assert_eq!(ids(&tabs, 0).len(), 1);
    assert_eq!(focused(&tabs, 0), ids(&tabs, 0).first().copied());
    assert_ne!(ids(&tabs, 0), ids(&tabs, 1));
}

// "splitting adds a pane after the focused one and focuses it"
#[test]
fn splitting_adds_a_pane_after_the_focused_one_and_focuses_it() {
    let mut tabs = with_tabs(1);
    let first = ids(&tabs, 0)[0];
    let new = split_first(&mut tabs, SplitDirection::Column);
    assert_eq!(ids(&tabs, 0).len(), 2);
    assert_eq!(ids(&tabs, 0)[0], first);
    assert_eq!(new, Some(ids(&tabs, 0)[1]));
    assert_eq!(focused(&tabs, 0), new);
    let layout = tabs.tabs()[0].layout();
    assert_eq!(layout.root_split().unwrap().direction, SplitDirection::Column);
}

// "refuses a fifth pane"
#[test]
fn refuses_a_fifth_pane() {
    let mut four = full_tab();
    assert_eq!(ids(&four, 0).len(), 4);
    let before = four.clone();
    assert_eq!(split_first(&mut four, SplitDirection::Row), None);
    assert_eq!(four, before);
}

// "closing a pane keeps the tab and focuses a remaining pane"
#[test]
fn closing_a_pane_keeps_the_tab_and_focuses_a_remaining_pane() {
    let mut tabs = with_tabs(1);
    split_first(&mut tabs, SplitDirection::Row);
    let [a, b] = [ids(&tabs, 0)[0], ids(&tabs, 0)[1]];
    assert!(tabs.close_pane(t(1), b));
    assert_eq!(ids(&tabs, 0), vec![a]);
    assert_eq!(focused(&tabs, 0), Some(a));
}

// "closing an unfocused pane keeps the focus"
#[test]
fn closing_an_unfocused_pane_keeps_the_focus() {
    let mut tabs = with_tabs(1);
    split_first(&mut tabs, SplitDirection::Row);
    split_first(&mut tabs, SplitDirection::Row);
    let [a, c] = [ids(&tabs, 0)[0], ids(&tabs, 0)[2]];
    assert!(tabs.close_pane(t(1), a));
    assert_eq!(focused(&tabs, 0), Some(c));
}

// "closing the last pane closes the tab"
#[test]
fn closing_the_last_pane_closes_the_tab() {
    let mut tabs = with_tabs(2);
    let pane = ids(&tabs, 1)[0];
    assert!(tabs.close_pane(t(2), pane));
    assert_eq!(tabs.tabs().len(), 1);
    assert_eq!(tabs.active_key(), Some(t(1)));
}

// TERM-19: a shell exit closes only its pane; the tab closes with its last pane.
#[test]
fn a_shell_exit_closes_only_its_pane_and_the_tab_with_its_last_pane() {
    let mut tabs = with_tabs(2);
    split_first(&mut tabs, SplitDirection::Row); // pane-3 in tab-1
    assert_eq!(tabs.tab_of_pane(p(3)), Some(t(1)));
    assert!(tabs.shell_exited(p(3)));
    assert_eq!(ids(&tabs, 0), vec![p(1)]);
    assert_eq!(keys(&tabs), vec![t(1), t(2)]);
    assert!(tabs.shell_exited(p(1)));
    assert_eq!(keys(&tabs), vec![t(2)]);
    assert!(!tabs.shell_exited(p(99)));
}

// "focuses a pane by id or by direction"
#[test]
fn focuses_a_pane_by_id_or_by_direction() {
    let mut tabs = with_tabs(1);
    split_first(&mut tabs, SplitDirection::Row);
    let [a, b] = [ids(&tabs, 0)[0], ids(&tabs, 0)[1]];
    assert!(tabs.focus_pane(t(1), a));
    assert_eq!(focused(&tabs, 0), Some(a));
    assert!(tabs.focus_direction(t(1), FocusDirection::Right));
    assert_eq!(focused(&tabs, 0), Some(b));
    let before = tabs.clone();
    assert!(!tabs.focus_direction(t(1), FocusDirection::Right));
    assert!(!tabs.focus_pane(t(1), p(99)));
    assert_eq!(tabs, before);
}

// "resizes a split with the ratio clamped"
#[test]
fn resizes_a_split_with_the_ratio_clamped() {
    let mut tabs = with_tabs(1);
    let new = split_first(&mut tabs, SplitDirection::Row).unwrap();
    assert!(tabs.resize_split(t(1), SplitId(new), 0.95));
    assert_eq!(tabs.tabs()[0].layout().root_split().unwrap().ratio, 0.85);
}

// "closes every tab but one, keeping it active"
#[test]
fn closes_every_tab_but_one_keeping_it_active() {
    let mut tabs = with_tabs(3);
    let before = tabs.clone();
    assert!(!tabs.close_others(t(99)));
    assert_eq!(tabs, before);
    assert!(tabs.close_others(t(2)));
    assert_eq!(keys(&tabs), vec![t(2)]);
    assert_eq!(tabs.active_key(), Some(t(2)));
}

// "closes the tabs to the right of one, activating it when the active tab closed"
#[test]
fn closes_the_tabs_to_the_right_of_one_activating_it_when_the_active_tab_closed() {
    let mut tabs = with_tabs(3);
    let mut keep_active = tabs.clone();
    assert!(tabs.close_right(t(1)));
    assert_eq!(keys(&tabs), vec![t(1)]);
    assert_eq!(tabs.active_key(), Some(t(1)));
    keep_active.activate(t(1));
    assert!(keep_active.close_right(t(2)));
    assert_eq!(keys(&keep_active), vec![t(1), t(2)]);
    assert_eq!(keep_active.active_key(), Some(t(1)));
}

// TAB-1: Close other tabs is disabled with fewer than 2 tabs; Close tabs to the right (below)
// on the last tab.
#[test]
fn the_menu_disables_close_others_with_one_tab_and_close_right_on_the_last() {
    let mut tabs = with_tabs(1);
    assert!(!tabs.can_close_others());
    assert!(!tabs.can_close_right(t(1)));
    tabs.open(Target::Local);
    assert!(tabs.can_close_others());
    assert!(tabs.can_close_right(t(1)));
    assert!(!tabs.can_close_right(t(2)));
}

// "moves a tab to another place, keeping the active one and every tab's panes"
#[test]
fn moves_a_tab_to_another_place_keeping_the_active_one_and_every_tabs_panes() {
    let tabs = with_tabs(3);
    let [a, b, c] = [t(1), t(2), t(3)];
    let moved = |key, to| {
        let mut next = tabs.clone();
        next.move_tab(key, to);
        next
    };
    assert_eq!(keys(&moved(a, 2)), vec![b, c, a]);
    assert_eq!(keys(&moved(c, 0)), vec![c, a, b]);
    let far = moved(b, 99);
    assert_eq!(keys(&far), vec![a, c, b]);
    assert_eq!(far.active_key(), tabs.active_key());
    assert_eq!(far.tab(b), tabs.tab(b));
    // Nothing to move: the same state.
    let mut same = tabs.clone();
    assert!(!same.move_tab(b, 1));
    assert!(!same.move_tab(t(99), 0));
    assert_eq!(same, tabs);
}

// "closes every tab"
#[test]
fn closes_every_tab() {
    let mut tabs = with_tabs(2);
    assert!(tabs.close_all());
    assert!(tabs.tabs().is_empty());
    assert_eq!(tabs.active_key(), None);
}

// "splits the focused terminal down with a chat pane linked to it, focused"
#[test]
fn splits_the_focused_terminal_down_with_a_chat_pane_linked_to_it_focused() {
    let mut tabs = with_tabs(1);
    assert!(tabs.open_chat(t(1), "err"));
    let tab = &tabs.tabs()[0];
    let split = tab.layout().root_split().unwrap();
    assert_eq!(split.direction, SplitDirection::Column);
    assert_eq!(*split.a, PaneLayout::Pane(p(1)));
    assert_eq!(*split.b, PaneLayout::Pane(p(2)));
    assert_eq!(
        tab.chats().iter().collect::<Vec<_>>(),
        vec![(
            &p(2),
            &ChatLink {
                terminal: p(1),
                selection: "err".to_owned()
            }
        )]
    );
    assert_eq!(tab.focused_pane(), p(2));
}

// "focuses the existing chat of a terminal instead of opening a second one"
#[test]
fn focuses_the_existing_chat_of_a_terminal_instead_of_opening_a_second_one() {
    let mut tabs = with_tabs(1);
    tabs.open_chat(t(1), "");
    tabs.focus_pane(t(1), p(1));
    assert_eq!(tabs.tabs()[0].chat_action(), ChatAction::Focus { pane: p(2) });
    assert!(tabs.open_chat(t(1), ""));
    assert_eq!(ids(&tabs, 0), vec![p(1), p(2)]);
    assert_eq!(focused(&tabs, 0), Some(p(2)));
}

// "from a chat pane, goes back to its terminal"
#[test]
fn from_a_chat_pane_goes_back_to_its_terminal() {
    let mut tabs = with_tabs(1);
    tabs.open_chat(t(1), "");
    assert_eq!(tabs.tabs()[0].chat_action(), ChatAction::Terminal { pane: p(1) });
    assert!(tabs.open_chat(t(1), ""));
    assert_eq!(focused(&tabs, 0), Some(p(1)));
}

// "does nothing when the tab is full" (chat)
#[test]
fn opening_a_chat_does_nothing_when_the_tab_is_full() {
    let mut tabs = full_tab();
    assert_eq!(tabs.tabs()[0].chat_action(), ChatAction::Full);
    let before = tabs.clone();
    assert!(!tabs.open_chat(t(1), ""));
    assert_eq!(tabs, before);
}

// "closing a terminal also closes its chat; closing the chat focuses its terminal"
#[test]
fn closing_a_terminal_also_closes_its_chat_closing_the_chat_focuses_its_terminal() {
    let mut tabs = with_tabs(1);
    tabs.split(t(1), SplitDirection::Row); // pane-2
    tabs.focus_pane(t(1), p(1));
    tabs.open_chat(t(1), ""); // pane-3 under pane-1
    assert_eq!(ids(&tabs, 0), vec![p(1), p(3), p(2)]);
    let mut closed_chat = tabs.clone();
    assert!(closed_chat.close_pane(t(1), p(3)));
    assert_eq!(ids(&closed_chat, 0), vec![p(1), p(2)]);
    assert!(closed_chat.tabs()[0].chats().is_empty());
    assert_eq!(focused(&closed_chat, 0), Some(p(1)));
    let mut closed_terminal = tabs.clone();
    assert!(closed_terminal.close_pane(t(1), p(1)));
    assert_eq!(ids(&closed_terminal, 0), vec![p(2)]);
    assert!(closed_terminal.tabs()[0].chats().is_empty());
    assert_eq!(focused(&closed_terminal, 0), Some(p(2)));
}

// "closing the only terminal of a tab with its chat closes the tab"
#[test]
fn closing_the_only_terminal_of_a_tab_with_its_chat_closes_the_tab() {
    let mut tabs = with_tabs(1);
    tabs.open_chat(t(1), "");
    assert!(tabs.close_pane(t(1), p(1)));
    assert!(tabs.tabs().is_empty());
}

// "opens a file at the left of the focused terminal, linked to it and focused"
#[test]
fn opens_a_file_at_the_left_of_the_focused_terminal_linked_to_it_and_focused() {
    let mut tabs = with_tabs(1);
    assert!(view(&mut tabs, "/p/a.ts"));
    let tab = &tabs.tabs()[0];
    let split = tab.layout().root_split().unwrap();
    assert_eq!(split.direction, SplitDirection::Row);
    assert_eq!(*split.a, PaneLayout::Pane(p(2)));
    assert_eq!(*split.b, PaneLayout::Pane(p(1)));
    assert_eq!(
        tab.viewers().iter().collect::<Vec<_>>(),
        vec![(
            &p(2),
            &ViewerLink {
                terminal: p(1),
                path: "/p/a.ts".to_owned(),
                origin: LOCAL,
                opened: 1
            }
        )]
    );
    assert_eq!(tab.focused_pane(), p(2));
    assert!(tab.is_terminal_pane(p(1)));
    assert!(!tab.is_terminal_pane(p(2)));
    assert_eq!(tab.terminal_pane(), p(1));
}

// "opens a file at the right of the terminal or below it as asked"
#[test]
fn opens_a_file_at_the_right_of_the_terminal_or_below_it_as_asked() {
    let at = |position| {
        let mut tabs = with_tabs(1);
        assert!(tabs.open_viewer(t(1), "/p/a.ts", LOCAL, position));
        tabs
    };
    let right = at(ViewerPosition::Right);
    let split = right.tabs()[0].layout().root_split().unwrap();
    assert_eq!(
        (split.direction, &*split.a, &*split.b),
        (SplitDirection::Row, &PaneLayout::Pane(p(1)), &PaneLayout::Pane(p(2)))
    );
    let below = at(ViewerPosition::Below);
    let tab = &below.tabs()[0];
    let split = tab.layout().root_split().unwrap();
    assert_eq!(
        (split.direction, &*split.a, &*split.b),
        (SplitDirection::Column, &PaneLayout::Pane(p(1)), &PaneLayout::Pane(p(2)))
    );
    assert_eq!(tab.focused_pane(), p(2));
    assert_eq!(tab.viewers()[&p(2)].terminal, p(1));
}

// "shows another file of the same terminal in its viewer instead of opening a second one"
#[test]
fn shows_another_file_of_the_same_terminal_in_its_viewer_instead_of_opening_a_second_one() {
    let mut tabs = with_tabs(1);
    view(&mut tabs, "/p/a.ts");
    view(&mut tabs, "/p/b.md");
    assert_eq!(ids(&tabs, 0), vec![p(2), p(1)]);
    assert_eq!(tabs.tabs()[0].viewers()[&p(2)].path, "/p/b.md");
    tabs.focus_pane(t(1), p(1));
    view(&mut tabs, "/p/c.rs");
    assert_eq!(ids(&tabs, 0), vec![p(2), p(1)]);
    assert_eq!(tabs.tabs()[0].viewers()[&p(2)].path, "/p/c.rs");
    assert_eq!(focused(&tabs, 0), Some(p(2)));
    // Every View counts, the same file too: the viewer reads it again.
    assert!(view(&mut tabs, "/p/c.rs"));
    assert_eq!(tabs.tabs()[0].viewers()[&p(2)].opened, 4);
}

// "does nothing when the tab is full" (viewer)
#[test]
fn opening_a_viewer_does_nothing_when_the_tab_is_full() {
    let mut tabs = full_tab();
    let before = tabs.clone();
    assert!(!view(&mut tabs, "/p/a.ts"));
    assert_eq!(tabs, before);
}

// "closing a terminal also closes its viewer; closing the viewer focuses its terminal"
#[test]
fn closing_a_terminal_also_closes_its_viewer_closing_the_viewer_focuses_its_terminal() {
    let mut tabs = with_tabs(1);
    tabs.split(t(1), SplitDirection::Row);
    view(&mut tabs, "/p/a.ts");
    // pane-2 (the split) was focused: the viewer (pane-3) opens at its left.
    assert_eq!(ids(&tabs, 0), vec![p(1), p(3), p(2)]);
    let mut closed_viewer = tabs.clone();
    assert!(closed_viewer.close_pane(t(1), p(3)));
    assert_eq!(ids(&closed_viewer, 0), vec![p(1), p(2)]);
    assert!(closed_viewer.tabs()[0].viewers().is_empty());
    assert_eq!(focused(&closed_viewer, 0), Some(p(2)));
    let mut closed_terminal = tabs.clone();
    assert!(closed_terminal.close_pane(t(1), p(2)));
    assert_eq!(ids(&closed_terminal, 0), vec![p(1)]);
    assert!(closed_terminal.tabs()[0].viewers().is_empty());
}

// "opens the AI chat of the viewer's terminal from a focused viewer"
#[test]
fn opens_the_ai_chat_of_the_viewers_terminal_from_a_focused_viewer() {
    let mut tabs = with_tabs(1);
    view(&mut tabs, "/p/a.ts");
    assert_eq!(tabs.tabs()[0].chat_action(), ChatAction::Open { terminal: p(1) });
}

// "colours a tab, clears it, and gives a duplicate the same colour"
#[test]
fn colours_a_tab_clears_it_and_gives_a_duplicate_the_same_colour() {
    let mut tabs = with_tabs(1);
    assert!(tabs.set_color(t(1), Some("#c0392b")));
    assert_eq!(tabs.tabs()[0].color(), Some("#c0392b"));
    let copy = tabs.duplicate(t(1));
    assert_eq!(copy, Some(t(2)));
    assert_eq!(tabs.active_key(), Some(t(2)));
    assert_eq!(tabs.tabs()[1].color(), Some("#c0392b"));
    assert_eq!(tabs.tabs()[1].target(), tabs.tabs()[0].target());
    assert!(tabs.set_color(t(1), None));
    assert_eq!(tabs.tabs()[0].color(), None);
    let before = tabs.clone();
    assert!(!tabs.set_color(t(99), Some("#000000")));
    assert!(!tabs.set_color(t(1), None));
    assert_eq!(tabs, before);
}

// MEXEC-1: chat and viewer panes never take part in multi-execution.
#[test]
fn lists_the_terminal_panes_of_every_tab_without_chats_and_viewers() {
    let mut tabs = with_tabs(2);
    tabs.open_chat(t(1), "");
    tabs.focus_pane(t(1), p(1));
    view(&mut tabs, "/p/a.ts");
    assert_eq!(tabs.terminal_panes(), vec![p(1), p(2)]);
    let map = tabs.pane_map();
    assert_eq!(map[&t(1)].len(), 3);
    assert_eq!(map[&t(2)], vec![p(2)]);
}
