//! The settings model of PlexusTerm, without any UI: settings.json with its 55 keys (PRD SET-1,
//! SET-2), the customisable keyboard shortcuts (KEY-1..KEY-5) and the terminal font catalogue
//! (FONT-1), compatible with PlexusTerm v0.2.15.

pub mod capture;
pub mod fonts;
pub mod json;
pub mod settings;
pub mod shortcuts;
pub mod storage;
