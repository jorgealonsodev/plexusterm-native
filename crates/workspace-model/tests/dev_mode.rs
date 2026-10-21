//! Port of v0.2.15 `src/features/devmode/devMode.test.ts` (DEV-1, DEV-1a, DEV-2a); the original
//! `it(...)` text is quoted above each test. The last tests cover the rules `useDevMode.ts` adds
//! (DEV-1a, checked in v0.2.15 only through `App.devMode.test.tsx`).

use std::collections::BTreeMap;

use workspace_model::{
    DEFAULT_DEV_COMMANDS, DevMode, DevState, LocateRequest, PaneId, TabId, clean_dev_commands, cli_of_command,
    cli_of_title, dev_commands_text, parse_dev_commands,
};

const TAB: TabId = TabId(1);
const P1: PaneId = PaneId(1);
const P2: PaneId = PaneId(2);
const P3: PaneId = PaneId(3);

fn commands() -> Vec<String> {
    DEFAULT_DEV_COMMANDS.iter().map(|c| (*c).to_owned()).collect()
}

fn list(names: &[&str]) -> Vec<String> {
    names.iter().map(|c| (*c).to_owned()).collect()
}

fn run_in(state: &mut DevState, command: &str, pane: PaneId, prompt: &str) -> bool {
    state.command(TAB, pane, command, prompt, &commands(), 1)
}

fn run(state: &mut DevState, command: &str) -> bool {
    run_in(state, command, P1, "me@box:~/proj$ ")
}

fn on_with(command: &str) -> DevState {
    let mut state = DevState::default();
    assert!(run(&mut state, command));
    state
}

fn title(state: &mut DevState, text: &str) -> bool {
    state.title(TAB, P1, text, &commands(), 7)
}

fn toggled(pane: PaneId, id: u64, hidden: Vec<PaneId>) -> DevState {
    let mut state = DevState::default();
    assert!(state.toggle(TAB, pane, id, hidden));
    state
}

fn panes(entries: &[(TabId, &[PaneId])]) -> BTreeMap<TabId, Vec<PaneId>> {
    entries.iter().map(|(tab, panes)| (*tab, panes.to_vec())).collect()
}

// "turns on for each listed command run as the first word"
#[test]
fn turns_on_for_each_listed_command_run_as_the_first_word() {
    for cli in ["claude", "codex", "agy", "opencode", "pi", "gentle-shell"] {
        assert_eq!(cli_of_command(cli, &commands()).as_deref(), Some(cli));
        let padded = format!("  {cli} --resume  ");
        assert_eq!(cli_of_command(&padded, &commands()).as_deref(), Some(cli));
    }
}

// "accepts a path to the program"
#[test]
fn accepts_a_path_to_the_program() {
    assert_eq!(
        cli_of_command("~/.local/bin/claude", &commands()).as_deref(),
        Some("claude")
    );
    assert_eq!(
        cli_of_command("C:\\tools\\codex.exe", &commands()).as_deref(),
        Some("codex")
    );
    assert_eq!(
        cli_of_command("C:\\tools\\CODEX.PS1", &list(&["CODEX"])).as_deref(),
        Some("CODEX")
    );
}

// "ignores other commands and the names further in the line"
#[test]
fn ignores_other_commands_and_the_names_further_in_the_line() {
    for line in ["claude-foo", "echo claude", "ls", "", "pip install x"] {
        assert_eq!(cli_of_command(line, &commands()), None, "{line}");
    }
}

// "follows the edited list"
#[test]
fn follows_the_edited_list() {
    assert_eq!(cli_of_command("aider", &list(&["aider"])).as_deref(), Some("aider"));
    assert_eq!(cli_of_command("claude", &list(&["aider"])), None);
    assert_eq!(cli_of_command("claude", &list(&[])), None);
}

// "recognises a CLI's own terminal title"
#[test]
fn recognises_a_clis_own_terminal_title() {
    let cases = [
        ("✳ Claude Code", "claude"),
        ("⠂ Claude Code", "claude"),
        ("Codex", "codex"),
        ("OpenCode", "opencode"),
        ("opencode - my-project", "opencode"),
        // pi titles the terminal with the Greek letter.
        ("π - PlexusTerm", "pi"),
        ("π", "pi"),
        ("Antigravity", "agy"),
    ];
    for (text, cli) in cases {
        assert_eq!(cli_of_title(text, &commands()).as_deref(), Some(cli), "{text}");
    }
}

