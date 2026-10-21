//! Local copies of remote files opened in desktop programs (edit-and-upload, SFTP-2 and SEC-e),
//! ported from PlexusTerm v0.2.15 `src-tauri/src/sftp/edit.rs`.
//!
//! Each SFTP session gets one private temporary folder `plexusterm-sftp-*` (mode 0700 on Unix);
//! every remote file opened from it gets its own subfolder so files with the same name never
//! clash. A polling task watches each copy and reports when it is saved; everything is removed
//! when the session's copies are dropped ([`TempFiles::remove`], or dropping the [`TempFiles`]).

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use tokio::task::JoinHandle;

/// How often the watchers look at the copies' modification time and size.
pub const WATCH_INTERVAL: Duration = Duration::from_millis(1000);

/// Prefix of each session's private temporary folder.
pub const TEMP_PREFIX: &str = "plexusterm-sftp-";

struct LocalCopy {
    local: PathBuf,
    watcher: Option<JoinHandle<()>>,
}

struct SessionCopies {
    dir: tempfile::TempDir,
    next: u32,
    /// Keyed by `"<purpose>:<remote path>"`.
    copies: HashMap<String, LocalCopy>,
}

impl Drop for SessionCopies {
    fn drop(&mut self) {
        for copy in self.copies.values_mut() {
            if let Some(watcher) = copy.watcher.take() {
                watcher.abort();
            }
        }
    }
}

/// Temporary copies of remote files, per SFTP session id.
#[derive(Default)]
pub struct TempFiles {
    sessions: Mutex<HashMap<u32, SessionCopies>>,
}

