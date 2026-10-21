//! Menus: the item list ([`Menu`]), a pointer-placed [`ContextMenu`] and a [`DropdownMenu`]
//! under its button (PRD §8.4 Menus, §8.5 Menus; NFR-7 roles `Menu` and `MenuItem`).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseDownEvent, Pixels, Point,
    Role, SharedString, Subscription, Window, anchored, deferred, div, point, prelude::*, px, relative, svg,
};
use ui_theme::Rgb;

use crate::assets::Icon;
use crate::button::Button;
use crate::logic::menu::{CONTEXT_MENU_MIN_WIDTH, MENU_GAP, MenuNav, MenuOutcome, dropdown_min_width, menu_key};
use crate::theme::{Elevation, LINE_HEIGHT, Radius, TextSize, Theme, hsla};

/// Paint order of popups: menus above dialogs (both z 20 in v0.2.15, menus open later).
pub(crate) const MENU_PRIORITY: usize = 3;

/// One entry of a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: SharedString,
    pub disabled: bool,
    pub danger: bool,
    /// A 12×12 colour swatch before the label (tab colour menus).
    pub swatch: Option<Rgb>,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            danger: false,
            swatch: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn swatch(mut self, colour: Rgb) -> Self {
        self.swatch = Some(colour);
        self
    }
}