// "ignores shell titles and lookalikes"
#[test]
fn ignores_shell_titles_and_lookalikes() {
    for text in ["jorge@host: ~/claude", "vim claude.md", "claudette", "", "πr² notes"] {
        assert_eq!(cli_of_title(text, &commands()), None, "{text}");
    }
}

// "keeps names, drops blanks, duplicates and invalid entries" (a non-string entry cannot be
// expressed in Rust's types; the settings parser drops it before this point)
#[test]
fn keeps_names_drops_blanks_duplicates_and_invalid_entries() {
    let saved = ["claude", " codex ", "", "claude", "two words", "a,b"];
    assert_eq!(clean_dev_commands(Some(saved)), list(&["claude", "codex"]));
    assert_eq!(clean_dev_commands(None::<[&str; 0]>), commands());
    assert_eq!(clean_dev_commands(Some([""; 0])), list(&[]));
}

// DEV-1: at most 50 names of at most 64 characters each.
#[test]
fn keeps_at_most_fifty_names_of_at_most_sixty_four_characters() {
    let many: Vec<String> = (0..60).map(|i| format!("cli{i}")).collect();
    let cleaned = clean_dev_commands(Some(&many));
    assert_eq!(cleaned.len(), 50);
    assert_eq!(cleaned.last().map(String::as_str), Some("cli49"));
    let long = "x".repeat(65);
    let exact = "é".repeat(64);
    assert_eq!(
        clean_dev_commands(Some([long.as_str(), exact.as_str()])),
        vec![exact.clone()]
    );
}

// "reads and writes the list as text"
#[test]
fn reads_and_writes_the_list_as_text() {
    assert_eq!(
        parse_dev_commands("claude, codex  aider"),
        list(&["claude", "codex", "aider"])
    );
    assert_eq!(parse_dev_commands(""), list(&[]));
    assert_eq!(dev_commands_text(&list(&["claude", "codex"])), "claude, codex");
}

// "turns a tab on when one of the CLIs runs, not for other commands"
#[test]
fn turns_a_tab_on_when_one_of_the_clis_runs_not_for_other_commands() {
    let mut state = DevState::default();
    assert!(!run(&mut state, "ls"));
    assert_eq!(state, DevState::default());
    let on = on_with("claude");
    let dev = on.get(TAB).unwrap();
    assert_eq!(dev.cli.as_deref(), Some("claude"));
    assert!(!dev.manual);
    assert_eq!(dev.pane, P1);
}

// "turns it off when the shell marks its prompt in the pane that ran the CLI"
#[test]
fn turns_it_off_when_the_shell_marks_its_prompt_in_the_pane_that_ran_the_cli() {
    let mut on = on_with("claude");
    assert!(!on.prompt_mark(TAB, P2));
    assert!(on.is_on(TAB));
    assert!(on.prompt_mark(TAB, P1));
    assert_eq!(on, DevState::default());
}

// "turns it off when output goes quiet on the same prompt the CLI was started from"
#[test]
fn turns_it_off_when_output_goes_quiet_on_the_same_prompt_the_cli_was_started_from() {
    let mut on = on_with("codex");
    // The CLI's own screen is not the shell's prompt.
    assert!(!on.quiet(TAB, P1, "│ > "));
    assert!(on.quiet(TAB, P1, "me@box:~/proj$ "));
    assert_eq!(on, DevState::default());
}

// DEV-1: the prompt comparison ignores digits and spaces (a clock, a duration).
#[test]
fn the_quiet_prompt_comparison_ignores_changing_numbers_and_spaces() {
    let mut on = DevState::default();
    assert!(run_in(&mut on, "codex", P1, "[12:01] ~ $ "));
    assert!(on.quiet(TAB, P1, "[9:58]   ~ $"));
    assert!(!on.is_on(TAB));
}

// "ignores an empty starting prompt for the quiet check"
#[test]
fn ignores_an_empty_starting_prompt_for_the_quiet_check() {
    let mut on = DevState::default();
    assert!(run_in(&mut on, "codex", P1, "  "));
    assert!(!on.quiet(TAB, P1, ""));
    assert!(on.is_on(TAB));
}

// "ignores commands typed inside the running CLI"
#[test]
fn ignores_commands_typed_inside_the_running_cli() {
    let mut on = on_with("claude");
    let before = on.clone();
    assert!(!run(&mut on, "codex"));
    assert_eq!(on, before);
}

// "turns on by title when nothing turned it on"
#[test]
fn turns_on_by_title_when_nothing_turned_it_on() {
    let mut on = DevState::default();
    assert!(on.title(TAB, P1, "✳ Claude Code", &commands(), 2));
    let dev = on.get(TAB).unwrap();
    assert_eq!(dev.cli.as_deref(), Some("claude"));
    assert!(!dev.manual);
    assert_eq!(dev.prompt, None);
    assert!(!on.quiet(TAB, P1, ""));
    assert!(on.prompt_mark(TAB, P1));
    assert_eq!(on, DevState::default());
}