impl TempFiles {
    fn sessions(&self) -> MutexGuard<'_, HashMap<u32, SessionCopies>> {
        // Every value behind this lock stays consistent, so a poisoned lock is still usable.
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The local path for `remote` in session `id` (stable for the same `purpose` and
    /// remote path); creates the session folder and the copy's subfolder when needed.
    /// Stops any watcher of a previous copy of the same file.
    pub fn local_path(&self, id: u32, purpose: &str, remote: &str) -> std::io::Result<PathBuf> {
        let mut sessions = self.sessions();
        let session = match sessions.entry(id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(SessionCopies {
                dir: private_dir()?,
                next: 0,
                copies: HashMap::new(),
            }),
        };
        let key = format!("{purpose}:{remote}");
        if let Some(copy) = session.copies.get_mut(&key) {
            if let Some(watcher) = copy.watcher.take() {
                watcher.abort();
            }
            return Ok(copy.local.clone());
        }
        session.next += 1;
        let folder = session.dir.path().join(session.next.to_string());
        std::fs::create_dir(&folder)?;
        let local = folder.join(local_name(remote));
        session.copies.insert(
            key,
            LocalCopy {
                local: local.clone(),
                watcher: None,
            },
        );
        Ok(local)
    }

    /// Calls `on_save` every time the copy of `remote` changes on disk, until it is
    /// opened again or the session's copies are removed. Must run inside a Tokio runtime.
    pub fn watch(&self, id: u32, purpose: &str, remote: &str, interval: Duration, on_save: impl Fn() + Send + 'static) {
        let mut sessions = self.sessions();
        let Some(copy) = sessions
            .get_mut(&id)
            .and_then(|session| session.copies.get_mut(&format!("{purpose}:{remote}")))
        else {
            return;
        };
        if let Some(watcher) = copy.watcher.take() {
            watcher.abort();
        }
        copy.watcher = Some(tokio::spawn(watch_file(copy.local.clone(), interval, on_save)));
    }

    /// The local copy of `remote`, if one was made in session `id`.
    pub fn existing(&self, id: u32, purpose: &str, remote: &str) -> Option<PathBuf> {
        let sessions = self.sessions();
        sessions
            .get(&id)?
            .copies
            .get(&format!("{purpose}:{remote}"))
            .map(|copy| copy.local.clone())
    }

    /// Stops the watchers of session `id` and deletes its temporary folder (call when the SSH
    /// session ends).
    pub fn remove(&self, id: u32) {
        let removed = self.sessions().remove(&id);
        drop(removed);
    }

    /// The private temporary folder of session `id`, once a copy was made in it.
    pub fn session_dir(&self, id: u32) -> Option<PathBuf> {
        let sessions = self.sessions();
        sessions.get(&id).map(|session| session.dir.path().to_path_buf())
    }
}

/// A new temporary folder only the current user can read (0700 on Unix).
fn private_dir() -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(TEMP_PREFIX);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir()
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Polls `path` and calls `on_save` when its modification time or size changes.
/// A copy that is briefly missing (editors that save by renaming) is skipped.
async fn watch_file(path: PathBuf, interval: Duration, on_save: impl Fn()) {
    let mut last = stamp(&path);
    loop {
        tokio::time::sleep(interval).await;
        let now = stamp(&path);
        if now.is_some() && now != last {
            last = now;
            on_save();
        }
    }
}

/// The first program named `name` in the `PATH`-style list `path`.
pub fn find_in_path(name: &str, path: Option<OsString>) -> Option<PathBuf> {
    let path = path?;
    let extensions: Vec<String> = if cfg!(windows) {
        vec![".exe".into(), ".cmd".into(), ".bat".into(), String::new()]
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).find_map(|dir| {
        extensions.iter().find_map(|ext| {
            let candidate = dir.join(format!("{name}{ext}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

/// Diff tools tried in order, with the arguments put before the two files.
pub const DIFF_TOOLS: [(&str, &[&str]); 4] = [("meld", &[]), ("kdiff3", &[]), ("kompare", &[]), ("code", &["--diff"])];

/// The first installed diff tool and its leading arguments.
pub fn find_diff_tool(path: Option<OsString>) -> Option<(PathBuf, &'static [&'static str])> {
    DIFF_TOOLS
        .iter()
        .find_map(|(name, args)| find_in_path(name, path.clone()).map(|program| (program, *args)))
}

/// Splits a desktop entry `Exec=` value into arguments and puts `file` in place of
/// its file/URL field code (`%f %F %u %U`), or at the end when it has none; other
/// field codes are dropped.
pub fn desktop_exec_args(exec: &str, file: &Path) -> Vec<OsString> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(word);
    }
    let mut placed = false;
    let mut args = Vec::new();
    for word in words {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" => {
                if !placed {
                    args.push(file.as_os_str().to_os_string());
                    placed = true;
                }
            }
            w if w.len() == 2 && w.starts_with('%') => {}
            w => args.push(OsString::from(w.replace("%%", "%"))),
        }
    }
    if !placed {
        args.push(file.as_os_str().to_os_string());
    }
    args
}

/// The `Exec=` value of the `[Desktop Entry]` group of a desktop file. Only Linux and BSD
/// read desktop files; the tests check it on every platform.
pub fn desktop_exec(contents: &str) -> Option<String> {
    let mut in_entry = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if let Some(value) = line.strip_prefix("Exec=").filter(|_| in_entry) {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// Folders searched for desktop files, as in the XDG base directory specification.
#[cfg(all(unix, not(target_os = "macos")))]
fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME") {
        Some(home) if !home.is_empty() => dirs.push(PathBuf::from(home)),
        _ => {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(home).join(".local/share"));
            }
        }
    }
    let data = std::env::var_os("XDG_DATA_DIRS")
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(std::env::split_paths(&data));
    dirs.into_iter().map(|d| d.join("applications")).collect()
}

/// The command line of the default text editor for `file`, if one can be found.
///
/// Linux and BSD: the desktop entry `xdg-mime` reports for `text/plain`;
/// macOS: `open -t`; Windows: Notepad.
pub fn text_editor_command(file: &Path) -> Option<Vec<OsString>> {
    #[cfg(target_os = "macos")]
    {
        Some(vec!["open".into(), "-t".into(), file.as_os_str().to_os_string()])
    }
    #[cfg(windows)]
    {
        Some(vec!["notepad.exe".into(), file.as_os_str().to_os_string()])
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let output = std::process::Command::new("xdg-mime")
            .args(["query", "default", "text/plain"])
            .output()
            .ok()?;
        let id = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if id.is_empty() {
            return None;
        }
        // `org.gnome.TextEditor.desktop` may live in a subfolder as `org/gnome/...`.
        let candidates = [id.clone(), id.replacen('-', "/", 1)];
        application_dirs().iter().find_map(|dir| {
            candidates.iter().find_map(|name| {
                let contents = std::fs::read_to_string(dir.join(name)).ok()?;
                desktop_exec(&contents).map(|exec| desktop_exec_args(&exec, file))
            })
        })
    }
}

/// The command line for the text editor chosen in Settings, or `None` when it is empty.
///
/// A `command` naming an existing file is one program, even with spaces in its path (what
/// Browse… picks). Otherwise it is split like a desktop entry `Exec=` value: words separated
/// by spaces, `"double quotes"` around words with spaces, and the file in place of `%f`
/// or appended at the end. The file is always a single argument and no shell is involved.
pub fn chosen_editor_command(command: &str, file: &Path) -> Option<Vec<OsString>> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if Path::new(command).is_file() {
        return Some(vec![command.into(), file.as_os_str().to_os_string()]);
    }
    Some(desktop_exec_args(command, file))
}

/// Starts `args[0]` with the rest as arguments, without waiting for it to exit.
pub fn spawn_detached(args: &[OsString]) -> std::io::Result<()> {
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| std::io::Error::other("empty command"))?;
    let mut child = std::process::Command::new(program)
        .args(rest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Reap the process when it exits so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// The local file name for the last segment of `remote`: a remote name is only data here, so
/// characters that mean something in local paths (`\\`, `:`, Windows-reserved ones, control
/// characters) become `_`, and `.`/`..`/empty become `file`; the copy can never leave its folder.
fn local_name(remote: &str) -> String {
    let last = remote.rsplit('/').find(|part| !part.is_empty()).unwrap_or("");
    let name: String = last
        .chars()
        .map(|c| match c {
            '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    match name.as_str() {
        "" | "." | ".." => "file".to_string(),
        _ => name,
    }
}
