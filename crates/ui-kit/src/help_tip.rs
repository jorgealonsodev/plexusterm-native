//! `HelpTip`: the only custom tooltip (PRD §8.5 Help tip; NFR-7 role `Button` described by a
//! `Tooltip` node). A 14 px `CircleHelp` in `muted`, `text` on hover or focus; the 260 px bubble
//! opens above the icon on hover or focus and Escape hides it.

use gpui::{
    Anchor, App, Context, FocusHandle, Focusable, KeyDownEvent, Role, SharedString, Subscription, Window, anchored,
    deferred, div, point, prelude::*, px, relative, svg,
};

use crate::assets::Icon;
use crate::theme::{Elevation, Radius, TextSize, Theme};

/// Icon size (not scaled) and bubble width (a fixed dialog-like width) of §8.5.
pub const HELP_ICON: f32 = 14.0;
pub const HELP_BUBBLE_WIDTH: f32 = 260.0;
/// `z-index: 1000`: above dialogs and menus.
const HELP_PRIORITY: usize = 1000;
const BUBBLE_LINE_HEIGHT: f32 = 1.4;
/// Space between the icon and the bubble.
const BUBBLE_GAP: f32 = 4.0;

pub struct HelpTip {
    text: SharedString,
    focus: FocusHandle,
    hovered: bool,
    /// Escape hid the bubble; it shows again on the next hover or focus.
    hidden: bool,
    _blur: Subscription,
}

impl Focusable for HelpTip {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl HelpTip {
    pub fn new(text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let blur = cx.on_blur(&focus, window, |tip, _, cx| {
            tip.hidden = false;
            cx.notify();
        });
        Self {
            text: text.into(),
            focus,
            hovered: false,
            hidden: false,
            _blur: blur,
        }
    }

    /// Whether the bubble is showing.
    pub fn is_open(&self, window: &Window) -> bool {
        !self.hidden && (self.hovered || self.focus.is_focused(window))
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() && !self.hidden {
            self.hidden = true;
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Render for HelpTip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let c = theme.colors;
        let focused = self.focus.is_focused(window);
        let open = self.is_open(window);
        let lit = self.hovered || focused;
        let bubble = div()
            .id("help-bubble")
            .role(Role::Tooltip)
            .aria_label(self.text.clone())
            .w(px(HELP_BUBBLE_WIDTH))
            .px(theme.space(8.0))
            .py(theme.space(6.0))
            .bg(c.surface_2)
            .border(px(1.0))
            .border_color(c.border)
            .rounded(Radius::Control.px())
            .shadow(Elevation::HelpTip.shadow())
            .text_color(c.text)
            .text_size(theme.text(TextSize::Ui))
            .line_height(relative(BUBBLE_LINE_HEIGHT))
            .child(self.text.clone());
        div()
            .id("help-tip")
            .role(Role::Button)
            .aria_label("Help")
            .aria_description(self.text.clone())
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_hover(cx.listener(|tip, hovered: &bool, _, cx| {
                tip.hovered = *hovered;
                if *hovered {
                    tip.hidden = false;
                }
                cx.notify();
            }))
            .relative()
            .flex_none()
            .size(px(HELP_ICON))
            .child(
                svg()
                    .path(Icon::CircleHelp.path())
                    .size(px(HELP_ICON))
                    .text_color(if lit { c.text } else { c.muted }),
            )
            .when(open, |d| {
                d.child(
                    div().absolute().top_0().left_0().child(
                        deferred(
                            anchored()
                                .anchor(Anchor::BottomLeft)
                                .offset(point(px(0.0), px(-BUBBLE_GAP)))
                                .snap_to_window_with_margin(px(BUBBLE_GAP))
                                .child(bubble),
                        )
                        .priority(HELP_PRIORITY),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Entity, TestAppContext};

    struct Host {
        tip: Entity<HelpTip>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().p_10().child(self.tip.clone())
        }
    }

    #[gpui::test]
    fn focus_opens_and_escape_hides(cx: &mut TestAppContext) {
        let (host, cx) = cx.add_window_view(|window, cx| Host {
            tip: cx.new(|cx| HelpTip::new("Explains the field.", window, cx)),
        });
        let tip = host.read_with(cx, |h, _| h.tip.clone());
        assert!(!cx.update(|window, cx| tip.read(cx).is_open(window)));
        cx.update(|window, cx| window.focus(&tip.focus_handle(cx), cx));
        assert!(cx.update(|window, cx| tip.read(cx).is_open(window)));
        cx.simulate_keystrokes("escape");
        assert!(!cx.update(|window, cx| tip.read(cx).is_open(window)));
    }
}
