//! The ui-theme tokens as GPUI values (PRD §8.1, §8.3, §8.4).
//!
//! Widgets read the active [`Theme`] from the app global ([`Theme::of`]); the app sets it with
//! [`Theme::set_global`] before the first window opens (THEME-1) and again on a scheme change.

use std::sync::LazyLock;

use gpui::{App, BoxShadow, Global, Hsla, Pixels, Rgba, SharedString, point, px};
use ui_theme::{DEFAULT_SCHEME_ID, Rgb, UiTokens, scheme_by_id, ui_tokens};

/// CSS `line-height: normal` of the chrome, as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.2;

/// The interface font size that gives `ui_scale` 1 (PRD §8.3).
pub const BASE_UI_FONT_SIZE: f32 = 13.0;

/// `ui_scale = uiFontSize / 13`, with the font size clamped to 10–20 (PRD §8.3).
pub fn ui_scale(ui_font_size: f32) -> f32 {
    if !ui_font_size.is_finite() {
        return 1.0;
    }
    ui_font_size.clamp(10.0, 20.0) / BASE_UI_FONT_SIZE
}

/// An opaque ui-theme colour as a GPUI colour.
pub fn hsla(c: Rgb) -> Hsla {
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: 1.0,
    }
    .into()
}

/// A ui-theme colour at `alpha` (0–1) as a GPUI colour, e.g. `accent@25%`.
pub fn hsla_alpha(c: Rgb, alpha: f32) -> Hsla {
    let mut color = hsla(c);
    color.a = alpha.clamp(0.0, 1.0);
    color
}

/// CSS `filter: brightness(factor)` on one colour: every channel scaled and clamped.
pub fn brightness(c: Rgb, factor: f32) -> Rgb {
    let channel = |v: u8| {
        // The clamp keeps the value within 0..=255, so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let scaled = (f32::from(v) * factor).round().clamp(0.0, 255.0) as u8;
        scaled
    };
    Rgb::new(channel(c.r), channel(c.g), channel(c.b))
}

/// The family GPUI resolves to the platform's interface font.
pub const SYSTEM_UI_FONT: &str = ".SystemUIFont";

/// Desktop interface families tried after Inter, in the order of the FONT-1 stack (`system-ui`,
/// `"Segoe UI"`, `sans-serif`). GPUI's `.SystemUIFont` is IBM Plex Sans on Linux and is only
/// used when none of these is installed.
const DESKTOP_SANS: [&str; 7] = [
    "Segoe UI",
    "Cantarell",
    "Ubuntu",
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
];

/// "Inter" when it is installed, else an installed desktop sans, else the platform UI font;
/// nothing is bundled for the chrome (FONT-1).
pub fn ui_font_family<'a>(installed: impl IntoIterator<Item = &'a str>) -> &'static str {
    let installed: Vec<&str> = installed.into_iter().collect();
    std::iter::once("Inter")
        .chain(DESKTOP_SANS)
        .find(|family| installed.contains(family))
        .unwrap_or(SYSTEM_UI_FONT)
}

/// The corner radii of SHAPE-1, in unscaled px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radius {
    /// 3: menu swatches.
    Swatch,
    /// 4: tab × and +, badges, viewer header buttons.
    Small,
    /// 5: menu items, tree rows, file rows.
    Item,
    /// 6: buttons, inputs, help bubble, code blocks, fieldsets.
    Control,
    /// 8: context/dropdown menus, find bar, status picker, toasts.
    Popup,
    /// 10: dialogs.
    Dialog,
}

impl Radius {
    pub const fn value(self) -> f32 {
        match self {
            Self::Swatch => 3.0,
            Self::Small => 4.0,
            Self::Item => 5.0,
            Self::Control => 6.0,
            Self::Popup => 8.0,
            Self::Dialog => 10.0,
        }
    }

    /// Radii are not scaled by `ui_scale` (PRD §8.3).
    pub fn px(self) -> Pixels {
        px(self.value())
    }
}

/// The drop shadows of SHAPE-1: black at an alpha, `x y blur` in unscaled px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// `0 16 48 .45`.
    Dialog,
    /// `0 12 32 .45`: context menu, dropdown list, status picker.
    Menu,
    /// `0 8 24 .35`: model picker, find bar, share toast.
    Popover,
    /// `0 6 18 .35`: chat history.
    ChatHistory,
    /// `0 4 12 .30`: help tip bubble.
    HelpTip,
}

/// One shadow of SHAPE-1 in plain numbers, so the table is testable without GPUI types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSpec {
    pub y: f32,
    pub blur: f32,
    pub alpha: f32,
}

impl Elevation {
    pub const fn spec(self) -> ShadowSpec {
        let (y, blur, alpha) = match self {
            Self::Dialog => (16.0, 48.0, 0.45),
            Self::Menu => (12.0, 32.0, 0.45),
            Self::Popover => (8.0, 24.0, 0.35),
            Self::ChatHistory => (6.0, 18.0, 0.35),
            Self::HelpTip => (4.0, 12.0, 0.30),
        };
        ShadowSpec { y, blur, alpha }
    }

