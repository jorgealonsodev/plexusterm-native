//! Dialog keyboard handling: Enter, Escape and the Tab focus trap (PRD §8.5, NFR-7).

use gpui::Keystroke;

/// Dialog widths of §8.4 (unscaled px).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogWidth {
    /// 440.
    #[default]
    Default,
    /// 640: macro editor.
    Wide,
    /// 680: settings.
    Settings,
}

impl DialogWidth {
    pub const fn value(self) -> f32 {
        match self {
            Self::Default => 440.0,
            Self::Wide => 640.0,
            Self::Settings => 680.0,
        }
    }
}

/// `.dialog { padding: 20px; gap: 12px }`, `.overlay { padding: 16px }`.
pub const DIALOG_PADDING: f32 = 20.0;
pub const DIALOG_GAP: f32 = 12.0;
pub const OVERLAY_PADDING: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKey {
    Enter,
    Escape,
    Tab,
    ShiftTab,
}

pub fn dialog_key(keystroke: &Keystroke) -> Option<DialogKey> {
    let m = &keystroke.modifiers;
    let plain = !m.modified();
    let only_shift = m.shift && !m.control && !m.alt && !m.platform && !m.function;
    match keystroke.key.as_str() {
        "enter" if plain => Some(DialogKey::Enter),
        "escape" if plain => Some(DialogKey::Escape),
        "tab" if plain => Some(DialogKey::Tab),
        "tab" if only_shift => Some(DialogKey::ShiftTab),
        _ => None,
    }
}

/// Where the focus is inside the dialog's trap order: content focusables first, then buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapLayout {
    pub content: usize,
    pub buttons: usize,
    /// The button Enter presses when the focus is not on a button (the primary one).
    pub default_button: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogCommand {
    /// Press button `n` (index among the buttons).
    Press(usize),
    Cancel,
    /// Move the focus to trap position `n`.
    Focus(usize),
    Ignore,
}

/// What `key` does with the focus at trap position `focused` (`None`: focus is elsewhere).
pub fn dialog_command(key: DialogKey, focused: Option<usize>, layout: TrapLayout) -> DialogCommand {
    let total = layout.content + layout.buttons;
    let focused = focused.filter(|&i| i < total);
    match key {
        DialogKey::Escape => DialogCommand::Cancel,
        DialogKey::Enter => match focused {
            Some(i) if i >= layout.content => DialogCommand::Press(i - layout.content),
            _ => layout
                .default_button
                .filter(|&b| b < layout.buttons)
                .map_or(DialogCommand::Ignore, DialogCommand::Press),
        },
        _ if total == 0 => DialogCommand::Ignore,
        DialogKey::Tab => DialogCommand::Focus(focused.map_or(0, |i| (i + 1) % total)),
        DialogKey::ShiftTab => DialogCommand::Focus(focused.map_or(total - 1, |i| (i + total - 1) % total)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap()
    }

    // One text field, then [Cancel, OK] with OK the default.
    const LAYOUT: TrapLayout = TrapLayout {
        content: 1,
        buttons: 2,
        default_button: Some(1),
    };

    #[test]
    fn widths_follow_section_8_4() {
        assert_eq!(DialogWidth::Default.value(), 440.0);
        assert_eq!(DialogWidth::Wide.value(), 640.0);
        assert_eq!(DialogWidth::Settings.value(), 680.0);
    }

    #[test]
    fn keys_map_to_dialog_keys() {
        assert_eq!(dialog_key(&key("enter")), Some(DialogKey::Enter));
        assert_eq!(dialog_key(&key("escape")), Some(DialogKey::Escape));
        assert_eq!(dialog_key(&key("tab")), Some(DialogKey::Tab));
        assert_eq!(dialog_key(&key("shift-tab")), Some(DialogKey::ShiftTab));
        assert_eq!(dialog_key(&key("ctrl-enter")), None);
        assert_eq!(dialog_key(&key("x")), None);
    }

    #[test]
    fn enter_presses_the_focused_button() {
        assert_eq!(
            dialog_command(DialogKey::Enter, Some(1), LAYOUT),
            DialogCommand::Press(0)
        );
        assert_eq!(
            dialog_command(DialogKey::Enter, Some(2), LAYOUT),
            DialogCommand::Press(1)
        );
    }

    #[test]
    fn enter_in_the_content_presses_the_default_button() {
        assert_eq!(
            dialog_command(DialogKey::Enter, Some(0), LAYOUT),
            DialogCommand::Press(1)
        );
        assert_eq!(dialog_command(DialogKey::Enter, None, LAYOUT), DialogCommand::Press(1));
        let no_default = TrapLayout {
            default_button: None,
            ..LAYOUT
        };
        assert_eq!(
            dialog_command(DialogKey::Enter, Some(0), no_default),
            DialogCommand::Ignore
        );
    }

    #[test]
    fn escape_cancels_from_anywhere() {
        assert_eq!(
            dialog_command(DialogKey::Escape, Some(2), LAYOUT),
            DialogCommand::Cancel
        );
        assert_eq!(dialog_command(DialogKey::Escape, None, LAYOUT), DialogCommand::Cancel);
    }

    #[test]
    fn tab_cycles_inside_the_dialog() {
        assert_eq!(dialog_command(DialogKey::Tab, Some(0), LAYOUT), DialogCommand::Focus(1));
        assert_eq!(dialog_command(DialogKey::Tab, Some(2), LAYOUT), DialogCommand::Focus(0));
        assert_eq!(dialog_command(DialogKey::Tab, None, LAYOUT), DialogCommand::Focus(0));
        assert_eq!(
            dialog_command(DialogKey::ShiftTab, Some(0), LAYOUT),
            DialogCommand::Focus(2)
        );
        assert_eq!(
            dialog_command(DialogKey::ShiftTab, None, LAYOUT),
            DialogCommand::Focus(2)
        );
        let empty = TrapLayout {
            content: 0,
            buttons: 0,
            default_button: None,
        };
        assert_eq!(dialog_command(DialogKey::Tab, None, empty), DialogCommand::Ignore);
    }
}
