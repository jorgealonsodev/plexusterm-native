//! Single-line text editing state behind `TextInput` (PRD §8.5 Inputs).
//!
//! The selection, marked-text (IME) and UTF-16 conversion rules follow GPUI's own
//! `crates/gpui/examples/input.rs` at the pinned Zed rev (Apache-2.0, Zed Industries), rewritten
//! without panicking slicing so it satisfies the workspace lints.

use std::ops::Range;

use gpui::Keystroke;
use unicode_segmentation::UnicodeSegmentation;

/// An editing command bound to a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCommand {
    Backspace,
    Delete,
    Left,
    Right,
    SelectLeft,
    SelectRight,
    Home,
    End,
    SelectAll,
    Copy,
    Cut,
    Paste,
}

/// Maps a keystroke to an editing command; plain characters are left to the input handler.
pub fn edit_command(keystroke: &Keystroke) -> Option<EditCommand> {
    let m = &keystroke.modifiers;
    let secondary_only = m.secondary() && !m.alt && !m.shift && !m.function;
    if secondary_only {
        return match keystroke.key.as_str() {
            "a" => Some(EditCommand::SelectAll),
            "c" => Some(EditCommand::Copy),
            "x" => Some(EditCommand::Cut),
            "v" => Some(EditCommand::Paste),
            _ => None,
        };
    }
    if m.control || m.alt || m.platform || m.function {
        return None;
    }
    Some(match (keystroke.key.as_str(), m.shift) {
        ("backspace", _) => EditCommand::Backspace,
        ("delete", _) => EditCommand::Delete,
        ("left", false) => EditCommand::Left,
        ("right", false) => EditCommand::Right,
        ("left", true) => EditCommand::SelectLeft,
        ("right", true) => EditCommand::SelectRight,
        ("home", false) => EditCommand::Home,
        ("end", false) => EditCommand::End,
        _ => return None,
    })
}

/// The text of one input with its selection (byte offsets, on grapheme boundaries).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditBuffer {
    content: String,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
}

