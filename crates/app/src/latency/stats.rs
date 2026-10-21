//! Pure parts of the latency log (TERM-17): line formats, parsing and percentiles.
//!
//! Kept free of GPUI and threads so the report example can include this file as is.

/// A mark of the per-key log, in the order they happen for one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// The key handler received a key; the value is the bytes sent to the session.
    Key,
    /// Output that arrived after pending keys reached the view; the value is how many keys it answers.
    Output,
    /// The frame showing that output was submitted; the value is the key's latency in µs.
    Frame,
}

impl Mark {
    fn letter(self) -> char {
        match self {
            Self::Key => 'K',
            Self::Output => 'O',
            Self::Frame => 'F',
        }
    }
}

/// One line of `keys.log`: `<mark> <µs since the epoch> <value>` (the v0.2.15 `diag.rs` shape).
pub fn mark_line(mark: Mark, epoch_us: u128, value: u64) -> String {
    format!("{} {epoch_us} {value}\n", mark.letter())
}

/// One line of `frames.log`: `<µs since the epoch> <µs since the previous submitted frame>`.
pub fn frame_line(epoch_us: u128, interval_us: u64) -> String {
    format!("{epoch_us} {interval_us}\n")
}

/// Key latencies (µs) from the `F` marks of a `keys.log`; other or malformed lines are skipped.
pub fn key_latencies(log: &str) -> Vec<u64> {
    log.lines()
        .filter_map(|line| {
            let mut fields = line.split_ascii_whitespace();
            match (fields.next(), fields.next(), fields.next()) {
                (Some("F"), Some(_), Some(value)) => value.parse().ok(),
                _ => None,
            }
        })
        .collect()
}

/// `(µs since the epoch, interval µs)` pairs of a `frames.log`; malformed lines are skipped.
pub fn frame_intervals(log: &str) -> Vec<(u128, u64)> {
    log.lines()
        .filter_map(|line| {
            let mut fields = line.split_ascii_whitespace();
            Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
        })
        .collect()
}

/// Nearest-rank percentile `p` (0..=100) of ascending `sorted` samples; `None` when empty.
pub fn percentile(sorted: &[u64], p: u32) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let p = u64::from(p.min(100));
    let count = sorted.len() as u64;
    // Smallest rank r with r/count >= p/100, at least 1.
    let rank = (p * count).div_ceil(100).max(1);
    let index = usize::try_from(rank - 1).ok()?;
    sorted.get(index).copied()
}

/// Count, percentiles and maximum of a set of samples (µs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub count: usize,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
}

/// Summarises `samples` (sorted in place); `None` when there are none.
pub fn summarize(samples: &mut [u64]) -> Option<Summary> {
    samples.sort_unstable();
    Some(Summary {
        count: samples.len(),
        p50: percentile(samples, 50)?,
        p95: percentile(samples, 95)?,
        p99: percentile(samples, 99)?,
        max: *samples.last()?,
    })
}

/// The `latency_report` arguments: `<folder> [<from µs> <to µs>]`. A window given only in part,
/// a window that is not two integers, or extra arguments are errors, never silently ignored.
pub fn parse_report_args(args: &[String]) -> Result<(std::path::PathBuf, Option<(u128, u128)>), String> {
    const USAGE: &str = "usage: latency_report <folder> [<from µs> <to µs>]";
    match args {
        [folder] => Ok((folder.into(), None)),
        [folder, from, to] => match (from.parse(), to.parse()) {
            (Ok(from), Ok(to)) => Ok((folder.into(), Some((from, to)))),
            _ => Err("latency_report: the window is two integers (µs since the epoch)".to_owned()),
        },
        _ => Err(USAGE.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn report_arguments_take_a_folder_and_an_optional_whole_window() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(parse_report_args(&args(&["logs"])), Ok(("logs".into(), None)));
        assert_eq!(
            parse_report_args(&args(&["logs", "10", "20"])),
            Ok(("logs".into(), Some((10, 20))))
        );
        assert!(
            parse_report_args(&args(&["logs", "10"])).is_err(),
            "half a window is an error"
        );
        assert!(
            parse_report_args(&args(&["logs", "10", "20", "30"])).is_err(),
            "extra arguments are an error"
        );
        assert!(parse_report_args(&args(&["logs", "x", "20"])).is_err());
        assert!(parse_report_args(&args(&[])).is_err());
    }

    use super::*;

    #[test]
    fn percentile_uses_nearest_rank() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&samples, 50), Some(50));
        assert_eq!(percentile(&samples, 95), Some(95));
        assert_eq!(percentile(&samples, 99), Some(99));
        assert_eq!(percentile(&samples, 100), Some(100));
        assert_eq!(percentile(&samples, 0), Some(1));
        assert_eq!(percentile(&[7, 9, 30], 50), Some(9));
        assert_eq!(percentile(&[7, 9, 30], 95), Some(30));
        assert_eq!(percentile(&[], 50), None);
    }

    #[test]
    fn summary_sorts_and_reports_the_maximum() {
        let mut samples = vec![900, 100, 300, 200, 5_000];
        let summary = summarize(&mut samples);
        assert_eq!(
            summary,
            Some(Summary {
                count: 5,
                p50: 300,
                p95: 5_000,
                p99: 5_000,
                max: 5_000
            })
        );
        assert_eq!(summarize(&mut []), None);
    }

    #[test]
    fn lines_have_the_documented_shape_and_parse_back() {
        assert_eq!(mark_line(Mark::Key, 1_700_000_000_000_001, 1), "K 1700000000000001 1\n");
        assert_eq!(mark_line(Mark::Output, 12, 2), "O 12 2\n");
        assert_eq!(mark_line(Mark::Frame, 13, 4_321), "F 13 4321\n");
        assert_eq!(frame_line(99, 16_667), "99 16667\n");

        let keys = format!(
            "{}{}{}{}garbage\nF x\n",
            mark_line(Mark::Key, 1, 1),
            mark_line(Mark::Output, 2, 1),
            mark_line(Mark::Frame, 3, 2_500),
            mark_line(Mark::Frame, 4, 7_000)
        );
        assert_eq!(key_latencies(&keys), vec![2_500, 7_000]);
        let frames = format!("{}{}bad line\n", frame_line(10, 0), frame_line(26, 16));
        assert_eq!(frame_intervals(&frames), vec![(10, 0), (26, 16)]);
    }
}
