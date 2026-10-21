//! PlexusTerm widgets on GPUI primitives (PRD §6.1 `ui-kit`, D4: own crate on the pinned Zed
//! rev; nothing is copied from Zed's GPL crates, D5).
//!
//! - [`theme`]: the ui-theme tokens as GPUI values ([`Theme`], radii, shadows, type scale,
//!   `ui_scale`, reduced motion). Set it with [`Theme::set_global`] before the first window.
//! - Widgets with the §8.5 states: [`Button`], [`TextInput`], [`Checkbox`], [`Radio`], [`Menu`]
//!   with [`ContextMenu`] and [`DropdownMenu`], [`Dialog`], [`HelpTip`]. Each one sets its
//!   AccessKit role and name (NFR-7) and works from the keyboard.
//! - [`Assets`]: the bundled Lucide icons; pass it to `Application::with_assets` (or chain it
//!   with [`assets::load_static`]).
//! - [`logic`]: the pure rules behind the widgets, unit-tested without a window.
//!
//! No widget animates: v0.2.15 has no transitions on these controls (§8.4 Motion), so
//! [`Theme::reduced_motion`] is only carried for the widgets that will.

pub mod assets;
pub mod button;
pub mod dialog;
pub mod help_tip;
pub mod logic;
pub mod menu;
pub mod text_input;
pub mod theme;
pub mod toggle;

pub use assets::{Assets, Icon};
pub use button::Button;
pub use dialog::{Dialog, DialogEvent};
pub use help_tip::HelpTip;
pub use logic::button::ButtonKind;
pub use logic::dialog::DialogWidth;
pub use menu::{ContextMenu, DropdownMenu, Menu, MenuEntry, MenuEvent, MenuItem};
pub use text_input::{TextInput, TextInputEvent};
pub use theme::{Elevation, Radius, TextSize, Theme};
pub use toggle::{Checkbox, Radio};