impl EditBuffer {
    /// A buffer holding `content` with the caret at its end.
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let end = content.len();
        Self {
            content,
            selected: end..end,
            reversed: false,
            marked: None,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.selected.clone()
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    /// Where the caret is drawn.
    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    pub fn selected_text(&self) -> &str {
        self.content.get(self.selected.clone()).unwrap_or_default()
    }

    /// Replaces the whole text and puts the caret at its end.
    pub fn set_content(&mut self, content: impl Into<String>) {
        *self = Self::new(content);
    }

    pub fn apply(&mut self, command: EditCommand) {
        match command {
            EditCommand::Left => {
                if self.selected.is_empty() {
                    self.move_to(self.previous_boundary(self.cursor()));
                } else {
                    self.move_to(self.selected.start);
                }
            }
            EditCommand::Right => {
                if self.selected.is_empty() {
                    self.move_to(self.next_boundary(self.selected.end));
                } else {
                    self.move_to(self.selected.end);
                }
            }
            EditCommand::SelectLeft => self.select_to(self.previous_boundary(self.cursor())),
            EditCommand::SelectRight => self.select_to(self.next_boundary(self.cursor())),
            EditCommand::Home => self.move_to(0),
            EditCommand::End => self.move_to(self.content.len()),
            EditCommand::SelectAll => {
                self.move_to(0);
                self.select_to(self.content.len());
            }
            EditCommand::Backspace => {
                if self.selected.is_empty() {
                    self.select_to(self.previous_boundary(self.cursor()));
                }
                self.replace(None, "");
            }
            EditCommand::Delete => {
                if self.selected.is_empty() {
                    self.select_to(self.next_boundary(self.cursor()));
                }
                self.replace(None, "");
            }
            // The clipboard is the view's job; `Cut` here only removes the selection.
            EditCommand::Cut => self.replace(None, ""),
            EditCommand::Copy | EditCommand::Paste => {}
        }
    }

    pub fn move_to(&mut self, offset: usize) {
        let offset = self.clamp(offset);
        self.selected = offset..offset;
        self.reversed = false;
    }

    pub fn select_to(&mut self, offset: usize) {
        let offset = self.clamp(offset);
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.offset_from_utf16(range.start);
        start..self.offset_from_utf16(range.end).max(start)
    }

    /// The text in a UTF-16 range, with the range actually covered.
    pub fn text_for_utf16_range(&self, range: &Range<usize>) -> (String, Range<usize>) {
        let range = self.range_from_utf16(range);
        let text = self.content.get(range.clone()).unwrap_or_default().to_owned();
        (text, self.range_to_utf16(&range))
    }

    pub fn set_selected_utf16(&mut self, range: &Range<usize>) {
        self.selected = self.range_from_utf16(range);
        self.reversed = false;
    }

    /// IME and typing: replaces `range` (UTF-16), else the marked text, else the selection.
    pub fn replace(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let range = self.target(range_utf16);
        self.splice(range.clone(), text);
        let caret = range.start + text.len();
        self.selected = caret..caret;
        self.reversed = false;
        self.marked = None;
    }

    /// IME composition: like [`Self::replace`] but the new text stays marked, and the selection
    /// is `selected_utf16` relative to the inserted text.
    pub fn replace_and_mark(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
    ) {
        let range = self.target(range_utf16);
        self.splice(range.clone(), text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        let start16 = self.offset_to_utf16(range.start);
        let len16 = text.encode_utf16().count();
        self.selected = match selected_utf16 {
            Some(sel) => self.range_from_utf16(&(start16 + sel.start.min(len16)..start16 + sel.end.min(len16))),
            None => {
                let caret = range.start + text.len();
                caret..caret
            }
        };
        self.reversed = false;
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    fn target(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .map(|r| self.range_from_utf16(&r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected.clone())
    }

    fn splice(&mut self, range: Range<usize>, text: &str) {
        let start = self.clamp(range.start);
        let end = self.clamp(range.end).max(start);
        if self.content.is_char_boundary(start) && self.content.is_char_boundary(end) {
            self.content.replace_range(start..end, text);
        }
    }

    fn clamp(&self, offset: usize) -> usize {
        offset.min(self.content.len())
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.content.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap()
    }

    #[test]
    fn keys_map_to_edit_commands() {
        assert_eq!(edit_command(&key("backspace")), Some(EditCommand::Backspace));
        assert_eq!(edit_command(&key("delete")), Some(EditCommand::Delete));
        assert_eq!(edit_command(&key("left")), Some(EditCommand::Left));
        assert_eq!(edit_command(&key("shift-right")), Some(EditCommand::SelectRight));
        assert_eq!(edit_command(&key("home")), Some(EditCommand::Home));
        assert_eq!(edit_command(&key("end")), Some(EditCommand::End));
        assert_eq!(edit_command(&key("secondary-a")), Some(EditCommand::SelectAll));
        assert_eq!(edit_command(&key("secondary-c")), Some(EditCommand::Copy));
        assert_eq!(edit_command(&key("secondary-x")), Some(EditCommand::Cut));
        assert_eq!(edit_command(&key("secondary-v")), Some(EditCommand::Paste));
        assert_eq!(edit_command(&key("a")), None);
        assert_eq!(edit_command(&key("shift-a")), None);
        assert_eq!(edit_command(&key("enter")), None);
    }

    #[test]
    fn typing_inserts_at_the_caret() {
        let mut b = EditBuffer::new("");
        b.replace(None, "hé");
        b.replace(None, "llo");
        assert_eq!(b.content(), "héllo");
        assert_eq!(b.cursor(), "héllo".len());
    }

    #[test]
    fn new_puts_the_caret_at_the_end() {
        let b = EditBuffer::new("abc");
        assert_eq!(b.selected_range(), 3..3);
    }

    #[test]
    fn backspace_and_delete_remove_whole_graphemes() {
        let mut b = EditBuffer::new("ae\u{301}b");
        b.apply(EditCommand::Left);
        b.apply(EditCommand::Backspace);
        assert_eq!(b.content(), "ab");
        b.apply(EditCommand::Home);
        b.apply(EditCommand::Delete);
        assert_eq!(b.content(), "b");
        b.apply(EditCommand::Home);
        b.apply(EditCommand::Backspace);
        assert_eq!(b.content(), "b");
    }

    #[test]
    fn selection_extends_and_collapses() {
        let mut b = EditBuffer::new("hello");
        b.apply(EditCommand::SelectLeft);
        b.apply(EditCommand::SelectLeft);
        assert_eq!(b.selected_range(), 3..5);
        assert!(b.is_reversed());
        assert_eq!(b.selected_text(), "lo");
        b.apply(EditCommand::Right);
        assert_eq!(b.selected_range(), 5..5);
        b.apply(EditCommand::SelectAll);
        assert_eq!(b.selected_text(), "hello");
        b.replace(None, "x");
        assert_eq!(b.content(), "x");
    }

    #[test]
    fn cut_removes_the_selection() {
        let mut b = EditBuffer::new("hello");
        b.apply(EditCommand::SelectAll);
        b.apply(EditCommand::Cut);
        assert_eq!(b.content(), "");
    }

    #[test]
    fn utf16_offsets_round_trip() {
        let b = EditBuffer::new("a😀b");
        assert_eq!(b.offset_to_utf16(5), 3);
        assert_eq!(b.offset_from_utf16(3), 5);
        assert_eq!(b.range_to_utf16(&(1..5)), 1..3);
        let (text, actual) = b.text_for_utf16_range(&(1..3));
        assert_eq!((text.as_str(), actual), ("😀", 1..3));
    }

    #[test]
    fn ime_composition_marks_then_commits() {
        let mut b = EditBuffer::new("x");
        b.replace_and_mark(None, "ka", Some(2..2));
        assert_eq!(b.content(), "xka");
        assert_eq!(b.marked_range(), Some(1..3));
        assert_eq!(b.selected_range(), 3..3);
        b.replace_and_mark(None, "か", Some(1..1));
        assert_eq!(b.content(), "xか");
        assert_eq!(b.marked_range(), Some(1..4));
        b.replace(None, "家");
        assert_eq!(b.content(), "x家");
        assert_eq!(b.marked_range(), None);
        assert_eq!(b.cursor(), "x家".len());
    }

    #[test]
    fn out_of_range_requests_are_clamped() {
        let mut b = EditBuffer::new("ab");
        b.set_selected_utf16(&(0..99));
        assert_eq!(b.selected_range(), 0..2);
        b.replace(Some(5..9), "c");
        assert_eq!(b.content(), "abc");
        b.move_to(42);
        assert_eq!(b.cursor(), 3);
    }
}
