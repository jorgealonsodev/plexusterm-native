//! The pane's copy of the screen, kept per row from damage frames (PRD §6.3 ST-4).
//!
//! Only the rows a [`Frame`] carries are rebuilt; every rebuilt row gets a new version, so the
//! view re-shapes exactly those rows and reuses the shaped text of the others.

use term_engine::{Cell, Flags, Frame};
use ui_theme::Palette;

use crate::colors::{CellStyle, cell_style};

/// A stretch of a row painted with one style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// UTF-8 bytes of the row's text.
    pub len: usize,
    pub style: CellStyle,
}

/// One screen row ready to shape: one character per cell, so it lines up with the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub spans: Vec<Span>,
    /// Changes whenever the row is rebuilt.
    pub version: u64,
}

#[derive(Debug, Default)]
pub struct Grid {
    pub columns: u16,
    pub rows: Vec<Row>,
    /// `(row, column)` of a visible cursor.
    pub cursor: Option<(usize, usize)>,
    next_version: u64,
}

impl Grid {
    /// Applies `frame`: its rows replace the cached ones; a `full` frame replaces all of them.
    pub fn apply(&mut self, frame: &Frame, palette: &Palette) {
        let rows = usize::from(frame.rows);
        if frame.full || frame.columns != self.columns {
            self.rows.clear();
        }
        self.columns = frame.columns;
        while self.rows.len() < rows {
            let blank = self.blank_row(palette);
            self.rows.push(blank);
        }
        self.rows.truncate(rows);
        for line in &frame.lines {
            self.next_version += 1;
            let version = self.next_version;
            if let Some(row) = self.rows.get_mut(line.row) {
                *row = build_row(&line.cells, palette, version);
            }
        }
        self.cursor = frame.cursor;
    }

    fn blank_row(&mut self, palette: &Palette) -> Row {
        self.next_version += 1;
        let cells = vec![Cell::default(); usize::from(self.columns)];
        build_row(&cells, palette, self.next_version)
    }
}

fn build_row(cells: &[Cell], palette: &Palette, version: u64) -> Row {
    let mut text = String::with_capacity(cells.len());
    let mut spans: Vec<Span> = Vec::new();
    for cell in cells {
        // The second half of a wide character keeps its column with a space; the wide glyph
        // itself is painted over both cells.
        let c = if cell.flags.contains(Flags::WIDE_CHAR_SPACER) || cell.c == '\0' {
            ' '
        } else {
            cell.c
        };
        let style = cell_style(cell, palette);
        let len = c.len_utf8();
        text.push(c);
        match spans.last_mut() {
            Some(span) if span.style == style => span.len += len,
            _ => spans.push(Span { len, style }),
        }
    }
    Row { text, spans, version }
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_engine::{Color, FrameLine};
    use ui_theme::{DEFAULT_SCHEME_ID, scheme_by_id};

    fn palette() -> Palette {
        scheme_by_id(DEFAULT_SCHEME_ID).palette
    }

    fn cells(text: &str, columns: usize) -> Vec<Cell> {
        let mut cells: Vec<Cell> = text.chars().map(|c| Cell { c, ..Cell::default() }).collect();
        cells.resize(columns, Cell::default());
        cells
    }

    fn frame(full: bool, lines: Vec<(usize, &str)>) -> Frame {
        Frame {
            columns: 4,
            rows: 3,
            full,
            lines: lines
                .into_iter()
                .map(|(row, text)| FrameLine {
                    row,
                    cells: cells(text, 4),
                })
                .collect(),
            cursor: Some((0, 1)),
        }
    }

    #[test]
    fn full_frame_replaces_every_row() {
        let p = palette();
        let mut grid = Grid::default();
        grid.apply(&frame(true, vec![(0, "ab"), (1, "cd"), (2, "ef")]), &p);
        let texts: Vec<&str> = grid.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["ab  ", "cd  ", "ef  "]);
        assert_eq!(grid.cursor, Some((0, 1)));
    }

    #[test]
    fn partial_frame_rebuilds_only_its_rows() {
        let p = palette();
        let mut grid = Grid::default();
        grid.apply(&frame(true, vec![(0, "ab"), (1, "cd"), (2, "ef")]), &p);
        let before: Vec<u64> = grid.rows.iter().map(|r| r.version).collect();
        grid.apply(&frame(false, vec![(1, "xy")]), &p);
        assert_eq!(grid.rows[1].text, "xy  ");
        assert_ne!(grid.rows[1].version, before[1]);
        assert_eq!(grid.rows[0].version, before[0]);
        assert_eq!(grid.rows[2].version, before[2]);
        assert_eq!(grid.rows[0].text, "ab  ");
    }

    #[test]
    fn spans_group_cells_of_one_style() {
        let p = palette();
        let mut row = cells("abcd", 4);
        row[2].fg = Color::Indexed(1);
        row[3].fg = Color::Indexed(1);
        let mut grid = Grid::default();
        let mut f = frame(true, vec![]);
        f.lines = vec![FrameLine { row: 0, cells: row }];
        grid.apply(&f, &p);
        let spans = &grid.rows[0].spans;
        assert_eq!(spans.len(), 2);
        assert_eq!((spans[0].len, spans[1].len), (2, 2));
        assert_eq!(spans[1].style.fg, p.ansi[1]);
    }

    #[test]
    fn wide_characters_keep_the_column_count() {
        let p = palette();
        let mut row = cells("字", 4);
        row[0].flags = Flags::WIDE_CHAR;
        row[1].flags = Flags::WIDE_CHAR_SPACER;
        let mut grid = Grid::default();
        let mut f = frame(true, vec![]);
        f.lines = vec![FrameLine { row: 0, cells: row }];
        grid.apply(&f, &p);
        assert_eq!(grid.rows[0].text.chars().count(), 4);
        assert!(grid.rows[0].text.starts_with('字'));
        assert_eq!(
            grid.rows[0].spans.iter().map(|s| s.len).sum::<usize>(),
            grid.rows[0].text.len()
        );
    }

    #[test]
    fn resize_changes_the_row_count() {
        let p = palette();
        let mut grid = Grid::default();
        grid.apply(&frame(true, vec![(0, "ab"), (1, "cd"), (2, "ef")]), &p);
        let mut smaller = frame(true, vec![(0, "ab")]);
        smaller.rows = 1;
        grid.apply(&smaller, &p);
        assert_eq!(grid.rows.len(), 1);
    }
}
