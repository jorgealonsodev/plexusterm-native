//! Shortcut names (PRD KEY-5), ported from v0.2.15 `src/features/settings/shortcutCapture.ts`:
//! `Ctrl+Alt+Shift+Meta+Key`, single characters upper-cased, the space bar `Space`, the plus key
//! written `++`.

use crate::shortcuts::{KeyLike, ShortcutOverrides, shortcut_conflict};

/// Modifier names, in the order a shortcut lists them.
const MODIFIERS: [&str; 4] = ["Ctrl", "Alt", "Shift", "Meta"];
/// Key names of the modifier keys themselves.
const MODIFIER_KEYS: [&str; 8] = ["Control", "Alt", "AltGraph", "Shift", "Meta", "OS", "Super", "Hyper"];

fn key_name(key: &str) -> String {
    if key == " " {
        "Space".to_owned()
    } else if key.encode_utf16().count() == 1 {
        key.to_uppercase()
    } else {
        key.to_owned()
    }
}

/// The shortcut a key press makes (`"Ctrl+Shift+Space"`), or `None` while only modifiers are held.
pub fn shortcut_from_event(e: &KeyLike<'_>) -> Option<String> {
    if e.key.is_empty() || MODIFIER_KEYS.contains(&e.key) {
        return None;
    }
    let held = [e.ctrl, e.alt, e.shift, e.meta];
    let mut parts: Vec<String> = MODIFIERS
        .iter()
        .zip(held)
        .filter(|(_, on)| *on)
        .map(|(m, _)| (*m).to_owned())
        .collect();
    parts.push(key_name(e.key));
    Some(parts.join("+"))
}

/// True when the key press is `shortcut`.
pub fn matches_shortcut(e: &KeyLike<'_>, shortcut: &str) -> bool {
    shortcut_from_event(e).as_deref() == Some(shortcut)
}

/// The modifiers and key of a shortcut name, or `None` when it is not one.
fn parse(shortcut: &str) -> Option<(Vec<&str>, &str)> {
    let (mods_text, key) = match shortcut.strip_suffix("++") {
        Some(rest) => (Some(rest), "+"),
        None => match shortcut.rsplit_once('+') {
            Some((mods, key)) => (Some(mods), key),
            None => (None, shortcut),
        },
    };
    if key.is_empty() || MODIFIER_KEYS.contains(&key) || MODIFIERS.contains(&key) {
        return None;
    }
    let mods: Vec<&str> = mods_text.map(|text| text.split('+').collect()).unwrap_or_default();
    let unique = mods
        .iter()
        .enumerate()
        .all(|(i, m)| !mods.iter().take(i).any(|seen| seen == m));
    (unique && mods.iter().all(|m| MODIFIERS.contains(m))).then_some((mods, key))
}

/// A shortcut with Ctrl, Alt or Meta and one key (plain keys and Shift+key belong to typing).
pub fn is_valid_shortcut(shortcut: &str) -> bool {
    parse(shortcut).is_some_and(|(mods, _)| mods.iter().any(|m| matches!(*m, "Ctrl" | "Alt" | "Meta")))
}

/// True when PlexusTerm already uses the combination with these overrides: any action's
/// binding, the macro keys or accept-word.
pub fn app_shortcut_conflict(shortcut: &str, overrides: &ShortcutOverrides) -> bool {
    parse(shortcut).is_some() && shortcut_conflict(shortcut, overrides, None, None).is_some()
}
