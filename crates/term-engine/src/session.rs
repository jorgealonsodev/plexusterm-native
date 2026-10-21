use std::any::Any;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Osc52, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::Processor;

use crate::frame::{Frame, FrameLine};
use crate::guard::SequenceGuard;
use crate::ring::{ByteRing, TryPushError};

/// Most output parsed per lock hold, so the UI can always take the lock between slices (ST-2).
const SLICE_BYTES: usize = 1024 * 1024;
const SLICE_TIME: Duration = Duration::from_millis(4);
/// Bytes taken from the ring per parser step.
const STEP_BYTES: usize = 64 * 1024;
/// Largest single write to the source (ST-5).
pub const WRITE_CHUNK: usize = 64 * 1024;
/// Bytes waiting to be written to the source (ST-3, ST-5).
const WRITE_CAPACITY: usize = 4 * 1024 * 1024;
/// Pastes (and keys typed behind them) waiting for the paste worker, by count and by bytes.
const PASTE_QUEUE: usize = 4096;
const PASTE_BYTES: usize = 64 * 1024 * 1024;
/// Longest window title passed on (ST-9).
pub const MAX_TITLE: usize = 4096;

/// Size and history of a new session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionConfig {
    pub columns: u16,
    pub rows: u16,
    /// Scrollback lines (TERM-2: 10 000).
    pub scrollback: usize,
}

/// Something the UI should know about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// New output is ready to paint; sent once per batch, not per byte.
    Wakeup,
    /// The program set the window title (an empty title resets it).
    Title(String),
    Bell,
    Ended(EndReason),
}

/// Why a session stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The source ended (the shell exited, the connection closed).
    SourceClosed,
    /// The session failed (a panic in its threads or hooks); only this session is affected (ST-8).
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Running,
    Ended(EndReason),
}

/// Callbacks into the owner of the session; they must not block.
///
/// `on_event` runs on the parser thread without the terminal lock held, so it may read frames.
/// `on_resize` runs on the thread that calls [`Session::snapshot`]; if it panics, the resulting
/// `Ended` event is delivered on that thread too. A panic in either hook ends
/// only this session (ST-8).
#[derive(Clone)]
pub struct SessionHooks {
    pub on_event: Arc<dyn Fn(SessionEvent) + Send + Sync>,
    /// Applies a new size to the source (PTY window size, SSH window change, Telnet NAWS).
    pub on_resize: Arc<dyn Fn(u16, u16) + Send + Sync>,
}

/// Returned when input cannot be queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The source is not consuming input and the queue is full.
    Full,
    Ended,
}

#[derive(Clone, Copy)]
struct GridSize {
    columns: usize,
    lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

impl GridSize {
    fn new(columns: u16, rows: u16) -> Self {
        Self {
            columns: usize::from(columns.max(1)),
            lines: usize::from(rows.max(1)),
        }
    }
}

/// Forwards terminal events: replies go straight back to the source; the rest wait until the
/// parser releases the terminal lock, so owner hooks never run under it.
struct Listener {
    writes: ByteRing,
    pending: Arc<Mutex<Vec<SessionEvent>>>,
}

impl Listener {
    /// Keeps at most one bell and the latest title per batch, so a burst never piles up events.
    fn queue(&self, event: SessionEvent) {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        match &event {
            SessionEvent::Bell if pending.contains(&SessionEvent::Bell) => return,
            SessionEvent::Title(_) => pending.retain(|e| !matches!(e, SessionEvent::Title(_))),
            _ => {}
        }
        pending.push(event);
    }
}

/// Window titles are cut to [`MAX_TITLE`] bytes, at a character boundary (ST-9).
fn capped_title(mut title: String) -> String {
    if title.len() > MAX_TITLE {
        let mut end = MAX_TITLE;
        while !title.is_char_boundary(end) {
            end -= 1;
        }
        title.truncate(end);
    }
    title
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            // Replies to terminal queries are tiny; with a full queue the source is stuck anyway.
            Event::PtyWrite(reply) => {
                let _ = self.writes.try_push(reply.as_bytes());
            }
            Event::Title(title) => self.queue(SessionEvent::Title(capped_title(title))),
            Event::ResetTitle => self.queue(SessionEvent::Title(String::new())),
            Event::Bell => self.queue(SessionEvent::Bell),
            _ => {}
        }
    }
}

struct Shared {
    term: FairMutex<Term<Listener>>,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    input: ByteRing,
    writes: ByteRing,
    dirty: AtomicBool,
    pending_resize: Mutex<Option<(u16, u16)>>,
    state: Mutex<SessionState>,
    hooks: SessionHooks,
}

impl Shared {
    /// Records the first end only; a later one (the parser draining a ring closed by a failure) is ignored.
    fn end(&self, reason: EndReason) {
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if *state != SessionState::Running {
                return;
            }
            *state = SessionState::Ended(reason.clone());
        }
        // Nothing reads an ended session's output and nothing more may reach its source.
        self.input.close();
        self.writes.close();
        // The owner's hook may itself be what failed; it must not take this thread down twice.
        let _ = catch_unwind(AssertUnwindSafe(|| (self.hooks.on_event)(SessionEvent::Ended(reason))));
    }
}

