//! One terminal pane on a local PTY (PRD §6.1 terminal widget, §6.3, TERM-1, TERM-2).
//!
//! The PTY reader and the parser run on their own threads (proto-pty, term-engine); this view
//! only copies damage frames into its row cache, shapes the rows that changed and paints.

use std::cell::{Cell as StdCell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    App, Bounds, Context, FocusHandle, Font, FontStyle, FontWeight, Hsla, KeyDownEvent, Pixels, Rgba, ShapedLine, Size,
    StrikethroughStyle, Task, TextAlign, TextRun, UnderlineStyle, Window, canvas, div, fill, font, point, prelude::*,
    px, size,
};
use proto_pty::{LocalPty, SpawnSpec};
use term_engine::{ByteRing, Session, SessionConfig, SessionHooks, SessionState};
use ui_theme::{Palette, Rgb};

use crate::grid::{Grid, Row};
use crate::keys::{KeyAction, Mods, encode};
use crate::latency::LatencyLog;

/// TERM-18 default family, then the M0 fallback (JetBrains Mono is not bundled yet).
const FONT_FAMILIES: [&str; 2] = ["JetBrains Mono", "DejaVu Sans Mono"];
const FONT_SIZE: f32 = 14.0;
/// TERM-2: host padding on `bg`.
pub const PADDING: f32 = 6.0;
/// TERM-2: bar cursor width.
const CURSOR_WIDTH: f32 = 2.0;
/// TERM-2: scrollback lines.
const SCROLLBACK: usize = 10_000;

/// Size of one cell in the chosen font.
#[derive(Clone, Copy, Debug)]
struct CellSize {
    width: Pixels,
    height: Pixels,
}

/// The rows as shaped for painting, each tagged with the row version it was shaped from.
type Shaped = Vec<Option<(u64, ShapedLine)>>;

pub struct TerminalView {
    session: Arc<Session>,
    /// Kept alive for the pane's life: dropping it ends the shell.
    _pty: Arc<LocalPty>,
    palette: Palette,
    font: Font,
    cell: CellSize,
    grid: Rc<RefCell<Grid>>,
    shaped: Rc<RefCell<Shaped>>,
    /// Last size asked of the session, so a pane resize is requested once.
    requested: Rc<StdCell<(u16, u16)>>,
    focus: FocusHandle,
    /// TERM-17: `None` inside unless `PLEXUS_LATENCY_LOG` is set.
    latency: Rc<RefCell<LatencyLog>>,
    /// NFR-3 benchmark builds: quit once the first frame with the prompt has been submitted.
    exit_after_prompt: Option<Instant>,
    /// Whether the pane has painted before (its first paint is never presented by its own draw).
    painted_before: Rc<StdCell<bool>>,
    _pump: Task<()>,
}

/// Why a terminal pane could not start.
#[derive(Debug)]
pub struct StartError(pub String);

/// A started local session, before it has a view.
pub struct LocalSession {
    session: Arc<Session>,
    pty: Arc<LocalPty>,
    wakes: mpsc::UnboundedReceiver<()>,
    pending: Arc<AtomicBool>,
    size: (u16, u16),
}

