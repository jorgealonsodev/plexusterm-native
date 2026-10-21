//! Terminal cell colours to the scheme's RGB values (PRD TERM-1, §8.2).

use term_engine::{Cell, Color, Flags, NamedColor};
use ui_theme::{Palette, Rgb, mix};

/// How far a dim (SGR 2) colour moves towards the background (xterm.js draws it at half opacity).
const DIM: f64 = 0.5;

/// The RGB value of a cell colour in `palette`.
pub fn rgb_of(color: Color, palette: &Palette) -> Rgb {
    match color {
        Color::Spec(rgb) => Rgb::new(rgb.r, rgb.g, rgb.b),
        Color::Indexed(index) => indexed(index, palette),
        Color::Named(name) => named(name, palette),
    }
}

/// The 256-colour table: 16 scheme colours, the 6×6×6 xterm cube, then 24 greys.
fn indexed(index: u8, palette: &Palette) -> Rgb {
    match index {
        0..=15 => palette
            .ansi
            .get(usize::from(index))
            .copied()
            .unwrap_or(palette.foreground),
        16..=231 => {
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            let i = index - 16;
            Rgb::new(level(i / 36), level((i % 36) / 6), level(i % 6))
        }
        _ => {
            let grey = 8 + 10 * (index - 232);
            Rgb::new(grey, grey, grey)
        }
    }
}

fn named(name: NamedColor, palette: &Palette) -> Rgb {
    let ansi = |i: usize| palette.ansi.get(i).copied().unwrap_or(palette.foreground);
    let dim = |c: Rgb| mix(c, palette.background, DIM);
    match name {
        NamedColor::Foreground | NamedColor::BrightForeground => palette.foreground,
        NamedColor::Background => palette.background,
        NamedColor::Cursor => palette.cursor,
        NamedColor::DimForeground => dim(palette.foreground),
        NamedColor::DimBlack => dim(ansi(0)),
        NamedColor::DimRed => dim(ansi(1)),
        NamedColor::DimGreen => dim(ansi(2)),
        NamedColor::DimYellow => dim(ansi(3)),
        NamedColor::DimBlue => dim(ansi(4)),
        NamedColor::DimMagenta => dim(ansi(5)),
        NamedColor::DimCyan => dim(ansi(6)),
        NamedColor::DimWhite => dim(ansi(7)),
        // Black..=BrightWhite are 0..=15.
        other => ansi(other as usize),
    }
}

