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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use term_engine::ByteRing;

/// Bytes read from the PTY per call.
const READ_CHUNK: usize = 64 * 1024;
/// After the process exits, how long the reader may wait for more output while idle in a read
/// (a background job holding the terminal). A reader pushing into a full ring is never cut off.
const DRAIN_AFTER_EXIT: Duration = Duration::from_millis(250);
/// How long a hung-up program has before it is killed.
const HANGUP_GRACE: Duration = Duration::from_secs(2);
/// How often the waiter checks whether the process ended (reaping happens only under a lock).
const EXIT_POLL: Duration = Duration::from_millis(20);

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

/// Ends the process: a hang-up first, then `SIGKILL` to the terminal's foreground job and to the
/// shell's process group if they are still running.
///
/// The child is only reaped under the `child` lock, and the kill of the shell's group takes that
/// lock and checks the exit first, so a reaped pid that the system reused is never signalled.
struct Terminator {
    child: Mutex<Box<dyn Child + Send + Sync>>,
    /// Sends the hang-up without waiting (unlike `Child::kill`, which blocks for a grace period).
    hangup: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    pid: Option<u32>,
    /// `None` while running; `Some(code)` once the process ended.
    exit: Mutex<Option<Option<u32>>>,
}

impl Terminator {
    fn exited(&self) -> bool {
        self.exit.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    /// Reaps the child if it ended; returns whether it has.
    fn poll(&self) -> bool {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        self.reap_if_done(child.as_mut())
    }

    fn reap_if_done(&self, child: &mut (dyn Child + Send + Sync)) -> bool {
        if self.exited() {
            return true;
        }
        let code = match child.try_wait() {
            Ok(None) => return false,
            Ok(Some(status)) => Some(status.exit_code()),
            Err(_) => None,
        };
        *self.exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(code);
        true
    }

    fn terminate(self: &Arc<Self>) {
        let foreground = {
            let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
            if self.reap_if_done(child.as_mut()) {
                return;
            }
            // Read before the hang-up: once the shell is gone the terminal has no foreground job.
            let foreground = self
                .master
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .process_group_leader();
            let _ = self.hangup.lock().unwrap_or_else(PoisonError::into_inner).kill();
            foreground
        };
        let this = self.clone();
        let _ = thread::Builder::new().name("pty-reaper".into()).spawn(move || {
            // The grace period covers the foreground job too: a shell passes the hang-up on and
            // exits at once, while its job may still be saving.
            let deadline = Instant::now() + HANGUP_GRACE;
            while Instant::now() < deadline && !(this.exited() && !this.job_running(foreground)) {
                thread::sleep(Duration::from_millis(20));
            }
            this.force_kill(foreground);
        });
    }

    fn shell_pid(&self) -> Option<i32> {
        self.pid.and_then(|pid| i32::try_from(pid).ok())
    }

    /// Whether the captured foreground job (other than the shell) still runs in this terminal's session.
    fn job_running(&self, foreground: Option<i32>) -> bool {
        match (foreground, self.shell_pid()) {
            (Some(group), Some(shell)) if group != shell => group_in_session(group, shell),
            _ => false,
        }
    }

    #[cfg(unix)]
    fn force_kill(&self, foreground: Option<i32>) {
        use rustix::process::{Pid, Signal, kill_process_group};
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        let shell = self.shell_pid();
        // A job that ignores the hang-up outlives its shell. Its group is checked to still belong to
        // this terminal's session, so an id reused by an unrelated group is never signalled.
        if self.job_running(foreground)
            && let Some(group) = foreground.and_then(Pid::from_raw)
        {
            let _ = kill_process_group(group, Signal::KILL);
        }
        if !self.reap_if_done(child.as_mut())
            && let Some(group) = shell.and_then(Pid::from_raw)
        {
            let _ = kill_process_group(group, Signal::KILL);
        }
    }

    #[cfg(not(unix))]
    fn force_kill(&self, _foreground: Option<i32>) {
        // Windows: the hang-up already terminates the process outright.
    }

    /// Ends a child whose threads never started: nobody else would reap it.
    fn reap_now(&self) {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = child.kill();
        let code = child.wait().ok().map(|status| status.exit_code());
        *self.exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(code);
    }
}

/// A running local terminal. Dropping it ends the process and the output stream.
pub struct LocalPty {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
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
    let master = Arc::new(Mutex::new(pair.master));
    let terminator = Arc::new(Terminator {
        pid: child.process_id(),
        hangup: Mutex::new(child.clone_killer()),
        child: Mutex::new(child),
        master: master.clone(),
        exit: Mutex::new(None),
    });