impl LocalSession {
    /// Spawns the user's shell on a PTY and starts its session (ST-7: before GPUI starts, so the
    /// shell's startup overlaps the GPU and window setup).
    pub fn start() -> Result<Self, StartError> {
        let (columns, rows) = (80, 24);
        let ring = ByteRing::new(ByteRing::SESSION_CAPACITY);
        let spec = SpawnSpec {
            columns,
            rows,
            ..SpawnSpec::default()
        };
        let (pty, writer) = proto_pty::spawn(spec, ring.clone()).map_err(|e| StartError(e.to_string()))?;
        let pty = Arc::new(pty);

        // The parser thread only flags that something happened; the view's foreground pump does
        // the UI work. At most one wake-up is ever queued, so the channel stays bounded.
        let (wake_tx, wakes) = mpsc::unbounded::<()>();
        let pending = Arc::new(AtomicBool::new(false));
        let hooks = SessionHooks {
            on_event: {
                let pending = pending.clone();
                Arc::new(move |_event| {
                    if !pending.swap(true, Ordering::AcqRel) {
                        let _ = wake_tx.unbounded_send(());
                    }
                })
            },
            on_resize: {
                let pty = pty.clone();
                Arc::new(move |columns, rows| {
                    let _ = pty.resize(columns, rows);
                })
            },
        };
        let config = SessionConfig {
            columns,
            rows,
            scrollback: SCROLLBACK,
        };
        let session = Arc::new(Session::start(config, ring, writer, hooks));
        Ok(Self {
            session,
            pty,
            wakes,
            pending,
            size: (columns, rows),
        })
    }
}

impl TerminalView {
    pub fn new(local: LocalSession, palette: Palette, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let LocalSession {
            session,
            pty,
            mut wakes,
            pending,
            size: (columns, rows),
        } = local;
        let pump = cx.spawn(async move |this, cx| {
            while wakes.next().await.is_some() {
                crate::startup::mark("first_wake");
                pending.store(false, Ordering::Release);
                let running = this.update(cx, |view, cx| {
                    if view.session.state() == SessionState::Running {
                        cx.notify();
                        true
                    } else {
                        // v0.2.15 closes a local pane when its shell ends; this is the only pane.
                        cx.quit();
                        false
                    }
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        });

        crate::startup::mark("view_new");
        let font = pick_font(window);
        crate::startup::mark("font_picked");
        let cell = measure(&font, window);
        crate::startup::mark("font_measured");
        // The shell was spawned at 80×24 before the font was known; asking for the real size now
        // means the first frame already has it, instead of a resize and a second frame.
        let initial = initial_grid(window.viewport_size(), cell);
        if initial != (columns, rows) {
            session.request_resize(initial.0, initial.1);
        }
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            session,
            _pty: pty,
            palette,
            font,
            cell,
            grid: Rc::default(),
            shaped: Rc::default(),
            requested: Rc::new(StdCell::new(initial)),
            focus,
            latency: Rc::new(RefCell::new(LatencyLog::from_env())),
            exit_after_prompt: exit_after_prompt(),
            painted_before: Rc::default(),
            _pump: pump,
        }
    }

    /// Copies what changed since the last frame into the row cache (the only lock the UI takes).
    ///
    /// TERM-17 attribution: any damaged output that reaches the view while keys are waiting counts
    /// as the echo of all of them, whatever produced it (a prompt redraw, a clock); output that
    /// arrives while no key is waiting is not attributed to any key ([`LatencyLog::output`]).
    fn sync(&mut self) {
        if let Some(frame) = self.session.snapshot() {
            self.grid.borrow_mut().apply(&frame, &self.palette);
            if !frame.lines.is_empty() {
                crate::startup::mark("first_output_in_view");
                self.latency.borrow_mut().output();
            }
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        let mods = Mods {
            ctrl: m.control,
            alt: m.alt,
            shift: m.shift,
            platform: m.platform,
        };
        match encode(&keystroke.key, keystroke.key_char.as_deref(), mods) {
            KeyAction::Send(bytes) => {
                // A full queue means the program is not reading; the key is dropped, never waited on.
                if self.session.write(&bytes).is_ok() {
                    self.latency.borrow_mut().key(bytes.len());
                }
            }
            KeyAction::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    let _ = self.session.paste(text.into_bytes());
                }
            }
            // M0 has no selection, so there is nothing to copy.
            KeyAction::Copy => {}
            KeyAction::Ignore => return,
        }
        cx.stop_propagation();
    }
}

fn pick_font(window: &Window) -> Font {
    let installed = window.text_system().all_font_names();
    FONT_FAMILIES
        .iter()
        .find(|family| installed.iter().any(|name| name == *family))
        .map_or_else(|| font("monospace"), |family| font(*family))
}

fn measure(font: &Font, window: &Window) -> CellSize {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(font);
    let font_size = px(FONT_SIZE);
    let width = text_system
        .advance(font_id, font_size, 'm')
        .map(|advance| advance.width)
        .unwrap_or(px(FONT_SIZE * 0.6));
    // TERM-18 line height 1.0: the font's own cell height.
    let height = text_system.ascent(font_id, font_size) + text_system.descent(font_id, font_size).abs();
    CellSize {
        width,
        height: height.ceil(),
    }
}

pub fn hsla(c: Rgb) -> Hsla {
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: 1.0,
    }
    .into()
}

