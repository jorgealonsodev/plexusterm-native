//! Renaming without ever replacing the destination (PRD FIX-2).
//!
//! v0.2.15 checked that the destination was free, then renamed: an entry created in between was
//! replaced (a file silently lost, or an empty folder swallowed). Here the kernel refuses the
//! rename itself when the destination exists: `renameat2(RENAME_NOREPLACE)` on Linux and
//! `renameatx_np(RENAME_EXCL)` on macOS, both through rustix's safe wrappers.
//!
//! Where that is unavailable (another system, or a file system that does not support the flag),
//! a file or symlink gets a hard link at the new name, which also fails when the name is taken,
//! and then loses its old name. Only a folder on such a system is still checked, then renamed.

use std::io::{Error, ErrorKind};
use std::path::Path;

/// Renames `from` to `to`, failing with [`ErrorKind::AlreadyExists`] when `to` exists.
pub(crate) fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
            Ok(()) => return Ok(()),
            // The kernel or the file system does not know the flag.
            Err(err) if unsupported(err) => {}
            Err(err) => return Err(err.into()),
        }
    }
    fallback(from, to)
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn unsupported(err: rustix::io::Errno) -> bool {
    use rustix::io::Errno;
    err == Errno::INVAL || err == Errno::NOSYS || err == Errno::NOTSUP || err == Errno::OPNOTSUPP
}

/// The rename without the kernel's help (see the module documentation).
fn fallback(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.is_dir() {
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(Error::new(ErrorKind::AlreadyExists, "the destination exists"));
        }
        return std::fs::rename(from, to);
    }
    // A hard link to a symlink links the symlink itself (`linkat` without `AT_SYMLINK_FOLLOW`).
    std::fs::hard_link(from, to)?;
    if let Err(err) = std::fs::remove_file(from) {
        // Undo: the entry keeps its one old name.
        let _ = std::fs::remove_file(to);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback keeps its promise too, for systems without the kernel flag.
    #[test]
    fn the_fallback_never_replaces_and_moves_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        assert_eq!(fallback(&a, &b).unwrap_err().kind(), ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "b");
        let c = dir.path().join("c.txt");
        fallback(&a, &c).unwrap();
        assert!(!a.exists());
        assert_eq!(std::fs::read_to_string(&c).unwrap(), "a");

        let (d, e) = (dir.path().join("d"), dir.path().join("e"));
        std::fs::create_dir_all(d.join("inner")).unwrap();
        std::fs::create_dir(&e).unwrap();
        assert_eq!(fallback(&d, &e).unwrap_err().kind(), ErrorKind::AlreadyExists);
        fallback(&d, &dir.path().join("f")).unwrap();
        assert!(dir.path().join("f/inner").is_dir());
    }
}
