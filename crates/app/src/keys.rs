//! Keystrokes to the bytes an xterm-compatible terminal expects (PRD TERM-1, TERM-8).
//!
//! Pure on purpose: the view hands over the key name, the typed character and the modifiers, so
//! the encoding is tested without a window. M0 limits: application cursor mode (DECCKM) and
//! application keypad are ignored (arrows always send `CSI A..D`, not `SS3 A..D`), and there is
//! no input method yet, so only characters the keyboard layout types directly arrive.

/// The modifier keys held with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Super / Windows / Command: never sent to the terminal.
    pub platform: bool,
}

/// What the app does with a keystroke before it reaches the terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Bytes for the program in the terminal.
    Send(Vec<u8>),
    /// Ctrl+Shift+V: paste the clipboard.
    Paste,
    /// Ctrl+Shift+C: copy the selection (M0 has no selection, so nothing is copied).
    Copy,
    /// Not a key the terminal takes.
    Ignore,
}

/// Maps a keystroke to what the terminal pane does with it.
///
/// `key` is the key's name (`"a"`, `"enter"`, `"f5"`, ...); `typed` is the character the layout
/// produces for it, when it produces one.
pub fn encode(key: &str, typed: Option<&str>, mods: Mods) -> KeyAction {
    if mods.platform {
        return KeyAction::Ignore;
    }
    if mods.ctrl && mods.shift && !mods.alt {
        match key {
            "v" => return KeyAction::Paste,
            "c" => return KeyAction::Copy,
            _ => {}
        }
    }
    match bytes_of(key, typed, mods) {
        Some(bytes) => KeyAction::Send(bytes),
        None => KeyAction::Ignore,
    }
}

/// xterm's modifier parameter (`1 + shift + 2·alt + 4·ctrl`), or `None` without modifiers.
fn modifier_param(mods: Mods) -> Option<u8> {
    let param = 1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.ctrl);
    (param > 1).then_some(param)
}

/// `CSI <final>` or, with modifiers, `CSI 1 ; <param> <final>` (cursor keys, Home, End).
fn csi_letter(letter: char, mods: Mods) -> Vec<u8> {
    match modifier_param(mods) {
        Some(param) => format!("\x1b[1;{param}{letter}").into_bytes(),
        None => format!("\x1b[{letter}").into_bytes(),
    }
}

/// `CSI <code> ~` or, with modifiers, `CSI <code> ; <param> ~` (Insert, Delete, PageUp, F5...).
fn csi_tilde(code: u8, mods: Mods) -> Vec<u8> {
    match modifier_param(mods) {
        Some(param) => format!("\x1b[{code};{param}~").into_bytes(),
        None => format!("\x1b[{code}~").into_bytes(),
    }
}

/// F1–F4 send `SS3 P..S`, or `CSI 1 ; <param> P..S` with modifiers.
fn ss3_function(letter: char, mods: Mods) -> Vec<u8> {
    match modifier_param(mods) {
        Some(param) => format!("\x1b[1;{param}{letter}").into_bytes(),
        None => format!("\x1bO{letter}").into_bytes(),
    }
}

fn with_alt(mods: Mods, mut bytes: Vec<u8>) -> Vec<u8> {
    if mods.alt {
        bytes.insert(0, 0x1b);
    }
    bytes
}

/// The control code Ctrl sends with `key`, as xterm does.
fn control_code(key: &str) -> Option<u8> {
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return (key == "space").then_some(0);
    };
    match c {
        'a'..='z' => Some(c as u8 & 0x1f),
        'A'..='Z' => Some(c.to_ascii_lowercase() as u8 & 0x1f),
        '@' | '2' => Some(0x00),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '/' => Some(0x1f),
        '8' | '?' => Some(0x7f),
        _ => None,
    }
}