// "learns the CLI from its title in a hand-made DEV mode, keeping it manual"
#[test]
fn learns_the_cli_from_its_title_in_a_hand_made_dev_mode_keeping_it_manual() {
    let mut state = toggled(P1, 3, vec![]);
    assert!(state.title(TAB, P1, "Codex", &commands(), 5));
    let dev = state.get(TAB).unwrap();
    assert!(dev.manual);
    assert_eq!(dev.cli.as_deref(), Some("codex"));
    assert_eq!(dev.id, 3);
    // A CLI already known is not replaced by a later title.
    let before = state.clone();
    assert!(!state.title(TAB, P1, "✳ Claude Code", &commands(), 6));
    assert_eq!(state, before);
}

// "knows the default CLIs in a DEV mode already on, even when the list leaves them out"
#[test]
fn knows_the_default_clis_in_a_dev_mode_already_on_even_when_the_list_leaves_them_out() {
    let codex = list(&["codex"]);
    // Unlisted, Claude Code does not turn DEV mode on by itself.
    let mut off = DevState::default();
    assert!(!off.title(TAB, P1, "✳ Claude Code", &codex, 2));
    assert!(!off.command(TAB, P1, "claude", "$ ", &codex, 2));
    assert_eq!(off, DevState::default());
    let on = toggled(P1, 3, vec![]);
    let mut by_title = on.clone();
    assert!(by_title.title(TAB, P1, "✳ Claude Code", &codex, 4));
    let dev = by_title.get(TAB).unwrap();
    assert!(dev.manual);
    assert_eq!(dev.cli.as_deref(), Some("claude"));
    let mut by_command = on.clone();
    assert!(by_command.command(TAB, P1, "claude", "$ ", &codex, 4));
    let dev = by_command.get(TAB).unwrap();
    assert!(dev.manual);
    assert_eq!(dev.cli.as_deref(), Some("claude"));
    // Its summary titles keep it known.
    let before = by_title.clone();
    assert!(!by_title.title(TAB, P1, "✳ Fix the build", &codex, 5));
    assert_eq!(by_title, before);
}

// "ends the CLI when the title stops being a CLI's: off when automatic, back to no CLI when by hand"
#[test]
fn ends_the_cli_when_the_title_stops_being_a_clis() {
    let titled = |state: &DevState, text: &str| {
        let mut next = state.clone();
        title(&mut next, text);
        next
    };
    // pi found by its title, then the shell's own title comes back.
    let pi = titled(&DevState::default(), "π - daily-work");
    let dev = pi.get(TAB).unwrap();
    assert_eq!((dev.cli.as_deref(), dev.manual), (Some("pi"), false));
    assert_eq!(titled(&pi, "daily-work"), DevState::default());
    // Started from the command line; Claude Code's working titles keep it on.
    let claude = titled(&titled(&on_with("claude"), "✳ Claude Code"), "✳ Fix the login bug");
    assert_eq!(claude.get(TAB).unwrap().cli.as_deref(), Some("claude"));
    assert_eq!(titled(&claude, "me@box: ~/proj"), DevState::default());
    // Shell titles that open with ASCII punctuation (zsh, Fedora's bash) end it too.
    assert_eq!(titled(&claude, "~/proj"), DevState::default());
    assert_eq!(titled(&claude, "[me@box proj]"), DevState::default());
    // A hand-made DEV mode stays on and forgets the CLI, so its icon goes back to the terminal.
    let manual = titled(&toggled(P1, 3, vec![]), "π - daily-work");
    let dev = manual.get(TAB).unwrap();
    assert_eq!((dev.cli.as_deref(), dev.manual), (Some("pi"), true));
    let back = titled(&manual, "daily-work");
    let dev = back.get(TAB).unwrap();
    assert_eq!((dev.cli.as_deref(), dev.manual), (None, true));
    // cmd.exe titles its window with the running command: still the CLI until the shell's title is back.
    let mut cmd = DevState::default();
    run_in(&mut cmd, "claude", P1, "C:\\Users\\me>");
    let running = titled(&cmd, "C:\\WINDOWS\\system32\\cmd.exe - claude");
    assert_eq!(running.get(TAB).unwrap().cli.as_deref(), Some("claude"));
    let resumed = titled(
        &running,
        "Administrador: C:\\WINDOWS\\system32\\cmd.exe - claude --resume",
    );
    assert_eq!(resumed.get(TAB).unwrap().cli.as_deref(), Some("claude"));
    assert_eq!(titled(&running, "C:\\WINDOWS\\system32\\cmd.exe"), DevState::default());
    // A folder that only contains the name in a longer word is the shell's title.
    assert_eq!(titled(&claude, "me@box: ~/claude-project"), DevState::default());
    // Another pane's title does not end the CLI.
    let mut other = pi.clone();
    assert!(!other.title(TAB, P2, "bash", &commands(), 8));
    assert_eq!(other, pi);
}

