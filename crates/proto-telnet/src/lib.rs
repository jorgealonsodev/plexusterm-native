//! Telnet sessions over TCP, ported from PlexusTerm v0.2.15 `src-tauri/src/telnet/mod.rs`
//! (PRD TEL-1). Option negotiation lives in [`protocol`].
//!
//! The server's output, with every Telnet command removed, is pushed into a
//! [`term_engine::ByteRing`]; a full ring stops this crate from reading the socket, so TCP flow
//! control holds the server back and no byte is dropped (PRD ST-3). A closed ring (nobody reads
//! the pane any more) ends the connection. Input and window changes come from [`TelnetInput`]
//! and [`TelnetResize`], the `output` writer and `on_resize` hook of a
//! [`term_engine::Session`]. The ring is closed when the connection ends, which ends the
//! session (`EndReason::SourceClosed`).

pub mod protocol;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use term_engine::{ByteRing, Closed, TryPushError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Notify, mpsc, watch};

use protocol::{Telnet, encode_input};

/// Port used when the session names none.
pub const DEFAULT_PORT: u16 = 23;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Writes (input and negotiation answers) waiting for the socket.
const INPUT_QUEUE: usize = 256;
const READ_CHUNK: usize = 8192;

/// First line of a Telnet pane, written by the pane before connecting (v0.2.15
/// `TerminalView.tsx`, `EN/terminal.ts` `telnetUnencrypted`).
pub const UNENCRYPTED_NOTICE: &str = "Telnet is not encrypted.";
/// [`UNENCRYPTED_NOTICE`] in yellow, exactly as v0.2.15 writes it into the pane.
pub const UNENCRYPTED_LINE: &str = "\x1b[33mTelnet is not encrypted.\x1b[0m\r\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelnetRequest {
    /// A host name or an IP literal; IPv6 literals may be bracketed (`[::1]`) or not (`::1`).
    pub host: String,
    pub port: u16,
    pub cols: u16,
    pub rows: u16,
}

