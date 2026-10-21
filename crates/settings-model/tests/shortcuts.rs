//! Ported from PlexusTerm v0.2.15 `src/features/panes/shortcuts.test.ts` (PRD KEY-1..KEY-5).

use settings_model::shortcuts::{
    Command, FocusDirection, FontStep, KeyLike, ReservedUse, ShortcutAction, ShortcutConflict, ShortcutGroup,
    ShortcutOverrides, SplitDirection, shortcut_conflict, shortcut_for, shortcut_label,
};

fn key(k: &str) -> KeyLike<'_> {
    KeyLike {
        key: k,
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    }
}
fn ctrl(k: &str) -> KeyLike<'_> {
    KeyLike { ctrl: true, ..key(k) }
}
fn alt(k: &str) -> KeyLike<'_> {
    KeyLike { alt: true, ..key(k) }
}
fn ctrl_alt(k: &str) -> KeyLike<'_> {
    KeyLike {
        ctrl: true,
        alt: true,
        ..key(k)
    }
}
fn ctrl_shift(k: &str) -> KeyLike<'_> {
    KeyLike {
        ctrl: true,
        shift: true,
        ..key(k)
    }
}
fn defaults(k: &KeyLike<'_>) -> Option<Command> {
    shortcut_for(k, &ShortcutOverrides::new())
}
fn overrides(entries: &[(ShortcutAction, &str)]) -> ShortcutOverrides {
    let mut o = ShortcutOverrides::new();
    for (action, shortcut) in entries {
        o.set(*action, *shortcut);
    }
    o
}

// it("maps the Ctrl+Shift shortcuts, whatever the letter case")
#[test]
fn maps_the_ctrl_shift_shortcuts_whatever_the_letter_case() {
    assert_eq!(defaults(&ctrl_shift("T")), Some(Command::NewTerminal));
    assert_eq!(defaults(&ctrl_shift("S")), Some(Command::QuickConnect));
    assert_eq!(defaults(&ctrl_shift("W")), Some(Command::ClosePane));
    assert_eq!(defaults(&ctrl_shift("D")), Some(Command::Split(SplitDirection::Row)));
    assert_eq!(defaults(&ctrl_shift("e")), Some(Command::Split(SplitDirection::Column)));
    assert_eq!(defaults(&ctrl_shift("M")), Some(Command::ToggleMultiExec));
}

// it("maps Alt+Arrow to focus moves")
#[test]
fn maps_alt_arrow_to_focus_moves() {
    assert_eq!(defaults(&alt("ArrowLeft")), Some(Command::Focus(FocusDirection::Left)));
    assert_eq!(
        defaults(&alt("ArrowRight")),
        Some(Command::Focus(FocusDirection::Right))
    );
    assert_eq!(defaults(&alt("ArrowUp")), Some(Command::Focus(FocusDirection::Up)));
    assert_eq!(defaults(&alt("ArrowDown")), Some(Command::Focus(FocusDirection::Down)));
}

// it("maps find, copy and paste (S15.1, S15.3)")
#[test]
fn maps_find_copy_and_paste() {
    assert_eq!(defaults(&ctrl_shift("F")), Some(Command::Find));
    assert_eq!(defaults(&ctrl_shift("c")), Some(Command::Copy));
    assert_eq!(defaults(&ctrl_shift("V")), Some(Command::Paste));
}

// it("maps Ctrl+= / Ctrl++ / Ctrl+- / Ctrl+0 to font size changes (S15.2)")
#[test]
fn maps_ctrl_equals_plus_minus_zero_to_font_size_changes() {
    assert_eq!(defaults(&ctrl("=")), Some(Command::FontSize(FontStep::Increase)));
    assert_eq!(defaults(&ctrl_shift("+")), Some(Command::FontSize(FontStep::Increase)));
    assert_eq!(defaults(&ctrl("+")), Some(Command::FontSize(FontStep::Increase)));
    assert_eq!(defaults(&ctrl("-")), Some(Command::FontSize(FontStep::Decrease)));
    assert_eq!(defaults(&ctrl("0")), Some(Command::FontSize(FontStep::Reset)));
    assert_eq!(defaults(&ctrl_alt("0")), None);
    assert_eq!(defaults(&alt("-")), None);
    // KEY-1: the Shift variants some layouts need.
    assert_eq!(defaults(&ctrl_shift("=")), Some(Command::FontSize(FontStep::Increase)));
    assert_eq!(defaults(&ctrl_shift("-")), Some(Command::FontSize(FontStep::Decrease)));
    assert_eq!(defaults(&ctrl_shift("0")), Some(Command::FontSize(FontStep::Reset)));
}

