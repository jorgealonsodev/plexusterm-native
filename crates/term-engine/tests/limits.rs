//! PRD §6.3 ST-9: input from the remote end is bounded before it reaches the parser.
#![allow(clippy::unwrap_used, reason = "test helpers outside #[test] functions")]

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use term_engine::{ByteRing, SequenceGuard, Session, SessionConfig, SessionEvent, SessionHooks};

struct Null;
impl Write for Null {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn filtered(guard: &mut SequenceGuard, chunks: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in chunks {
        guard.filter(chunk, &mut out);
    }
    out
}

#[test]
fn ordinary_output_and_short_sequences_pass_unchanged() {
    let mut guard = SequenceGuard::default();
    let input = b"plain \x1b[1mbold\x1b[0m \x1b]0;title\x07 \x1b]8;;https://a.b\x1b\\link\x1b]8;;\x1b\\ \x1bPq#0\x1b\\";
    assert_eq!(filtered(&mut guard, &[input]), input.to_vec());
}

#[test]
fn an_oversized_osc_is_cancelled_and_never_buffered() {
    let mut guard = SequenceGuard::default();
    let mut out = Vec::new();
    guard.filter(b"\x1b]2;", &mut out);
    for _ in 0..64 {
        guard.filter(&[b'x'; 4096], &mut out);
    }
    guard.filter(b"\x07after", &mut out);
    assert_eq!(
        out, b"after",
        "the whole string and its terminator are dropped; what follows flows again"
    );
}

#[test]
fn the_limit_holds_across_chunk_boundaries_and_with_esc_backslash() {
    let mut guard = SequenceGuard::default();
    let mut out = Vec::new();
    guard.filter(b"\x1bP", &mut out);
    for _ in 0..100 {
        guard.filter(&[b'q'; 1024], &mut out);
    }
    guard.filter(b"\x1b", &mut out);
    guard.filter(b"\\ok", &mut out);
    assert_eq!(out, b"ok");
}

#[test]
fn a_string_ended_by_a_new_escape_is_forwarded_with_it() {
    let input = b"\x1b]0;t\x1b[1mX";
    assert_eq!(filtered(&mut SequenceGuard::default(), &[input]), input.to_vec());
}

#[test]
fn escapes_split_across_chunks_pass_unchanged() {
    let mut guard = SequenceGuard::default();
    let out = filtered(
        &mut guard,
        &[b"a\x1b", b"[31mred\x1b", b"\x1b", b"]2;ti", b"tle\x1b", b"\\z"],
    );
    assert_eq!(out, b"a\x1b[31mred\x1b\x1b]2;title\x1b\\z".to_vec());
}

#[test]
fn hyperlink_targets_are_limited_to_8_kib() {
    let mut guard = SequenceGuard::default();
    let long = format!("\x1b]8;;https://example.com/{}\x07text", "a".repeat(9 * 1024));
    let out = filtered(&mut guard, &[long.as_bytes()]);
    assert_eq!(out, b"text", "an OSC 8 above 8 KiB is dropped");
    let short = format!("\x1b]8;;https://example.com/{}\x07", "a".repeat(1024));
    assert_eq!(
        filtered(&mut SequenceGuard::default(), &[short.as_bytes()]),
        short.as_bytes()
    );
}

fn session_with_events() -> (Session, ByteRing, Arc<Mutex<Vec<SessionEvent>>>) {
    let ring = ByteRing::new(1024 * 1024);
    let events = Arc::new(Mutex::new(Vec::new()));
    let hooks = SessionHooks {
        on_event: Arc::new({
            let events = events.clone();
            move |e| events.lock().unwrap().push(e)
        }),
        on_resize: Arc::new(|_, _| {}),
    };
    let session = Session::start(
        SessionConfig {
            columns: 40,
            rows: 5,
            scrollback: 100,
        },
        ring.clone(),
        Box::new(Null),
        hooks,
    );
    (session, ring, events)
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn window_titles_are_capped_at_4_kib() {
    let (session, ring, events) = session_with_events();
    ring.push(format!("\x1b]2;{}\x07done", "t".repeat(6000)).as_bytes())
        .unwrap();
    wait_for("the output after the title", || {
        session.full_frame().text_of_row(0).starts_with("done")
    });
    wait_for("the title event", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, SessionEvent::Title(_)))
    });
    let titles: Vec<usize> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| {
            if let SessionEvent::Title(t) = e {
                Some(t.len())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(titles, vec![4096]);
}

#[test]
fn a_burst_of_bells_is_reported_once_per_batch() {
    let (session, ring, events) = session_with_events();
    ring.push(&[0x07; 5000]).unwrap();
    ring.push(b"end").unwrap();
    wait_for("the text after the bells", || {
        session.full_frame().text_of_row(0).starts_with("end")
    });
    let bells = events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| **e == SessionEvent::Bell)
        .count();
    assert!((1..=3).contains(&bells), "{bells} bell events for one burst");
}

