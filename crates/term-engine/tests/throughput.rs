//! NFR-2 tier-1 engine throughput, without a display (PRD §10).
//!
//! Ignored by default; run it in release:
//! `cargo test -p term-engine --release --test throughput -- --ignored --nocapture`.
//! It feeds 100 MiB of mixed output (plain text, SGR colours, CJK, cursor motion) through a
//! `ByteRing` into a `Session` while a "UI" thread takes a snapshot every millisecond, and prints
//! the parse rate and how long the UI waited for a snapshot (lock wait plus frame copy, ST-2).
//! With `PLEXUS_THROUGHPUT_CORPUS=<file>` it also writes the corpus there, for the in-app
//! `cat` measurement.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers outside #[test] functions"
)]

use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use term_engine::{ByteRing, EndReason, Session, SessionConfig, SessionHooks, SessionState};

const TOTAL: usize = 100 * 1024 * 1024;
const BLOCK: usize = 1024 * 1024;
const END_MARK: &str = "END-OF-CORPUS";

/// About one MiB of mixed output; `seed` varies colours and positions between blocks.
fn block(seed: usize) -> Vec<u8> {
    let mut out = String::with_capacity(BLOCK + 4096);
    let mut n = seed * 10_000;
    while out.len() < BLOCK {
        match n % 4 {
            0 => {
                let _ = write!(
                    out,
                    "{n:>8} plain text: the quick brown fox jumps over the lazy dog 0123456789\r\n"
                );
            }
            1 => {
                let _ = write!(
                    out,
                    "\x1b[1;31mERROR\x1b[0m \x1b[38;5;{}mindexed\x1b[0m \x1b[38;2;{};{};{}mtruecolor\x1b[0m \
                     \x1b[48;5;{}m\x1b[4mbg+underline\x1b[0m \x1b[3;32mitalic green\x1b[0m {n}\r\n",
                    n % 256,
                    n % 255,
                    (n * 7) % 255,
                    (n * 13) % 255,
                    (n * 3) % 256,
                );
            }
            2 => {
                let _ = write!(out, "日本語のテキスト 中文字符 한국어 텍스트 全角ＡＢＣ {n}\r\n");
            }
            _ => {
                // Absolute and relative cursor motion, erase to end of line, then back to the bottom.
                let _ = write!(
                    out,
                    "\x1b7\x1b[{};{}H*\x1b[K\x1b[2A\x1b[3C+\x1b[1B\x1b[2D-\x1b8\x1b[2K{n} moved\r\n",
                    1 + n % 38,
                    1 + (n * 7) % 140,
                );
            }
        }
        n += 1;
    }
    out.into_bytes()
}

fn corpus_blocks() -> impl Iterator<Item = Vec<u8>> {
    (0..TOTAL / BLOCK).map(block)
}

fn micros(d: Duration) -> u128 {
    d.as_micros()
}

#[test]
#[ignore = "throughput measurement; run in release with --ignored --nocapture"]
fn feeds_100_mib_of_mixed_output() {
    if let Some(path) = std::env::var_os("PLEXUS_THROUGHPUT_CORPUS") {
        let mut file = io::BufWriter::new(std::fs::File::create(&path).unwrap());
        for bytes in corpus_blocks() {
            file.write_all(&bytes).unwrap();
        }
        writeln!(file, "{END_MARK}").unwrap();
        file.flush().unwrap();
        println!("corpus written to {}", path.to_string_lossy());
    }

    let blocks: Vec<Vec<u8>> = corpus_blocks().collect();
    let total: usize = blocks.iter().map(Vec::len).sum::<usize>() + END_MARK.len();

    let ring = ByteRing::new(ByteRing::SESSION_CAPACITY);
    let hooks = SessionHooks {
        on_event: Arc::new(|_| {}),
        on_resize: Arc::new(|_, _| {}),
    };
    let config = SessionConfig {
        columns: 140,
        rows: 38,
        scrollback: 10_000,
    };
    let session = Arc::new(Session::start(config, ring.clone(), Box::new(io::sink()), hooks));

    let started = Instant::now();
    let producer = thread::spawn({
        let ring = ring.clone();
        move || {
            for bytes in blocks {
                for chunk in bytes.chunks(64 * 1024) {
                    // Blocks while the ring is full (ST-3 backpressure): never dropped.
                    ring.push(chunk).unwrap();
                }
            }
            ring.push(END_MARK.as_bytes()).unwrap();
            ring.close();
        }
    });

    // The "UI": one snapshot per millisecond, timing each call.
    let mut waits: Vec<u128> = Vec::new();
    let mut frames = 0_usize;
    while session.state() == SessionState::Running {
        let asked = Instant::now();
        if session.snapshot().is_some() {
            frames += 1;
        }
        waits.push(micros(asked.elapsed()));
        thread::sleep(Duration::from_millis(1));
    }
    let elapsed = started.elapsed();
    producer.join().unwrap();

    assert_eq!(session.state(), SessionState::Ended(EndReason::SourceClosed));
    let screen = session.full_frame();
    let last = (0..usize::from(screen.rows))
        .map(|row| screen.text_of_row(row))
        .any(|text| text.contains(END_MARK));
    assert!(last, "the whole corpus was parsed, up to its last line");

    waits.sort_unstable();
    let pick = |p: usize| waits[(waits.len() * p / 100).min(waits.len() - 1)];
    #[allow(clippy::cast_precision_loss)]
    let mib_s = total as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64();
    println!(
        "throughput: {total} bytes in {:.3} s = {mib_s:.1} MiB/s; snapshots {} (with damage {frames}); \
         UI snapshot time p50 {} µs, p99 {} µs, max {} µs",
        elapsed.as_secs_f64(),
        waits.len(),
        pick(50),
        pick(99),
        waits.last().unwrap(),
    );
}
