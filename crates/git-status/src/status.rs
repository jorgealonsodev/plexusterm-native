//! File status for the file lists (local Explorer and SFTP browser), as VS Code reads it (TREE-3).

use std::path::Path;
use std::time::Duration;

use crate::{quote, through_sh};

/// Most status entries kept for one list; the rest are dropped (e.g. thousands of ignored files).
pub const MAX_STATUS_ENTRIES: usize = 10_000;
/// Bytes of `git status` output read for one local list (and of a `HEAD` text).
pub const MAX_STATUS_BYTES: u64 = 8 * 1024 * 1024;
/// A list reads its status at most once per this interval (TREE-3); the caller keeps the pace.
pub const STATUS_INTERVAL: Duration = Duration::from_secs(2);

/// A file's git status, named after VS Code's `Status` (extensions/git/src/api/git.d.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    IndexModified,
    IndexAdded,
    IndexDeleted,
    IndexRenamed,
    IndexCopied,
    IndexTypeChanged,
    Modified,
    Deleted,
    Untracked,
    Ignored,
    IntentToAdd,
    IntentToRename,
    TypeChanged,
    AddedByUs,
    AddedByThem,
    DeletedByUs,
    DeletedByThem,
    BothAdded,
    BothDeleted,
    BothModified,
}

/// One status of a path under the listed folder. `path` is relative to that folder, `/`-separated;
/// a trailing `/` marks a whole folder (untracked or ignored as a block), and an empty path means
/// the listed folder itself is covered (it sits inside such a folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
}

/// The statuses of `XY` in `git status --porcelain=v1` (both columns may give one each), mapped
/// like VS Code's `Repository` does: a conflict pair or `??`/`!!` is one status; otherwise the
/// index column (`X`) and the working tree column (`Y`) give one each.
pub fn statuses_of_code(x: u8, y: u8) -> Vec<FileStatus> {
    use FileStatus::*;
    let pair = match (x, y) {
        (b'?', b'?') => Some(Untracked),
        (b'!', b'!') => Some(Ignored),
        (b'D', b'D') => Some(BothDeleted),
        (b'A', b'U') => Some(AddedByUs),
        (b'U', b'D') => Some(DeletedByThem),
        (b'U', b'A') => Some(AddedByThem),
        (b'D', b'U') => Some(DeletedByUs),
        (b'A', b'A') => Some(BothAdded),
        (b'U', b'U') => Some(BothModified),
        _ => None,
    };
    if let Some(status) = pair {
        return vec![status];
    }
    let index = match x {
        b'M' => Some(IndexModified),
        b'A' => Some(IndexAdded),
        b'D' => Some(IndexDeleted),
        b'R' => Some(IndexRenamed),
        b'C' => Some(IndexCopied),
        b'T' => Some(IndexTypeChanged),
        _ => None,
    };
    let tree = match y {
        b'M' => Some(Modified),
        b'D' => Some(Deleted),
        b'A' => Some(IntentToAdd),
        b'R' => Some(IntentToRename),
        b'T' => Some(TypeChanged),
        _ => None,
    };
    index.into_iter().chain(tree).collect()
}

/// Parses what [`remote_status_command`] prints: the listed folder's path inside its repository
/// (`git rev-parse --show-prefix`, e.g. `src/` or empty at the top) and a NUL, then
/// `git status --porcelain=v1 -z` (paths from the top of the repository; a rename or copy is
/// followed by its original path). Only paths under the folder are kept, relative to it, at most
/// [`MAX_STATUS_ENTRIES`]; a record cut short (output capped) and garbage are skipped.
pub fn parse_status(output: &[u8]) -> Vec<StatusEntry> {
    let Some(split) = output.iter().position(|&b| b == 0) else {
        return Vec::new();
    };
    let (Some(prefix), Some(rest)) = (output.get(..split), output.get(split + 1..)) else {
        return Vec::new();
    };
    let prefix = String::from_utf8_lossy(prefix);
    // Every complete record ends with a NUL; whatever follows the last one was cut.
    let complete = rest
        .iter()
        .rposition(|&b| b == 0)
        .and_then(|end| rest.get(..end))
        .unwrap_or_default();
    let mut fields = complete.split(|&b| b == 0);
    let mut entries = Vec::new();
    while let Some(record) = fields.next() {
        if entries.len() >= MAX_STATUS_ENTRIES {
            break;
        }
        let (Some(&x), Some(&y), Some(b' '), Some(path)) =
            (record.first(), record.get(1), record.get(2), record.get(3..))
        else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        if matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C') {
            fields.next(); // The original path.
        }
        let path = String::from_utf8_lossy(path);
        let Some(relative) = relative_to(&prefix, &path) else {
            continue;
        };
        for status in statuses_of_code(x, y) {
            if entries.len() < MAX_STATUS_ENTRIES {
                entries.push(StatusEntry {
                    path: relative.clone(),
                    status,
                });
            }
        }
    }
    entries
}

