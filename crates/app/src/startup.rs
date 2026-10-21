//! NFR-3 startup phases (PRD §10), printed only by builds with the `bench` feature.
//!
//! With `PLEXUS_EXIT_AFTER_FIRST_PROMPT=1`, every phase prints once to stderr as
//! `plexusterm: phase <name> <µs since main>`, and `main` prints the wall clock at its start
//! (`plexusterm: main at <µs since the epoch>`) so a launcher can measure exec → `main`. Without
//! the feature, [`mark`] is an empty inline function.

#[cfg(feature = "bench")]
mod imp {
    use std::sync::{Mutex, OnceLock, PoisonError};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    static STARTED: OnceLock<Instant> = OnceLock::new();
    static SEEN: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    /// When `main` started.
    pub fn started() -> Instant {
        *STARTED.get_or_init(Instant::now)
    }

    /// Whether this run measures its startup.
    pub fn enabled() -> bool {
        std::env::var_os("PLEXUS_EXIT_AFTER_FIRST_PROMPT").as_deref() == Some(std::ffi::OsStr::new("1"))
    }

    /// Called first thing in `main`.
    pub fn begin() {
        let _ = started();
        if enabled() {
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros())
                .unwrap_or(0);
            eprintln!("plexusterm: main at {epoch}");
        }
    }

    /// Prints `phase` the first time it is reached.
    pub fn mark(phase: &'static str) {
        if !enabled() {
            return;
        }
        let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
        if seen.contains(&phase) {
            return;
        }
        seen.push(phase);
        eprintln!("plexusterm: phase {phase} {} µs", started().elapsed().as_micros());
    }
}

#[cfg(feature = "bench")]
pub use imp::{begin, enabled, mark, started};

#[cfg(not(feature = "bench"))]
#[inline(always)]
pub fn begin() {}

#[cfg(not(feature = "bench"))]
#[inline(always)]
pub fn mark(_phase: &'static str) {}