    /// Shadows are not scaled by `ui_scale` (PRD §8.3).
    pub fn shadow(self) -> Vec<BoxShadow> {
        let spec = self.spec();
        vec![BoxShadow {
            color: Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: spec.alpha,
            },
            offset: point(px(0.0), px(spec.y)),
            blur_radius: px(spec.blur),
            spread_radius: px(0.0),
            inset: false,
        }]
    }
}

/// The type scale of PRD §8.3, in unscaled px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSize {
    /// 9: tab log badge.
    Tiny,
    /// 10: sidebar tab labels, DEV footer button.
    Label,
    /// 11: badges, hints, pane footer.
    Small,
    /// 12: small buttons, help bubble, search.
    Ui,
    /// 13: base.
    Base,
    /// 14: DEV column rows, find-bar buttons.
    Row,
    /// 15: dialog h2, tab × and +.
    Title,
    /// 20: Seti icon.
    Icon,
}

impl TextSize {
    pub const fn value(self) -> f32 {
        match self {
            Self::Tiny => 9.0,
            Self::Label => 10.0,
            Self::Small => 11.0,
            Self::Ui => 12.0,
            Self::Base => 13.0,
            Self::Row => 14.0,
            Self::Title => 15.0,
            Self::Icon => 20.0,
        }
    }
}

/// The THEME-1 colours as GPUI colours, plus the fixed ones of §8.1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub bg: Hsla,
    pub surface: Hsla,
    pub surface_2: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub on_warning: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    /// `rgba(5,8,12,.6)` on every scheme.
    pub scrim: Hsla,
    /// `#b4232f`.
    pub recording_bg: Hsla,
}

/// The fixed dialog scrim of §8.1.
pub const SCRIM: Rgb = Rgb::new(5, 8, 12);
pub const SCRIM_ALPHA: f32 = 0.6;
/// The fixed recording banner background of §8.1.
pub const RECORDING_BG: Rgb = Rgb::new(0xb4, 0x23, 0x2f);

impl Colors {
    pub fn from_tokens(t: &UiTokens) -> Self {
        Self {
            bg: hsla(t.bg),
            surface: hsla(t.surface),
            surface_2: hsla(t.surface_2),
            border: hsla(t.border),
            text: hsla(t.text),
            muted: hsla(t.muted),
            accent: hsla(t.accent),
            on_accent: hsla(t.on_accent),
            on_warning: hsla(t.on_warning),
            danger: hsla(t.danger),
            warning: hsla(t.warning),
            success: hsla(t.success),
            scrim: hsla_alpha(SCRIM, SCRIM_ALPHA),
            recording_bg: hsla(RECORDING_BG),
        }
    }
}

/// Everything a widget needs to paint itself in the active scheme.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// The derived tokens, for pure style rules that work on [`Rgb`].
    pub tokens: UiTokens,
    pub colors: Colors,
    /// Scheme kind (§8.1): drives control appearance.
    pub dark: bool,
    /// `uiFontSize / 13` (§8.3).
    pub ui_scale: f32,
    /// OS or user reduced motion (§8.4, NFR-7): widgets skip every optional transition.
    pub reduced_motion: bool,
    /// FONT-1 family for the chrome.
    pub font_family: SharedString,
}

impl Global for Theme {}

static DEFAULT_THEME: LazyLock<Theme> =
    LazyLock::new(|| Theme::new(&ui_tokens(scheme_by_id(DEFAULT_SCHEME_ID)), BASE_UI_FONT_SIZE));

impl Theme {
    /// The theme of `tokens` at interface font size `ui_font_size`, on the platform UI font.
    pub fn new(tokens: &UiTokens, ui_font_size: f32) -> Self {
        Self {
            tokens: *tokens,
            colors: Colors::from_tokens(tokens),
            dark: tokens.dark,
            ui_scale: ui_scale(ui_font_size),
            reduced_motion: false,
            font_family: SYSTEM_UI_FONT.into(),
        }
    }

    /// The theme of a built-in scheme (unknown ids fall back to the default scheme).
    pub fn for_scheme(scheme_id: &str, ui_font_size: f32) -> Self {
        Self::new(&ui_tokens(scheme_by_id(scheme_id)), ui_font_size)
    }

    pub fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    pub fn with_font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Picks the FONT-1 family from the fonts installed on this system.
    pub fn with_installed_ui_font(self, cx: &App) -> Self {
        let names = cx.text_system().all_font_names();
        let family = ui_font_family(names.iter().map(String::as_str));
        self.with_font_family(family)
    }

    /// The active theme, or the default scheme's when the app has not set one.
    pub fn of(cx: &App) -> &Self {
        cx.try_global::<Self>().unwrap_or(&DEFAULT_THEME)
    }