// it("maps Ctrl+Shift+R to recording and Ctrl+Alt+1..9 to macro hotkeys (S16.2, S16.4)")
#[test]
fn maps_ctrl_shift_r_to_recording_and_ctrl_alt_digits_to_macro_hotkeys() {
    assert_eq!(defaults(&ctrl_shift("r")), Some(Command::RecordMacro));
    assert_eq!(defaults(&ctrl_alt("1")), Some(Command::PlayMacro(1)));
    assert_eq!(defaults(&ctrl_alt("9")), Some(Command::PlayMacro(9)));
    // Ctrl+Alt+0 is not a hotkey, and Ctrl+Alt+Shift+1 belongs to the terminal.
    assert_eq!(defaults(&ctrl_alt("0")), None);
    assert_eq!(
        defaults(&KeyLike {
            shift: true,
            ..ctrl_alt("!")
        }),
        None
    );
    // A character typed with AltGr (reported as Ctrl+Alt on some systems) still reaches the terminal.
    assert_eq!(defaults(&ctrl_alt("@")), None);
}

// it("leaves every other key to the terminal")
#[test]
fn leaves_every_other_key_to_the_terminal() {
    assert_eq!(defaults(&ctrl("d")), None); // Ctrl+D is EOF for the shell
    assert_eq!(defaults(&ctrl("c")), None); // Ctrl+C interrupts
    assert_eq!(defaults(&ctrl("v")), None);
    assert_eq!(defaults(&ctrl("1")), None);
    assert_eq!(defaults(&ctrl_shift("X")), None);
    assert_eq!(defaults(&key("ArrowLeft")), None);
    assert_eq!(defaults(&ctrl_alt("ArrowLeft")), None);
    assert_eq!(
        defaults(&KeyLike {
            alt: true,
            ..ctrl_shift("D")
        }),
        None
    );
}

// describe("sidebar shortcut") it("Ctrl+Shift+B folds or opens the sidebar")
#[test]
fn ctrl_shift_b_folds_or_opens_the_sidebar() {
    assert_eq!(defaults(&ctrl_shift("B")), Some(Command::ToggleSidebar));
    assert_eq!(defaults(&ctrl_shift("b")), Some(Command::ToggleSidebar));
    assert_eq!(defaults(&ctrl("b")), None);
}

// it("an override replaces every default combination of its action")
#[test]
fn an_override_replaces_every_default_combination_of_its_action() {
    let o = overrides(&[
        (ShortcutAction::NewTerminal, "Ctrl+Alt+N"),
        (ShortcutAction::FontIncrease, "Ctrl+Alt+="),
    ]);
    assert_eq!(shortcut_for(&ctrl_alt("n"), &o), Some(Command::NewTerminal));
    assert_eq!(shortcut_for(&ctrl_shift("T"), &o), None);
    assert_eq!(
        shortcut_for(&ctrl_alt("="), &o),
        Some(Command::FontSize(FontStep::Increase))
    );
    assert_eq!(shortcut_for(&ctrl("="), &o), None);
    assert_eq!(shortcut_for(&ctrl("+"), &o), None);
    // Other actions keep their defaults.
    assert_eq!(
        shortcut_for(&ctrl_shift("D"), &o),
        Some(Command::Split(SplitDirection::Row))
    );
}

// it("an empty override leaves the action unbound")
#[test]
fn an_empty_override_leaves_the_action_unbound() {
    let o = overrides(&[(ShortcutAction::ToggleSidebar, "")]);
    assert_eq!(shortcut_for(&ctrl_shift("B"), &o), None);
    assert_eq!(shortcut_label(ShortcutAction::ToggleSidebar, &o), "");
}