/// One terminal session: a parser thread fed by a [`ByteRing`], a writer thread draining input
/// to the source and a paste worker (PRD §6.3 ST-1..ST-6, ST-8). The UI only takes [`Frame`]s.
pub struct Session {
    shared: Arc<Shared>,
    pastes: SyncSender<Vec<u8>>,
    /// Pastes not fully queued yet; while any is, typed keys wait behind them.
    pasting: Arc<AtomicUsize>,
    /// Bytes waiting for the paste worker.
    paste_bytes: Arc<AtomicUsize>,
}

impl Session {
    /// Starts the parser on `input` and the writer on `output`.
    pub fn start(config: SessionConfig, input: ByteRing, output: Box<dyn Write + Send>, hooks: SessionHooks) -> Self {
        let writes = ByteRing::new(WRITE_CAPACITY);
        let term_config = Config {
            scrolling_history: config.scrollback,
            // v0.2.15 (xterm.js) has no OSC 52 clipboard access.
            osc52: Osc52::Disabled,
            ..Config::default()
        };
        let events = Arc::new(Mutex::new(Vec::new()));
        let listener = Listener {
            writes: writes.clone(),
            pending: events.clone(),
        };
        let term = Term::new(term_config, &GridSize::new(config.columns, config.rows), listener);
        let shared = Arc::new(Shared {
            term: FairMutex::new(term),
            events,
            input: input.clone(),
            writes: writes.clone(),
            dirty: AtomicBool::new(false),
            pending_resize: Mutex::new(None),
            state: Mutex::new(SessionState::Running),
            hooks,
        });

        let parser = {
            let shared = shared.clone();
            move || match catch_unwind(AssertUnwindSafe(|| parse_until_end(&shared, &input))) {
                Ok(()) => shared.end(EndReason::SourceClosed),
                Err(panic) => shared.end(EndReason::Failed(panic_message(panic.as_ref()))),
            }
        };
        let writer = {
            let writes = writes.clone();
            move || drain_writes(&writes, output)
        };
        let (pastes, queued) = mpsc::sync_channel::<Vec<u8>>(PASTE_QUEUE);
        let pasting = Arc::new(AtomicUsize::new(0));
        let paste_bytes = Arc::new(AtomicUsize::new(0));
        let paster = {
            let (pasting, paste_bytes) = (pasting.clone(), paste_bytes.clone());
            // One worker for every paste keeps pastes, and keys typed behind them, in order.
            move || {
                for bytes in queued {
                    for chunk in bytes.chunks(WRITE_CHUNK) {
                        if writes.push(chunk).is_err() {
                            return;
                        }
                    }
                    paste_bytes.fetch_sub(bytes.len(), Ordering::AcqRel);
                    pasting.fetch_sub(1, Ordering::AcqRel);
                }
            }
        };
        let started = spawn_named("term-parser", parser)
            && spawn_named("term-writer", writer)
            && spawn_named("term-paste", paster);
        if !started {
            shared.end(EndReason::Failed("could not start the session's threads".to_owned()));
        }
        Self {
            shared,
            pastes,
            pasting,
            paste_bytes,
        }
    }

    /// Queues typed input without ever waiting (ST-5); keys typed during a paste go after it.
    pub fn write(&self, bytes: &[u8]) -> Result<(), WriteError> {
        if self.state() != SessionState::Running {
            return Err(WriteError::Ended);
        }
        if self.pasting.load(Ordering::Acquire) > 0 {
            return self.queue_paste(bytes.to_vec());
        }
        self.shared.writes.try_push(bytes).map_err(|e| match e {
            TryPushError::Full => WriteError::Full,
            TryPushError::Closed => WriteError::Ended,
        })
    }

    /// Queues a paste of any size without waiting; it is written in order, in 64 KiB chunks.
    pub fn paste(&self, bytes: Vec<u8>) -> Result<(), WriteError> {
        if self.state() != SessionState::Running {
            return Err(WriteError::Ended);
        }
        self.queue_paste(bytes)
    }

    fn queue_paste(&self, bytes: Vec<u8>) -> Result<(), WriteError> {
        let size = bytes.len();
        let queued = self.paste_bytes.fetch_add(size, Ordering::AcqRel);
        if queued.saturating_add(size) > PASTE_BYTES {
            self.paste_bytes.fetch_sub(size, Ordering::AcqRel);
            return Err(WriteError::Full);
        }
        self.pasting.fetch_add(1, Ordering::AcqRel);
        self.pastes.try_send(bytes).map_err(|e| {
            self.pasting.fetch_sub(1, Ordering::AcqRel);
            self.paste_bytes.fetch_sub(size, Ordering::AcqRel);
            match e {
                TrySendError::Full(_) => WriteError::Full,
                TrySendError::Disconnected(_) => WriteError::Ended,
            }
        })
    }