impl From<MenuItem> for MenuEntry {
    fn from(item: MenuItem) -> Self {
        Self::Item(item)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuEvent {
    /// Entry `n` (index in the entry list, separators included) was chosen.
    Activated(usize),
    /// Escape, or a press outside the menu at `pointer`.
    Dismissed { pointer: Option<Point<Pixels>> },
}

/// The list of a context or dropdown menu. It holds the keyboard focus while open; Up/Down,
/// Home/End move the highlight past separators and disabled items, Enter/Space choose, Escape
/// dismisses.
pub struct Menu {
    label: SharedString,
    entries: Vec<MenuEntry>,
    nav: MenuNav,
    focus: FocusHandle,
    /// Unscaled min width; scaled by `ui_scale` at render.
    min_width: f32,
}

impl EventEmitter<MenuEvent> for Menu {}

impl Focusable for Menu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Menu {
    /// `label` names the menu for assistive technology.
    pub fn new(label: impl Into<SharedString>, entries: Vec<MenuEntry>, cx: &mut Context<Self>) -> Self {
        Self {
            label: label.into(),
            entries,
            nav: MenuNav::default(),
            focus: cx.focus_handle(),
            min_width: CONTEXT_MENU_MIN_WIDTH,
        }
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.nav.highlighted
    }

    pub fn set_min_width(&mut self, width: f32) {
        self.min_width = width;
    }

    /// Focuses the menu; `highlight_first` for keyboard opening.
    pub fn open(&mut self, highlight_first: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.nav = MenuNav::default();
        if highlight_first {
            self.nav.handle(crate::logic::menu::MenuKey::Home, &self.selectable());
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn selectable(&self) -> Vec<bool> {
        self.entries
            .iter()
            .map(|e| matches!(e, MenuEntry::Item(item) if !item.disabled))
            .collect()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = menu_key(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        match self.nav.handle(key, &self.selectable()) {
            MenuOutcome::Moved => {}
            MenuOutcome::Activate(i) => cx.emit(MenuEvent::Activated(i)),
            MenuOutcome::Dismiss => cx.emit(MenuEvent::Dismissed { pointer: None }),
        }
        cx.notify();
    }
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let c = theme.colors;
        let size_of_set = self.entries.iter().filter(|e| matches!(e, MenuEntry::Item(_))).count();
        let mut position = 0;
        let mut children = Vec::with_capacity(self.entries.len());
        for (i, entry) in self.entries.iter().enumerate() {
            let MenuEntry::Item(item) = entry else {
                children.push(
                    div()
                        .id(("separator", i))
                        .h(px(1.0))
                        .mx(theme.space(6.0))
                        .my(theme.space(4.0))
                        .bg(c.border)
                        .into_any_element(),
                );
                continue;
            };
            position += 1;
            let highlighted = self.nav.highlighted == Some(i);
            let colour = if item.disabled {
                c.muted
            } else if item.danger {
                c.danger
            } else {
                c.text
            };
            let row = div()
                .id(("item", i))
                .role(Role::MenuItem)
                .aria_label(item.label.clone())
                .aria_position_in_set(position)
                .aria_size_of_set(size_of_set)
                .flex()
                .items_center()
                .gap(theme.space(8.0))
                .whitespace_nowrap()
                .px(theme.space(10.0))
                .py(theme.space(6.0))
                .rounded(Radius::Item.px())
                .text_color(colour)
                .when(highlighted, |d| d.bg(c.surface_2).aria_active_descendant())
                .when_some(item.swatch, |d, swatch| {
                    d.child(
                        div()
                            .flex_none()
                            .size(theme.space(12.0))
                            .rounded(Radius::Swatch.px())
                            .bg(hsla(swatch)),
                    )
                })
                .child(item.label.clone());
            let row = if item.disabled {
                row.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                    .into_any_element()
            } else {
                row.cursor_pointer()
                    .on_hover(cx.listener(move |menu, hovered: &bool, _, cx| {
                        if *hovered {
                            menu.nav.hover(i, &menu.selectable());
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(MenuEvent::Activated(i))))
                    .into_any_element()
            };
            children.push(row);
        }
        div()
            .id("menu")
            .role(Role::Menu)
            .aria_label(self.label.clone())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down_out(cx.listener(|_, event: &MouseDownEvent, _, cx| {
                cx.emit(MenuEvent::Dismissed {
                    pointer: Some(event.position),
                })
            }))
            .occlude()
            .flex()
            .flex_col()
            .min_w(theme.space(self.min_width))
            .p(theme.space(4.0))
            .bg(c.surface)
            .border(px(1.0))
            .border_color(c.border)
            .rounded(Radius::Popup.px())
            .shadow(Elevation::Menu.shadow())
            .text_color(c.text)
            .text_size(theme.text(TextSize::Base))
            .line_height(relative(LINE_HEIGHT))
            .children(children)
    }
}

/// A menu opened at the pointer, kept 4 px inside the window. Render it anywhere in the view
/// (it draws above everything) and call [`ContextMenu::deploy`] from a right-click handler.
pub struct ContextMenu {
    menu: Entity<Menu>,
    position: Option<Point<Pixels>>,
    restore_focus: Option<FocusHandle>,
    _sub: Subscription,
}

impl EventEmitter<MenuEvent> for ContextMenu {}

impl ContextMenu {
    pub fn new(
        label: impl Into<SharedString>,
        entries: Vec<MenuEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let menu = cx.new(|cx| Menu::new(label, entries, cx));
        let sub = cx.subscribe_in(&menu, window, |this: &mut Self, _, event: &MenuEvent, window, cx| {
            this.close(window, cx);
            cx.emit(*event);
        });
        Self {
            menu,
            position: None,
            restore_focus: None,
            _sub: sub,
        }
    }

    pub fn menu(&self) -> &Entity<Menu> {
        &self.menu
    }

    pub fn is_open(&self) -> bool {
        self.position.is_some()
    }

    /// Opens the menu at `position` (window coordinates) and moves the focus into it.
    pub fn deploy(&mut self, position: Point<Pixels>, keyboard: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_focus = window.focused(cx);
        self.position = Some(position);
        self.menu.update(cx, |menu, cx| menu.open(keyboard, window, cx));
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.position.take().is_some()
            && let Some(focus) = self.restore_focus.take()
        {
            window.focus(&focus, cx);
        }
        cx.notify();
    }
}

impl Render for ContextMenu {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().when_some(self.position, |d, position| {
            d.child(
                deferred(
                    anchored()
                        .position(position)
                        .snap_to_window_with_margin(px(MENU_GAP))
                        .child(self.menu.clone()),
                )
                .priority(MENU_PRIORITY),
            )
        })
    }
}

/// A button that opens its menu 4 px below itself, at least `max(200, button width)` wide.
pub struct DropdownMenu {
    label: SharedString,
    small: bool,
    menu: Entity<Menu>,
    open: bool,
    trigger_focus: FocusHandle,
    trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The press that dismissed the menu was on the trigger: its click must not reopen it.
    swallow_click: bool,
    _sub: Subscription,
}

impl EventEmitter<MenuEvent> for DropdownMenu {}

impl Focusable for DropdownMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.trigger_focus.clone()
    }
}

impl DropdownMenu {
    pub fn new(
        label: impl Into<SharedString>,
        entries: Vec<MenuEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let label = label.into();
        let menu = cx.new(|cx| Menu::new(label.clone(), entries, cx));
        let trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::default();
        let sub = cx.subscribe_in(&menu, window, |this: &mut Self, _, event: &MenuEvent, window, cx| {
            if let MenuEvent::Dismissed { pointer: Some(p) } = event {
                this.swallow_click = this.trigger_bounds.get().is_some_and(|b| b.contains(p));
            }
            this.close(window, cx);
            cx.emit(*event);
        });
        Self {
            label,
            small: false,
            menu,
            open: false,
            trigger_focus: cx.focus_handle(),
            trigger_bounds,
            swallow_click: false,
            _sub: sub,
        }
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn menu(&self) -> &Entity<Menu> {
        &self.menu
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, keyboard: bool, window: &mut Window, cx: &mut Context<Self>) {
        let width = self.trigger_bounds.get().map_or(0.0, |b| f32::from(b.size.width));
        self.open = true;
        self.menu.update(cx, |menu, cx| {
            let scale = Theme::of(cx).ui_scale;
            menu.set_min_width(dropdown_min_width(width, scale) / scale);
            menu.open(keyboard, window, cx);
        });
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            window.focus(&self.trigger_focus, cx);
        }
        cx.notify();
    }
}

impl Render for DropdownMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let anchor = self
            .trigger_bounds
            .get()
            .map(|b| point(b.left(), b.bottom() + px(MENU_GAP)));
        let button = Button::new("dropdown-trigger", self.label.clone())
            .track_focus(&self.trigger_focus)
            .expanded(self.open)
            .measure(self.trigger_bounds.clone())
            .when(self.small, Button::small)
            .trailing(
                svg()
                    .path(Icon::ChevronDown.path())
                    .size(px(12.0))
                    .text_color(theme.colors.muted),
            )
            .on_click(cx.listener(|this, event: &gpui::ClickEvent, window, cx| {
                if std::mem::take(&mut this.swallow_click) {
                    return;
                }
                if this.open {
                    this.close(window, cx);
                } else {
                    this.open(event.is_keyboard(), window, cx);
                }
            }));
        // A flex row so the button keeps its content width wherever the host places it.
        div()
            .flex()
            .flex_none()
            .child(button)
            .when_some(anchor.filter(|_| self.open), |d, anchor| {
                d.child(
                    deferred(
                        anchored()
                            .position(anchor)
                            .snap_to_window_with_margin(px(MENU_GAP))
                            .child(self.menu.clone()),
                    )
                    .priority(MENU_PRIORITY),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    struct Host {
        dropdown: Entity<DropdownMenu>,
        events: Vec<MenuEvent>,
        _sub: Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().p_4().child(self.dropdown.clone())
        }
    }

    fn entries() -> Vec<MenuEntry> {
        vec![
            MenuItem::new("Open").into(),
            MenuEntry::Separator,
            MenuItem::new("Locked").disabled(true).into(),
            MenuItem::new("Delete").danger().into(),
        ]
    }

    fn open(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
        let (host, cx) = cx.add_window_view(|window, cx| {
            let dropdown = cx.new(|cx| DropdownMenu::new("Actions", entries(), window, cx));
            let sub = cx.subscribe_in(&dropdown, window, |host: &mut Host, _, e: &MenuEvent, _, _| {
                host.events.push(*e)
            });
            Host {
                dropdown,
                events: Vec::new(),
                _sub: sub,
            }
        });
        cx.run_until_parked();
        (host, cx)
    }

    #[gpui::test]
    fn keyboard_opens_navigates_and_chooses(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        let dropdown = host.read_with(cx, |h, _| h.dropdown.clone());
        cx.update(|window, cx| {
            dropdown.update(cx, |d, cx| d.open(true, window, cx));
        });
        cx.run_until_parked();
        let menu = dropdown.read_with(cx, |d, _| d.menu().clone());
        assert_eq!(menu.read_with(cx, |m, _| m.highlighted()), Some(0));
        let width = menu.read_with(cx, |m, _| m.min_width);
        assert_eq!(width, 200.0, "a narrow button opens a 200 px list");
        cx.simulate_keystrokes("down");
        assert_eq!(
            menu.read_with(cx, |m, _| m.highlighted()),
            Some(3),
            "skips separator and disabled"
        );
        cx.simulate_keystrokes("enter");
        let (events, is_open) = host.read_with(cx, |h, cx| (h.events.clone(), h.dropdown.read(cx).is_open()));
        assert_eq!(events, vec![MenuEvent::Activated(3)]);
        assert!(!is_open);
        let trigger_focused = cx.update(|window, cx| dropdown.focus_handle(cx).is_focused(window));
        assert!(trigger_focused, "focus returns to the button");
    }

    #[gpui::test]
    fn escape_dismisses(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        let dropdown = host.read_with(cx, |h, _| h.dropdown.clone());
        cx.update(|window, cx| dropdown.update(cx, |d, cx| d.open(false, window, cx)));
        cx.simulate_keystrokes("escape");
        let events = host.read_with(cx, |h, _| h.events.clone());
        assert_eq!(events, vec![MenuEvent::Dismissed { pointer: None }]);
    }
}
