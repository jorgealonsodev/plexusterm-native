//! Menu keyboard navigation and sizing (PRD §8.4 Menus, §8.5 Menus).

use gpui::Keystroke;

/// `.context-menu { min-width: 190px }`.
pub const CONTEXT_MENU_MIN_WIDTH: f32 = 190.0;
/// Dropdown lists: `max(200, button width)`.
pub const DROPDOWN_MIN_WIDTH: f32 = 200.0;
/// Dropdown lists open 4 px below their button; context menus stay 4 px inside the window.
pub const MENU_GAP: f32 = 4.0;

/// The width a dropdown list opens at under a button `button_width` px wide (already scaled);
/// the 200 px floor is a chrome size and scales with `ui_scale` (PRD §8.3).
pub fn dropdown_min_width(button_width: f32, ui_scale: f32) -> f32 {
    button_width.max(DROPDOWN_MIN_WIDTH * ui_scale)
}

/// A key a focused menu understands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Home,
    End,
    Activate,
    Dismiss,
}

/// Maps a keystroke to a menu key; modified keys are left to the app.
pub fn menu_key(keystroke: &Keystroke) -> Option<MenuKey> {
    if keystroke.modifiers.modified() {
        return None;
    }
    Some(match keystroke.key.as_str() {
        "up" => MenuKey::Up,
        "down" => MenuKey::Down,
        "home" | "pageup" => MenuKey::Home,
        "end" | "pagedown" => MenuKey::End,
        "enter" | "space" => MenuKey::Activate,
        "escape" => MenuKey::Dismiss,
        _ => return None,
    })
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    /// The highlight may have moved; nothing to report.
    Moved,
    Activate(usize),
    Dismiss,
}

/// The highlighted entry of a menu. `selectable[i]` is false for separators and disabled items,
/// which the highlight skips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuNav {
    pub highlighted: Option<usize>,
}

impl MenuNav {
    pub fn handle(&mut self, key: MenuKey, selectable: &[bool]) -> MenuOutcome {
        let first = selectable.iter().position(|&s| s);
        let last = selectable.iter().rposition(|&s| s);
        match key {
            MenuKey::Down => {
                let next = self.highlighted.and_then(|i| {
                    selectable
                        .iter()
                        .enumerate()
                        .skip(i + 1)
                        .find_map(|(j, &s)| s.then_some(j))
                });
                self.highlighted = next.or(first);
            }
            MenuKey::Up => {
                let previous = self.highlighted.and_then(|i| {
                    selectable
                        .iter()
                        .enumerate()
                        .take(i)
                        .rev()
                        .find_map(|(j, &s)| s.then_some(j))
                });
                self.highlighted = previous.or(last);
            }
            MenuKey::Home => self.highlighted = first,
            MenuKey::End => self.highlighted = last,
            MenuKey::Activate => {
                if let Some(i) = self.highlighted
                    && selectable.get(i).copied().unwrap_or(false)
                {
                    return MenuOutcome::Activate(i);
                }
            }
            MenuKey::Dismiss => return MenuOutcome::Dismiss,
        }
        MenuOutcome::Moved
    }

    /// The pointer moved over entry `index`.
    pub fn hover(&mut self, index: usize, selectable: &[bool]) {
        if selectable.get(index).copied().unwrap_or(false) {
            self.highlighted = Some(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap()
    }

    // Open, separator, Disabled, Rename, Delete
    const ITEMS: [bool; 5] = [true, false, false, true, true];

    #[test]
    fn keys_map_to_menu_keys() {
        assert_eq!(menu_key(&key("up")), Some(MenuKey::Up));
        assert_eq!(menu_key(&key("down")), Some(MenuKey::Down));
        assert_eq!(menu_key(&key("home")), Some(MenuKey::Home));
        assert_eq!(menu_key(&key("end")), Some(MenuKey::End));
        assert_eq!(menu_key(&key("enter")), Some(MenuKey::Activate));
        assert_eq!(menu_key(&key("space")), Some(MenuKey::Activate));
        assert_eq!(menu_key(&key("escape")), Some(MenuKey::Dismiss));
        assert_eq!(menu_key(&key("ctrl-down")), None);
        assert_eq!(menu_key(&key("a")), None);
    }

    #[test]
    fn down_skips_separators_and_disabled_items_and_wraps() {
        let mut nav = MenuNav::default();
        assert_eq!(nav.handle(MenuKey::Down, &ITEMS), MenuOutcome::Moved);
        assert_eq!(nav.highlighted, Some(0));
        nav.handle(MenuKey::Down, &ITEMS);
        assert_eq!(nav.highlighted, Some(3));
        nav.handle(MenuKey::Down, &ITEMS);
        assert_eq!(nav.highlighted, Some(4));
        nav.handle(MenuKey::Down, &ITEMS);
        assert_eq!(nav.highlighted, Some(0));
    }

    #[test]
    fn up_from_nothing_goes_to_the_last_item_and_wraps() {
        let mut nav = MenuNav::default();
        nav.handle(MenuKey::Up, &ITEMS);
        assert_eq!(nav.highlighted, Some(4));
        nav.handle(MenuKey::Up, &ITEMS);
        nav.handle(MenuKey::Up, &ITEMS);
        assert_eq!(nav.highlighted, Some(0));
        nav.handle(MenuKey::Up, &ITEMS);
        assert_eq!(nav.highlighted, Some(4));
    }

    #[test]
    fn home_end_and_activation() {
        let mut nav = MenuNav::default();
        assert_eq!(nav.handle(MenuKey::Activate, &ITEMS), MenuOutcome::Moved);
        nav.handle(MenuKey::End, &ITEMS);
        assert_eq!(nav.highlighted, Some(4));
        assert_eq!(nav.handle(MenuKey::Activate, &ITEMS), MenuOutcome::Activate(4));
        nav.handle(MenuKey::Home, &ITEMS);
        assert_eq!(nav.highlighted, Some(0));
        assert_eq!(nav.handle(MenuKey::Dismiss, &ITEMS), MenuOutcome::Dismiss);
    }

    #[test]
    fn a_menu_without_selectable_items_highlights_nothing() {
        let mut nav = MenuNav::default();
        nav.handle(MenuKey::Down, &[false, false]);
        assert_eq!(nav.highlighted, None);
        assert_eq!(nav.handle(MenuKey::Activate, &[false, false]), MenuOutcome::Moved);
    }

    #[test]
    fn hover_highlights_only_selectable_items() {
        let mut nav = MenuNav::default();
        nav.hover(3, &ITEMS);
        assert_eq!(nav.highlighted, Some(3));
        nav.hover(2, &ITEMS);
        assert_eq!(nav.highlighted, Some(3));
        nav.hover(9, &ITEMS);
        assert_eq!(nav.highlighted, Some(3));
    }

    #[test]
    fn dropdown_width_is_at_least_200_or_the_button() {
        assert_eq!(dropdown_min_width(120.0, 1.0), 200.0);
        assert_eq!(dropdown_min_width(260.0, 1.0), 260.0);
        assert_eq!(dropdown_min_width(120.0, 1.5), 300.0);
        assert_eq!(CONTEXT_MENU_MIN_WIDTH, 190.0);
        assert_eq!(MENU_GAP, 4.0);
    }
}