/// What one cell is painted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellStyle {
    pub fg: Rgb,
    /// `None` when the cell shows the pane background, so nothing is painted under it.
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// The colours and attributes of `cell`, with inverse, dim and hidden applied.
pub fn cell_style(cell: &Cell, palette: &Palette) -> CellStyle {
    let flags = cell.flags;
    let mut fg = rgb_of(cell.fg, palette);
    let mut bg = rgb_of(cell.bg, palette);
    if flags.contains(Flags::DIM) {
        fg = mix(fg, palette.background, DIM);
    }
    if flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    CellStyle {
        fg,
        bg: (bg != palette.background).then_some(bg),
        bold: flags.contains(Flags::BOLD),
        italic: flags.contains(Flags::ITALIC),
        underline: flags.intersects(Flags::ALL_UNDERLINES),
        strikeout: flags.contains(Flags::STRIKEOUT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::Rgb as Spec;
    use ui_theme::{DEFAULT_SCHEME_ID, scheme_by_id};

    fn palette() -> Palette {
        scheme_by_id(DEFAULT_SCHEME_ID).palette
    }

    #[test]
    fn named_colours_come_from_the_scheme() {
        let p = palette();
        assert_eq!(rgb_of(Color::Named(NamedColor::Red), &p), p.ansi[1]);
        assert_eq!(rgb_of(Color::Named(NamedColor::BrightWhite), &p), p.ansi[15]);
        assert_eq!(rgb_of(Color::Named(NamedColor::Foreground), &p), p.foreground);
        assert_eq!(rgb_of(Color::Named(NamedColor::Background), &p), p.background);
        assert_eq!(rgb_of(Color::Named(NamedColor::Cursor), &p), p.cursor);
        assert_eq!(rgb_of(Color::Named(NamedColor::BrightForeground), &p), p.foreground);
        assert_eq!(
            rgb_of(Color::Named(NamedColor::DimRed), &p),
            mix(p.ansi[1], p.background, 0.5)
        );
        assert_eq!(
            rgb_of(Color::Named(NamedColor::DimForeground), &p),
            mix(p.foreground, p.background, 0.5)
        );
    }

    #[test]
    fn indexed_0_to_15_are_the_ansi_colours() {
        let p = palette();
        for i in 0..16u8 {
            assert_eq!(rgb_of(Color::Indexed(i), &p), p.ansi[usize::from(i)], "index {i}");
        }
    }

    #[test]
    fn indexed_16_to_231_are_the_xterm_cube() {
        let p = palette();
        assert_eq!(rgb_of(Color::Indexed(16), &p), Rgb::new(0, 0, 0));
        assert_eq!(rgb_of(Color::Indexed(21), &p), Rgb::new(0, 0, 255));
        assert_eq!(rgb_of(Color::Indexed(59), &p), Rgb::new(95, 95, 95));
        assert_eq!(rgb_of(Color::Indexed(110), &p), Rgb::new(135, 175, 215));
        assert_eq!(rgb_of(Color::Indexed(196), &p), Rgb::new(255, 0, 0));
        assert_eq!(rgb_of(Color::Indexed(231), &p), Rgb::new(255, 255, 255));
    }

    #[test]
    fn indexed_232_to_255_are_greys() {
        let p = palette();
        assert_eq!(rgb_of(Color::Indexed(232), &p), Rgb::new(8, 8, 8));
        assert_eq!(rgb_of(Color::Indexed(244), &p), Rgb::new(128, 128, 128));
        assert_eq!(rgb_of(Color::Indexed(255), &p), Rgb::new(238, 238, 238));
    }

    #[test]
    fn truecolor_is_kept() {
        let p = palette();
        let c = Color::Spec(Spec { r: 1, g: 2, b: 3 });
        assert_eq!(rgb_of(c, &p), Rgb::new(1, 2, 3));
    }

    #[test]
    fn default_cell_paints_no_background() {
        let p = palette();
        let style = cell_style(&Cell::default(), &p);
        assert_eq!(style.fg, p.foreground);
        assert_eq!(style.bg, None);
        assert!(!style.bold && !style.italic && !style.underline && !style.strikeout);
    }

    #[test]
    fn attributes_are_carried() {
        let p = palette();
        let mut cell = Cell {
            flags: Flags::BOLD | Flags::ITALIC | Flags::UNDERLINE | Flags::STRIKEOUT,
            ..Cell::default()
        };
        let style = cell_style(&cell, &p);
        assert!(style.bold && style.italic && style.underline && style.strikeout);
        cell.flags = Flags::DOUBLE_UNDERLINE;
        assert!(cell_style(&cell, &p).underline);
    }

    #[test]
    fn coloured_background_is_painted() {
        let p = palette();
        let cell = Cell {
            bg: Color::Indexed(4),
            ..Cell::default()
        };
        assert_eq!(cell_style(&cell, &p).bg, Some(p.ansi[4]));
    }

    #[test]
    fn inverse_swaps_foreground_and_background() {
        let p = palette();
        let cell = Cell {
            flags: Flags::INVERSE,
            ..Cell::default()
        };
        let style = cell_style(&cell, &p);
        assert_eq!(style.fg, p.background);
        assert_eq!(style.bg, Some(p.foreground));
    }

    #[test]
    fn dim_and_hidden_change_the_foreground() {
        let p = palette();
        let mut cell = Cell {
            fg: Color::Indexed(2),
            flags: Flags::DIM,
            ..Cell::default()
        };
        assert_eq!(cell_style(&cell, &p).fg, mix(p.ansi[2], p.background, 0.5));
        cell.flags = Flags::HIDDEN;
        assert_eq!(cell_style(&cell, &p).fg, p.background);
    }
}