// it("actions without a default do nothing until bound")
#[test]
fn actions_without_a_default_do_nothing_until_bound() {
    assert!(ShortcutAction::NextTab.defaults().is_empty());
    assert_eq!(defaults(&ctrl("PageDown")), None);
    let o = overrides(&[(ShortcutAction::NextTab, "Ctrl+PageDown")]);
    assert_eq!(shortcut_for(&ctrl("PageDown"), &o), Some(Command::NextTab));
    let o = overrides(&[(ShortcutAction::OpenSettings, "Ctrl+Meta+1")]);
    assert_eq!(
        shortcut_for(
            &KeyLike {
                meta: true,
                ..ctrl("1")
            },
            &o
        ),
        Some(Command::OpenSettings)
    );
}

// it("keeps Meta combinations, AltGr characters and the macro keys out of the defaults")
#[test]
fn keeps_meta_combinations_altgr_characters_and_the_macro_keys_out_of_the_defaults() {
    assert_eq!(
        defaults(&KeyLike {
            meta: true,
            ..ctrl_shift("T")
        }),
        None
    );
    assert_eq!(
        defaults(&KeyLike {
            meta: true,
            ..ctrl("=")
        }),
        None
    );
    assert_eq!(
        defaults(&KeyLike {
            meta: true,
            ..ctrl_alt("1")
        }),
        None
    );
    let o = overrides(&[(ShortcutAction::NewTerminal, "Ctrl+Alt+N")]);
    assert_eq!(shortcut_for(&ctrl_alt("@"), &o), None);
    assert_eq!(shortcut_for(&ctrl_alt("3"), &o), Some(Command::PlayMacro(3)));
}

// it("shows the first effective combination of an action")
#[test]
fn shows_the_first_effective_combination_of_an_action() {
    let none = ShortcutOverrides::new();
    assert_eq!(shortcut_label(ShortcutAction::FontIncrease, &none), "Ctrl+=");
    let o = overrides(&[(ShortcutAction::ClosePane, "Alt+W")]);
    assert_eq!(shortcut_label(ShortcutAction::ClosePane, &o), "Alt+W");
    assert_eq!(shortcut_label(ShortcutAction::KeyGen, &none), "");
}

// it("names what already uses a combination")
#[test]
fn names_what_already_uses_a_combination() {
    use ShortcutAction::{ClosePane, CloseTab, FontIncrease, KeyGen, NewTerminal, PortScan};
    let none = ShortcutOverrides::new();
    let action = |a| Some(ShortcutConflict::Action(a));
    assert_eq!(
        shortcut_conflict("Ctrl+Shift+T", &none, Some(CloseTab), None),
        action(NewTerminal)
    );
    assert_eq!(shortcut_conflict("Ctrl+Shift+T", &none, Some(NewTerminal), None), None);
    let moved = overrides(&[(NewTerminal, "Alt+N")]);
    assert_eq!(shortcut_conflict("Ctrl+Shift+T", &moved, Some(CloseTab), None), None);
    assert_eq!(
        shortcut_conflict("Ctrl++", &none, Some(CloseTab), None),
        action(FontIncrease)
    );
    let keygen = overrides(&[(KeyGen, "Alt+K")]);
    assert_eq!(
        shortcut_conflict("Alt+K", &keygen, Some(PortScan), None),
        action(KeyGen)
    );
    assert_eq!(
        shortcut_conflict("Ctrl+Alt+4", &none, Some(CloseTab), None),
        Some(ShortcutConflict::Reserved(ReservedUse::PlayMacro))
    );
    assert_eq!(
        shortcut_conflict("Ctrl+ArrowRight", &none, Some(CloseTab), None),
        Some(ShortcutConflict::Reserved(ReservedUse::AcceptWord))
    );
    assert_eq!(
        shortcut_conflict("Ctrl+Shift+Space", &none, Some(CloseTab), Some("Ctrl+Shift+Space")),
        Some(ShortcutConflict::Assistant)
    );
    // Checking the assistant itself: the actions and fixed keys count.
    assert_eq!(shortcut_conflict("Ctrl+Shift+W", &none, None, None), action(ClosePane));
    let cleared = overrides(&[(ClosePane, "")]);
    assert_eq!(shortcut_conflict("Ctrl+Shift+W", &cleared, None, None), None);
}

