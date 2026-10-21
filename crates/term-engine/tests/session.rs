//! PRD §6.3: parser off the UI thread (ST-1), damage-only frames (ST-4), non-blocking input with
//! chunked pastes (ST-5), coalesced resizes (ST-6) and per-session fault isolation (ST-8).

#![allow(clippy::unwrap_used, reason = "test helpers outside #[test] functions")]

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use term_engine::{
    ByteRing, EndReason, Frame, Session, SessionConfig, SessionEvent, SessionHooks, SessionState, WriteError,
};

/// Records every write call, so tests can check order and chunk sizes.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<Vec<u8>>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().push(buf.to_vec());
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Sink {
    fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().concat()
    }
    fn writes(&self) -> Vec<usize> {
        self.0.lock().unwrap().iter().map(Vec::len).collect()
    }
}

struct Harness {
    session: Session,
    ring: ByteRing,
    sink: Sink,
    events: Arc<Mutex<Vec<SessionEvent>>>,
    resizes: Arc<Mutex<Vec<(u16, u16)>>>,
}

fn start_with(on_event: impl Fn(&SessionEvent) + Send + Sync + 'static) -> Harness {
    let ring = ByteRing::new(1024);
    let sink = Sink::default();
    let events = Arc::new(Mutex::new(Vec::new()));
    let resizes = Arc::new(Mutex::new(Vec::new()));
    let hooks = SessionHooks {
        on_event: Arc::new({
            let events = events.clone();
            move |event: SessionEvent| {
                on_event(&event);
                events.lock().unwrap().push(event);
            }
        }),
        on_resize: Arc::new({
            let resizes = resizes.clone();
            move |cols, rows| resizes.lock().unwrap().push((cols, rows))
        }),
    };
    let config = SessionConfig {
        columns: 20,
        rows: 5,
        scrollback: 100,
    };
    let session = Session::start(config, ring.clone(), Box::new(sink.clone()), hooks);
    Harness {
        session,
        ring,
        sink,
        events,
        resizes,
    }
}

