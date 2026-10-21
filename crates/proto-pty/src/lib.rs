//! Local terminal sessions on a pseudo terminal, ported from PlexusTerm v0.2.15
//! `src-tauri/src/pty/mod.rs` (PRD TERM-1, TERM-15, TERM-16).
//!
//! The PTY output is pushed into a [`ByteRing`]: a full ring blocks the reader thread, so a
//! busy parser holds back the program instead of output being dropped (PRD §6.3 ST-3).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use term_engine::ByteRing;

/// Bytes read from the PTY per call.
const READ_CHUNK: usize = 64 * 1024;

/// Environment variables for the child; `Debug` shows only their names, so values such as
/// `MOSH_KEY` never reach logs or panic messages (TERM-1b).
#[derive(Clone, Default)]
pub struct ChildEnv(Vec<(String, String)>);

impl ChildEnv {
    pub fn new(vars: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(vars.into_iter().collect())
    }
}

impl std::fmt::Debug for ChildEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(name, _)| name)).finish()
    }
}

/// What to run in a new local terminal.
#[derive(Clone, Debug, Default)]
pub struct SpawnSpec {
    /// Program to run; missing or blank runs the user's shell.
    pub program: Option<String>,
    pub args: Vec<String>,
    /// Starting directory (`~/` expanded); missing or blank starts in the home directory.
    pub cwd: Option<String>,
    pub columns: u16,
    pub rows: u16,
    /// Extra environment, set only by native code (never from settings or saved sessions).
    pub env: ChildEnv,
}

#[derive(Debug)]
pub struct PtyError(String);

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PtyError {}

fn error(e: impl std::fmt::Display) -> PtyError {
    PtyError(e.to_string())
}

/// A running local terminal. Dropping it kills the process.
pub struct LocalPty {
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pid: Option<u32>,
    /// `None` while running; `Some(code)` once the process ended (`code` is `None` when the
    /// system did not report one).
    exit: Arc<Mutex<Option<Option<u32>>>>,
}

/// Starts `spec` on a new PTY. Output goes to `output`, which is closed when the process ends;
/// input goes through the returned writer.
pub fn spawn(spec: SpawnSpec, output: ByteRing) -> Result<(LocalPty, Box<dyn Write + Send>), PtyError> {
    let pair = native_pty_system()
        .openpty(size(spec.columns, spec.rows))
        .map_err(error)?;
    let mut cmd = CommandBuilder::new(program_of(&spec));
    cmd.args(&spec.args);
    if let Some(dir) = cwd_of(&spec) {
        cmd.cwd(dir);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    for (name, value) in &spec.env.0 {
        cmd.env(name, value);
    }

    let mut child = pair.slave.spawn_command(cmd).map_err(error)?;
    // Drop our copy of the slave so the reader sees the end when the child exits.
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(error)?;
    let writer = pair.master.take_writer().map_err(error)?;
    let killer = child.clone_killer();
    let pid = child.process_id();
    let exit = Arc::new(Mutex::new(None));

    let mut stopper = child.clone_killer();
    let reader_thread = {
        let exit = exit.clone();
        move || {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let Some(bytes) = buf.get(..n) else { break };
                        if output.push(bytes).is_err() {
                            // The session ended and nobody reads this terminal any more: end the
                            // program too, or waiting for it below would never return.
                            let _ = stopper.kill();
                            break;
                        }
                    }
                }
            }
            let code = child.wait().ok().map(|status| status.exit_code());
            *exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(code);
            output.close();
        }
    };
    let pty = LocalPty {
        master: Mutex::new(pair.master),
        killer: Mutex::new(killer),
        pid,
        exit,
    };
    if thread::Builder::new()
        .name("pty-reader".into())
        .spawn(reader_thread)
        .is_err()
    {
        pty.kill();
        return Err(PtyError("could not start the terminal's reader".into()));
    }
    Ok((pty, writer))
}

impl LocalPty {
    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), PtyError> {
        self.master
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .resize(size(columns, rows))
            .map_err(error)
    }

    /// The exit code once the process ended (`Some(None)` when the system gave none).
    pub fn exit_code(&self) -> Option<Option<u32>> {
        *self.exit.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The working folder of the shell (not of a program it runs), when the system can tell.
    pub fn cwd(&self) -> Option<String> {
        self.pid.and_then(process_cwd)
    }

    /// The working folder of the program in the terminal's foreground, else of its shell (TERM-15a).
    pub fn program_cwd(&self) -> Option<String> {
        #[cfg(unix)]
        {
            let leader = self
                .master
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .process_group_leader();
            if let Some(dir) = leader.and_then(|pid| u32::try_from(pid).ok()).and_then(process_cwd) {
                return Some(dir);
            }
        }
        self.cwd()
    }

    /// Ends the process; the output stream then ends too.
    pub fn kill(&self) {
        // The process may already be gone.
        let _ = self.killer.lock().unwrap_or_else(PoisonError::into_inner).kill();
    }
}

impl Drop for LocalPty {
    fn drop(&mut self) {
        self.kill();
    }
}

fn size(columns: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: columns.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn program_of(spec: &SpawnSpec) -> String {
    match spec.program.as_deref().map(str::trim) {
        Some(program) if !program.is_empty() => program.to_owned(),
        _ => default_shell(),
    }
}

/// The starting directory: the requested one with a leading `~` expanded, or the home
/// directory when it is missing or blank.
fn cwd_of(spec: &SpawnSpec) -> Option<PathBuf> {
    match spec.cwd.as_deref().map(str::trim) {
        None | Some("" | "~") => home_dir(),
        Some(dir) => Some(expand_home(dir)),
    }
}

/// Unix: `$SHELL`, else `/bin/bash`. Windows: `pwsh.exe`, else `powershell.exe`, else `%COMSPEC%`.
pub fn default_shell() -> String {
    if cfg!(windows) {
        let path = std::env::var_os("PATH").unwrap_or_default();
        ["pwsh.exe", "powershell.exe"]
            .into_iter()
            .find_map(|name| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join(name))
                    .find(|p| p.is_file())
            })
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| std::env::var("COMSPEC").ok())
            .unwrap_or_else(|| "cmd.exe".into())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Expands a leading `~/` (also `~\` on Windows) to the home directory.
fn expand_home(path: &str) -> PathBuf {
    let rest = path
        .strip_prefix("~/")
        .or_else(|| if cfg!(windows) { path.strip_prefix("~\\") } else { None });
    match (rest, home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .map(|dir| dir.to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<String> {
    // `-Fn` prints `p<pid>`, `fcwd` and `n<path>` lines.
    let out = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix('n').map(str::to_string))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_cwd(_pid: u32) -> Option<String> {
    None
}
