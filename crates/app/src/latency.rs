//! TERM-17 latency log, on only with `PLEXUS_LATENCY_LOG=<folder>` (PRD §4.1 row 11, §10).
//!
//! `<folder>/keys.log` gets one line per mark (`K` key received, `O` its echo reached the view,
//! `F` the frame showing it was submitted, with the key's latency in µs) and `<folder>/frames.log`
//! one line per submitted frame with the interval since the previous one. Only times and sizes,
//! never the text typed. Lines go to a writer thread, so the UI never waits on the disk; with the
//! variable unset the log is `None` and costs one branch per key and per frame.
//!
//! What "submitted" means at the pinned GPUI rev: the pane's paint callback runs inside
//! `Window::draw`, which GPUI's frame callback follows with `Window::present` (the platform
//! renderer's draw and surface present) in the same call. The paint callback queues a foreground
//! task, which the executor runs only after that frame callback returns, so the time it records
//! is at or after the present call returned. It does not include compositor or scan-out time.
//! Two GPUI draws are not followed by a present in the same call. The view's first paint happens
//! in `open_window`, so the pane reports it from the next frame callback, which presents that
//! scene. GPUI also draws a dirty window before dispatching a key, and a paint there is reported
//! when the key callback returns, up to one refresh before the next frame callback presents it;
//! such a key reads early. That needs an echo to reach the view between a frame and the next key,
//! which the 80 ms typing gap of `docs/benchmarks/m0.md` rarely leaves; it was not counted.

#[allow(
    dead_code,
    reason = "its parsing and percentiles serve examples/latency_report.rs and tests"
)]
pub mod stats;

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use self::stats::{Mark, frame_line, mark_line};

/// The environment variable naming the log folder.
pub const ENV: &str = "PLEXUS_LATENCY_LOG";
/// Buffered lines reach the files at least this often, so a killed app loses at most this much.
const FLUSH_AFTER: Duration = Duration::from_millis(200);
/// Keys waiting for an echo; older ones are forgotten (a program that never echoes).
const MAX_WAITING: usize = 1024;

enum Line {
    Key(String),
    Frame(String),
}

/// The per-key and per-frame log; a no-op when disabled.
#[derive(Default)]
pub struct LatencyLog {
    active: Option<Box<Active>>,
}

struct Active {
    lines: Option<Sender<Line>>,
    writer: Option<JoinHandle<()>>,
    /// Keys sent to the session whose output has not reached the view yet.
    waiting: Vec<Instant>,
    /// Keys whose output is in the frame being drawn.
    echoed: Vec<Instant>,
    last_frame: Option<Instant>,
}

/// A painted frame, carried to the moment it has been submitted.
pub struct Painted {
    keys: Vec<Instant>,
}

impl LatencyLog {
    /// From the process environment.
    pub fn from_env() -> Self {
        Self::from_value(std::env::var_os(ENV).as_deref().map(Path::new))
    }

    /// Enabled when `folder` is set and not empty; a folder that cannot be opened disables it.
    pub fn from_value(folder: Option<&Path>) -> Self {
        match folder.filter(|f| !f.as_os_str().is_empty()) {
            None => Self::default(),
            Some(folder) => Self::open(folder).unwrap_or_else(|error| {
                eprintln!("plexusterm: {ENV}: cannot write to {}: {error}", folder.display());
                Self::default()
            }),
        }
    }

    fn open(folder: &Path) -> io::Result<Self> {
        fs::create_dir_all(folder)?;
        let open = |name: &str| -> io::Result<BufWriter<File>> {
            let file = OpenOptions::new().create(true).append(true).open(folder.join(name))?;
            Ok(BufWriter::new(file))
        };
        let (keys, frames) = (open("keys.log")?, open("frames.log")?);
        let (lines, received) = mpsc::channel();
        let writer = thread::Builder::new()
            .name("latency-log".to_owned())
            .spawn(move || write_lines(&received, keys, frames))?;
        Ok(Self {
            active: Some(Box::new(Active {
                lines: Some(lines),
                writer: Some(writer),
                waiting: Vec::new(),
                echoed: Vec::new(),
                last_frame: None,
            })),
        })
    }

    #[cfg(test)]
    pub fn is_enabled(&self) -> bool {
        self.active.is_some()
    }

    /// The key handler sent `bytes` to the session now.
    pub fn key(&mut self, bytes: usize) {
        let Some(active) = self.active.as_deref_mut() else {
            return;
        };
        if active.waiting.len() >= MAX_WAITING {
            active.waiting.remove(0);
        }
        active.waiting.push(Instant::now());
        active.send(Line::Key(mark_line(Mark::Key, epoch_us(), bytes as u64)));
    }

    /// Output reached the view for the frame being drawn: it is the echo of every waiting key.
    pub fn output(&mut self) {
        let Some(active) = self.active.as_deref_mut() else {
            return;
        };
        if active.waiting.is_empty() {
            return;
        }
        let count = active.waiting.len() as u64;
        let waiting = std::mem::take(&mut active.waiting);
        active.echoed.extend(waiting);
        active.send(Line::Key(mark_line(Mark::Output, epoch_us(), count)));
    }

