//! Back ends of the Tools menu (PRD TOOL-0..TOOL-4), ported from PlexusTerm v0.2.15
//! `src-tauri/src/tools` without any UI types: plain functions and structs, with callbacks
//! where v0.2.15 used Tauri channels.
//!
//! - [`keygen`]: SSH key generator (generate, load OpenSSH/PPK, save, export PPK v3, install).
//! - [`scan`]: TCP connect port scanner and network scanner, streamed and cancellable.
//! - [`wol`]: Wake On LAN magic packet.
//! - [`monitor`]: remote monitor sample over an existing SSH session.

pub mod keygen;

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, still usable after a panic elsewhere: every value behind these locks stays
/// consistent between statements.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