fn start() -> Harness {
    start_with(|_| {})
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

fn next_frame(h: &Harness) -> Frame {
    let mut frame = None;
    wait_until("a frame", || {
        frame = h.session.snapshot();
        frame.is_some()
    });
    frame.unwrap()
}

#[test]
fn parses_output_off_the_caller_thread_and_reports_it_as_a_frame() {
    let h = start();
    h.ring.push(b"hello").unwrap();
    let frame = next_frame(&h);
    assert!(frame.full, "the first frame repaints everything");
    assert_eq!(frame.text_of_row(0).trim_end(), "hello");
    assert_eq!(frame.cursor, Some((0, 5)));
    assert!(h.events.lock().unwrap().contains(&SessionEvent::Wakeup));
    assert!(
        h.session.snapshot().is_none(),
        "nothing new to paint until more output arrives"
    );
}

#[test]
fn later_frames_carry_only_the_damaged_rows() {
    let h = start();
    h.ring.push(b"first").unwrap();
    next_frame(&h);
    h.ring.push(b"\r\n\r\nthird").unwrap();
    let frame = next_frame(&h);
    assert!(!frame.full);
    let rows: Vec<_> = frame.lines.iter().map(|l| l.row).collect();
    assert!(rows.contains(&2), "the written row is repainted: {rows:?}");
    assert!(!rows.contains(&4), "untouched rows are not repainted: {rows:?}");
    assert_eq!(frame.text_of_row(2).trim_end(), "third");
}

#[test]
fn answers_terminal_queries_through_the_write_queue() {
    let h = start();
    h.ring.push(b"\x1b[6n").unwrap();
    wait_until("the cursor position report", || h.sink.bytes() == b"\x1b[1;1R");
}

#[test]
fn keystrokes_reach_the_source_in_order() {
    let h = start();
    h.session.write(b"ls").unwrap();
    h.session.write(b" -l\r").unwrap();
    wait_until("the typed bytes", || h.sink.bytes() == b"ls -l\r");
}

#[test]
fn large_pastes_are_written_in_chunks_of_at_most_64_kib_and_keep_their_order() {
    let h = start();
    let paste: Vec<u8> = (0..200 * 1024).map(|i| b'a' + (i % 26) as u8).collect();
    h.session.paste(paste.clone()).unwrap();
    wait_until("the whole paste", || h.sink.bytes().len() == paste.len());
    assert_eq!(h.sink.bytes(), paste);
    assert!(h.sink.writes().iter().all(|&n| n <= 64 * 1024), "{:?}", h.sink.writes());
}

#[test]
fn resizes_are_coalesced_and_applied_once_with_the_last_size() {
    let h = start();
    h.ring.push(b"x").unwrap();
    next_frame(&h);
    h.session.request_resize(100, 30);
    h.session.request_resize(120, 40);
    let frame = next_frame(&h);
    assert_eq!((frame.columns, frame.rows), (120, 40));
    assert!(frame.full, "a resize repaints everything");
    assert_eq!(*h.resizes.lock().unwrap(), vec![(120, 40)]);
    assert!(h.session.snapshot().is_none());
}

#[test]
fn the_end_of_the_source_ends_the_session() {
    let h = start();
    h.ring.push(b"bye").unwrap();
    h.ring.close();
    wait_until("the end", || {
        h.session.state() == SessionState::Ended(EndReason::SourceClosed)
    });
    assert!(
        h.events
            .lock()
            .unwrap()
            .contains(&SessionEvent::Ended(EndReason::SourceClosed))
    );
    assert_eq!(
        next_frame(&h).text_of_row(0).trim_end(),
        "bye",
        "output before the end is kept"
    );
}

#[test]
fn a_panic_inside_the_session_ends_only_that_session() {
    let h = start_with(|event| {
        if matches!(event, SessionEvent::Title(title) if title == "boom") {
            panic!("injected failure");
        }
    });
    let other = start();
    h.ring.push(b"\x1b]2;boom\x07").unwrap();
    wait_until("the failed session to end", || {
        matches!(h.session.state(), SessionState::Ended(EndReason::Failed(_)))
    });
    let SessionState::Ended(EndReason::Failed(message)) = h.session.state() else {
        unreachable!()
    };
    assert!(message.contains("injected failure"), "{message}");

    other.ring.push(b"still alive").unwrap();
    assert_eq!(next_frame(&other).text_of_row(0).trim_end(), "still alive");
}

#[test]
fn reports_title_changes_and_bells() {
    let h = start();
    h.ring.push(b"\x1b]0;my title\x07\x07").unwrap();
    wait_until("title and bell", || {
        let events = h.events.lock().unwrap();
        events.contains(&SessionEvent::Title("my title".into())) && events.contains(&SessionEvent::Bell)
    });
}

#[test]
fn a_flood_keeps_memory_bounded_and_ends_on_the_newest_screen() {
    let h = start();
    let flood = {
        let ring = h.ring.clone();
        thread::spawn(move || {
            for i in 0..4000 {
                ring.push(format!("line {i:05} {}\r\n", "x".repeat(40)).as_bytes())
                    .unwrap();
            }
            ring.push(b"END").unwrap();
        })
    };
    while !flood.is_finished() {
        assert!(h.ring.len() <= 1024, "the ring never exceeds its capacity");
        let _ = h.session.snapshot();
    }
    flood.join().unwrap();
    wait_until("the last line", || {
        let screen = h.session.full_frame();
        (0..5).any(|r| screen.text_of_row(r).trim_end() == "END")
    });
}

/// A source that consumes input slowly, so the write queue fills up.
#[derive(Clone, Default)]
struct SlowSink(Arc<Mutex<Vec<u8>>>);

impl Write for SlowSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        thread::sleep(Duration::from_micros(300));
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn quiet_hooks() -> SessionHooks {
    SessionHooks {
        on_event: Arc::new(|_| {}),
        on_resize: Arc::new(|_, _| {}),
    }
}

fn config() -> SessionConfig {
    SessionConfig {
        columns: 20,
        rows: 5,
        scrollback: 100,
    }
}

#[test]
fn consecutive_pastes_and_keys_reach_a_slow_source_in_order() {
    let sink = SlowSink::default();
    let session = Session::start(config(), ByteRing::new(1024), Box::new(sink.clone()), quiet_hooks());
    let first = vec![b'A'; 6 * 1024 * 1024];
    let second = vec![b'B'; 6 * 1024 * 1024];
    session.paste(first.clone()).unwrap();
    session.paste(second.clone()).unwrap();
    session.write(b"\r").unwrap();
    let expected = [first, second, b"\r".to_vec()].concat();
    let deadline = Instant::now() + Duration::from_secs(30);
    while sink.0.lock().unwrap().len() < expected.len() {
        assert!(Instant::now() < deadline, "timed out waiting for the pastes");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        *sink.0.lock().unwrap() == expected,
        "pastes and the key after them arrive whole and in order"
    );
}

#[test]
fn input_after_the_end_is_refused_and_never_written() {
    let h = start();
    h.ring.close();
    wait_until("the end", || {
        h.session.state() == SessionState::Ended(EndReason::SourceClosed)
    });
    assert_eq!(h.session.paste(b"late paste".to_vec()), Err(WriteError::Ended));
    assert_eq!(h.session.write(b"late key"), Err(WriteError::Ended));
    thread::sleep(Duration::from_millis(20));
    assert!(h.sink.bytes().is_empty());
}

#[test]
fn hooks_run_without_the_terminal_lock_so_they_may_read_the_screen() {
    let slot: Arc<std::sync::OnceLock<std::sync::Weak<Session>>> = Arc::default();
    let seen = Arc::new(Mutex::new(None));
    let hooks = SessionHooks {
        on_event: Arc::new({
            let (slot, seen) = (slot.clone(), seen.clone());
            move |event| {
                if let (SessionEvent::Title(_), Some(session)) = (event, slot.get().and_then(std::sync::Weak::upgrade))
                {
                    *seen.lock().unwrap() = Some(session.full_frame().text_of_row(0));
                }
            }
        }),
        on_resize: Arc::new(|_, _| {}),
    };
    let ring = ByteRing::new(1024);
    let session = Arc::new(Session::start(config(), ring.clone(), Box::new(Sink::default()), hooks));
    slot.set(Arc::downgrade(&session)).unwrap();
    ring.push(b"seen\x1b]2;title\x07").unwrap();
    wait_until("the hook to read the screen", || seen.lock().unwrap().is_some());
    assert_eq!(seen.lock().unwrap().as_deref().map(str::trim_end), Some("seen"));
}

#[test]
fn a_failing_resize_hook_ends_only_its_session() {
    let hooks = SessionHooks {
        on_event: Arc::new(|_| {}),
        on_resize: Arc::new(|_, _| panic!("resize failed")),
    };
    let session = Session::start(config(), ByteRing::new(1024), Box::new(Sink::default()), hooks);
    session.request_resize(80, 24);
    let frame = session.snapshot();
    assert!(frame.is_some(), "the frame of the resize is still returned");
    let SessionState::Ended(EndReason::Failed(message)) = session.state() else {
        panic!("{:?}", session.state())
    };
    assert!(message.contains("resize failed"), "{message}");
}

#[test]
fn many_keys_typed_during_a_long_paste_are_all_accepted_in_order() {
    let sink = SlowSink::default();
    let session = Session::start(config(), ByteRing::new(1024), Box::new(sink.clone()), quiet_hooks());
    let paste = vec![b'P'; 6 * 1024 * 1024];
    session.paste(paste.clone()).unwrap();
    let keys: Vec<u8> = (0..300).map(|i| b'a' + (i % 26) as u8).collect();
    for key in &keys {
        session.write(&[*key]).unwrap();
    }
    let expected = [paste, keys].concat();
    let deadline = Instant::now() + Duration::from_secs(30);
    while sink.0.lock().unwrap().len() < expected.len() {
        assert!(Instant::now() < deadline, "timed out waiting for the keys");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(*sink.0.lock().unwrap() == expected);
}

/// A source that never consumes input, so pastes stay queued.
struct StuckSink;
impl Write for StuckSink {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        thread::sleep(Duration::from_secs(3600));
        Ok(0)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn pastes_beyond_the_byte_budget_are_refused_and_the_budget_is_refunded() {
    let session = Session::start(config(), ByteRing::new(1024), Box::new(StuckSink), quiet_hooks());
    session.paste(vec![b'a'; 40 * 1024 * 1024]).unwrap();
    assert_eq!(
        session.paste(vec![b'b'; 30 * 1024 * 1024]),
        Err(WriteError::Full),
        "70 MiB queued is over the 64 MiB budget"
    );
    session.paste(vec![b'c'; 20 * 1024 * 1024]).unwrap();
    assert_eq!(session.paste(vec![b'd'; 8 * 1024 * 1024]), Err(WriteError::Full));
}
