//! Ported from PlexusTerm v0.2.15 `src-tauri/src/pty/mod.rs` tests (they run /bin/sh).
#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers outside #[test] functions"
)]

use std::io::Write;
use std::time::{Duration, Instant};

use proto_pty::{ChildEnv, LocalPty, SpawnSpec, spawn};
use term_engine::ByteRing;

fn sh(script: &str) -> SpawnSpec {
    SpawnSpec {
        program: Some("/bin/sh".into()),
        args: vec!["-c".into(), script.into()],
        columns: 80,
        rows: 24,
        ..SpawnSpec::default()
    }
}

fn start(spec: SpawnSpec) -> (LocalPty, Box<dyn Write + Send>, ByteRing) {
    let ring = ByteRing::new(64 * 1024);
    let (pty, writer) = spawn(spec, ring.clone()).expect("spawn");
    (pty, writer, ring)
}

/// Reads until `wanted` appears (or the stream ends), returning everything read.
fn read_until(ring: &ByteRing, wanted: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains(wanted) {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {wanted:?}; got {:?}",
            String::from_utf8_lossy(&seen)
        );
        match ring.try_pop(4096) {
            Some(bytes) => seen.extend(bytes),
            None if ring_closed(ring) => break,
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    String::from_utf8_lossy(&seen).into_owned()
}

fn ring_closed(ring: &ByteRing) -> bool {
    ring.is_closed()
}

fn wait_for_exit(pty: &LocalPty) -> Option<u32> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(code) = pty.exit_code() {
            return code;
        }
        assert!(Instant::now() < deadline, "the process never exited");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn streams_output_and_reports_exit_code() {
    let (pty, _w, ring) = start(sh("printf hello; exit 3"));
    assert!(read_until(&ring, "hello").contains("hello"));
    assert_eq!(wait_for_exit(&pty), Some(3));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ring_closed(&ring) {
        assert!(Instant::now() < deadline, "the output stream never ended");
        let _ = ring.try_pop(4096);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn writes_input_to_the_process() {
    let (_pty, mut w, ring) = start(sh("read line; printf 'got:%s' \"$line\""));
    w.write_all(b"ping\n").unwrap();
    assert!(read_until(&ring, "got:ping").contains("got:ping"));
}

#[test]
fn writes_reach_the_process_in_order() {
    let (_pty, mut w, ring) = start(sh("read a; read b; read c; printf '%s-%s-%s' \"$a\" \"$b\" \"$c\""));
    for word in ["one\n", "two\n", "three\n"] {
        w.write_all(word.as_bytes()).unwrap();
    }
    assert!(read_until(&ring, "one-two-three").contains("one-two-three"));
}

#[test]
fn sets_the_terminal_type_for_programs() {
    let (_pty, _w, ring) = start(sh("printf '[%s %s]' \"$TERM\" \"$COLORTERM\""));
    assert!(read_until(&ring, "]").contains("[xterm-256color truecolor]"));
}

#[test]
fn resize_changes_the_terminal_size() {
    let (pty, mut w, ring) = start(sh("read go; stty size"));
    pty.resize(100, 30).unwrap();
    w.write_all(b"\n").unwrap();
    assert!(read_until(&ring, "30 100").contains("30 100"));
}

#[test]
fn extra_environment_reaches_the_process() {
    let mut spec = sh("printf '<%s>' \"$PLEXUS_TEST_VAR\"");
    spec.env = ChildEnv::new([("PLEXUS_TEST_VAR".to_owned(), "secret-value".to_owned())]);
    let (_pty, _w, ring) = start(spec);
    assert!(read_until(&ring, ">").contains("<secret-value>"));
}

#[test]
fn extra_environment_is_never_shown_in_debug_output() {
    let env = ChildEnv::new([("MOSH_KEY".to_owned(), "do-not-log".to_owned())]);
    let shown = format!("{env:?}");
    assert!(shown.contains("MOSH_KEY"));
    assert!(!shown.contains("do-not-log"));
}

#[test]
fn a_starting_directory_with_a_tilde_starts_in_the_home_directory() {
    let home = std::env::var("HOME").unwrap();
    let mut spec = sh("pwd -P; printf END");
    spec.cwd = Some("~".into());
    let (_pty, _w, ring) = start(spec);
    let out = read_until(&ring, "END");
    let real_home = std::fs::canonicalize(&home).unwrap();
    assert!(out.contains(real_home.to_str().unwrap()), "{out}");
}

#[test]
fn a_blank_program_or_directory_keeps_the_defaults() {
    let spec = SpawnSpec {
        program: Some("   ".into()),
        args: vec!["-c".into(), "printf 'dir:%s:default-shell-ok' \"$(pwd -P)\"".into()],
        cwd: Some(" ".into()),
        columns: 80,
        rows: 24,
        ..SpawnSpec::default()
    };
    let (_pty, _w, ring) = start(spec);
    let out = read_until(&ring, "default-shell-ok");
    let home = std::fs::canonicalize(std::env::var("HOME").unwrap()).unwrap();
    assert!(
        out.contains(&format!("dir:{}:default-shell-ok", home.display())),
        "{out}"
    );
}

#[test]
fn a_program_that_does_not_exist_is_an_error() {
    let spec = SpawnSpec {
        program: Some("/no/such/program".into()),
        ..SpawnSpec::default()
    };
    assert!(spawn(spec, ByteRing::new(1024)).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn cwd_follows_the_shell_folder() {
    let (pty, _w, ring) = start(sh("cd /usr && printf READY && sleep 5"));
    read_until(&ring, "READY");
    assert_eq!(pty.cwd().as_deref(), Some("/usr"));
}

#[cfg(target_os = "linux")]
#[test]
fn program_cwd_follows_the_program_in_the_foreground() {
    // An interactive bash runs the inner program as its own foreground process group.
    let spec = SpawnSpec {
        program: Some("/bin/bash".into()),
        args: vec![
            "--norc".into(),
            "-i".into(),
            "-c".into(),
            "cd /usr && sh -c 'cd /usr/bin && printf READY && sleep 5'; true".into(),
        ],
        columns: 80,
        rows: 24,
        ..SpawnSpec::default()
    };
    let (pty, _w, ring) = start(spec);
    read_until(&ring, "READY");
    assert_eq!(pty.cwd().as_deref(), Some("/usr"), "the shell stays in its own folder");
    assert_eq!(pty.program_cwd().as_deref(), Some("/usr/bin"));
}

#[test]
fn killing_ends_the_process_and_the_stream() {
    let (pty, _w, ring) = start(sh("printf READY; sleep 30"));
    read_until(&ring, "READY");
    pty.kill();
    wait_for_exit(&pty);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ring_closed(&ring) {
        assert!(Instant::now() < deadline, "the output stream never ended");
        let _ = ring.try_pop(4096);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn output_waits_for_the_reader_instead_of_being_dropped() {
    let ring = ByteRing::new(1024);
    let (_pty, _w) = spawn(
        sh("i=0; while [ $i -lt 2000 ]; do printf 'line %05d\\n' $i; i=$((i+1)); done; printf DONE"),
        ring.clone(),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let out = read_until(&ring, "DONE");
    let lines: Vec<_> = out
        .lines()
        .filter(|l| l.starts_with("line "))
        .map(|l| l.trim_end().to_owned())
        .collect();
    let expected: Vec<_> = (0..2000).map(|i| format!("line {i:05}")).collect();
    assert_eq!(
        lines, expected,
        "every line arrives once, in order, although the ring was full"
    );
}

#[test]
fn a_session_that_stops_reading_ends_its_program() {
    let ring = ByteRing::new(1024);
    let (pty, _w) = spawn(
        sh("while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxx\\n'; done"),
        ring.clone(),
    )
    .unwrap();
    read_until(&ring, "xxxx");
    ring.close();
    wait_for_exit(&pty);
}

#[test]
fn a_shell_runs_end_to_end_into_a_session_frame() {
    use std::sync::Arc;
    use term_engine::{Session, SessionConfig, SessionHooks};

    let ring = ByteRing::new(ByteRing::SESSION_CAPACITY);
    let (pty, writer) = spawn(sh("read line; printf 'echo:%s' \"$line\"; sleep 5"), ring.clone()).unwrap();
    let pty = Arc::new(pty);
    let hooks = SessionHooks {
        on_event: Arc::new(|_| {}),
        on_resize: Arc::new({
            let pty = pty.clone();
            move |cols, rows| {
                let _ = pty.resize(cols, rows);
            }
        }),
    };
    let session = Session::start(
        SessionConfig {
            columns: 80,
            rows: 24,
            scrollback: 10_000,
        },
        ring,
        writer,
        hooks,
    );
    session.write(b"hi\r").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = session.full_frame();
        if (0..24).any(|row| screen.text_of_row(row).contains("echo:hi")) {
            break;
        }
        assert!(Instant::now() < deadline, "the shell's answer never reached the screen");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_exit_is_seen_while_a_background_job_keeps_the_terminal_open() {
    let (pty, _w, ring) = start(sh("(trap '' HUP; sleep 30) & printf READY; exit 4"));
    read_until(&ring, "READY");
    assert_eq!(wait_for_exit(&pty), Some(4));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ring_closed(&ring) {
        assert!(
            Instant::now() < deadline,
            "the stream stayed open after the shell exited"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_program_that_ignores_the_hang_up_is_killed() {
    let (pty, _w, ring) = start(sh("trap '' HUP; printf READY; while :; do sleep 1; done"));
    read_until(&ring, "READY");
    pty.kill();
    wait_for_exit(&pty);
}

#[test]
fn dropping_the_terminal_ends_its_stream_even_when_nobody_reads() {
    let ring = ByteRing::new(64);
    let (pty, _w) = spawn(sh("while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxx'; done"), ring.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    drop(pty);
    assert!(ring_closed(&ring));
}
