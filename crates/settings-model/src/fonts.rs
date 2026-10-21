//! The terminal font catalogue (PRD FONT-1, SET-3a), ported from v0.2.15
//! `src/features/settings/fontCatalog.ts`: the fonts shipped with PlexusTerm, which look the same
//! everywhere, and the well-known monospaced fonts found installed on this computer.
//!
//! v0.2.15 found installed fonts by measuring canvas text; here the caller enumerates the
//! system's font families and [`installed_fonts`] picks the candidates among them.

use crate::settings::DEFAULT_FONT_FAMILY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundledFont {
    pub name: &'static str,
    /// The value saved in the settings: the bundled family, then the generic one (kept as
    /// v0.2.15 writes it, so settings.json stays compatible).
    pub family: &'static str,
}

/// Shipped with the app: JetBrains Mono, Fira Code, Cascadia Code and Source Code Pro (SIL Open
/// Font License) and Hack (MIT and Bitstream Vera licenses).
pub const BUNDLED_FONTS: &[BundledFont] = &[
    BundledFont {
        name: "JetBrains Mono",
        family: "\"JetBrains Mono Variable\", monospace",
    },
    BundledFont {
        name: "Fira Code",
        family: "\"Fira Code Variable\", monospace",
    },
    BundledFont {
        name: "Cascadia Code",
        family: "\"Cascadia Code Variable\", monospace",
    },
    BundledFont {
        name: "Source Code Pro",
        family: "\"Source Code Pro Variable\", monospace",
    },
    BundledFont {
        name: "Hack",
        family: "\"Hack Web\", monospace",
    },
];

/// Monospaced families looked for on this computer, beyond the bundled ones, in picker order.
pub const INSTALLED_CANDIDATES: &[&str] = &[
    "Cascadia Mono",
    "Consolas",
    "Lucida Console",
    "Courier New",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "Ubuntu Mono",
    "Droid Sans Mono",
    "Fira Mono",
    "Inconsolata",
    "Iosevka",
    "IBM Plex Mono",
    "Roboto Mono",
    "Victor Mono",
    "Anonymous Pro",
    "Space Mono",
    "Go Mono",
    "Menlo",
    "Monaco",
    "SF Mono",
    "MesloLGS NF",
    "Hack Nerd Font",
    "JetBrainsMono Nerd Font",
    "FiraCode Nerd Font",
    "CaskaydiaCove Nerd Font",
    "Terminus",
];

/// The candidates among `system_families` (the families installed here), in catalogue order.
pub fn installed_fonts(system_families: &[String]) -> Vec<&'static str> {
    INSTALLED_CANDIDATES
        .iter()
        .copied()
        .filter(|name| system_families.iter().any(|f| f == name))
        .collect()
}

/// The catalogue entry a font family setting matches, or `""` when it was typed by hand.
pub fn catalog_choice(font_family: &str, installed: &[String]) -> String {
    let value = font_family.trim();
    if value == DEFAULT_FONT_FAMILY
        || BUNDLED_FONTS.iter().any(|font| font.family == value)
        || installed.iter().any(|name| name == value)
    {
        value.to_owned()
    } else {
        String::new()
    }
}
