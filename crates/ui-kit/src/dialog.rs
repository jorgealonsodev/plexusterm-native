//! `Dialog`: a modal over the `rgba(5,8,12,.6)` scrim (PRD §8.4 Dialogs, §8.5; NFR-7 role
//! `Dialog`, modal, focus trap).
//!
//! Enter presses the focused button (the default button when the focus is in the content),
//! Escape cancels, Tab and Shift+Tab cycle through the content's focusables and the buttons
//! without leaving the dialog.

use gpui::{
    AnyView, App, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Role, SharedString, Window,
    anchored, deferred, div, point, prelude::*, px, relative,
};

use crate::button::Button;
use crate::logic::button::ButtonKind;
use crate::logic::dialog::{
    DIALOG_GAP, DIALOG_PADDING, DialogCommand, DialogWidth, OVERLAY_PADDING, TrapLayout, dialog_command, dialog_key,
};
use crate::theme::{Elevation, LINE_HEIGHT, Radius, TextSize, Theme};

/// Dialogs paint above the window's content and below menus opened from them.
const DIALOG_PRIORITY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogEvent {
    /// Button `n` (in the order given) was pressed.
    Pressed(usize),
    /// Escape.
    Cancelled,
}

struct DialogButton {
    label: SharedString,
    kind: ButtonKind,
    focus: FocusHandle,
}

/// A modal dialog. The owner renders it as a child while it is open and drops it on an event.
pub struct Dialog {
    title: SharedString,
    width: DialogWidth,
    message: Option<SharedString>,
    body: Option<AnyView>,
    content_focus: Vec<FocusHandle>,
    buttons: Vec<DialogButton>,
    default_button: Option<usize>,
    focus: FocusHandle,
}

impl EventEmitter<DialogEvent> for Dialog {}

