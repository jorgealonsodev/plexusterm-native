//! `Button` (PRD §8.5 COMP-1 Button; NFR-7 role `Button`).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ElementId, FocusHandle, FontWeight, IntoElement, Pixels, Role, SharedString,
    Toggled, Window, canvas, div, prelude::*, px, relative,
};

use crate::logic::button::{ButtonKind, ButtonState, button_look, button_metrics};
use crate::theme::{LINE_HEIGHT, Radius, Theme, hsla};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A push button: default, primary or danger, regular or small, with pressed, recording and
/// disabled states. Enter and Space press it while it has the keyboard focus.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: ButtonKind,
    small: bool,
    state: ButtonState,
    focus: Option<FocusHandle>,
    expanded: Option<bool>,
    trailing: Option<AnyElement>,
    on_click: Option<ClickHandler>,
    measure: Option<Rc<Cell<Option<Bounds<Pixels>>>>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: ButtonKind::Default,
            small: false,
            state: ButtonState::default(),
            focus: None,
            expanded: None,
            trailing: None,
            on_click: None,
            measure: None,
        }
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn primary(self) -> Self {
        self.kind(ButtonKind::Primary)
    }

    pub fn danger(self) -> Self {
        self.kind(ButtonKind::Danger)
    }

    /// `btn--small`: padding 2/8, 12 px.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// `btn--pressed`: a toggle that is on (exposed as toggled to assistive technology).
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.state.pressed = pressed;
        self
    }

    /// `btn--recording`.
    pub fn recording(mut self, recording: bool) -> Self {
        self.state.recording = recording;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    /// Uses the caller's focus handle, so it can focus the button or ask whether it is focused.
    pub fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    /// For a button that opens a popup: reported as expanded/collapsed.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// An element after the label (e.g. a dropdown chevron), with the 6 px `btn--icon` gap.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Stores the button's window bounds at every prepaint (popups anchor to it).
    pub fn measure(mut self, bounds: Rc<Cell<Option<Bounds<Pixels>>>>) -> Self {
        self.measure = Some(bounds);
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let look = button_look(&theme.tokens, self.kind, self.state);
        let (pad_y, pad_x, font) = button_metrics(self.small);
        let disabled = self.state.disabled;
        let (hover_bg, hover_border, hover_text) =
            (hsla(look.hover_bg), hsla(look.hover_border), hsla(look.hover_text));
        let focus_border = hsla(theme.tokens.accent);

        let base = div().id(self.id).role(Role::Button).aria_label(self.label.clone());
        let base = match &self.focus {
            Some(focus) => base.track_focus(focus),
            None => base.focusable(),
        };
        base.tab_stop(!disabled)
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.space(6.0))
            .whitespace_nowrap()
            .px(theme.space(pad_x))
            .py(theme.space(pad_y))
            .rounded(Radius::Control.px())
            .border(px(1.0))
            .border_color(hsla(look.border))
            .bg(hsla(look.bg))
            .text_color(hsla(look.text))
            .text_size(theme.space(font))
            .line_height(relative(LINE_HEIGHT))
            .opacity(look.opacity)
            .when(look.semibold, |d| d.font_weight(FontWeight::SEMIBOLD))
            .when(self.state.pressed, |d| d.aria_toggled(Toggled::True))
            .when_some(self.expanded, |d, expanded| d.aria_expanded(expanded))
            .when(disabled, |d| {
                d.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
            })
            .when(!disabled, |d| {
                d.cursor_pointer()
                    .hover(move |s| s.bg(hover_bg).border_color(hover_border).text_color(hover_text))
                    .focus_visible(move |s| s.border_color(focus_border))
            })
            .when_some(self.on_click.filter(|_| !disabled), |d, handler| {
                d.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .child(self.label)
            .children(self.trailing)
            .when_some(self.measure, |d, cell| {
                d.relative().child(
                    canvas(move |bounds, _, _| cell.set(Some(bounds)), |_, _, _, _| {})
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                )
            })
    }
}
