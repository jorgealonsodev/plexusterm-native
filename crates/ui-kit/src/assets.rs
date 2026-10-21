//! Bundled Lucide icons (PRD §8.6; ISC, see `icons/LICENSE-lucide.txt`).

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// The icons this crate draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// `CircleHelp`: the help tip.
    CircleHelp,
    /// `Check`: a ticked checkbox.
    Check,
    /// `ChevronDown`: dropdown buttons.
    ChevronDown,
}

const ICONS: [(Icon, &str, &[u8]); 3] = [
    (
        Icon::CircleHelp,
        "icons/circle-help.svg",
        include_bytes!("../icons/circle-help.svg"),
    ),
    (Icon::Check, "icons/check.svg", include_bytes!("../icons/check.svg")),
    (
        Icon::ChevronDown,
        "icons/chevron-down.svg",
        include_bytes!("../icons/chevron-down.svg"),
    ),
];

impl Icon {
    /// The asset path GPUI's `svg()` loads.
    pub fn path(self) -> &'static str {
        ICONS
            .iter()
            .find(|(icon, _, _)| *icon == self)
            .map_or("", |(_, path, _)| path)
    }
}

/// The SVG bytes of a bundled asset path, for apps that chain several asset sources.
pub fn load_static(path: &str) -> Option<&'static [u8]> {
    ICONS.iter().find(|(_, p, _)| *p == path).map(|(_, _, bytes)| *bytes)
}

/// The asset source to pass to `Application::with_assets` (or to chain from the app's own).
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(load_static(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(_, p, _)| p.starts_with(path))
            .map(|(_, p, _)| SharedString::from(*p))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_resolves_to_svg_bytes() {
        for icon in [Icon::CircleHelp, Icon::Check, Icon::ChevronDown] {
            let bytes = load_static(icon.path()).unwrap();
            assert!(bytes.starts_with(b"<svg"), "{icon:?}");
        }
        assert_eq!(Assets.list("icons/").unwrap().len(), 3);
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
    }
}
