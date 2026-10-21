//! Ported from PlexusTerm v0.2.15 `src/features/settings/shortcutCapture.test.ts` (PRD KEY-5).

use settings_model::capture::{app_shortcut_conflict, is_valid_shortcut, matches_shortcut, shortcut_from_event};
use settings_model::shortcuts::{KeyLike, ShortcutAction, ShortcutOverrides};

fn key(k: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> KeyLike<'_> {
    KeyLike {
        key: k,
        ctrl,
        shift,
        alt,
        meta,
    }
}

// describe("shortcutFromEvent") it("names a key combination")
#[test]
fn names_a_key_combination() {
    assert_eq!(
        shortcut_from_event(&key(" ", true, true, false, false)).as_deref(),
        Some("Ctrl+Shift+Space")
    );
    assert_eq!(
        shortcut_from_event(&key("k", false, false, true, false)).as_deref(),
        Some("Alt+K")
    );
    assert_eq!(
        shortcut_from_event(&key("F2", true, false, false, true)).as_deref(),
        Some("Ctrl+Meta+F2")
    );
    // KEY-5: modifiers always in the order Ctrl, Alt, Shift, Meta; the plus key is written "++".
    assert_eq!(
        shortcut_from_event(&key("+", true, true, true, true)).as_deref(),
        Some("Ctrl+Alt+Shift+Meta++")
    );
    assert!(matches_shortcut(&key("t", true, true, false, false), "Ctrl+Shift+T"));
}

// it("waits while only modifiers are held")
#[test]
fn waits_while_only_modifiers_are_held() {
    assert_eq!(shortcut_from_event(&key("Control", true, false, false, false)), None);
    assert_eq!(shortcut_from_event(&key("Shift", true, true, false, false)), None);
    assert_eq!(shortcut_from_event(&key("", true, false, false, false)), None);
}

// describe("isValidShortcut") it("needs Ctrl, Alt or Meta and one key")
#[test]
fn needs_ctrl_alt_or_meta_and_one_key() {
    assert!(is_valid_shortcut("Ctrl+Shift+Space"));
    assert!(!is_valid_shortcut("Shift+A"));
    assert!(!is_valid_shortcut("A"));
    assert!(!is_valid_shortcut("Ctrl+"));
    assert!(!is_valid_shortcut("Ctrl+Ctrl+A"));
    // The plus key itself.
    assert!(is_valid_shortcut("Ctrl++"));
    assert!(!is_valid_shortcut("++"));
    assert!(!is_valid_shortcut("Ctrl+Hyper+A"));
}

// describe("appShortcutConflict") it("refuses combinations PlexusTerm already uses")
#[test]
fn refuses_combinations_plexusterm_already_uses() {
    let none = ShortcutOverrides::new();
    assert!(app_shortcut_conflict("Ctrl+Shift+T", &none));
    assert!(app_shortcut_conflict("Ctrl+=", &none));
    assert!(app_shortcut_conflict("Ctrl+Alt+3", &none));
    assert!(app_shortcut_conflict("Alt+ArrowLeft", &none));
    assert!(app_shortcut_conflict("Ctrl+ArrowRight", &none));
    assert!(!app_shortcut_conflict("Ctrl+Shift+Space", &none));
    assert!(!app_shortcut_conflict("Alt+K", &none));
    // With overrides: a moved action frees its default.
    let mut moved = ShortcutOverrides::new();
    moved.set(ShortcutAction::NewTerminal, "Alt+K");
    assert!(!app_shortcut_conflict("Ctrl+Shift+T", &moved));
    assert!(app_shortcut_conflict("Alt+K", &moved));
}