// describe("Toggle DEV mode (S10)") it("is an unbound terminal action that answers to the combination it is given")
#[test]
fn toggle_dev_mode_is_an_unbound_terminal_action_that_answers_to_the_combination_it_is_given() {
    assert!(ShortcutAction::ToggleDevMode.defaults().is_empty());
    assert_eq!(ShortcutAction::ToggleDevMode.group(), ShortcutGroup::Terminal);
    assert_eq!(defaults(&ctrl_shift("y")), None);
    let o = overrides(&[(ShortcutAction::ToggleDevMode, "Ctrl+Shift+Y")]);
    assert_eq!(shortcut_for(&ctrl_shift("y"), &o), Some(Command::ToggleDevMode));
}

// Native: the action table keeps v0.2.15's ids, order and defaults (KEY-1, KEY-2, KEY-3).
#[test]
fn lists_every_action_with_its_v0_2_15_id_in_settings_order() {
    let ids: Vec<&str> = ShortcutAction::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(
        ids,
        [
            "newTerminal",
            "closePane",
            "closeTab",
            "nextTab",
            "previousTab",
            "splitRight",
            "splitDown",
            "focusLeft",
            "focusRight",
            "focusUp",
            "focusDown",
            "toggleMultiExec",
            "quickConnect",
            "newSerial",
            "newTelnet",
            "newMosh",
            "find",
            "copy",
            "paste",
            "selectAll",
            "clear",
            "saveOutput",
            "toggleLogging",
            "toggleDevMode",
            "recentFolders",
            "askAi",
            "fontIncrease",
            "fontDecrease",
            "fontReset",
            "toggleSidebar",
            "toggleDevColumn",
            "showSessions",
            "showSftp",
            "showExplorer",
            "showTunnels",
            "showMacros",
            "recordMacro",
            "keyGen",
            "portScan",
            "networkScan",
            "wakeOnLan",
            "toggleRemoteMonitor",
            "openSettings",
        ]
    );
    for action in ShortcutAction::ALL {
        assert_eq!(ShortcutAction::from_id(action.id()), Some(action));
    }
    assert_eq!(ShortcutAction::from_id("launchRocket"), None);
    assert_eq!(
        ShortcutAction::FontIncrease.defaults(),
        ["Ctrl+=", "Ctrl++", "Ctrl+Shift+=", "Ctrl+Shift++"]
    );
    assert_eq!(ShortcutAction::FontDecrease.defaults(), ["Ctrl+-", "Ctrl+Shift+-"]);
    assert_eq!(ShortcutAction::FontReset.defaults(), ["Ctrl+0", "Ctrl+Shift+0"]);
    let bound: Vec<&str> = ShortcutAction::ALL
        .iter()
        .filter(|a| !a.defaults().is_empty())
        .map(|a| a.id())
        .collect();
    assert_eq!(
        bound,
        [
            "newTerminal",
            "closePane",
            "splitRight",
            "splitDown",
            "focusLeft",
            "focusRight",
            "focusUp",
            "focusDown",
            "toggleMultiExec",
            "quickConnect",
            "find",
            "copy",
            "paste",
            "fontIncrease",
            "fontDecrease",
            "fontReset",
            "toggleSidebar",
            "recordMacro",
        ]
    );
}

// Native: overrides keep the order they were set in, and a new value keeps its place (JS object order).
#[test]
fn overrides_keep_their_insertion_order() {
    let mut o = overrides(&[(ShortcutAction::KeyGen, "Alt+K"), (ShortcutAction::CloseTab, "")]);
    o.set(ShortcutAction::KeyGen, "Alt+J");
    let entries: Vec<(&str, &str)> = o.iter().map(|(a, s)| (a.id(), s)).collect();
    assert_eq!(entries, [("keyGen", "Alt+J"), ("closeTab", "")]);
    o.remove(ShortcutAction::KeyGen);
    assert_eq!(o.get(ShortcutAction::KeyGen), None);
    assert_eq!(o.get(ShortcutAction::CloseTab), Some(""));
}
