//! `TextInput`: a single-line field (PRD §8.5 Inputs; NFR-7 role `TextInput`).
//!
//! Text, IME composition and the candidate window go through GPUI's platform input handler
//! (`EntityInputHandler` + `Window::handle_input`); editing keys are handled here. The element
//! that shapes and paints the line follows GPUI's `examples/input.rs` at the pinned rev
//! (Apache-2.0, Zed Industries).

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, FontStyle, GlobalElementId, Hsla, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Role, ShapedLine, SharedString, Style,
    TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, div, fill, point, prelude::*, px, relative, size,
};

use crate::logic::edit::{EditBuffer, EditCommand, edit_command};
use crate::theme::{LINE_HEIGHT, Radius, TextSize, Theme, hsla_alpha};

/// `::placeholder { color: muted; font-style: italic; opacity: .8 }`.
pub const PLACEHOLDER_OPACITY: f32 = 0.8;
/// No `::selection` rule in v0.2.15; the accent at this alpha stands in for the platform colour.
const SELECTION_ALPHA: f32 = 0.35;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// The text changed (typing, IME commit, paste, cut, delete).
    Changed,
}

/// A single-line text field.
pub struct TextInput {
    focus: FocusHandle,
    buffer: EditBuffer,
    placeholder: SharedString,
    label: SharedString,
    /// `search` inputs pad 5/8 instead of 6/8.
    search: bool,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    selecting: bool,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    /// `label` names the field for assistive technology.
    pub fn new(label: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            buffer: EditBuffer::default(),
            placeholder: SharedString::default(),
            label: label.into(),
            search: false,
            last_layout: None,
            last_bounds: None,
            selecting: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.buffer.set_content(text);
        self
    }

    /// Search-field padding (5/8).
    pub fn search(mut self) -> Self {
        self.search = true;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.content()
    }

    pub fn buffer(&self) -> &EditBuffer {
        &self.buffer
    }

    pub fn set_text(&mut self, text: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.set_content(text);
        self.changed(window, cx);
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus);
        cx.emit(TextInputEvent::Changed);
        cx.notify();
    }

    fn selection_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = edit_command(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        let before = self.buffer.content().to_owned();
        match command {
            EditCommand::Copy | EditCommand::Cut => {
                let selected = self.buffer.selected_text();
                if !selected.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
                }
            }
            EditCommand::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.buffer.replace(None, &text.replace(['\n', '\r'], " "));
                }
            }
            _ => {}
        }
        self.buffer.apply(command);
        if self.buffer.content() != before {
            self.changed(window, cx);
        } else {
            self.selection_moved(window, cx);
        }
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref()) else {
            return 0;
        };
        if self.buffer.content().is_empty() || position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.buffer.content().len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.selecting = true;
        window.focus(&self.focus, cx);
        let index = self.index_for_position(event.position);
        if event.modifiers.shift {
            self.buffer.select_to(index);
        } else {
            self.buffer.move_to(index);
        }
        self.selection_moved(window, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.buffer.select_to(self.index_for_position(event.position));
            self.selection_moved(window, cx);
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, actual) = self.buffer.text_for_utf16_range(&range_utf16);
        *actual_range = Some(actual);
        Some(text)
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selected_range()),
            reversed: self.buffer.is_reversed(),
        })
    }

    fn set_selected_text_range(&mut self, range_utf16: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.set_selected_utf16(&range_utf16);
        self.selection_moved(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buffer.content().encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked_range()
            .map(|range| self.buffer.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace(range_utf16, text);
        self.changed(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace_and_mark(range_utf16, text, selected_utf16);
        self.changed(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.buffer.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + layout.x_for_index(range.start), bounds.top()),
            point(bounds.left() + layout.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        let index = self.last_layout.as_ref()?.closest_index_for_x(local.x);
        Some(self.buffer.offset_to_utf16(index))
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx);
        let c = theme.colors;
        let focused = self.focus.is_focused(window);
        let pad_y = if self.search { 5.0 } else { 6.0 };
        let paint = LinePaint {
            text: c.text,
            placeholder: hsla_alpha(theme.tokens.muted, PLACEHOLDER_OPACITY),
            caret: c.text,
            selection: hsla_alpha(theme.tokens.accent, SELECTION_ALPHA),
        };
        div()
            .id("text-input")
            .role(Role::TextInput)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.buffer.content().to_owned()))
            .when(!self.placeholder.is_empty(), |d| {
                d.aria_placeholder(self.placeholder.clone())
            })
            .track_focus(&self.focus)
            .tab_stop(true)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .px(theme.space(8.0))
            .py(theme.space(pad_y))
            .bg(c.bg)
            .border(px(1.0))
            .border_color(if focused { c.accent } else { c.border })
            .rounded(Radius::Control.px())
            .text_color(c.text)
            .text_size(theme.text(TextSize::Base))
            .line_height(relative(LINE_HEIGHT))
            .overflow_hidden()
            .child(LineElement {
                input: cx.entity(),
                paint,
            })
    }
}

