//! Button colours and metrics per state (PRD §8.5 COMP-1 Button).

use ui_theme::{Rgb, UiTokens};

use crate::theme::brightness;

/// The button variants of `ST/App.css`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonKind {
    #[default]
    Default,
    /// bg + border `accent`, text `on_accent`, 600, hover brightness 1.1.
    Primary,
    /// border + text `danger`, 600.
    Danger,
}

/// State flags that change a button's look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    /// `btn--pressed`: a toggle button that is on.
    pub pressed: bool,
    /// `btn--recording`.
    pub recording: bool,
    pub disabled: bool,
}

/// Resolved colours: at rest and under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonLook {
    pub bg: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub hover_bg: Rgb,
    pub hover_border: Rgb,
    pub hover_text: Rgb,
    pub semibold: bool,
    pub opacity: f32,
}

/// Disabled buttons (the later rule at `App.css:881`).
pub const DISABLED_OPACITY: f32 = 0.45;
/// `.btn--primary:hover { filter: brightness(1.1) }`.
pub const PRIMARY_HOVER_BRIGHTNESS: f32 = 1.1;

pub fn button_look(t: &UiTokens, kind: ButtonKind, state: ButtonState) -> ButtonLook {
    let (bg, mut border, mut text, semibold) = match kind {
        ButtonKind::Default => (t.surface_2, t.border, t.text, false),
        ButtonKind::Primary => (t.accent, t.accent, t.on_accent, true),
        ButtonKind::Danger => (t.surface_2, t.danger, t.danger, true),
    };
    if state.pressed {
        (border, text) = (t.accent, t.accent);
    }
    if state.recording {
        (border, text) = (t.danger, t.danger);
    }
    let (hover_bg, hover_border, hover_text) = if state.disabled {
        // `.btn:disabled:hover` keeps the resting colours.
        (bg, border, text)
    } else if kind == ButtonKind::Primary {
        let lighten = |c| brightness(c, PRIMARY_HOVER_BRIGHTNESS);
        (lighten(bg), lighten(t.accent), lighten(text))
    } else {
        (bg, t.accent, text)
    };
    ButtonLook {
        bg,
        border,
        text,
        hover_bg,
        hover_border,
        hover_text,
        semibold,
        opacity: if state.disabled { DISABLED_OPACITY } else { 1.0 },
    }
}

/// Unscaled padding and font size: `(padding_y, padding_x, font_size)`.
pub fn button_metrics(small: bool) -> (f32, f32, f32) {
    if small { (2.0, 8.0, 12.0) } else { (4.0, 10.0, 13.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui_theme::{scheme_by_id, ui_tokens};

    fn mocha() -> UiTokens {
        ui_tokens(scheme_by_id("catppuccin-mocha"))
    }

    #[test]
    fn default_button_is_surface_2_with_border_and_accent_hover() {
        let t = mocha();
        let look = button_look(&t, ButtonKind::Default, ButtonState::default());
        assert_eq!((look.bg, look.border, look.text), (t.surface_2, t.border, t.text));
        assert_eq!(look.hover_border, t.accent);
        assert_eq!((look.hover_bg, look.hover_text), (t.surface_2, t.text));
        assert!(!look.semibold);
        assert_eq!(look.opacity, 1.0);
    }

    #[test]
    fn primary_is_accent_on_accent_semibold_and_brightens_on_hover() {
        let t = mocha();
        let look = button_look(&t, ButtonKind::Primary, ButtonState::default());
        assert_eq!((look.bg, look.border, look.text), (t.accent, t.accent, t.on_accent));
        assert!(look.semibold);
        assert_eq!(look.hover_bg, brightness(t.accent, 1.1));
        assert_eq!(look.hover_border, brightness(t.accent, 1.1));
        assert_eq!(look.hover_text, brightness(t.on_accent, 1.1));
    }

    #[test]
    fn danger_is_danger_border_and_text_semibold() {
        let t = mocha();
        let look = button_look(&t, ButtonKind::Danger, ButtonState::default());
        assert_eq!((look.bg, look.border, look.text), (t.surface_2, t.danger, t.danger));
        assert!(look.semibold);
        assert_eq!(look.hover_border, t.accent);
    }

    #[test]
    fn pressed_and_recording_recolour_border_and_text() {
        let t = mocha();
        let pressed = ButtonState {
            pressed: true,
            ..ButtonState::default()
        };
        let look = button_look(&t, ButtonKind::Default, pressed);
        assert_eq!((look.border, look.text), (t.accent, t.accent));
        let recording = ButtonState {
            recording: true,
            ..ButtonState::default()
        };
        let look = button_look(&t, ButtonKind::Default, recording);
        assert_eq!((look.border, look.text), (t.danger, t.danger));
    }

    #[test]
    fn disabled_is_045_and_hover_keeps_the_border() {
        let t = mocha();
        let disabled = ButtonState {
            disabled: true,
            ..ButtonState::default()
        };
        let look = button_look(&t, ButtonKind::Default, disabled);
        assert_eq!(look.opacity, 0.45);
        assert_eq!(look.hover_border, look.border);
        assert_eq!(look.hover_bg, look.bg);
        let primary = button_look(&t, ButtonKind::Primary, disabled);
        assert_eq!(primary.hover_bg, primary.bg);
    }

    #[test]
    fn metrics_regular_and_small() {
        assert_eq!(button_metrics(false), (4.0, 10.0, 13.0));
        assert_eq!(button_metrics(true), (2.0, 8.0, 12.0));
    }
}
