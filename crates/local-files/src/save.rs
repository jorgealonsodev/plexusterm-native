//! Saving a file's new contents (PRD FIX-1).
//!
//! v0.2.15 truncated the file and wrote it again, so a crash or a full disk midway left it empty
//! or cut. Here the contents go to a new file next to the real one (the final target of any
//! symlinks, so the link stays a link), which gets the old file's mode (and owner, on Unix) and
//! is flushed to disk before it is renamed over the old file; the folder is flushed after.
//!
//! Some files cannot be replaced by another one without losing something the in-place write of
//! v0.2.15 kept, so they are still written in place: a file with other hard links (they would
//! keep the old text), a file whose owner cannot be given to the new file (someone else's file
//! the user may write), anything that is not a regular file, and a file in a folder the user
//! cannot create files in. Extended attributes and ACLs are not copied.

use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Write};
use std::path::Path;

/// Replaces the contents of the existing file `path` with `bytes`.
pub(crate) fn replace_contents(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let target = fs::canonicalize(path)?;
    let meta = fs::metadata(&target)?;
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return write_in_place(&target, bytes);
    };
    if !meta.is_file() || has_other_links(&meta) {
        return write_in_place(&target, bytes);
    }
    let mut prefix = std::ffi::OsString::from(".");
    prefix.push(name);
    prefix.push(".");
    let temp = match tempfile::Builder::new()
        .prefix(&prefix)
        .suffix(".plexusterm-save")
        .tempfile_in(dir)
    {
        Ok(temp) => temp,
        // A file the user may write in a folder the user may not: written in place.
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return write_in_place(&target, bytes),
        Err(err) => return Err(err),
    };
    // Dropping `temp` on any early return removes the new file.
    if !same_owner(temp.as_file(), &meta)? {
        drop(temp);
        return write_in_place(&target, bytes);
    }
    let mut file = temp.as_file();
    file.write_all(bytes)?;
    // After the owner: changing the owner clears the set-user-id and set-group-id bits.
    file.set_permissions(meta.permissions())?;
    file.sync_all()?;
    temp.persist(&target).map_err(|err| err.error)?;
    sync_dir(dir);
    Ok(())
}

/// v0.2.15's in-place write.
fn write_in_place(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new().write(true).truncate(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn has_other_links(meta: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.nlink() > 1
}

#[cfg(not(unix))]
fn has_other_links(_meta: &Metadata) -> bool {
    false
}

/// Gives the new file `meta`'s owner and group; `false` when the system does not allow it.
#[cfg(unix)]
fn same_owner(file: &File, meta: &Metadata) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let new = file.metadata()?;
    if (new.uid(), new.gid()) == (meta.uid(), meta.gid()) {
        return Ok(true);
    }
    Ok(std::os::unix::fs::fchown(file, Some(meta.uid()), Some(meta.gid())).is_ok())
}

#[cfg(not(unix))]
fn same_owner(_file: &File, _meta: &Metadata) -> std::io::Result<bool> {
    Ok(true)
}

/// Makes the rename in `dir` durable (Unix); best effort, the new contents are already on disk.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(dir) = File::open(dir) {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}
