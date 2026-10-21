//! Local terminal sessions on a pseudo terminal, ported from PlexusTerm v0.2.15
//! `src-tauri/src/pty/mod.rs` (PRD TERM-1, TERM-15, TERM-16).
//!
//! The PTY output is pushed into a [`ByteRing`]: a full ring blocks the reader thread, so a
//! busy parser holds back the program instead of output being dropped (PRD §6.3 ST-3). The
//! process is waited for on its own thread, so its exit is seen even while a background job
//! keeps the terminal open, and ending a terminal escalates to `SIGKILL` for programs that
//! ignore the hang-up (ST-8, ST-16).

use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use term_engine::ByteRing;

/// Bytes read from the PTY per call.
const READ_CHUNK: usize = 64 * 1024;
/// After the process exits, how long the reader may keep draining output still in the PTY.
const DRAIN_AFTER_EXIT: Duration = Duration::from_millis(250);
/// How long a hung-up program has before it is killed.
const HANGUP_GRACE: Duration = Duration::from_secs(2);

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

/// Ends the process: a hang-up first, then `SIGKILL` to its process group if it is still running.
struct Terminator {
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pid: Option<u32>,
    /// `None` while running; `Some(code)` once the process ended.
    exit: Mutex<Option<Option<u32>>>,
}

impl Terminator {
    fn exited(&self) -> bool {
        self.exit.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    fn terminate(self: &Arc<Self>) {
        if self.exited() {
            return;
        }
        // The process may already be gone.
        let _ = self.killer.lock().unwrap_or_else(PoisonError::into_inner).kill();
        let this = self.clone();
        let _ = thread::Builder::new().name("pty-reaper".into()).spawn(move || {
            let deadline = Instant::now() + HANGUP_GRACE;
            while Instant::now() < deadline {
                if this.exited() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            this.force_kill();
        });
    }

    #[cfg(unix)]
    fn force_kill(&self) {
        use rustix::process::{Pid, Signal, kill_process_group};
        let pid = self.pid.and_then(|pid| i32::try_from(pid).ok()).and_then(Pid::from_raw);
        if let Some(pid) = pid {
            // The shell leads its own process group (the PTY's session).
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }

    #[cfg(not(unix))]
    fn force_kill(&self) {
        // Windows: the killer already terminates the process outright.
    }
}

/// A running local terminal. Dropping it ends the process and the output stream.
pub struct LocalPty {
    master: Mutex<Box<dyn MasterPty + Send>>,
    terminator: Arc<Terminator>,
    output: ByteRing,
}

/// Starts `spec` on a new PTY. Output goes to `output`, which is closed once the process ended
/// and its remaining output was drained; input goes through the returned writer.
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
    // Drop our copy of the slave so the reader sees the end when the last program closes it.
    drop(pair.slave);
    let handles = pair
        .master
        .try_clone_reader()
        .and_then(|reader| Ok((reader, pair.master.take_writer()?)));
    let (reader, writer) = match handles {
        Ok(handles) => handles,
        Err(e) => {
            reap(child.as_mut());
            return Err(error(e));
        }
    };
    let terminator = Arc::new(Terminator {
        killer: Mutex::new(child.clone_killer()),
        pid: child.process_id(),
        exit: Mutex::new(None),
    });

    let (drained_tx, drained) = mpsc::channel::<()>();
    let reader_thread = {
        let (output, terminator) = (output.clone(), terminator.clone());
        move || {
            read_into(reader, &output, &terminator);
            let _ = drained_tx.send(());
        }
    };
    let waiter_thread = {
        let (output, terminator) = (output.clone(), terminator.clone());
        let mut child = child;
        move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            *terminator.exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(code);
            // Let the reader take what the program printed last; a background job that keeps the
            // terminal open must not keep the session alive after its shell ended.
            let _ = drained.recv_timeout(DRAIN_AFTER_EXIT);
            output.close();
        }
    };
    let pty = LocalPty {
        master: Mutex::new(pair.master),
        terminator,
        output,
    };
    let started = thread::Builder::new()
        .name("pty-reader".into())
        .spawn(reader_thread)
        .is_ok()
        && thread::Builder::new()
            .name("pty-waiter".into())
            .spawn(waiter_thread)
            .is_ok();
    if !started {
        // Dropping `pty` hangs up and, if needed, kills the program.
        return Err(PtyError("could not start the terminal's threads".into()));
    }
    Ok((pty, writer))
}

fn read_into(mut reader: Box<dyn Read + Send>, output: &ByteRing, terminator: &Arc<Terminator>) {
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                let Some(bytes) = buf.get(..n) else { return };
                if output.push(bytes).is_err() {
                    // Nobody reads this terminal any more: end its program.
                    terminator.terminate();
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            // EIO once the last program closed the terminal, or a real failure; the waiter
            // reports the exit either way.
            Err(_) => return,
        }
    }
}

/// Ends a child that never got its threads.
fn reap(child: &mut (dyn Child + Send + Sync)) {
    let _ = child.kill();
    let _ = child.wait();
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
        *self.terminator.exit.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The working folder of the shell (not of a program it runs), when the system can tell.
    pub fn cwd(&self) -> Option<String> {
        self.terminator.pid.and_then(process_cwd)
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

    /// Hangs up the process and kills it if it does not end; the output stream then ends too.
    pub fn kill(&self) {
        self.terminator.terminate();
    }
}

impl Drop for LocalPty {
    fn drop(&mut self) {
        // Closing the ring also releases a reader blocked on a full ring.
        self.output.close();
        self.terminator.terminate();
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