// "toggles by hand, and a hand-made DEV mode survives the prompt coming back"
#[test]
fn toggles_by_hand_and_a_hand_made_dev_mode_survives_the_prompt_coming_back() {
    let mut state = toggled(P1, 3, vec![]);
    let dev = state.get(TAB).unwrap();
    assert_eq!((dev.manual, dev.cli.as_deref()), (true, None));
    assert!(run(&mut state, "claude"));
    let dev = state.get(TAB).unwrap();
    assert_eq!((dev.manual, dev.cli.as_deref()), (true, Some("claude")));
    assert!(state.prompt_mark(TAB, P1));
    let dev = state.get(TAB).unwrap();
    assert_eq!((dev.manual, dev.cli.as_deref()), (true, None));
    assert!(state.toggle(TAB, P1, 4, vec![]));
    assert_eq!(state, DevState::default());
}

// "keeps the panes a hand-made DEV mode hides, and none otherwise"
#[test]
fn keeps_the_panes_a_hand_made_dev_mode_hides_and_none_otherwise() {
    let on = toggled(P2, 3, vec![P1, P3]);
    let dev = on.get(TAB).unwrap();
    assert_eq!((dev.pane, dev.hidden.clone()), (P2, vec![P1, P3]));
    assert!(toggled(P1, 3, vec![]).get(TAB).unwrap().hidden.is_empty());
    assert!(on_with("claude").get(TAB).unwrap().hidden.is_empty());
}

// "shows another of its terminals, hiding the one shown"
#[test]
fn shows_another_of_its_terminals_hiding_the_one_shown() {
    let mut on = toggled(P2, 3, vec![P1, P3]);
    assert!(on.show(TAB, P1, vec![P2, P3]));
    let dev = on.get(TAB).unwrap();
    assert_eq!((dev.pane, dev.hidden.clone(), dev.manual), (P1, vec![P2, P3], true));
    let mut off = DevState::default();
    assert!(!off.show(TAB, P1, vec![]));
    assert_eq!(off, DevState::default());
}

// "turns an automatic DEV mode off by hand"
#[test]
fn turns_an_automatic_dev_mode_off_by_hand() {
    let mut on = on_with("claude");
    assert!(on.toggle(TAB, P1, 4, vec![]));
    assert_eq!(on, DevState::default());
}

// "records the project folder of the DEV mode it was looked up for only"
#[test]
fn records_the_project_folder_of_the_dev_mode_it_was_looked_up_for_only() {
    let on = on_with("claude");
    let mut anchored = on.clone();
    assert!(anchored.anchor(TAB, 1, "/p/src", "/p"));
    let dev = anchored.get(TAB).unwrap();
    assert_eq!(
        (dev.folder.as_deref(), dev.root.as_deref()),
        (Some("/p/src"), Some("/p"))
    );
    let mut stale = on.clone();
    assert!(!stale.anchor(TAB, 99, "/x", "/x"));
    assert_eq!(stale, on);
}

// "forgets closed tabs and automatic DEV modes whose pane closed"
#[test]
fn forgets_closed_tabs_and_automatic_dev_modes_whose_pane_closed() {
    let on = on_with("claude");
    let mut kept = on.clone();
    assert!(!kept.forget(&panes(&[(TAB, &[P1])])));
    assert_eq!(kept, on);
    let mut closed_tab = on.clone();
    assert!(closed_tab.forget(&panes(&[])));
    assert_eq!(closed_tab, DevState::default());
    let mut closed_pane = on.clone();
    assert!(closed_pane.forget(&panes(&[(TAB, &[P2])])));
    assert_eq!(closed_pane, DevState::default());
    let manual = toggled(P1, 3, vec![]);
    let mut survives = manual.clone();
    assert!(!survives.forget(&panes(&[(TAB, &[P2])])));
    assert_eq!(survives, manual);
}