    /// The pane painted a frame; pass the result to [`Self::submitted`] once it has been presented.
    pub fn painted(&mut self) -> Option<Painted> {
        let active = self.active.as_deref_mut()?;
        Some(Painted {
            keys: std::mem::take(&mut active.echoed),
        })
    }

    /// The painted frame has been submitted at `at`.
    pub fn submitted(&mut self, painted: Painted, at: Instant) {
        let Some(active) = self.active.as_deref_mut() else {
            return;
        };
        let now = epoch_us();
        for key in painted.keys {
            let latency = micros(at.saturating_duration_since(key));
            active.send(Line::Key(mark_line(Mark::Frame, now, latency)));
        }
        let interval = active
            .last_frame
            .map_or(0, |last| micros(at.saturating_duration_since(last)));
        active.last_frame = Some(at);
        active.send(Line::Frame(frame_line(now, interval)));
    }
}

impl Active {
    fn send(&self, line: Line) {
        if let Some(lines) = &self.lines {
            // A writer that failed only loses the diagnostics, never the pane.
            let _ = lines.send(line);
        }
    }
}

impl Drop for Active {
    fn drop(&mut self) {
        // Closing the channel makes the writer flush and end.
        self.lines = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

fn write_lines(lines: &mpsc::Receiver<Line>, mut keys: BufWriter<File>, mut frames: BufWriter<File>) {
    let mut flushed = Instant::now();
    loop {
        let written = match lines.recv_timeout(FLUSH_AFTER) {
            Ok(Line::Key(line)) => keys.write_all(line.as_bytes()),
            Ok(Line::Frame(line)) => frames.write_all(line.as_bytes()),
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Err(RecvTimeoutError::Disconnected) => {
                let _ = keys.flush().and_then(|()| frames.flush());
                return;
            }
        };
        // Also during a steady stream of lines, which never leaves the channel quiet.
        let written = written.and_then(|()| {
            if flushed.elapsed() >= FLUSH_AFTER {
                flushed = Instant::now();
                keys.flush().and_then(|()| frames.flush())
            } else {
                Ok(())
            }
        });
        if written.is_err() {
            return;
        }
    }
}

fn epoch_us() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("plexusterm-latency-{name}-{}-{nanos}", std::process::id()))
    }

    fn marks(log: &str) -> Vec<char> {
        log.lines().filter_map(|line| line.chars().next()).collect()
    }

    #[test]
    fn unset_or_empty_writes_nothing() {
        let mut log = LatencyLog::from_value(None);
        assert!(!log.is_enabled());
        log.key(1);
        log.output();
        assert!(log.painted().is_none());

        let empty = LatencyLog::from_value(Some(Path::new("")));
        assert!(!empty.is_enabled());
    }

    #[test]
    fn set_writes_key_output_and_frame_marks_in_order() {
        let dir = scratch("order");
        let mut log = LatencyLog::from_value(Some(&dir));
        assert!(log.is_enabled());

        // A frame with no key pending only adds a frame-time line.
        let idle = log.painted().unwrap();
        log.submitted(idle, Instant::now());
        log.key(1);
        log.key(3);
        // Their echo reaches the view in the next render.
        log.output();
        let painted = log.painted().unwrap();
        std::thread::sleep(Duration::from_millis(2));
        log.submitted(painted, Instant::now());
        // Output with no key waiting is not an echo.
        log.output();
        drop(log);

        let keys = fs::read_to_string(dir.join("keys.log")).unwrap();
        let frames = fs::read_to_string(dir.join("frames.log")).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(marks(&keys), vec!['K', 'K', 'O', 'F', 'F'], "{keys}");
        assert!(
            keys.lines().nth(1).unwrap().ends_with(" 3"),
            "bytes of the second key: {keys}"
        );
        assert!(
            keys.lines().nth(2).unwrap().ends_with(" 2"),
            "two keys answered: {keys}"
        );
        let latencies = stats::key_latencies(&keys);
        assert_eq!(latencies.len(), 2);
        assert!(latencies.iter().all(|&us| us >= 2_000), "{latencies:?}");
        let intervals = stats::frame_intervals(&frames);
        assert_eq!(intervals.len(), 2, "{frames}");
        assert_eq!(intervals[0].1, 0, "the first frame has no previous one");
        assert!(intervals[1].1 >= 2_000, "{frames}");
    }

    #[test]
    fn keys_wait_for_output_painted_after_them() {
        let dir = scratch("wait");
        let mut log = LatencyLog::from_value(Some(&dir));
        log.key(1);
        // This frame was painted before any output arrived: the key is still waiting.
        let early = log.painted().unwrap();
        log.submitted(early, Instant::now());
        log.output();
        let late = log.painted().unwrap();
        log.submitted(late, Instant::now());
        drop(log);
        let keys = fs::read_to_string(dir.join("keys.log")).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(marks(&keys), vec!['K', 'O', 'F'], "{keys}");
    }
}