    /// Installs this theme for every window; views re-read it on their next render.
    pub fn set_global(self, cx: &mut App) {
        cx.set_global(self);
        cx.refresh_windows();
    }

    /// A chrome length: `value × ui_scale` (§8.3). Borders, radii, shadows, icons and dialog
    /// widths are not scaled and use plain `px`.
    pub fn space(&self, value: f32) -> Pixels {
        px(value * self.ui_scale)
    }

    /// A type-scale size at the current `ui_scale`.
    pub fn text(&self, size: TextSize) -> Pixels {
        self.space(size.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scale_is_font_size_over_13_clamped_to_10_20() {
        assert_eq!(ui_scale(13.0), 1.0);
        assert_eq!(ui_scale(19.5), 1.5);
        assert_eq!(ui_scale(8.0), 10.0 / 13.0);
        assert_eq!(ui_scale(30.0), 20.0 / 13.0);
        assert_eq!(ui_scale(f32::NAN), 1.0);
    }

    #[test]
    fn hsla_matches_gpui_rgb() {
        let expected: Hsla = gpui::rgb(0x89b4fa).into();
        assert_eq!(hsla(Rgb::new(0x89, 0xb4, 0xfa)), expected);
    }

    #[test]
    fn brightness_scales_and_clamps_channels() {
        assert_eq!(brightness(Rgb::new(0x89, 0xb4, 0xfa), 1.1), Rgb::new(0x97, 0xc6, 0xff));
    }

    #[test]
    fn inter_is_used_only_when_installed() {
        assert_eq!(ui_font_family(["DejaVu Sans", "Inter"]), "Inter");
        assert_eq!(ui_font_family(["Some Font"]), ".SystemUIFont");
    }

    #[test]
    fn without_inter_an_installed_desktop_sans_is_used() {
        // GPUI's `.SystemUIFont` is IBM Plex Sans on Linux; when that is missing the fallback
        // face has no italic or semibold, so an installed desktop family is preferred.
        assert_eq!(ui_font_family(["DejaVu Sans", "Noto Sans"]), "Noto Sans");
        assert_eq!(ui_font_family(["DejaVu Sans"]), "DejaVu Sans");
    }

    #[test]
    fn radii_follow_shape_1() {
        let all = [
            Radius::Swatch,
            Radius::Small,
            Radius::Item,
            Radius::Control,
            Radius::Popup,
            Radius::Dialog,
        ];
        assert_eq!(all.map(Radius::value), [3.0, 4.0, 5.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn shadows_follow_shape_1() {
        let spec = |e: Elevation| {
            let s = e.spec();
            (s.y, s.blur, s.alpha)
        };
        assert_eq!(spec(Elevation::Dialog), (16.0, 48.0, 0.45));
        assert_eq!(spec(Elevation::Menu), (12.0, 32.0, 0.45));
        assert_eq!(spec(Elevation::Popover), (8.0, 24.0, 0.35));
        assert_eq!(spec(Elevation::ChatHistory), (6.0, 18.0, 0.35));
        assert_eq!(spec(Elevation::HelpTip), (4.0, 12.0, 0.30));
        let shadow = Elevation::Dialog.shadow();
        assert_eq!(shadow.len(), 1);
        assert_eq!(shadow[0].offset, point(px(0.0), px(16.0)));
        assert_eq!(shadow[0].blur_radius, px(48.0));
        assert_eq!(shadow[0].color.a, 0.45);
    }

    #[test]
    fn scaled_lengths_and_type_scale() {
        let theme = Theme::for_scheme(DEFAULT_SCHEME_ID, 19.5);
        assert_eq!(theme.space(10.0), px(15.0));
        assert_eq!(theme.text(TextSize::Ui), px(18.0));
        let base = Theme::for_scheme(DEFAULT_SCHEME_ID, 13.0);
        assert_eq!(base.text(TextSize::Base), px(13.0));
    }

    #[test]
    fn colours_come_from_the_tokens_and_scrim_is_fixed() {
        let mocha = Theme::for_scheme("catppuccin-mocha", 13.0);
        assert_eq!(mocha.colors.bg, hsla(Rgb::new(0x1e, 0x1e, 0x2e)));
        assert_eq!(mocha.colors.accent, hsla(Rgb::new(0x89, 0xb4, 0xfa)));
        assert!(mocha.dark);
        let latte = Theme::for_scheme("catppuccin-latte", 13.0);
        assert!(!latte.dark);
        assert_eq!(latte.colors.scrim, mocha.colors.scrim);
        assert_eq!(mocha.colors.scrim, hsla_alpha(Rgb::new(5, 8, 12), 0.6));
    }

    #[test]
    fn of_falls_back_to_the_default_scheme() {
        let app = gpui::TestAppContext::single();
        let theme = app.update(|cx| Theme::of(cx).clone());
        assert_eq!(theme, Theme::for_scheme(DEFAULT_SCHEME_ID, 13.0));
    }
}