#[test]
fn the_session_parser_never_sees_an_oversized_string() {
    let (session, ring, events) = session_with_events();
    let mut flood = b"\x1b]2;".to_vec();
    flood.extend(std::iter::repeat_n(b'x', 200 * 1024));
    flood.extend_from_slice(b"\x07after");
    ring.push(&flood).unwrap();
    wait_for("the text after the string", || {
        session.full_frame().text_of_row(0).starts_with("after")
    });
    let titles = events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, SessionEvent::Title(_)))
        .count();
    assert_eq!(titles, 0, "a cancelled title never reaches the owner");
}

#[test]
fn can_and_sub_end_a_string_and_are_forwarded_with_it() {
    for end in [0x18u8, 0x1a] {
        let input = [b"\x1b]2;short".as_slice(), &[end], b"next"].concat();
        assert_eq!(filtered(&mut SequenceGuard::default(), &[&input]), input);
    }
    let mut long = b"\x1b]2;".to_vec();
    long.extend(std::iter::repeat_n(b'x', 70 * 1024));
    long.extend_from_slice(b"\x18next");
    assert_eq!(
        filtered(&mut SequenceGuard::default(), &[&long]),
        b"next",
        "an oversized string ended by CAN is dropped with it"
    );
}

#[test]
fn sos_pm_and_apc_strings_are_bounded_too() {
    for introducer in *b"X^_" {
        let short = [b"\x1b".as_slice(), &[introducer], b"data\x1b\\"].concat();
        assert_eq!(filtered(&mut SequenceGuard::default(), &[&short]), short);
        let mut long = vec![0x1b, introducer];
        long.extend(std::iter::repeat_n(b'y', 70 * 1024));
        long.extend_from_slice(b"\x1b\\after");
        assert_eq!(filtered(&mut SequenceGuard::default(), &[&long]), b"after");
    }
}

#[test]
fn esc_esc_inside_a_string_ends_it_and_keeps_the_second_escape() {
    let input = b"\x1b]2;t\x1b\x1b[1mX";
    assert_eq!(filtered(&mut SequenceGuard::default(), &[input]), input.to_vec());
}

#[test]
fn only_the_latest_title_of_a_batch_is_reported() {
    let (session, ring, events) = session_with_events();
    ring.push(b"\x1b]2;first\x07\x1b]2;second\x07\x1b]2;third\x07done")
        .unwrap();
    wait_for("the text", || session.full_frame().text_of_row(0).starts_with("done"));
    // Events are emitted right after the batch releases the terminal, so wait for them too.
    wait_for("the title event", || {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, SessionEvent::Title(_)))
    });
    let titles: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| {
            if let SessionEvent::Title(t) = e {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();
    // The three titles arrive in one push, so the parser handles them in one batch.
    assert_eq!(titles, vec!["third".to_owned()]);
}
