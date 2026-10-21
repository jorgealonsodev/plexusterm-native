//! `Checkbox` and `Radio` (PRD §8.5 Inputs: `accent-color: accent`; NFR-7 roles `CheckBox` and
//! `RadioButton`). They look like the platform controls of v0.2.15 with the accent tint.

use std::rc::Rc;

use gpui::{App, ElementId, IntoElement, Role, SharedString, Toggled, Window, div, prelude::*, px, relative, svg};

use crate::assets::Icon;
use crate::theme::{LINE_HEIGHT, Radius, TextSize, Theme};

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// The native control box: 13 px (not scaled, like icons).
const BOX: f32 = 13.0;
const DOT: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Check,
    Radio,
}

/// A labelled check box; a click, Space or Enter toggles it.
#[derive(IntoElement)]
pub struct Checkbox(Toggle);

/// A labelled radio button; activating it reports `true` (a group selects one).
#[derive(IntoElement)]
pub struct Radio(Toggle);

struct Toggle {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    shape: Shape,
    on_toggle: Option<ToggleHandler>,
}

macro_rules! toggle_builder {
    ($ty:ident, $shape:expr) => {
        impl $ty {
            pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
                Self(Toggle {
                    id: id.into(),
                    label: label.into(),
                    checked: false,
                    disabled: false,
                    shape: $shape,
                    on_toggle: None,
                })
            }

            pub fn checked(mut self, checked: bool) -> Self {
                self.0.checked = checked;
                self
            }

            pub fn disabled(mut self, disabled: bool) -> Self {
                self.0.disabled = disabled;
                self
            }

            /// Called with the new checked state.
            pub fn on_toggle(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
                self.0.on_toggle = Some(Rc::new(handler));
                self
            }
        }

        impl RenderOnce for $ty {
            fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
                self.0.render(window, cx)
            }
        }
    };
}

toggle_builder!(Checkbox, Shape::Check);
toggle_builder!(Radio, Shape::Radio);

impl Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let c = theme.colors;
        let (checked, disabled, shape) = (self.checked, self.disabled, self.shape);
        let accent = c.accent;
        let control = div()
            .flex_none()
            .size(px(BOX))
            .flex()
            .items_center()
            .justify_center()
            .border(px(1.0))
            .bg(if checked && shape == Shape::Check { accent } else { c.bg })
            .border_color(if checked { accent } else { c.muted })
            .map(|d| match shape {
                Shape::Check => d.rounded(Radius::Swatch.px()),
                Shape::Radio => d.rounded_full(),
            })
            .when(checked && shape == Shape::Check, |d| {
                d.child(
                    svg()
                        .path(Icon::Check.path())
                        .size(px(BOX - 3.0))
                        .text_color(c.on_accent),
                )
            })
            .when(checked && shape == Shape::Radio, |d| {
                d.child(div().size(px(DOT)).rounded_full().bg(accent))
            });
        let next = if shape == Shape::Radio { true } else { !checked };
        div()
            .id(self.id)
            .role(match shape {
                Shape::Check => Role::CheckBox,
                Shape::Radio => Role::RadioButton,
            })
            .aria_label(self.label.clone())
            .aria_toggled(if checked { Toggled::True } else { Toggled::False })
            .focusable()
            .tab_stop(!disabled)
            .flex()
            .items_center()
            .gap(theme.space(6.0))
            .text_size(theme.text(TextSize::Base))
            .line_height(relative(LINE_HEIGHT))
            .text_color(c.text)
            .when(disabled, |d| {
                d.opacity(0.45)
                    .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
            })
            .when(!disabled, |d| {
                d.cursor_pointer().focus_visible(move |s| s.text_color(accent))
            })
            .when_some(self.on_toggle.filter(|_| !disabled), |d, handler| {
                d.on_click(move |_, window, cx| handler(&next, window, cx))
            })
            .child(control)
            .child(self.label)
    }
}