/// `path` (from the top of the repository) relative to the folder at `prefix`, or `None` when it
/// is not under it. A folder entry (`dir/`) that holds the listed folder covers it: `""`.
fn relative_to(prefix: &str, path: &str) -> Option<String> {
    if let Some(rest) = path.strip_prefix(prefix) {
        return Some(rest.to_string());
    }
    (path.ends_with('/') && prefix.starts_with(path)).then(String::new)
}

/// The statuses of the files under the local `folder`; empty outside a repository, without git,
/// or when git fails. Two read-only git runs: the folder's place in its repository, then the
/// status of that folder only (`-- .`), untracked and ignored folders reported as one entry
/// without walking into them, and no optional locks (never in the way of a git the user runs).
pub fn status_of(folder: &Path) -> Vec<StatusEntry> {
    if !folder.is_dir() {
        return Vec::new();
    }
    let Some(prefix) = run_git(folder, &["rev-parse", "--show-prefix"]) else {
        return Vec::new();
    };
    let Some(status) = run_git(folder, &STATUS_ARGS) else {
        return Vec::new();
    };
    let prefix = String::from_utf8_lossy(&prefix);
    let mut output = prefix.trim_end_matches('\n').as_bytes().to_vec();
    output.push(0);
    output.extend_from_slice(&status);
    parse_status(&output)
}

/// A local file's text in the last commit (`HEAD`), for the viewer's changed lines; `None` when it
/// is not in one (outside a repository, untracked, no commit yet), is not UTF-8, or is too large.
/// Read with `cat-file`, which never runs the repository's filters or text conversions.
pub fn head_text_of(file: &Path) -> Option<String> {
    let folder = file.parent()?;
    let name = file.file_name()?.to_str()?;
    let blob = run_git(folder, &["cat-file", "blob", &format!("HEAD:./{name}")])?;
    if blob.len() as u64 >= MAX_STATUS_BYTES {
        return None;
    }
    String::from_utf8(blob).ok()
}

const STATUS_ARGS: [&str; 6] = ["status", "--porcelain=v1", "-z", "--ignored=matching", "--", "."];

/// Runs `git --no-optional-locks <args>` in `folder`; its stdout (at most [`MAX_STATUS_BYTES`]),
/// or `None` when git is missing or fails.
fn run_git(folder: &Path, args: &[&str]) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        // Never the repository's own fsmonitor command: a folder from elsewhere (a downloaded
        // archive with a `.git`) could otherwise run any program just by being listed.
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().ok()?;
    let mut out = Vec::new();
    let read = child.stdout.take()?.take(MAX_STATUS_BYTES).read_to_end(&mut out);
    if out.len() as u64 >= MAX_STATUS_BYTES {
        // Capped: stop git rather than wait for output nobody reads.
        let _ = child.kill();
        let _ = child.wait();
        return Some(out);
    }
    let status = child.wait().ok()?;
    (read.is_ok() && status.success()).then_some(out)
}

/// The read-only command that prints what [`parse_status`] reads for the remote `folder`, or
/// nothing outside a repository or without git. It always ends with status 0 and runs through `sh`
/// whatever the user's login shell is. `None` for a folder that cannot be passed safely.
pub fn remote_status_command(folder: &str) -> Option<String> {
    if folder.is_empty() || folder.contains('\0') {
        return None;
    }
    let dir = quote(folder);
    let status = STATUS_ARGS.join(" ");
    let script = format!(
        "{{ p=$(git --no-optional-locks -C {dir} rev-parse --show-prefix) && printf '%s\\000' \"$p\" && git --no-optional-locks -c core.fsmonitor=false -C {dir} {status}; }} 2>/dev/null; exit 0"
    );
    Some(through_sh(&script))
}
