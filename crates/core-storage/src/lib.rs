//! Small JSON documents in the app config directory, ported from PlexusTerm v0.2.15
//! `src-tauri/src/storage.rs` (PRD DATA-1, DATA-2).
//!
//! Writes are atomic (temp file + rename) and private to the user (0600 on Unix, the
//! profile ACLs on Windows), so a crash never leaves a half-written file and other users
//! cannot read it. The directory and file names are the ones v0.2.15 uses, so both apps read
//! the same files.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

/// The Tauri identifier of v0.2.15; the config directory is named after it.
pub const APP_IDENTIFIER: &str = "dev.plexusterm.app";

pub const SESSIONS_FILE: &str = "sessions.json";
pub const TUNNELS_FILE: &str = "tunnels.json";
pub const SETTINGS_FILE: &str = "settings.json";
pub const MACROS_FILE: &str = "macros.json";
pub const HISTORY_FILE: &str = "history.json";
pub const AI_CHATS_FILE: &str = "ai-chats.json";
pub const OPENCODE_GO_MODELS_FILE: &str = "opencode-go-models.json";

/// Largest file read for an import (`.ini` bookmarks, `~/.ssh/config`).
pub const IMPORT_MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Largest saved output or log chunk.
pub const TEXT_FILE_MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Upper bound for history.json: 200 hosts x 2000 commands fit well below it.
pub const HISTORY_MAX_BYTES: usize = 32 * 1024 * 1024;

/// A temp file of another process this old belongs to a writer that died.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(600);

/// Makes temp file names unique across concurrent writes in this process.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
/// Serializes writes made by this process: Windows refuses concurrent renames onto one file.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub enum StorageError {
    InvalidJson(String),
    Io(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(f, "invalid JSON: {detail}"),
            Self::Io(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for StorageError {}

/// `<base>/dev.plexusterm.app`, exactly as Tauri's `app_config_dir` builds it.
pub fn config_dir_in(base: Option<PathBuf>) -> Option<PathBuf> {
    base.map(|base| base.join(APP_IDENTIFIER))
}

/// The config directory: `$XDG_CONFIG_HOME` (or `~/.config`) on Linux, Roaming `%APPDATA%` on
/// Windows, followed by [`APP_IDENTIFIER`] (PRD DATA-1).
pub fn config_dir() -> Option<PathBuf> {
    config_dir_in(dirs::config_dir())
}

/// Returns the file contents, or `None` when it does not exist yet.
pub fn load(path: &Path) -> Result<Option<String>, StorageError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io(path, err)),
    }
}

/// Validates `contents` as JSON and atomically replaces the file.
pub fn save(path: &Path, contents: &str) -> Result<(), StorageError> {
    serde_json::from_str::<serde_json::Value>(contents).map_err(|err| StorageError::InvalidJson(err.to_string()))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| io(parent, err))?;
    }
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    remove_stale_temps(path);
    replace_atomically(path, contents.as_bytes(), Some(0o600)).map_err(|err| io(path, err))
}

/// Replaces history.json, refusing invalid JSON and oversized contents.
pub fn save_history(path: &Path, contents: &str) -> Result<(), StorageError> {
    if contents.len() > HISTORY_MAX_BYTES {
        return Err(StorageError::Io(format!(
            "{}: contents are too large (limit {HISTORY_MAX_BYTES} bytes)",
            path.display()
        )));
    }
    save(path, contents)
}

/// Reads a file the user picked for import, refusing anything larger than `max_bytes`.
pub fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, StorageError> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|err| io(path, err))?;
    let mut bytes = Vec::new();
    // Read one byte past the limit to detect oversized files without loading them whole.
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| io(path, err))?;
    if bytes.len() as u64 > max_bytes {
        return Err(StorageError::Io(format!(
            "{}: file is too large (limit {max_bytes} bytes)",
            path.display()
        )));
    }
    Ok(bytes)
}

/// Writes a text file the user chose (e.g. saved terminal output), replacing it.
/// Refuses contents larger than `max_bytes` before touching the file.
pub fn write_text(path: &Path, contents: &str, max_bytes: u64) -> Result<(), StorageError> {
    if contents.len() as u64 > max_bytes {
        return Err(too_large(path, max_bytes));
    }
    // In place, as v0.2.15: a file the user chose keeps its owner, group, ACLs and links, and a
    // writable file in a read-only directory can still be written (a temp + rename would not).
    std::fs::write(path, contents).map_err(|err| io(path, err))
}

/// Appends to a text file the user chose (e.g. a session log), creating it when missing.
/// Refuses chunks larger than `max_bytes`; the parent directory must exist.
pub fn append_text(path: &Path, contents: &str, max_bytes: u64) -> Result<(), StorageError> {
    if contents.len() as u64 > max_bytes {
        return Err(too_large(path, max_bytes));
    }
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|err| io(path, err))?;
    file.write_all(contents.as_bytes()).map_err(|err| io(path, err))
}

fn io(path: &Path, err: std::io::Error) -> StorageError {
    StorageError::Io(format!("{}: {err}", path.display()))
}

fn too_large(path: &Path, max_bytes: u64) -> StorageError {
    StorageError::Io(format!(
        "{}: contents are too large (limit {max_bytes} bytes)",
        path.display()
    ))
}

/// Renames `from` onto `to`, retrying briefly on Windows where an antivirus or the indexer may
/// hold the target open.
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(err) if cfg!(windows) && err.kind() == std::io::ErrorKind::PermissionDenied && attempt < 10 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20 * attempt));
            }
            result => return result,
        }
    }
}

/// Writes `contents` to a new temp file next to `path`, makes it durable and renames it over
/// `path`, then makes the rename durable: the file is either the old one or the new one, never
/// partial, and a reported success survives a crash. `mode` (Unix) sets the new file's permissions.
fn replace_atomically(path: &Path, contents: &[u8], mode: Option<u32>) -> std::io::Result<()> {
    let tmp = temp_path(path);
    let result = write_new(&tmp, contents, mode).and_then(|()| replace_file(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    sync_parent(path);
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    name.push(format!(".tmp-{}-{seq}", std::process::id()));
    path.with_file_name(name)
}

/// Removes `<name>.tmp-<pid>-<n>` files of this document left by a writer that died before its
/// rename. Temp files of this process, and recent ones of another process (v0.2.15 may be
/// writing the same document right now), are kept.
fn remove_stale_temps(path: &Path) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    let prefix = format!("{name}.tmp-");
    let ours = format!("{prefix}{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else { continue };
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age >= STALE_TEMP_AGE));
        if file_name.starts_with(&prefix) && !file_name.starts_with(&ours) && old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Creates `path` (it must not exist) with `mode` on Unix and writes `contents` durably.
fn write_new(path: &Path, contents: &[u8], mode: Option<u32>) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(mode) = mode {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = match options.open(path) {
        // A temp file of a crashed process that had our pid (pid namespaces reuse small pids).
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(path)?;
            options.open(path)?
        }
        result => result?,
    };
    // The umask may have narrowed the requested mode; set it exactly.
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
    }
    file.write_all(contents)?;
    file.sync_all()
}

/// Makes a rename in `path`'s directory durable (Unix); best effort.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leftover_temp_file_with_our_own_name_is_replaced() {
        let dir = std::env::temp_dir().join(format!("plexus-core-storage-{}-{}", std::process::id(), line!()));
        std::fs::create_dir(&dir).unwrap();
        let tmp = dir.join("settings.json.tmp-1-0");
        std::fs::write(&tmp, "stale").unwrap();
        write_new(&tmp, b"fresh", Some(0o600)).unwrap();
        assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "fresh");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