    let (drained_tx, drained) = mpsc::channel::<()>();
    let progress = Arc::new(ReaderProgress {
        pushing: AtomicBool::new(false),
        last_read: Mutex::new(Instant::now()),
    });
    let reader_thread = {
        let (output, terminator, progress) = (output.clone(), terminator.clone(), progress.clone());
        move || {
            read_into(reader, &output, &terminator, &progress);
            let _ = drained_tx.send(());
        }
    };
    let waiter_thread = {
        let (output, terminator) = (output.clone(), terminator.clone());
        move || {
            while !terminator.poll() {
                thread::sleep(EXIT_POLL);
            }
            // Let the reader take everything the program printed; only a reader idle in a read
            // (a background job keeping the terminal open) stops the session from ending.
            loop {
                match drained.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) if progress.idle_for(DRAIN_AFTER_EXIT) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
            output.close();
        }
    };
    let pty = LocalPty {
        master,
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
        pty.output.close();
        pty.terminator.reap_now();
        return Err(PtyError("could not start the terminal's threads".into()));
    }
    Ok((pty, writer))
}

/// What the reader is doing, so the waiter can tell backpressure from a terminal held open.
struct ReaderProgress {
    pushing: AtomicBool,
    last_read: Mutex<Instant>,
}

impl ReaderProgress {
    fn idle_for(&self, quiet: Duration) -> bool {
        !self.pushing.load(Ordering::Acquire)
            && self.last_read.lock().unwrap_or_else(PoisonError::into_inner).elapsed() >= quiet
    }
}

fn read_into(
    mut reader: Box<dyn Read + Send>,
    output: &ByteRing,
    terminator: &Arc<Terminator>,
    progress: &ReaderProgress,
) {
    let mut buf = vec![0u8; READ_CHUNK];
    loop {
        let read = reader.read(&mut buf);
        *progress.last_read.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
        match read {
            Ok(0) => return,
            Ok(n) => {
                let Some(bytes) = buf.get(..n) else { return };
                progress.pushing.store(true, Ordering::Release);
                let pushed = output.push(bytes);
                *progress.last_read.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
                progress.pushing.store(false, Ordering::Release);
                if pushed.is_err() {
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

    /// The shell's process id, when the system gives one.
    pub fn pid(&self) -> Option<u32> {
        self.terminator.pid
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

/// Whether any live process of `group` runs inside the session `session` (the shell's pid).
///
/// Every process is checked, not just the group's leader: in a pipeline the leader (the first
/// command) may have finished while the rest of the job still runs.
#[cfg(target_os = "linux")]
fn group_in_session(group: i32, session: i32) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()))
        })
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("stat")).ok())
        .any(|stat| {
            // After the command name: state, ppid, pgrp, session, ...
            let mut fields = stat
                .rsplit_once(')')
                .map(|(_, rest)| rest)
                .unwrap_or_default()
                .split_whitespace();
            let state = fields.next();
            let pgrp = fields.nth(1).and_then(|f| f.parse::<i32>().ok());
            let sid = fields.next().and_then(|f| f.parse::<i32>().ok());
            state.is_some_and(|s| s != "Z") && pgrp == Some(group) && sid == Some(session)
        })
}

/// Without `/proc` the session cannot be checked; the group is assumed to still be the job's.
#[cfg(not(target_os = "linux"))]
fn group_in_session(_group: i32, _session: i32) -> bool {
    true
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