#[derive(Clone, Copy)]
struct LinePaint {
    text: Hsla,
    placeholder: Hsla,
    caret: Hsla,
    selection: Hsla,
}

struct LineElement {
    input: Entity<TextInput>,
    paint: LinePaint,
}

struct LinePrepaint {
    line: Option<ShapedLine>,
    caret: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for LineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LineElement {
    type RequestLayoutState = ();
    type PrepaintState = LinePrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let buffer = &input.buffer;
        let style = window.text_style();
        let empty = buffer.content().is_empty();
        let mut font = style.font();
        let (shown, color): (SharedString, Hsla) = if empty {
            font.style = FontStyle::Italic;
            (input.placeholder.clone(), self.paint.placeholder)
        } else {
            (buffer.content().to_owned().into(), self.paint.text)
        };
        let run = TextRun {
            len: shown.len(),
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match buffer.marked_range().filter(|_| !empty) {
            Some(marked) => {
                let underline = Some(UnderlineStyle {
                    color: Some(color),
                    thickness: px(1.0),
                    wavy: false,
                });
                [
                    TextRun {
                        len: marked.start,
                        ..run.clone()
                    },
                    TextRun {
                        len: marked.end.saturating_sub(marked.start),
                        underline,
                        ..run.clone()
                    },
                    TextRun {
                        len: shown.len().saturating_sub(marked.end),
                        ..run
                    },
                ]
                .into_iter()
                .filter(|run| run.len > 0)
                .collect()
            }
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window.text_system().shape_line(shown, font_size, &runs, None);
        let selected = buffer.selected_range();
        let (selection, caret) = if selected.is_empty() || empty {
            let x = if empty {
                px(0.0)
            } else {
                line.x_for_index(buffer.cursor())
            };
            let caret = fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top()),
                    size(px(1.0), bounds.size.height),
                ),
                self.paint.caret,
            );
            (None, Some(caret))
        } else {
            let quad = fill(
                Bounds::from_corners(
                    point(bounds.left() + line.x_for_index(selected.start), bounds.top()),
                    point(bounds.left() + line.x_for_index(selected.end), bounds.bottom()),
                ),
                self.paint.selection,
            );
            (Some(quad), None)
        };
        LinePrepaint {
            line: Some(line),
            caret,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.input.clone()), cx);
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        // A failed paint only loses this frame's text; the next frame retries.
        let _ = line.paint(bounds.origin, window.line_height(), TextAlign::Left, None, window, cx);
        if focus.is_focused(window)
            && let Some(caret) = prepaint.caret.take()
        {
            window.paint_quad(caret);
        }
        let empty = self.input.read(cx).buffer.content().is_empty();
        self.input.update(cx, |input, _| {
            // The placeholder's layout must not be used for hit testing the (empty) text.
            input.last_layout = (!empty).then_some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    struct Host {
        input: Entity<TextInput>,
        changes: usize,
        _sub: gpui::Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.input.clone())
        }
    }

    #[allow(clippy::unwrap_used, reason = "test helper")]
    fn open(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
        let (host, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| TextInput::new("Name", cx).with_placeholder("Name"));
            let sub = cx.subscribe_in(&input, window, |host: &mut Host, _, _: &TextInputEvent, _, _| {
                host.changes += 1
            });
            Host {
                input,
                changes: 0,
                _sub: sub,
            }
        });
        let input = host.read_with(cx, |h, _| h.input.clone());
        cx.update(|window, cx| window.focus(&input.focus_handle(cx), cx));
        (host, cx)
    }

    #[gpui::test]
    fn typing_goes_through_the_input_handler(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        cx.simulate_input("héllo");
        cx.simulate_keystrokes("backspace left shift-left");
        let (text, selected, changes) = host.read_with(cx, |h, cx| {
            let input = h.input.read(cx);
            (
                input.text().to_owned(),
                input.buffer().selected_text().to_owned(),
                h.changes,
            )
        });
        assert_eq!(text, "héll");
        assert_eq!(selected, "l");
        assert_eq!(changes, 6, "five characters and one backspace");
    }

    #[gpui::test]
    fn ime_composition_replaces_the_marked_text(cx: &mut TestAppContext) {
        let (host, cx) = open(cx);
        let input = host.read_with(cx, |h, _| h.input.clone());
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "ka", Some(2..2), window, cx);
                input.replace_and_mark_text_in_range(None, "か", Some(1..1), window, cx);
                input.replace_text_in_range(None, "家", window, cx);
            });
        });
        let text = input.read_with(cx, |input, _| input.text().to_owned());
        assert_eq!(text, "家");
    }
}
