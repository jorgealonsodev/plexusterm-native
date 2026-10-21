//! Color schemes and the interface tokens derived from them.
//!
//! The whole window follows the active terminal scheme, exactly as PlexusTerm v0.2.15
//! (`uiTheme.ts`): panels and borders are layered from the background towards the text and
//! status colours come from the ANSI palette (PRD §8.1). The 57 built-in schemes are generated
//! data (PRD Appendix C, `scripts/gen_schemes.py`).

#[rustfmt::skip]
mod schemes;

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `#rrggbb` or `#rrggbbaa`; an alpha suffix is ignored, as `uiTheme.ts` does.
    pub fn parse(value: &str) -> Option<Self> {
        Rgba::parse(value).map(|c| Self::new(c.r, c.g, c.b))
    }
}

/// An sRGB colour with alpha (scheme selection backgrounds carry one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#rrggbb` (opaque) or `#rrggbbaa`.
    pub fn parse(value: &str) -> Option<Self> {
        let digits = value.strip_prefix('#')?;
        if !(digits.len() == 6 || digits.len() == 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| digits.get(i..i + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
        let alpha = if digits.len() == 8 { byte(6)? } else { 0xff };
        Some(Self::new(byte(0)?, byte(2)?, byte(4)?, alpha))
    }
}

/// A scheme's terminal colours; `ansi` holds black…white then brightBlack…brightWhite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub foreground: Rgb,
    pub cursor: Rgb,
    pub cursor_accent: Rgb,
    pub selection: Rgba,
    pub ansi: [Rgb; 16],
}

impl Palette {
    pub const fn red(&self) -> Rgb {
        self.ansi[1]
    }
    pub const fn green(&self) -> Rgb {
        self.ansi[2]
    }
    pub const fn yellow(&self) -> Rgb {
        self.ansi[3]
    }
    pub const fn blue(&self) -> Rgb {
        self.ansi[4]
    }
    pub const fn magenta(&self) -> Rgb {
        self.ansi[5]
    }
    pub const fn cyan(&self) -> Rgb {
        self.ansi[6]
    }
    pub const fn bright_blue(&self) -> Rgb {
        self.ansi[12]
    }
}

/// A built-in colour scheme; `dark` groups it in the Settings picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub palette: Palette,
}

/// The scheme used on first start and for unknown ids.
pub const DEFAULT_SCHEME_ID: &str = "catppuccin-mocha";

/// Every built-in scheme, in picker order.
pub fn schemes() -> &'static [Scheme] {
    &schemes::SCHEMES
}

/// The scheme with this id, or the default one for an unknown id.
pub fn scheme_by_id(id: &str) -> &'static Scheme {
    let all = schemes();
    all.iter()
        .find(|s| s.id == id)
        .or_else(|| all.iter().find(|s| s.id == DEFAULT_SCHEME_ID))
        .unwrap_or(&schemes::SCHEMES[1])
}

/// `weight` of the way from `from` to `to`, per channel, rounded like JavaScript's `Math.round`.
pub fn mix(from: Rgb, to: Rgb, weight: f64) -> Rgb {
    let channel = |a: u8, b: u8| {
        let (a, b) = (f64::from(a), f64::from(b));
        // `Math.round` rounds halves up; every value here is within 0..=255, so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rounded = (a + (b - a) * weight + 0.5).floor().clamp(0.0, 255.0) as u8;
        rounded
    };
    Rgb::new(channel(from.r, to.r), channel(from.g, to.g), channel(from.b, to.b))
}

/// Rec. 709 weights on the non-linearised sRGB channels, as `uiTheme.ts` computes it (not WCAG).
fn luminance(c: Rgb) -> f64 {
    let unit = |v: u8| f64::from(v) / 255.0;
    0.2126 * unit(c.r) + 0.7152 * unit(c.g) + 0.0722 * unit(c.b)
}

/// Black or white, whichever reads better on `background`.
pub fn readable_on(background: Rgb) -> Rgb {
    if luminance(background) > 0.4 {
        Rgb::new(0, 0, 0)
    } else {
        Rgb::new(0xff, 0xff, 0xff)
    }
}

/// Git status colours of the file lists (VS Code's `gitDecoration.*` defaults).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GitColors {
    pub added: Rgb,
    pub modified: Rgb,
    pub deleted: Rgb,
    pub renamed: Rgb,
    pub untracked: Rgb,
    pub ignored: Rgb,
    pub stage_modified: Rgb,
    pub stage_deleted: Rgb,
    pub conflicting: Rgb,
}

/// Code colours of the file viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntaxColors {
    pub keyword: Rgb,
    pub string: Rgb,
    pub number: Rgb,
    pub function: Rgb,
    pub type_: Rgb,
    pub tag: Rgb,
    pub property: Rgb,
    pub comment: Rgb,
}

/// The interface colours of the whole window for one scheme (PRD §8.1 THEME-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiTokens {
    pub dark: bool,
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface_2: Rgb,
    pub border: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub on_warning: Rgb,
    pub danger: Rgb,
    pub warning: Rgb,
    pub success: Rgb,
    pub syntax: SyntaxColors,
    pub git: GitColors,
}

fn git_colors(dark: bool) -> GitColors {
    let pick = |on_dark: Rgb, on_light: Rgb| if dark { on_dark } else { on_light };
    GitColors {
        added: pick(Rgb::new(0x81, 0xb8, 0x8b), Rgb::new(0x58, 0x7c, 0x0c)),
        modified: pick(Rgb::new(0xe2, 0xc0, 0x8d), Rgb::new(0x89, 0x55, 0x03)),
        deleted: pick(Rgb::new(0xc7, 0x4e, 0x39), Rgb::new(0xad, 0x07, 0x07)),
        renamed: pick(Rgb::new(0x73, 0xc9, 0x91), Rgb::new(0x00, 0x71, 0x00)),
        untracked: pick(Rgb::new(0x73, 0xc9, 0x91), Rgb::new(0x00, 0x71, 0x00)),
        ignored: pick(Rgb::new(0x8c, 0x8c, 0x8c), Rgb::new(0x8e, 0x8e, 0x90)),
        stage_modified: pick(Rgb::new(0xe2, 0xc0, 0x8d), Rgb::new(0x89, 0x55, 0x03)),
        stage_deleted: pick(Rgb::new(0xc7, 0x4e, 0x39), Rgb::new(0xad, 0x07, 0x07)),
        conflicting: pick(Rgb::new(0xe4, 0x67, 0x6b), Rgb::new(0xad, 0x07, 0x07)),
    }
}

/// Derives the interface colours of the window from a scheme.
pub fn ui_tokens(scheme: &Scheme) -> UiTokens {
    let p = &scheme.palette;
    let (bg, fg) = (p.background, p.foreground);
    UiTokens {
        dark: scheme.dark,
        bg,
        surface: mix(bg, fg, 0.04),
        surface_2: mix(bg, fg, 0.08),
        border: mix(bg, fg, 0.16),
        text: fg,
        muted: mix(bg, fg, 0.6),
        accent: p.blue(),
        on_accent: readable_on(p.blue()),
        on_warning: readable_on(p.yellow()),
        danger: p.red(),
        warning: p.yellow(),
        success: p.green(),
        syntax: SyntaxColors {
            keyword: p.magenta(),
            string: p.green(),
            number: p.yellow(),
            function: p.blue(),
            type_: p.cyan(),
            tag: p.red(),
            property: p.bright_blue(),
            comment: mix(bg, fg, 0.6),
        },
        git: git_colors(scheme.dark),
    }
}
