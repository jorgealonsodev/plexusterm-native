//! Summarises a `PLEXUS_LATENCY_LOG` folder (TERM-17, PRD §10 tier 1).
//!
//! `cargo run -p plexusterm --example latency_report -- <folder> [<from µs> <to µs>]` prints the
//! key latency percentiles of `keys.log` and the frame-interval percentiles of `frames.log`;
//! with a window (µs since the epoch) only intervals that start and end inside it count, e.g.
//! while a `cat` of a large file runs. The first frame (interval 0) never counts.

use std::path::Path;
use std::process::ExitCode;

#[path = "../src/latency/stats.rs"]
#[allow(dead_code, reason = "the line writers are used by the app, not by this report")]
mod stats;

use stats::{Summary, frame_intervals, key_latencies, summarize};

fn ms(us: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "µs values are far below 2^52")]
    let ms = us as f64 / 1000.0;
    format!("{ms:.3}")
}

fn print(label: &str, summary: Option<Summary>, extra: &str) {
    match summary {
        Some(s) => println!(
            "{label}: n={} p50={} ms p95={} ms p99={} ms max={} ms{extra}",
            s.count,
            ms(s.p50),
            ms(s.p95),
            ms(s.p99),
            ms(s.max)
        ),
        None => println!("{label}: no samples"),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(folder) = args.first().map(Path::new) else {
        eprintln!("usage: latency_report <folder> [<from µs> <to µs>]");
        return ExitCode::FAILURE;
    };
    let window: Option<(u128, u128)> = match (args.get(1), args.get(2)) {
        (Some(from), Some(to)) => match (from.parse(), to.parse()) {
            (Ok(from), Ok(to)) => Some((from, to)),
            _ => {
                eprintln!("latency_report: the window is two integers (µs since the epoch)");
                return ExitCode::FAILURE;
            }
        },
        _ => None,
    };
    let read = |name: &str| std::fs::read_to_string(folder.join(name)).unwrap_or_default();

    let mut keys = key_latencies(&read("keys.log"));
    print("key_received -> frame_submitted", summarize(&mut keys), "");

    let mut intervals: Vec<u64> = frame_intervals(&read("frames.log"))
        .into_iter()
        .filter(|&(at, interval)| {
            interval > 0 && window.is_none_or(|(from, to)| at.saturating_sub(u128::from(interval)) >= from && at <= to)
        })
        .map(|(_, interval)| interval)
        .collect();
    let over_100ms = intervals.iter().filter(|&&us| us > 100_000).count();
    print(
        "frame interval",
        summarize(&mut intervals),
        &format!(" over_100ms={over_100ms}"),
    );
    ExitCode::SUCCESS
}
