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
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|err| io(path, err))?;
    file.write_all(contents.as_bytes()).map_err(|err| io(path, err))?;
    // Durable once reported. Pipes, devices and some network or FUSE file systems cannot be
    // synced (EINVAL / ENOTSUP); the bytes were written, so that is not a failure.
    match file.sync_all() {
        Err(err) if !syncable(&err) => Ok(()),
        result => result.map_err(|err| io(path, err)),
    }
}

/// Whether a failed fsync is a real error rather than a target that cannot be synced.
fn syncable(err: &std::io::Error) -> bool {
    !matches!(
        err.kind(),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::Unsupported
    )
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
    let tmp = write_temp(path, contents, mode, temp_path)?;
    let result = replace_file(&tmp, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    sync_parent(path);
    Ok(())
}

/// `<name>.tmp-<pid>-<seq><nanos>`: unique within this process by `seq`, and across processes
/// sharing a pid (pid namespaces) by the clock.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    name.push(format!(".tmp-{}-{seq}{nanos:09}", std::process::id()));
    path.with_file_name(name)
}

/// Writes `contents` to a new temp file beside `path` and returns its path. A name that is
/// already taken (another live writer, or a leftover) is never touched: another name is tried.
fn write_temp(
    path: &Path,
    contents: &[u8],
    mode: Option<u32>,
    mut next_name: impl FnMut(&Path) -> PathBuf,
) -> std::io::Result<PathBuf> {
    let mut attempts = 0;
    loop {
        let tmp = next_name(path);
        match write_new(&tmp, contents, mode) {
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && attempts < 8 => attempts += 1,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::AlreadyExists {
                    let _ = std::fs::remove_file(&tmp);
                }
                return Err(err);
            }
            Ok(()) => return Ok(tmp),
        }
    }
}

/// Removes `<name>.tmp-*` files of this document left by a writer that died before its rename.
/// Only files older than [`STALE_TEMP_AGE`] go: a live writer (this process, v0.2.15, or another
/// process with the same pid in another namespace) finishes its temp file long before that.
fn remove_stale_temps(path: &Path) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    let prefix = format!("{name}.tmp-");
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else { continue };
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age >= STALE_TEMP_AGE));
        if file_name.starts_with(&prefix) && old {
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
    let mut file = options.open(path)?;
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

    fn scratch(line: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plexus-core-storage-{}-{line}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn a_taken_temp_name_is_left_alone_and_another_name_is_used() {
        let dir = scratch(line!());
        let target = dir.join("settings.json");
        let taken = dir.join("settings.json.tmp-1-0");
        std::fs::write(&taken, "another writer's data").unwrap();
        let mut names = vec![dir.join("settings.json.tmp-1-1"), taken.clone()];
        let tmp = write_temp(&target, b"fresh", Some(0o600), |_| names.pop().unwrap()).unwrap();
        assert_eq!(tmp, dir.join("settings.json.tmp-1-1"));
        assert_eq!(
            std::fs::read_to_string(&taken).unwrap(),
            "another writer's data",
            "never unlinked"
        );
        assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "fresh");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_temp_files_of_this_pid_are_swept_too() {
        let dir = scratch(line!());
        let target = dir.join("settings.json");
        let leftover = dir.join(format!("settings.json.tmp-{}-0", std::process::id()));
        let file = std::fs::File::create(&leftover).unwrap();
        file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
            .unwrap();
        remove_stale_temps(&target);
        assert!(!leftover.exists(), "a crashed process with our pid left it");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