// DEV-1a (useDevMode): the title fallback turns DEV mode on only when a CLI title appears, so a
// manual turn-off sticks while the title stays.
#[test]
fn the_title_fallback_turns_on_only_when_a_cli_title_appears() {
    let mut dev = DevMode::new();
    dev.title(TAB, P1, "✳ Claude Code", &commands());
    assert!(dev.state().is_on(TAB));
    dev.toggle(TAB, P1, vec![]);
    assert!(!dev.state().is_on(TAB));
    dev.title(TAB, P1, "✳ Claude Code", &commands());
    assert!(!dev.state().is_on(TAB));
    dev.title(TAB, P1, "bash", &commands());
    dev.title(TAB, P1, "✳ Claude Code", &commands());
    assert!(dev.state().is_on(TAB));
}

// DEV-1a: a CLI started before the manual turn-on (even one the list leaves out) is known at once.
#[test]
fn turning_dev_mode_on_by_hand_while_a_cli_runs_learns_it() {
    let codex = list(&["codex"]);
    let mut dev = DevMode::new();
    dev.command(TAB, P1, "claude", "$ ", &codex);
    assert!(!dev.state().is_on(TAB));
    dev.toggle(TAB, P1, vec![]);
    let tab = dev.state().get(TAB).unwrap();
    assert_eq!((tab.manual, tab.cli.as_deref()), (true, Some("claude")));
    // Off and on again by hand while it still runs: learnt again.
    dev.toggle(TAB, P1, vec![]);
    dev.toggle(TAB, P1, vec![]);
    assert_eq!(dev.state().get(TAB).unwrap().cli.as_deref(), Some("claude"));
}

// DEV-1a: a DEV mode turned on after the CLI ended knows no CLI.
#[test]
fn a_dev_mode_turned_on_after_the_cli_ended_knows_no_cli() {
    let codex = list(&["codex"]);
    let mut dev = DevMode::new();
    dev.command(TAB, P1, "claude", "$ ", &codex);
    dev.prompt_mark(TAB, P1);
    dev.toggle(TAB, P1, vec![]);
    assert_eq!(dev.state().get(TAB).unwrap().cli, None);
    // The title leaving the CLI's ends it as well.
    let mut dev = DevMode::new();
    dev.title(TAB, P2, "✳ Claude Code", &codex);
    dev.title(TAB, P2, "~/proj", &codex);
    dev.toggle(TAB, P2, vec![]);
    assert_eq!(dev.state().get(TAB).unwrap().cli, None);
}

// DEV-1a: a DEV mode on without a CLI learns it from any CLI title, even one already showing.
#[test]
fn a_dev_mode_without_a_cli_learns_it_from_any_cli_title() {
    let mut dev = DevMode::new();
    dev.toggle(TAB, P1, vec![]);
    dev.title(TAB, P1, "✳ Claude Code", &list(&[]));
    assert_eq!(dev.state().get(TAB).unwrap().cli.as_deref(), Some("claude"));
    dev.prompt_mark(TAB, P1);
    assert_eq!(dev.state().get(TAB).unwrap().cli, None);
    // The same CLI title again (no new title appeared): learnt again all the same.
    dev.title(TAB, P1, "✳ Claude Code", &list(&[]));
    assert_eq!(dev.state().get(TAB).unwrap().cli.as_deref(), Some("claude"));
}

// useDevMode: a tab that turns on asks once for its project folder; only that DEV mode's answer
// is recorded.
#[test]
fn a_tab_turning_on_asks_for_its_project_folder_once() {
    let mut dev = DevMode::new();
    dev.command(TAB, P1, "claude", "$ ", &commands());
    let requests = dev.take_locate_requests();
    assert_eq!(requests.len(), 1);
    let LocateRequest { tab, id, pane } = requests[0];
    assert_eq!((tab, pane), (TAB, P1));
    dev.title(TAB, P1, "✳ Claude Code", &commands());
    assert!(dev.take_locate_requests().is_empty());
    assert!(dev.anchor(tab, id, "/p/src", "/p"));
    assert_eq!(dev.state().get(TAB).unwrap().root.as_deref(), Some("/p"));
    assert!(!dev.anchor(tab, id + 100, "/x", "/x"));
}

// useDevMode: forgetting closed panes also forgets the CLIs that ran in them.
#[test]
fn forgetting_closed_panes_forgets_the_clis_that_ran_in_them() {
    let mut dev = DevMode::new();
    dev.command(TAB, P1, "claude", "$ ", &list(&[]));
    dev.forget(&panes(&[(TAB, &[P2])]));
    dev.toggle(TAB, P1, vec![]);
    assert_eq!(dev.state().get(TAB).unwrap().cli, None);
}