fn bytes_of(key: &str, typed: Option<&str>, mods: Mods) -> Option<Vec<u8>> {
    let bytes = match key {
        "enter" => {
            // TERM-8: Shift+Enter alone sends a line feed.
            if mods.shift && !mods.ctrl && !mods.alt {
                return Some(b"\n".to_vec());
            }
            with_alt(mods, b"\r".to_vec())
        }
        "backspace" => with_alt(mods, if mods.ctrl { vec![0x08] } else { vec![0x7f] }),
        "tab" if mods.shift => b"\x1b[Z".to_vec(),
        "tab" => with_alt(mods, b"\t".to_vec()),
        "escape" => with_alt(mods, b"\x1b".to_vec()),
        "up" => csi_letter('A', mods),
        "down" => csi_letter('B', mods),
        "right" => csi_letter('C', mods),
        "left" => csi_letter('D', mods),
        "home" => csi_letter('H', mods),
        "end" => csi_letter('F', mods),
        "insert" => csi_tilde(2, mods),
        "delete" => csi_tilde(3, mods),
        "pageup" => csi_tilde(5, mods),
        "pagedown" => csi_tilde(6, mods),
        "f1" => ss3_function('P', mods),
        "f2" => ss3_function('Q', mods),
        "f3" => ss3_function('R', mods),
        "f4" => ss3_function('S', mods),
        "f5" => csi_tilde(15, mods),
        "f6" => csi_tilde(17, mods),
        "f7" => csi_tilde(18, mods),
        "f8" => csi_tilde(19, mods),
        "f9" => csi_tilde(20, mods),
        "f10" => csi_tilde(21, mods),
        "f11" => csi_tilde(23, mods),
        "f12" => csi_tilde(24, mods),
        _ if mods.ctrl => with_alt(mods, vec![control_code(key)?]),
        _ => {
            let text = match typed {
                Some(text) if !text.is_empty() => text.to_owned(),
                // Alt can leave no typed character; the key itself is then what was meant.
                _ if key == "space" => " ".to_owned(),
                _ if key.chars().count() == 1 => {
                    if mods.shift {
                        key.to_uppercase()
                    } else {
                        key.to_owned()
                    }
                }
                _ => return None,
            };
            with_alt(mods, text.into_bytes())
        }
    };
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        ctrl: false,
        alt: false,
        shift: false,
        platform: false,
    };
    const SHIFT: Mods = Mods { shift: true, ..NONE };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        ..NONE
    };

    fn sent(key: &str, typed: Option<&str>, mods: Mods) -> Vec<u8> {
        match encode(key, typed, mods) {
            KeyAction::Send(bytes) => bytes,
            other => panic!("{key} {mods:?} gave {other:?}"),
        }
    }

    #[test]
    fn printable_text_is_sent_as_typed() {
        assert_eq!(sent("a", Some("a"), NONE), b"a");
        assert_eq!(sent("a", Some("A"), SHIFT), b"A");
        assert_eq!(sent("1", Some("!"), NONE), b"!");
        assert_eq!(sent("space", Some(" "), NONE), b" ");
        assert_eq!(sent("e", Some("é"), NONE), "é".as_bytes());
    }

    #[test]
    fn enter_sends_cr_and_shift_enter_sends_lf() {
        assert_eq!(sent("enter", None, NONE), b"\r");
        // TERM-8: Shift+Enter alone sends LF; Ctrl/Alt+Enter do not.
        assert_eq!(sent("enter", None, SHIFT), b"\n");
        assert_eq!(sent("enter", None, CTRL), b"\r");
        assert_eq!(
            sent(
                "enter",
                None,
                Mods {
                    shift: true,
                    ctrl: true,
                    ..NONE
                }
            ),
            b"\r"
        );
        assert_eq!(sent("enter", None, ALT), b"\x1b\r");
    }

    #[test]
    fn editing_keys() {
        assert_eq!(sent("backspace", None, NONE), b"\x7f");
        assert_eq!(sent("backspace", None, CTRL), b"\x08");
        assert_eq!(sent("backspace", None, ALT), b"\x1b\x7f");
        assert_eq!(sent("tab", None, NONE), b"\t");
        assert_eq!(sent("tab", None, SHIFT), b"\x1b[Z");
        assert_eq!(sent("escape", None, NONE), b"\x1b");
        assert_eq!(sent("insert", None, NONE), b"\x1b[2~");
        assert_eq!(sent("delete", None, NONE), b"\x1b[3~");
        assert_eq!(sent("delete", None, CTRL), b"\x1b[3;5~");
    }

    #[test]
    fn cursor_keys() {
        assert_eq!(sent("up", None, NONE), b"\x1b[A");
        assert_eq!(sent("down", None, NONE), b"\x1b[B");
        assert_eq!(sent("right", None, NONE), b"\x1b[C");
        assert_eq!(sent("left", None, NONE), b"\x1b[D");
        assert_eq!(sent("home", None, NONE), b"\x1b[H");
        assert_eq!(sent("end", None, NONE), b"\x1b[F");
        assert_eq!(sent("pageup", None, NONE), b"\x1b[5~");
        assert_eq!(sent("pagedown", None, NONE), b"\x1b[6~");
        // xterm modifier parameter: 1 + shift + 2·alt + 4·ctrl.
        assert_eq!(sent("left", None, CTRL), b"\x1b[1;5D");
        assert_eq!(sent("right", None, SHIFT), b"\x1b[1;2C");
        assert_eq!(sent("up", None, ALT), b"\x1b[1;3A");
        assert_eq!(sent("end", None, CTRL_SHIFT), b"\x1b[1;6F");
        assert_eq!(sent("pageup", None, SHIFT), b"\x1b[5;2~");
    }

    #[test]
    fn function_keys() {
        let plain: [(&str, &[u8]); 12] = [
            ("f1", b"\x1bOP"),
            ("f2", b"\x1bOQ"),
            ("f3", b"\x1bOR"),
            ("f4", b"\x1bOS"),
            ("f5", b"\x1b[15~"),
            ("f6", b"\x1b[17~"),
            ("f7", b"\x1b[18~"),
            ("f8", b"\x1b[19~"),
            ("f9", b"\x1b[20~"),
            ("f10", b"\x1b[21~"),
            ("f11", b"\x1b[23~"),
            ("f12", b"\x1b[24~"),
        ];
        for (key, bytes) in plain {
            assert_eq!(sent(key, None, NONE), bytes, "{key}");
        }
        assert_eq!(sent("f1", None, SHIFT), b"\x1b[1;2P");
        assert_eq!(sent("f5", None, CTRL), b"\x1b[15;5~");
    }

    #[test]
    fn ctrl_letters_send_control_codes() {
        assert_eq!(sent("a", None, CTRL), b"\x01");
        assert_eq!(sent("c", None, CTRL), b"\x03");
        assert_eq!(sent("d", None, CTRL), b"\x04");
        assert_eq!(sent("z", None, CTRL), b"\x1a");
        assert_eq!(sent("space", None, CTRL), b"\x00");
        assert_eq!(sent("[", None, CTRL), b"\x1b");
        assert_eq!(sent("\\", None, CTRL), b"\x1c");
        assert_eq!(sent("]", None, CTRL), b"\x1d");
        assert_eq!(
            sent(
                "a",
                None,
                Mods {
                    ctrl: true,
                    alt: true,
                    ..NONE
                }
            ),
            b"\x1b\x01"
        );
    }

    #[test]
    fn alt_prefixes_escape() {
        assert_eq!(sent("b", Some("b"), ALT), b"\x1bb");
        assert_eq!(sent("b", None, ALT), b"\x1bb");
        assert_eq!(
            sent(
                "b",
                Some("B"),
                Mods {
                    alt: true,
                    shift: true,
                    ..NONE
                }
            ),
            b"\x1bB"
        );
        assert_eq!(sent("escape", None, ALT), b"\x1b\x1b");
    }

    #[test]
    fn clipboard_shortcuts_are_not_sent() {
        assert_eq!(encode("v", None, CTRL_SHIFT), KeyAction::Paste);
        assert_eq!(encode("c", None, CTRL_SHIFT), KeyAction::Copy);
        // Plain Ctrl+V / Ctrl+C still reach the program.
        assert_eq!(sent("v", None, CTRL), b"\x16");
        assert_eq!(sent("c", None, CTRL), b"\x03");
    }

    #[test]
    fn platform_modifier_and_unknown_keys_are_ignored() {
        assert_eq!(
            encode("a", Some("a"), Mods { platform: true, ..NONE }),
            KeyAction::Ignore
        );
        assert_eq!(encode("shift", None, SHIFT), KeyAction::Ignore);
        assert_eq!(encode("capslock", None, NONE), KeyAction::Ignore);
    }
}