impl TelnetRequest {
    /// A request for `host` on the default port 23.
    pub fn new(host: impl Into<String>, cols: u16, rows: u16) -> Self {
        Self {
            host: host.into(),
            port: DEFAULT_PORT,
            cols,
            rows,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TelnetError {
    #[error("{0}")]
    Connection(String),
    #[error("unknown telnet session {0}")]
    UnknownSession(u32),
}

fn ended() -> TelnetError {
    TelnetError::Connection("the Telnet session has ended".into())
}

/// Size and lifetime of a connection, shared by the manager, [`TelnetResize`] and the tasks.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Control {
    size: (u16, u16),
    closed: bool,
}

/// Input of a Telnet session: the `output` writer of a [`term_engine::Session`].
///
/// Escapes byte 255 and sends a bare CR (Enter) as CR NUL. `write` waits while the socket is
/// not taking input (bounded queue), so it must run on a plain thread (term-engine's writer
/// thread), never inside an async task.
pub struct TelnetInput(mpsc::Sender<Vec<u8>>);

impl Write for TelnetInput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.0
            .blocking_send(encode_input(buf))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the Telnet session has ended"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Window changes of a Telnet session: the `on_resize` hook of a [`term_engine::Session`].
/// Never blocks; the latest size wins and is sent (NAWS, once the server enabled it) before
/// input queued after it.
#[derive(Clone)]
pub struct TelnetResize(Arc<watch::Sender<Control>>);

impl TelnetResize {
    pub fn resize(&self, cols: u16, rows: u16) {
        self.0.send_modify(|control| control.size = (cols, rows));
    }

    /// This handle as a [`term_engine::SessionHooks::on_resize`].
    pub fn into_hook(self) -> Arc<dyn Fn(u16, u16) + Send + Sync> {
        Arc::new(move |cols, rows| self.resize(cols, rows))
    }
}

/// A connection opened by [`TelnetManager::connect`].
pub struct TelnetSession {
    pub id: u32,
    pub input: TelnetInput,
    pub resize: TelnetResize,
}

impl std::fmt::Debug for TelnetSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelnetSession")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

struct Entry {
    input: mpsc::Sender<Vec<u8>>,
    control: Arc<watch::Sender<Control>>,
    output: ByteRing,
}

pub struct TelnetManager {
    connect_timeout: Duration,
    next_id: AtomicU32,
    sessions: Mutex<HashMap<u32, Entry>>,
}

impl Default for TelnetManager {
    fn default() -> Self {
        Self {
            connect_timeout: CONNECT_TIMEOUT,
            next_id: AtomicU32::new(0),
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Pushes `data` into `ring`, waiting on the blocking pool while it is full (ST-3).
async fn push(ring: &ByteRing, data: &[u8]) -> Result<(), Closed> {
    match ring.try_push(data) {
        Ok(()) => Ok(()),
        Err(TryPushError::Closed) => Err(Closed),
        Err(TryPushError::Full) => {
            let (ring, data) = (ring.clone(), data.to_vec());
            tokio::task::spawn_blocking(move || ring.push(&data))
                .await
                .unwrap_or(Err(Closed))
        }
    }
}

impl TelnetManager {
    #[cfg(test)]
    fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Connects and starts negotiating; the output goes to `output`, which is closed when the
    /// connection ends.
    pub async fn connect(&self, req: TelnetRequest, output: ByteRing) -> Result<TelnetSession, TelnetError> {
        let address = format!("{}:{}", req.host.trim(), req.port);
        let stream = match tokio::time::timeout(self.connect_timeout, TcpStream::connect(&address)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                return Err(TelnetError::Connection(format!("cannot connect to {address}: {err}")));
            }
            Err(_) => {
                return Err(TelnetError::Connection(format!(
                    "cannot connect to {address}: timed out after {} s",
                    self.connect_timeout.as_secs()
                )));
            }
        };
        let _ = stream.set_nodelay(true);
        let (mut reader, mut writer) = stream.into_split();

        let mut telnet = Telnet::new(req.cols, req.rows);
        let (tx, mut inputs) = mpsc::channel::<Vec<u8>>(INPUT_QUEUE);
        // Negotiation answers go through the same queue as user input, in order.
        let _ = tx.try_send(telnet.start());
        let telnet = Arc::new(Mutex::new(telnet));
        let control = Arc::new(
            watch::channel(Control {
                size: (req.cols, req.rows),
                closed: false,
            })
            .0,
        );
        let mut controls = control.subscribe();
        let input = TelnetInput(tx.clone());

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.sessions).insert(
            id,
            Entry {
                input: tx.clone(),
                control: control.clone(),
                output: output.clone(),
            },
        );

        // Wakes the reader when the session is closed from our side.
        let stop = Arc::new(Notify::new());
        let (writer_telnet, writer_stop) = (telnet.clone(), stop.clone());
        let mut applied = (req.cols, req.rows);
        tokio::spawn(async move {
            loop {
                let bytes = tokio::select! {
                    // A resize goes out before input queued after it.
                    biased;
                    changed = controls.changed() => {
                        let now = *controls.borrow_and_update();
                        if changed.is_err() || now.closed {
                            break;
                        }
                        if now.size == applied {
                            continue;
                        }
                        applied = now.size;
                        match lock(&writer_telnet).resize(now.size.0, now.size.1) {
                            Some(bytes) => bytes,
                            None => continue,
                        }
                    }
                    data = inputs.recv() => match data {
                        Some(data) => data,
                        None => break,
                    },
                };
                if writer.write_all(&bytes).await.is_err() {
                    break;
                }
            }
            let _ = writer.shutdown().await;
            writer_stop.notify_one();
        });

        let reader_control = control.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                let read = tokio::select! {
                    read = reader.read(&mut buf) => read,
                    _ = stop.notified() => break,
                };
                let chunk = match read {
                    Ok(n) if n > 0 => match buf.get(..n) {
                        Some(chunk) => chunk,
                        None => break,
                    },
                    _ => break,
                };
                let received = lock(&telnet).receive(chunk);
                if !received.reply.is_empty() && tx.send(received.reply).await.is_err() {
                    break;
                }
                // Nobody reads this terminal any more: end the connection.
                if !received.data.is_empty() && push(&output, &received.data).await.is_err() {
                    break;
                }
            }
            reader_control.send_modify(|control| control.closed = true);
            output.close();
        });

        Ok(TelnetSession {
            id,
            input,
            resize: TelnetResize(control),
        })
    }

    /// Sends user input, with IAC escaped and bare CRs sent as CR NUL; waits while the
    /// input queue is full.
    pub async fn write(&self, id: u32, data: &[u8]) -> Result<(), TelnetError> {
        let tx = tx_for_input(&self.sessions, id)?;
        tx.send(encode_input(data)).await.map_err(|_| ended())
    }

    /// Re-sends the window size when the server enabled NAWS.
    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), TelnetError> {
        let sessions = lock(&self.sessions);
        let entry = sessions.get(&id).ok_or(TelnetError::UnknownSession(id))?;
        if entry.control.borrow().closed {
            return Err(ended());
        }
        entry.control.send_modify(|control| control.size = (cols, rows));
        Ok(())
    }

    /// Disconnects and forgets the session; its output ring is closed.
    pub fn close(&self, id: u32) -> Result<(), TelnetError> {
        let entry = lock(&self.sessions)
            .remove(&id)
            .ok_or(TelnetError::UnknownSession(id))?;
        entry.control.send_modify(|control| control.closed = true);
        entry.output.close();
        Ok(())
    }
}

fn tx_for_input(sessions: &Mutex<HashMap<u32, Entry>>, id: u32) -> Result<mpsc::Sender<Vec<u8>>, TelnetError> {
    lock(sessions)
        .get(&id)
        .map(|entry| entry.input.clone())
        .ok_or(TelnetError::UnknownSession(id))
}
