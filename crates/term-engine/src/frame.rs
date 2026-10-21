use alacritty_terminal::term::cell::{Cell, Flags};

/// Cells of one screen row that must be repainted.
#[derive(Clone, Debug)]
pub struct FrameLine {
    /// Row on screen, 0 at the top of the viewport.
    pub row: usize,
    pub cells: Vec<Cell>,
}

/// What the UI paints for one session: the damaged rows (or all of them when `full`) and the cursor.
#[derive(Clone, Debug)]
pub struct Frame {
    pub columns: u16,
    pub rows: u16,
    /// Every visible row is included and the whole pane must be repainted.
    pub full: bool,
    pub lines: Vec<FrameLine>,
    /// `(row, column)` of a visible cursor.
    pub cursor: Option<(usize, usize)>,
}

impl Frame {
    /// The characters of `row` as included in this frame (empty when the row is not included).
    pub fn text_of_row(&self, row: usize) -> String {
        self.lines
            .iter()
            .find(|line| line.row == row)
            .map(|line| {
                line.cells
                    .iter()
                    .filter(|cell| !cell.flags.contains(Flags::WIDE_CHAR_SPACER))
                    .map(|cell| cell.c)
                    .collect()
            })
            .unwrap_or_default()
    }
}
