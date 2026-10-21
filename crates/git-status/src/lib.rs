//! The git branch of a terminal's folder and the git status of the file lists, ported from
//! PlexusTerm v0.2.15 `src-tauri/src/git.rs` (PRD DEV-3, TREE-3, TREE-4, VIEW-2), and the
//! VS Code decorations of the file lists from its frontend `features/files/gitStatus.ts` (TREE-1).
//!
//! Local branches are read straight from the repository files, without starting a `git` process:
//! walk up from the folder to the first `.git` (a folder, or a file `gitdir: <path>` in worktrees
//! and submodules) and read its `HEAD`. Local statuses run read-only `git` commands. Remote
//! folders (SSH tabs) run one read-only `git` command over the session's existing connection
//! ([`ssh_git_branch`], [`ssh_git_status`]).

use std::path::{Path, PathBuf};

mod decoration;
mod remote;
mod status;

pub use decoration::{Decoration, GitColor, GitLabel, StatusIndex, decoration_for, index_status, root_decoration};
pub use remote::{ssh_git_branch, ssh_git_status};
pub use status::{
    FileStatus, MAX_STATUS_BYTES, MAX_STATUS_ENTRIES, STATUS_INTERVAL, StatusEntry, head_text_of, parse_status,
    remote_status_command, status_of, statuses_of_code,
};

/// `HEAD` is one short line; anything longer is not a `HEAD` file.
const MAX_HEAD_BYTES: u64 = 4096;
/// Longest branch name shown.
const MAX_BRANCH_CHARS: usize = 200;
/// A detached `HEAD` shows its commit abbreviated like `git rev-parse --short`.
const SHORT_HASH: usize = 7;

/// The branch checked out in the repository holding `folder` (the short commit hash when `HEAD`
/// is detached), or `None` outside a repository or when `HEAD` cannot be read or makes no sense.
pub fn branch_of(folder: &Path) -> Option<String> {
    let (_, gitdir) = repository_of(folder)?;
    head_of(&gitdir?)
}

/// The folder of the repository holding `folder` (the first one up with a `.git`) and its git
/// folder (`None` when a `.git` file points nowhere).
fn repository_of(folder: &Path) -> Option<(&Path, Option<PathBuf>)> {
    let mut dir = Some(folder);
    while let Some(current) = dir {
        let dotgit = current.join(".git");
        if dotgit.is_dir() {
            return Some((current, Some(dotgit)));
        }
        if dotgit.is_file() {
            return Some((current, gitdir_of(&dotgit)));
        }
        dir = current.parent();
    }
    None
}

/// The repository of a file or folder, as VS Code's status bar shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    /// The repository's folder.
    pub root: String,
    /// The branch checked out (the short commit when `HEAD` is detached).
    pub branch: String,
    /// Some file is changed, staged or untracked (ignored files do not count).
    pub dirty: bool,
}

/// The repository holding `path` (a file or a folder), or `None` outside one or when its `HEAD`
/// cannot be read. Whether it has changes is one read-only `git status` of the whole repository.
pub fn repo_of(path: &Path) -> Option<RepoInfo> {
    let folder = if path.is_dir() { path } else { path.parent()? };
    let (root, gitdir) = repository_of(folder)?;
    let branch = head_of(&gitdir?)?;
    let dirty = status_of(root).iter().any(|entry| entry.status != FileStatus::Ignored);
    Some(RepoInfo {
        root: root.to_string_lossy().into_owned(),
        branch,
        dirty,
    })
}

/// The repository folder a `.git` file points to (`gitdir: <path>`, relative to the file's folder).
fn gitdir_of(file: &Path) -> Option<PathBuf> {
    let text = read_small(file)?;
    let target = text.lines().next()?.strip_prefix("gitdir:")?.trim();
    if target.is_empty() {
        return None;
    }
    Some(file.parent()?.join(target))
}

fn read_small(file: &Path) -> Option<String> {
    let meta = std::fs::metadata(file).ok()?;
    if !meta.is_file() || meta.len() > MAX_HEAD_BYTES {
        return None;
    }
    std::fs::read_to_string(file).ok()
}

/// The branch or short hash in `<gitdir>/HEAD`.
fn head_of(gitdir: &Path) -> Option<String> {
    parse_head(&read_small(&gitdir.join("HEAD"))?)
}

/// `ref: refs/heads/<branch>` gives the branch; a full object id gives its first 7 digits.
pub fn parse_head(text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    if let Some(reference) = line.strip_prefix("ref:") {
        let reference = reference.trim();
        let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
        return clean_name(name);
    }
    let is_hash = (line.len() == 40 || line.len() == 64) && line.bytes().all(|b| b.is_ascii_hexdigit());
    if !is_hash {
        return None;
    }
    line.get(..SHORT_HASH).map(str::to_ascii_lowercase)
}

/// A name fit to show: not empty, no control characters, not absurdly long.
fn clean_name(name: &str) -> Option<String> {
    let ok = !name.is_empty() && name.chars().count() <= MAX_BRANCH_CHARS && !name.chars().any(char::is_control);
    ok.then(|| name.to_string())
}

/// Quotes `text` as one word for POSIX `sh`: `'it'\''s'`. Only `sh` reads it: see [`through_sh`].
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Standard base64 (RFC 4648, with padding).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            let letter = ALPHABET.get((n >> (18 - 6 * i) & 63) as usize);
            match letter {
                Some(&letter) if i <= chunk.len() => out.push(char::from(letter)),
                _ => out.push('='),
            }
        }
    }
    out
}

/// A command that runs the `sh` `script` whatever the user's login shell is. The SSH server hands
/// the command to that shell first, and shells disagree on quoting (fish reads `\'` inside single
/// quotes as an escaped quote), so the login shell only sees base64 letters piped into `sh`.
fn through_sh(script: &str) -> String {
    format!("echo {} | base64 -d | sh 2>/dev/null", base64(script.as_bytes()))
}

/// The read-only command that prints the branch of the remote `folder` (or its short commit when
/// detached), or nothing outside a repository or without git. It always ends with status 0, and
/// it runs through `sh` whatever the user's login shell is. `None` for a folder that cannot be
/// passed safely.
pub fn remote_command(folder: &str) -> Option<String> {
    if folder.is_empty() || folder.contains('\0') {
        return None;
    }
    let dir = quote(folder);
    let script = format!(
        "{{ git -C {dir} symbolic-ref --short -q HEAD || git -C {dir} rev-parse --short HEAD; }} 2>/dev/null; exit 0"
    );
    Some(through_sh(&script))
}

/// The branch printed by [`remote_command`], or `None` when it printed none.
pub fn parse_remote(output: &str) -> Option<String> {
    clean_name(output.lines().next()?.trim())
}

#[cfg(test)]
mod tests;
