//! Opening a local file in a desktop program: its default one, the text editor or a chosen
//! program (PRD LOC-1 **Open**, **Open with default text editor**, **Open with…**), ported from
//! PlexusTerm v0.2.15 `src-tauri/src/sftp/commands.rs` (`launch`) and `sftp/edit.rs`.
//!
//! No shell ever runs: a chosen editor command is split into words and the file is always one
//! argument of its own.

use std::ffi::OsString;
use std::path::Path;

/// Which program opens a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWith {
    /// The system's default program for the file.
    Default,
    /// The text editor chosen in Settings (`command`), or the system's default one.
    TextEditor { command: Option<String> },
    /// A program chosen by the user.
    Program { path: String },
}

fn open_failure(file: &Path, err: impl std::fmt::Display) -> String {
    format!("cannot open {}: {err}", file.display())
}

/// Opens `file` as `with` says, without waiting for the program to exit.
pub fn launch(with: &OpenWith, file: &Path) -> Result<(), String> {
    match with {
        OpenWith::Default => open::that_detached(file).map_err(|err| open_failure(file, err)),
        OpenWith::Program { path: program } => {
            open::with_detached(file, program.as_str()).map_err(|err| open_failure(file, err))
        }
        // The chosen editor runs directly, never through a shell.
        OpenWith::TextEditor { command: Some(command) } if !command.trim().is_empty() => {
            match chosen_editor_command(command, file) {
                Some(args) => spawn_detached(&args).map_err(|err| open_failure(file, err)),
                None => Err(open_failure(file, "no text editor command")),
            }
        }
        // Without a known text editor, fall back to the default program.
        OpenWith::TextEditor { .. } => match text_editor_command(file) {
            Some(command) => spawn_detached(&command).map_err(|err| open_failure(file, err)),
            None => open::that_detached(file).map_err(|err| open_failure(file, err)),
        },
    }
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

/// The `Exec=` value of the `[Desktop Entry]` group of a desktop file (only Linux and BSD read
/// desktop files).
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
fn application_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
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