    /// Records a new size; applied once, with the last size, by the next [`Session::snapshot`] (ST-6).
    pub fn request_resize(&self, columns: u16, rows: u16) {
        *self
            .shared
            .pending_resize
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((columns, rows));
    }

    /// What changed since the last snapshot, or `None` when nothing did (ST-4).
    pub fn snapshot(&self) -> Option<Frame> {
        let resize = self
            .shared
            .pending_resize
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let dirty = self.shared.dirty.swap(false, Ordering::AcqRel);
        if !dirty && resize.is_none() {
            return None;
        }
        let mut term = self.shared.term.lock();
        if let Some((columns, rows)) = resize {
            term.resize(GridSize::new(columns, rows));
        }
        let damaged: Option<Vec<usize>> = match term.damage() {
            TermDamage::Full => None,
            TermDamage::Partial(lines) => Some(lines.map(|l| l.line).collect()),
        };
        let frame = read_frame(&term, damaged.as_deref());
        term.reset_damage();
        drop(term);
        if let Some((columns, rows)) = resize {
            let on_resize = &self.shared.hooks.on_resize;
            if let Err(panic) = catch_unwind(AssertUnwindSafe(|| on_resize(columns, rows))) {
                self.shared.end(EndReason::Failed(panic_message(panic.as_ref())));
            }
        }
        Some(frame)
    }

    /// The whole visible screen, without consuming damage (for a full repaint).
    pub fn full_frame(&self) -> Frame {
        read_frame(&self.shared.term.lock(), None)
    }

    pub fn state(&self) -> SessionState {
        self.shared.state.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Ends the threads; the source sees its ring closed and stops reading.
        self.shared.input.close();
        self.shared.writes.close();
    }
}

fn parse_until_end(shared: &Shared, input: &ByteRing) {
    let mut parser: Processor = Processor::new();
    let mut guard = SequenceGuard::default();
    let mut allowed = Vec::with_capacity(STEP_BYTES);
    while let Some(first) = input.pop(STEP_BYTES) {
        let started = Instant::now();
        let mut parsed = 0;
        let mut term = shared.term.lock();
        let mut step = Some(first);
        while let Some(bytes) = step {
            allowed.clear();
            guard.filter(&bytes, &mut allowed);
            parser.advance(&mut *term, &allowed);
            parsed += bytes.len();
            step = if parsed < SLICE_BYTES && started.elapsed() < SLICE_TIME {
                input.try_pop(STEP_BYTES)
            } else {
                None
            };
        }
        drop(term);
        let events = std::mem::take(&mut *shared.events.lock().unwrap_or_else(PoisonError::into_inner));
        for event in events {
            (shared.hooks.on_event)(event);
        }
        if !shared.dirty.swap(true, Ordering::AcqRel) {
            (shared.hooks.on_event)(SessionEvent::Wakeup);
        }
    }
}

fn drain_writes(writes: &ByteRing, mut output: Box<dyn Write + Send>) {
    while let Some(chunk) = writes.pop(WRITE_CHUNK) {
        if output.write_all(&chunk).and_then(|()| output.flush()).is_err() {
            // The source is gone; its reader reports the end of the session.
            writes.close();
            return;
        }
    }
}

fn read_frame(term: &Term<Listener>, damaged: Option<&[usize]>) -> Frame {
    // One flag per row keeps the per-cell check constant time while the lock is held (ST-2).
    let wanted = damaged.map(|rows| {
        let mut flags = vec![false; term.screen_lines()];
        for &row in rows {
            if let Some(flag) = flags.get_mut(row) {
                *flag = true;
            }
        }
        flags
    });
    let content = term.renderable_content();
    let offset = i32::try_from(content.display_offset).unwrap_or(i32::MAX);
    let mut lines: Vec<FrameLine> = Vec::new();
    for indexed in content.display_iter {
        let Ok(row) = usize::try_from(indexed.point.line.0.saturating_add(offset)) else {
            continue;
        };
        if wanted
            .as_ref()
            .is_some_and(|rows| !rows.get(row).copied().unwrap_or(false))
        {
            continue;
        }
        match lines.last_mut() {
            Some(line) if line.row == row => line.cells.push(indexed.cell.clone()),
            _ => lines.push(FrameLine {
                row,
                cells: vec![indexed.cell.clone()],
            }),
        }
    }
    let cursor = term.mode().contains(TermMode::SHOW_CURSOR).then(|| {
        let point = content.cursor.point;
        (
            usize::try_from(point.line.0.saturating_add(offset)).unwrap_or(0),
            point.column.0,
        )
    });
    Frame {
        columns: u16::try_from(term.columns()).unwrap_or(u16::MAX),
        rows: u16::try_from(term.screen_lines()).unwrap_or(u16::MAX),
        full: damaged.is_none(),
        lines,
        cursor,
    }
}

fn panic_message(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the session failed".to_owned())
}

fn spawn_named(name: &str, body: impl FnOnce() + Send + 'static) -> bool {
    thread::Builder::new().name(name.to_owned()).spawn(body).is_ok()
}
