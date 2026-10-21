//! File operations on the local disk for the sidebar's Explorer and the file viewer, ported from
//! PlexusTerm v0.2.15 `src-tauri/src/localfs.rs` (PRD LOC-1, LOC-2, VIEW-2, VIEW-5).
//!
//! Entries have the same shape as the SFTP browser's, so both panels share their dialogs.
//! Paths are native (`/home/me` or `C:\Users\me`; [`native`] expands a leading `~/`); errors read
//! `cannot <action> <path>: <cause>`.
//!
//! Two v0.2.15 races are fixed: saving writes a new file and renames it over the old one
//! ([`write_text`], PRD FIX-1), and renames and moves never replace an entry created between a
//! check and the rename ([`rename`], [`move_into`], PRD FIX-2).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

mod noreplace;
mod open;
mod save;
mod text;

pub use open::{
    OpenWith, chosen_editor_command, desktop_exec, desktop_exec_args, launch, spawn_detached, text_editor_command,
};
pub use text::{
    MAX_ATTACH_BYTES, MAX_IMAGE_BYTES, MAX_VIEW_BYTES, TextFile, check_unchanged, decode_text, encode_text,
    image_too_large, is_image_path, listing_text, not_an_image, read_image, read_text, text_for, text_from, view_text,
    write_text,
};

/// Highest permission bits `chmod` accepts (`07777`).
pub const MAX_MODE: u32 = 0o7777;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

/// One file, folder or symlink of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Unix seconds.
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
    /// Owner ids (Unix); the names are only known over SFTP.
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub user: Option<String>,
    pub group: Option<String>,
}

pub(crate) fn failure(action: &str, path: &Path, err: impl std::fmt::Display) -> String {
    format!("cannot {action} {}: {err}", path.display())
}

fn entry_of(path: &Path, name: String) -> Result<Entry, std::io::Error> {
    let link = fs::symlink_metadata(path)?;
    // A symlink keeps its kind; its size and date are those of the file it points to, when it exists.
    let meta = if link.file_type().is_symlink() {
        fs::metadata(path).unwrap_or(link.clone())
    } else {
        link.clone()
    };
    let kind = if link.file_type().is_symlink() {
        EntryKind::Symlink
    } else if meta.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| age.as_secs());
    #[cfg(unix)]
    let (permissions, uid, gid) = {
        use std::os::unix::fs::MetadataExt;
        (Some(link.mode() & MAX_MODE), Some(link.uid()), Some(link.gid()))
    };
    #[cfg(not(unix))]
    let (permissions, uid, gid) = (None, None, None);
    Ok(Entry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        size: if meta.is_dir() { 0 } else { meta.len() },
        modified,
        permissions,
        uid,
        gid,
        user: None,
        group: None,
    })
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// A path as the user typed it, with a leading `~/` (also `~\` on Windows) expanded to the home folder.
pub fn native(path: &str) -> PathBuf {
    proto_ssh::platform::expand_home(path)
}

/// The user's home folder.
pub fn home() -> Result<String, String> {
    proto_ssh::platform::home_dir()
        .map(|home| home.to_string_lossy().into_owned())
        .ok_or_else(|| "cannot resolve the home folder".to_string())
}

/// Lists `path`: folders first, then by case-insensitive name. Unreadable entries are skipped.
pub fn list(path: &Path) -> Result<Vec<Entry>, String> {
    let dir = fs::read_dir(path).map_err(|err| failure("list", path, err))?;
    let mut entries: Vec<Entry> = dir
        .filter_map(Result::ok)
        .filter_map(|item| entry_of(&item.path(), item.file_name().to_string_lossy().into_owned()).ok())
        .collect();
    entries.sort_by_cached_key(|entry| {
        (
            entry.kind != EntryKind::Dir,
            entry.name.to_lowercase(),
            entry.name.clone(),
        )
    });
    Ok(entries)
}

/// One file or folder; a symlink is reported as such.
pub fn stat(path: &Path) -> Result<Entry, String> {
    entry_of(path, name_of(path)).map_err(|err| failure("read", path, err))
}

pub fn mkdir(path: &Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|err| failure("create", path, err))
}

/// Creates an empty file; fails if anything already has that name.
pub fn create_file(path: &Path) -> Result<(), String> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(drop)
        .map_err(|err| failure("create", path, err))
}

/// The error of a rename that would have replaced `to`, or the rename's own one.
fn rename_failure(action: &str, from: &Path, to: &Path, err: &std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::AlreadyExists {
        failure(action, from, format!("{} already exists", to.display()))
    } else {
        failure(action, from, err)
    }
}

/// Renames `from` to `to`; never replaces an existing `to`, even one created while renaming.
pub fn rename(from: &Path, to: &Path) -> Result<(), String> {
    noreplace::rename_noreplace(from, to).map_err(|err| rename_failure("rename", from, to, &err))
}

/// Moves `from` into the folder `dir`, keeping its name, and returns where it ended up.
/// Never replaces an existing entry (not even one created while moving), and never moves a
/// folder into itself or one of its subfolders.
pub fn move_into(from: &Path, dir: &Path) -> Result<PathBuf, String> {
    let from = without_trailing_separators(from);
    let dir = without_trailing_separators(dir);
    let name = from
        .file_name()
        .ok_or_else(|| failure("move", &from, "not a file name"))?;
    if dir.starts_with(&from) {
        return Err(failure("move", &from, "a folder cannot be moved into itself"));
    }
    if from.parent() == Some(dir.as_path()) {
        return Err(failure("move", &from, format!("it is already in {}", dir.display())));
    }
    let to = dir.join(name);
    noreplace::rename_noreplace(&from, &to).map_err(|err| rename_failure("move", &from, &to, &err))?;
    Ok(to)
}

/// Removes a file, a symlink (never what it points to), or a folder with everything inside it.
pub fn remove(path: &Path, is_dir: bool) -> Result<(), String> {
    // `link/` would resolve the symlink and delete what it points to.
    let path = &without_trailing_separators(path);
    let link = fs::symlink_metadata(path).map_err(|err| failure("delete", path, err))?;
    let result = if is_dir && link.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|err| failure("delete", path, err))
}

/// `path` without trailing `/` (or `\` on Windows); a root keeps its separator.
fn without_trailing_separators(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    let trimmed = text.trim_end_matches(std::path::is_separator);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        return path.to_path_buf();
    }
    PathBuf::from(trimmed)
}

pub fn chmod(path: &Path, mode: u32) -> Result<(), String> {
    const ACTION: &str = "change permissions of";
    if mode > MAX_MODE {
        return Err(failure(ACTION, path, format!("invalid mode {mode:#o}")));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|err| failure(ACTION, path, err))
    }
    #[cfg(not(unix))]
    {
        Err(failure(ACTION, path, "not supported on this system"))
    }
}

#[cfg(test)]
mod tests;
