//! Tests use a pseudo-terminal pair as the serial device; they never open real hardware.
//! Ported from v0.2.15 `src-tauri/src/serial/tests.rs`; output is read from the session's
//! [`ByteRing`] instead of a Tauri channel.

use super::*;
use std::sync::mpsc::channel;
use std::time::Instant;

use portable_pty::{MasterPty, PtyPair, PtySize, native_pty_system};

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
fn pty_pair() -> (PtyPair, String) {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let path = pair
        .master
        .tty_name()
        .expect("pty slave path")
        .to_string_lossy()
        .into_owned();
    assert!(path.starts_with("/dev/pts/"), "unexpected pty path {path}");
    (pair, path)
}

fn request(path: &str) -> SerialRequest {
    SerialRequest {
        path: path.into(),
        baud: 115_200,
        data_bits: 8,
        parity: ParitySetting::None,
        stop_bits: 1,
        flow: FlowSetting::None,
    }
}

fn ring() -> ByteRing {
    ByteRing::new(ByteRing::SESSION_CAPACITY)
}

/// Collects ring output until `needle` appears; the flag tells whether the session ended
/// (ring closed and drained).
fn wait_for(ring: &ByteRing, needle: &str) -> (String, bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut out = Vec::new();
    while Instant::now() < deadline {
        if let Some(bytes) = ring.try_pop(usize::MAX) {
            out.extend(bytes);
            if !needle.is_empty() && String::from_utf8_lossy(&out).contains(needle) {
                return (String::from_utf8_lossy(&out).into_owned(), false);
            }
            continue;
        }
        if ring.is_closed() && ring.is_empty() {
            return (String::from_utf8_lossy(&out).into_owned(), true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (String::from_utf8_lossy(&out).into_owned(), false)
}

/// Reads from the master side on a helper thread until `needle` shows up.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
fn master_reads(master: &dyn MasterPty, needle: &str) -> String {
    let mut reader = master.try_clone_reader().unwrap();
    let (tx, rx) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut out = Vec::new();
    while Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            out.extend(chunk);
            if String::from_utf8_lossy(&out).contains(needle) {
                break;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Opens `path` again, retrying while the previous handles are still being released.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
fn reopens(path: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mgr = SerialManager::default();
    while Instant::now() < deadline {
        if let Ok(session) = mgr.open(request(path), ring()) {
            mgr.close(session.id).unwrap();
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn streams_device_output_to_the_sink() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let out = ring();
    let session = mgr.open(request(&path), out.clone()).unwrap();

    let mut writer = pair.master.take_writer().unwrap();
    writer.write_all("boot: ñ ok\r\n".as_bytes()).unwrap();
    writer.flush().unwrap();

    let (text, ended) = wait_for(&out, "ok");
    assert!(text.contains("boot: ñ ok"), "output was {text:?}");
    assert!(!ended);
    mgr.close(session.id).unwrap();
}

#[test]
fn writes_input_to_the_device() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let session = mgr.open(request(&path), ring()).unwrap();

    mgr.write(session.id, b"show version\r").unwrap();
    let out = master_reads(&*pair.master, "show version");
    assert!(out.contains("show version"), "master read {out:?}");
    mgr.close(session.id).unwrap();
}

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
fn assert_invalid(req: SerialRequest, needle: &str) {
    let mgr = SerialManager::default();
    match mgr.open(req, ring()) {
        Err(err @ SerialError::InvalidSettings(_)) => {
            assert!(err.to_string().contains(needle), "message was {err}");
        }
        other => panic!("expected invalidSettings, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn rejects_invalid_settings_before_opening() {
    // The path does not exist: validation must fail first, without touching it.
    let missing = "/nonexistent/plexusterm-serial";
    assert_invalid(
        SerialRequest {
            data_bits: 9,
            ..request(missing)
        },
        "data bits",
    );
    assert_invalid(
        SerialRequest {
            baud: 0,
            ..request(missing)
        },
        "baud",
    );
    assert_invalid(
        SerialRequest {
            stop_bits: 3,
            ..request(missing)
        },
        "stop bits",
    );
    assert_invalid(request("  "), "path");
}

#[test]
fn missing_device_is_not_found() {
    let mgr = SerialManager::default();
    let err = mgr
        .open(request("/dev/plexusterm-test-missing-serial"), ring())
        .expect_err("open should fail");
    assert!(matches!(err, SerialError::NotFound(_)), "got {err:?}");
    assert!(err.to_string().contains("not found"), "message was {err}");
}

#[test]
fn device_already_open_is_busy() {
    let (_pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let session = mgr.open(request(&path), ring()).unwrap();
    let err = mgr.open(request(&path), ring()).expect_err("second open");
    assert!(matches!(err, SerialError::Busy(_)), "got {err:?}");
    mgr.close(session.id).unwrap();
}

#[test]
fn permission_errors_map_to_permission_denied() {
    let err = open_error(
        "/dev/ttyUSB9",
        serialport::Error::new(serialport::ErrorKind::Io(IoKind::PermissionDenied), "Permission denied"),
    );
    assert!(matches!(err, SerialError::PermissionDenied(_)), "got {err:?}");
}

#[test]
fn unplugged_device_reports_the_error_and_ends() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let out = ring();
    mgr.open(request(&path), out.clone()).unwrap();
    // Closing the master side hangs up the slave, like a USB adapter being pulled out.
    drop(pair);

    let (text, ended) = wait_for(&out, "");
    assert!(ended, "session did not end; output {text:?}");
    assert!(text.contains("disconnected"), "output was {text:?}");
}

#[test]
fn close_stops_the_reader_and_forgets_the_session() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let out = ring();
    let session = mgr.open(request(&path), out.clone()).unwrap();
    mgr.close(session.id).unwrap();
    // Output sent after closing never reaches the pane, and closing is not a disconnection.
    let mut writer = pair.master.take_writer().unwrap();
    writer.write_all(b"late\r\n").unwrap();
    let (text, ended) = wait_for(&out, "");
    assert!(ended, "the ring was not closed");
    assert!(
        !text.contains("disconnected"),
        "closing must not report a disconnection: {text:?}"
    );
    assert!(!text.contains("late"), "the reader kept running: {text:?}");

    assert!(matches!(
        mgr.write(session.id, b"x"),
        Err(SerialError::UnknownSession(_))
    ));
    assert!(matches!(mgr.close(session.id), Err(SerialError::UnknownSession(_))));
}

// --- Native additions (term-engine integration, PRD SER-1, ST-3) ---

#[test]
fn permission_denied_has_the_dialout_hint() {
    assert_eq!(
        SerialError::PermissionDenied("/dev/ttyUSB9".into()).to_string(),
        "permission denied opening /dev/ttyUSB9: add your user to the \"dialout\" group (sudo usermod -aG dialout $USER), then log out and back in"
    );
}

#[test]
fn the_session_ended_line_is_the_v0215_note() {
    assert_eq!(
        SESSION_ENDED_LINE,
        "\r\n[Session ended. Close this tab or open the port again.]\r\n"
    );
    assert!(SESSION_ENDED_LINE.contains(SESSION_ENDED_NOTE));
}

#[test]
fn input_handle_writes_enter_as_cr_and_resize_is_ignored() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let session = mgr.open(request(&path), ring()).unwrap();
    resize_hook()(132, 50);
    let mut input = session.input;
    input.write_all(b"reboot\r").unwrap();
    let out = master_reads(&*pair.master, "reboot\r");
    assert!(out.contains("reboot\r"), "master read {out:?}");
    assert!(
        !out.contains("reboot\r\n") && !out.contains("reboot\n"),
        "Enter was changed: {out:?}"
    );
    mgr.close(session.id).unwrap();
}

#[test]
fn the_port_can_be_opened_again_after_close() {
    let (_pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let session = mgr.open(request(&path), ring()).unwrap();
    mgr.close(session.id).unwrap();
    assert!(reopens(&path), "the device stayed busy after closing");
}

#[test]
fn a_full_ring_holds_the_reader_back_without_losing_bytes() {
    const CAPACITY: usize = 64;
    const TOTAL: usize = 256 * 1024;
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let out = ByteRing::new(CAPACITY);
    let session = mgr.open(request(&path), out.clone()).unwrap();
    // Letters only: nothing a raw line discipline could change.
    let payload: Vec<u8> = (0..TOTAL).map(|i| b'a' + (i % 26) as u8).collect();
    let expected = payload.clone();
    let mut writer = pair.master.take_writer().unwrap();
    let sender = std::thread::spawn(move || {
        writer.write_all(&payload).unwrap();
        writer.flush().unwrap();
        writer
    });

    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(out.len(), CAPACITY, "the ring should be full");
    assert!(
        !sender.is_finished(),
        "the device could send everything into a full ring"
    );

    let mut got = Vec::with_capacity(TOTAL);
    let deadline = Instant::now() + Duration::from_secs(30);
    while got.len() < TOTAL && Instant::now() < deadline {
        match out.try_pop(usize::MAX) {
            Some(bytes) => got.extend(bytes),
            None => std::thread::sleep(Duration::from_micros(100)),
        }
    }
    assert_eq!(got.len(), TOTAL, "bytes were lost or never arrived");
    assert!(got == expected, "bytes arrived out of order or changed");
    let _writer = sender.join().unwrap();
    mgr.close(session.id).unwrap();
}

#[test]
fn a_closed_ring_ends_the_session_and_releases_the_device() {
    let (pair, path) = pty_pair();
    let mgr = SerialManager::default();
    let out = ring();
    mgr.open(request(&path), out.clone()).unwrap();
    out.close();
    // The reader notices the closed ring on the next output it has to deliver.
    let mut writer = pair.master.take_writer().unwrap();
    writer.write_all(b"anyone there?\r\n").unwrap();
    writer.flush().unwrap();
    assert!(reopens(&path), "the device stayed open after its ring was closed");
}