impl Focusable for Dialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Dialog {
    pub fn new(title: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            title: title.into(),
            width: DialogWidth::Default,
            message: None,
            body: None,
            content_focus: Vec::new(),
            buttons: Vec::new(),
            default_button: None,
            focus: cx.focus_handle(),
        }
    }

    pub fn width(mut self, width: DialogWidth) -> Self {
        self.width = width;
        self
    }

    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// A view shown under the message; `focusables` join the focus trap before the buttons.
    pub fn body(mut self, body: impl Into<AnyView>, focusables: Vec<FocusHandle>) -> Self {
        self.body = Some(body.into());
        self.content_focus = focusables;
        self
    }

    /// Adds a button; the first primary button becomes the default one.
    pub fn button(mut self, label: impl Into<SharedString>, kind: ButtonKind, cx: &mut Context<Self>) -> Self {
        if kind == ButtonKind::Primary && self.default_button.is_none() {
            self.default_button = Some(self.buttons.len());
        }
        self.buttons.push(DialogButton {
            label: label.into(),
            kind,
            focus: cx.focus_handle(),
        });
        self
    }

    /// Focuses the first content focusable, else the default button, else the first button.
    pub fn focus_initial(&self, window: &mut Window, cx: &mut App) {
        let target = self
            .content_focus
            .first()
            .or_else(|| self.default_button.and_then(|i| self.buttons.get(i)).map(|b| &b.focus))
            .or_else(|| self.buttons.first().map(|b| &b.focus))
            .unwrap_or(&self.focus);
        window.focus(target, cx);
    }

    fn trap(&self) -> impl Iterator<Item = &FocusHandle> {
        self.content_focus.iter().chain(self.buttons.iter().map(|b| &b.focus))
    }

    fn focused_index(&self, window: &Window, cx: &App) -> Option<usize> {
        let content = self.content_focus.len();
        self.trap().enumerate().find_map(|(i, handle)| {
            let inside = if i < content {
                handle.contains_focused(window, cx)
            } else {
                handle.is_focused(window)
            };
            inside.then_some(i)
        })
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = dialog_key(&event.keystroke) else {
            return;
        };
        let layout = TrapLayout {
            content: self.content_focus.len(),
            buttons: self.buttons.len(),
            default_button: self.default_button,
        };
        let command = dialog_command(key, self.focused_index(window, cx), layout);
        match command {
            DialogCommand::Ignore => return,
            DialogCommand::Press(i) => cx.emit(DialogEvent::Pressed(i)),
            DialogCommand::Cancel => cx.emit(DialogEvent::Cancelled),
            DialogCommand::Focus(i) => {
                if let Some(handle) = self.trap().nth(i).cloned() {
                    window.focus(&handle, cx);
                }
            }
        }
        // Handled here in the capture phase: the focused button must not also see Enter as a
        // keyboard click, and Tab must not move the focus out of the dialog.
        window.prevent_default();
        cx.stop_propagation();
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let c = theme.colors;
        let viewport = window.viewport_size();
        let buttons = self.buttons.iter().enumerate().map(|(i, b)| {
            Button::new(("dialog-button", i), b.label.clone())
                .kind(b.kind)
                .track_focus(&b.focus)
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(DialogEvent::Pressed(i))))
        });
        let panel = div()
            .id("dialog")
            .role(Role::Dialog)
            .aria_label(self.title.clone())
            .a11y_synthetic_children(|builder| builder.parent_node().set_modal())
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::on_key_down))
            .occlude()
            .flex()
            .flex_col()
            .w(px(self.width.value()))
            .max_w(viewport.width - px(2.0 * OVERLAY_PADDING))
            .p(theme.space(DIALOG_PADDING))
            .gap(theme.space(DIALOG_GAP))
            .bg(c.surface)
            .border(px(1.0))
            .border_color(c.border)
            .rounded(Radius::Dialog.px())
            .shadow(Elevation::Dialog.shadow())
            .text_color(c.text)
            .text_size(theme.text(TextSize::Base))
            .line_height(relative(LINE_HEIGHT))
            .child(
                div()
                    .id("dialog-title")
                    .role(Role::Heading)
                    .aria_level(2)
                    .aria_label(self.title.clone())
                    .text_size(theme.text(TextSize::Title))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title.clone()),
            )
            .when_some(self.message.clone(), |d, message| d.child(message))
            .when_some(self.body.clone(), |d, body| d.child(body))
            .when(!self.buttons.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(theme.space(8.0))
                        .children(buttons),
                )
            });
        let scrim = div()
            .id("dialog-scrim")
            .occlude()
            .w(viewport.width)
            .h(viewport.height)
            .p(px(OVERLAY_PADDING))
            .flex()
            .items_center()
            .justify_center()
            .bg(c.scrim)
            .child(panel);
        deferred(anchored().position(point(px(0.0), px(0.0))).child(scrim)).priority(DIALOG_PRIORITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_input::TextInput;
    use gpui::{Entity, Subscription, TestAppContext, VisualTestContext};

    struct Host {
        dialog: Entity<Dialog>,
        input: Entity<TextInput>,
        events: Vec<DialogEvent>,
        _sub: Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.dialog.clone())
        }
    }

    fn open(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
        let (host, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| TextInput::new("Name", cx));
            let input_focus = input.focus_handle(cx);
            let dialog = cx.new(|cx| {
                Dialog::new("Rename", cx)
                    .body(input.clone(), vec![input_focus])
                    .button("Cancel", ButtonKind::Default, cx)
                    .button("Rename", ButtonKind::Primary, cx)
            });
            let sub = cx.subscribe_in(&dialog, window, |host: &mut Host, _, e: &DialogEvent, _, _| {
                host.events.push(*e)
            });
            Host {
                dialog,
                input,
                events: Vec::new(),
                _sub: sub,
            }
        });
        let dialog = host.read_with(cx, |h, _| h.dialog.clone());
        cx.update(|window, cx| dialog.update(cx, |d, cx| d.focus_initial(window, cx)));
        cx.run_until_parked();
        (host, cx)
    }

    fn events(host: &Entity<Host>, cx: &mut VisualTestContext) -> Vec<DialogEvent> {
        host.read_with(cx, |h, _| h.events.clone())
    }

    #[gpui::test]
    fn enter_in_the_field_presses_the_default_button(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        cx.simulate_input("x");
        cx.simulate_keystrokes("enter");
        assert_eq!(events(&host, cx), vec![DialogEvent::Pressed(1)]);
        let text = host.read_with(cx, |h, cx| h.input.read(cx).text().to_owned());
        assert_eq!(text, "x");
    }

    #[gpui::test]
    fn tab_traps_the_focus_and_enter_presses_the_focused_button_once(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        cx.simulate_keystrokes("tab");
        cx.simulate_keystrokes("enter");
        assert_eq!(events(&host, cx), vec![DialogEvent::Pressed(0)], "Cancel, pressed once");
        cx.simulate_keystrokes("tab tab");
        let input = host.read_with(cx, |h, _| h.input.clone());
        let back_in_field = cx.update(|window, cx| input.focus_handle(cx).is_focused(window));
        assert!(back_in_field, "Tab after the last button wraps to the field");
        // Shift+Tab from the field wraps back to the last button, Rename.
        cx.simulate_keystrokes("shift-tab");
        cx.simulate_keystrokes("enter");
        assert_eq!(
            events(&host, cx),
            vec![DialogEvent::Pressed(0), DialogEvent::Pressed(1)]
        );
    }

    #[gpui::test]
    fn escape_cancels(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        cx.simulate_keystrokes("escape");
        assert_eq!(events(&host, cx), vec![DialogEvent::Cancelled]);
    }
}