fn text_runs(row: &Row, base: &Font) -> Vec<TextRun> {
    row.spans
        .iter()
        .map(|span| {
            let style = span.style;
            let color = hsla(style.fg);
            let mut font = base.clone();
            if style.bold {
                font.weight = FontWeight::BOLD;
            }
            if style.italic {
                font.style = FontStyle::Italic;
            }
            TextRun {
                len: span.len,
                font,
                color,
                background_color: style.bg.map(hsla),
                underline: style.underline.then_some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(color),
                    wavy: false,
                }),
                strikethrough: style.strikeout.then_some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(color),
                }),
            }
        })
        .collect()
}

/// Columns and rows of the pane in a window whose content area is `viewport`: the canvas fills
/// the root `div` inside its [`PADDING`] on every side.
fn initial_grid(viewport: Size<Pixels>, cell: CellSize) -> (u16, u16) {
    let inset = px(2.0 * PADDING);
    let content = size(
        (viewport.width - inset).max(px(0.0)),
        (viewport.height - inset).max(px(0.0)),
    );
    fit(Bounds::new(point(px(PADDING), px(PADDING)), content), cell)
}

/// Rows and columns that fit in `bounds` (at least one of each).
fn fit(bounds: Bounds<Pixels>, cell: CellSize) -> (u16, u16) {
    let count = |space: Pixels, step: Pixels| {
        let n = (f32::from(space) / f32::from(step)).floor();
        // Clamped to 1..=u16::MAX first, so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n.clamp(1.0, f32::from(u16::MAX)) as u16;
        n
    };
    (
        count(bounds.size.width, cell.width),
        count(bounds.size.height, cell.height),
    )
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::startup::mark("first_render");
        self.sync();
        let bg = hsla(self.palette.background);
        let cursor_color = hsla(self.palette.cursor);
        let (grid, shaped, requested) = (self.grid.clone(), self.shaped.clone(), self.requested.clone());
        let (session, font, cell) = (self.session.clone(), self.font.clone(), self.cell);
        let (latency, exit_after_prompt) = (self.latency.clone(), self.exit_after_prompt);
        let painted_before = self.painted_before.clone();

        let prepaint = {
            let grid = grid.clone();
            let shaped = shaped.clone();
            move |bounds: Bounds<Pixels>, window: &mut Window, _cx: &mut App| {
                // ST-6: ask for the new size; the session applies it on the next snapshot.
                let fits = fit(bounds, cell);
                if fits != requested.get() {
                    requested.set(fits);
                    session.request_resize(fits.0, fits.1);
                    window.refresh();
                    crate::startup::mark("resize_refresh");
                }
                // ST-4: shape only rows whose version changed since they were last shaped.
                let grid = grid.borrow();
                let mut shaped = shaped.borrow_mut();
                shaped.resize_with(grid.rows.len(), || None);
                shaped.truncate(grid.rows.len());
                for (row, slot) in grid.rows.iter().zip(shaped.iter_mut()) {
                    if slot.as_ref().is_some_and(|(version, _)| *version == row.version) {
                        continue;
                    }
                    let line = window.text_system().shape_line(
                        row.text.clone().into(),
                        px(FONT_SIZE),
                        &text_runs(row, &font),
                        Some(cell.width),
                    );
                    *slot = Some((row.version, line));
                }
            }
        };
        let paint = move |bounds: Bounds<Pixels>, (): (), window: &mut Window, cx: &mut App| {
            crate::startup::mark("first_paint");
            let shaped = shaped.borrow();
            for (index, slot) in shaped.iter().enumerate() {
                let Some((_, line)) = slot else { continue };
                #[allow(clippy::cast_precision_loss, reason = "row indexes are far below 2^24")]
                let origin = bounds.origin + point(px(0.0), cell.height * index as f32);
                let _ = line.paint_background(origin, cell.height, TextAlign::Left, None, window, cx);
                let _ = line.paint(origin, cell.height, TextAlign::Left, None, window, cx);
            }
            if let Some((row, column)) = grid.borrow().cursor {
                #[allow(clippy::cast_precision_loss, reason = "screen positions are far below 2^24")]
                let origin = bounds.origin + point(cell.width * column as f32, cell.height * row as f32);
                window.paint_quad(fill(
                    Bounds::new(origin, size(px(CURSOR_WIDTH), cell.height)),
                    cursor_color,
                ));
            }
            crate::startup::mark("first_paint_done");
            let painted = latency.borrow_mut().painted();
            let quit_after =
                exit_after_prompt.filter(|_| grid.borrow().rows.iter().any(|row| !row.text.trim().is_empty()));
            // GPUI's `open_window` draws once without presenting, and that is always the view's
            // first paint; the first frame callback then presents that scene. A paint inside a
            // frame callback is presented by the same callback.
            let presented_by_this_callback = painted_before.replace(true);
            if painted.is_some() || quit_after.is_some() || exit_after_prompt.is_some() {
                // Runs once the callback that presents this paint has returned.
                let report = move |cx: &mut App| {
                    cx.spawn(async move |cx| {
                        let submitted = Instant::now();
                        crate::startup::mark("first_frame_submitted");
                        if let Some(painted) = painted {
                            latency.borrow_mut().submitted(painted, submitted);
                        }
                        if let Some(started) = quit_after {
                            eprintln!(
                                "plexusterm: first prompt frame submitted {} µs after start",
                                submitted.saturating_duration_since(started).as_micros()
                            );
                            cx.update(|cx| cx.quit());
                        }
                    })
                    .detach();
                };
                if presented_by_this_callback {
                    report(cx);
                } else {
                    window.on_next_frame(move |_, cx| report(cx));
                }
            }
        };

        div()
            .size_full()
            .bg(bg)
            .p(px(PADDING))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .child(canvas(prepaint, paint).size_full())
    }
}

/// `PLEXUS_EXIT_AFTER_FIRST_PROMPT=1`, honoured only by builds with the `bench` feature (PRD §10,
/// §4.1 row 10). The time printed counts from the start of `main`; the process start is
/// measured from outside.
#[cfg(feature = "bench")]
fn exit_after_prompt() -> Option<Instant> {
    crate::startup::enabled().then(crate::startup::started)
}

#[cfg(not(feature = "bench"))]
fn exit_after_prompt() -> Option<Instant> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_grid_fits_the_canvas_inside_the_padding() {
        let cell = CellSize {
            width: px(8.4),
            height: px(17.0),
        };
        // 1200 - 2 * 6 = 1188 px / 8.4 = 141.4; 760 - 2 * 6 = 748 px / 17 = 44.
        assert_eq!(initial_grid(size(px(1200.0), px(760.0)), cell), (141, 44));
        // The same answer as the paint-time fit of the padded bounds.
        let bounds = Bounds::new(point(px(PADDING), px(PADDING)), size(px(1188.0), px(748.0)));
        assert_eq!(fit(bounds, cell), (141, 44));
        // Smaller than the padding still gives one cell.
        assert_eq!(initial_grid(size(px(4.0), px(4.0)), cell), (1, 1));
    }
}
