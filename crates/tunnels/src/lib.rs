//! SSH tunnels: local (`-L`), remote (`-R`) and dynamic SOCKS5 (`-D`) port forwards, ported from
//! PlexusTerm v0.2.15 `src-tauri/src/tunnels/mod.rs` (PRD TUN-1) without any UI types.
//!
//! Every tunnel opens its own SSH connection with [`proto_ssh::connect_authenticated`], using the
//! known_hosts file, SSH agent, saved passwords and host key prompt of an
//! [`proto_ssh::SshManager`] ([`SshManager::context`]), so tunnels and terminal sessions trust and
//! log in alike. Status changes are reported to a callback, in order (the v0.2.15 `tunnel-status`
//! event).
//!
//! A tunnel the user edited or deleted while its host key prompt was open is not started: the
//! manager asks [`TunnelManager::with_still_wanted`]'s check after a "trust" answer, and a tunnel
//! that is no longer saved as it was ends as if the key had been refused. The app calls
//! [`TunnelManager::stop_all`] on exit.

#[cfg(test)]
mod interop_tests;
pub mod socks;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use proto_ssh::{
    Chain, Connection, ForwardedChannel, HostKeyPrompt, PasswordStorage, PromptFuture, SharedPrompt, SshError,
    SshManager, SshTarget, TrustRequest, connect_authenticated, disconnect_jumps,
};
use russh::{Channel, client};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::sync::oneshot;
use tokio::task::JoinSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TunnelKind {
    Local,
    Remote,
    Dynamic,
}

/// A saved tunnel, with the field names of `tunnels.json` (DATA-1a).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: TunnelKind,
    pub session_id: String,
    pub listen_host: String,
    pub listen_port: u16,
    /// Unused for dynamic tunnels.
    #[serde(default)]
    pub dest_host: String,
    #[serde(default)]
    pub dest_port: u16,
}

/// True when both describe the same forward through the same session; the name does not matter
/// (v0.2.15 `sameForward`, which decides whether a tunnel is "still wanted").
pub fn same_forward(a: &TunnelDefinition, b: &TunnelDefinition) -> bool {
    a.kind == b.kind
        && a.session_id == b.session_id
        && a.listen_host == b.listen_host
        && a.listen_port == b.listen_port
        && a.dest_host == b.dest_host
        && a.dest_port == b.dest_port
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelStatus {
    Stopped,
    Starting,
    Running {
        /// Shown next to the tunnel, e.g. when its password could not be saved.
        warning: Option<String>,
    },
    Error {
        message: String,
    },
}

/// A status change of tunnel `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusUpdate {
    pub id: String,
    pub status: TunnelStatus,
}

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error(transparent)]
    Ssh(#[from] SshError),
    #[error("cannot listen on {address}: {message}")]
    Listen { address: String, message: String },
    #[error("the tunnel is already running")]
    AlreadyRunning { id: String },
}

type Notify = Arc<dyn Fn(StatusUpdate) + Send + Sync>;
type StillWanted = Arc<dyn Fn(&TunnelDefinition) -> bool + Send + Sync>;

/// How often a running tunnel checks that its SSH connection is still open.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
/// Upper bound for the polite part of stopping (cancel forward, disconnect).
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
/// Time a SOCKS client gets to send its greeting and its request.
const SOCKS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The tunnel table stays consistent between statements, so a poisoned lock is still usable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Resources of a running tunnel. Dropping it aborts the listener, the
/// forwarded connections and the watcher, which releases the SSH connection.
struct Running {
    tasks: JoinSet<()>,
    connection: Arc<Connection>,
    /// Jump host connections the tunnel goes through, outermost first.
    jumps: Vec<Connection>,
    /// Address and port the server listens on for a remote forward.
    remote_forward: Option<(String, u32)>,
    /// Keyring problem after the login, reported in the running status.
    warning: Option<String>,
}

impl Running {
    /// Stops every task first (closing the listener), then ends the SSH connection.
    async fn shutdown(mut self) {
        self.tasks.shutdown().await;
        let connection = self.connection;
        let polite = async {
            if let Some((address, port)) = self.remote_forward {
                let _ = connection.cancel_tcpip_forward(address, port).await;
            }
            let _ = connection.disconnect(russh::Disconnect::ByApplication, "", "en").await;
            disconnect_jumps(&self.jumps).await;
        };
        let _ = tokio::time::timeout(STOP_TIMEOUT, polite).await;
    }
}

struct Entry {
    status: TunnelStatus,
    /// Start attempt that owns this entry; a stop or a new start changes it.
    generation: u64,
    running: Option<Running>,
    /// Set while starting: firing it aborts the start and releases its listener.
    cancel: Option<oneshot::Sender<()>>,
}

struct Shared {
    entries: Mutex<HashMap<String, Entry>>,
    next_generation: AtomicU64,
    notify: Notify,
}

impl Shared {
    fn generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Reports a status change. Callers hold the `entries` lock, so updates
    /// leave in the same order as the changes they describe: a `stop` racing
    /// a finishing `start` can never be followed by a stale `Running`.
    fn notify(&self, id: &str, status: TunnelStatus) {
        (self.notify)(StatusUpdate {
            id: id.to_string(),
            status,
        });
    }

    /// Marks a running tunnel as failed, unless it was stopped or restarted meanwhile.
    fn fail(&self, id: &str, generation: u64, message: String) {
        let running = {
            let mut entries = lock(&self.entries);
            match entries.get_mut(id) {
                Some(entry) if entry.generation == generation => {
                    entry.status = TunnelStatus::Error { message };
                    self.notify(id, entry.status.clone());
                    entry.running.take()
                }
                _ => return,
            }
        };
        drop(running);
    }

    /// Marks `id` stopped (caller holds the lock) and returns what must be shut down.
    fn halt(&self, id: &str, entry: &mut Entry) -> Option<Running> {
        entry.generation = self.generation();
        entry.status = TunnelStatus::Stopped;
        self.notify(id, TunnelStatus::Stopped);
        if let Some(cancel) = entry.cancel.take() {
            let _ = cancel.send(());
        }
        entry.running.take()
    }
}

/// Where a tunnel takes its connections from.
enum Side {
    /// Local and dynamic tunnels: a listener on this machine.
    Local(TcpListener),
    /// Remote tunnels: the server's `forwarded-tcpip` channels.
    Remote(UnboundedReceiver<ForwardedChannel>),
}

pub struct TunnelManager {
    ssh: Arc<SshManager>,
    /// Overrides the keyring bound of `ssh` (used by tests).
    storage_timeout: Option<Duration>,
    still_wanted: Option<StillWanted>,
    shared: Arc<Shared>,
}

impl TunnelManager {
    /// Tunnels log in like the sessions of `ssh` (its known_hosts, agent, saved passwords and
    /// host key prompt). `notify` receives every status change, in order. It runs while the
    /// tunnel table is locked, so it must be quick and must not call back into the manager.
    pub fn new(ssh: Arc<SshManager>, notify: impl Fn(StatusUpdate) + Send + Sync + 'static) -> Self {
        Self {
            ssh,
            storage_timeout: None,
            still_wanted: None,
            shared: Arc::new(Shared {
                entries: Mutex::new(HashMap::new()),
                next_generation: AtomicU64::new(0),
                notify: Arc::new(notify),
            }),
        }
    }

    /// `still_wanted(tunnel)` tells whether `tunnel` is still saved as it was when its start
    /// began (see [`same_forward`]). It is asked when a host key prompt answers "trust": a tunnel
    /// edited or deleted meanwhile is not started (the key is treated as refused).
    pub fn with_still_wanted(
        mut self,
        still_wanted: impl Fn(&TunnelDefinition) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.still_wanted = Some(Arc::new(still_wanted));
        self
    }

    #[cfg(test)]
    fn with_storage_timeout(mut self, timeout: Duration) -> Self {
        self.storage_timeout = Some(timeout);
        self
    }

    /// Opens the tunnel's SSH connection and starts forwarding, without
    /// touching saved passwords (tests; the app uses `start_with`).
    #[cfg(test)]
    pub async fn start(
        &self,
        tunnel: TunnelDefinition,
        target: SshTarget,
        trusted_fingerprint: Option<String>,
    ) -> Result<(), TunnelError> {
        self.start_with(tunnel, target, trusted_fingerprint, None).await
    }

    /// Opens the tunnel's SSH connection and starts forwarding, then saves or forgets the session's
    /// password as `storage` asks once the login succeeded.
    ///
    /// Every failure is also recorded as the tunnel's `Error` status. A tunnel stopped while
    /// starting returns `Ok(())` and stays stopped.
    pub async fn start_with(
        &self,
        tunnel: TunnelDefinition,
        target: SshTarget,
        trusted_fingerprint: Option<String>,
        storage: Option<PasswordStorage>,
    ) -> Result<(), TunnelError> {
        let id = tunnel.id.clone();
        let generation = self.shared.generation();
        let (cancel, cancelled) = oneshot::channel();
        {
            let mut entries = lock(&self.shared.entries);
            if let Some(entry) = entries.get(&id)
                && matches!(entry.status, TunnelStatus::Starting | TunnelStatus::Running { .. })
            {
                return Err(TunnelError::AlreadyRunning { id });
            }
            entries.insert(
                id.clone(),
                Entry {
                    status: TunnelStatus::Starting,
                    generation,
                    running: None,
                    cancel: Some(cancel),
                },
            );
            self.shared.notify(&id, TunnelStatus::Starting);
        }

        let result = tokio::select! {
            result = self.open(tunnel, target, trusted_fingerprint, storage, generation) => result,
            // `stop` already marked the tunnel stopped; dropping `open` releases its listener.
            _ = cancelled => return Ok(()),
        };

        let status = match &result {
            Ok(running) => TunnelStatus::Running {
                warning: running.warning.clone(),
            },
            Err(err) => TunnelStatus::Error {
                message: err.to_string(),
            },
        };
        let (outcome, orphan) = {
            let mut entries = lock(&self.shared.entries);
            match entries.get_mut(&id).filter(|e| e.generation == generation) {
                Some(entry) => {
                    self.shared.notify(&id, status.clone());
                    entry.status = status;
                    (Some(result.map(|running| entry.running = Some(running))), None)
                }
                None => (None, result.ok()),
            }
        };
        match outcome {
            Some(outcome) => outcome,
            None => {
                // Stopped while starting: release what was opened.
                if let Some(running) = orphan {
                    running.shutdown().await;
                }
                Ok(())
            }
        }
    }

    /// The host key prompt of `ssh`, answering "trust" only while `tunnel` is still wanted.
    fn prompt_for(&self, tunnel: &TunnelDefinition) -> Option<SharedPrompt> {
        let prompt = self.ssh.context().prompt.cloned()?;
        let Some(still_wanted) = self.still_wanted.clone() else {
            return Some(prompt);
        };
        Some(Arc::new(WantedPrompt {
            prompt,
            still_wanted,
            tunnel: tunnel.clone(),
        }))
    }

    async fn open(
        &self,
        tunnel: TunnelDefinition,
        target: SshTarget,
        trusted_fingerprint: Option<String>,
        storage: Option<PasswordStorage>,
        generation: u64,
    ) -> Result<Running, TunnelError> {
        let default_host = match tunnel.kind {
            TunnelKind::Remote => "localhost",
            TunnelKind::Local | TunnelKind::Dynamic => "127.0.0.1",
        };
        let listen_host = match tunnel.listen_host.trim() {
            "" => default_host.to_string(),
            host => host.to_string(),
        };
        let listen_address = address_label(&listen_host, tunnel.listen_port);
        let dest = (tunnel.dest_host.clone(), tunnel.dest_port);

        // Local listeners are bound first so a busy port fails before connecting.
        let (side, forwarded_tx) = match tunnel.kind {
            TunnelKind::Remote => {
                let (tx, rx) = unbounded_channel();
                (Side::Remote(rx), Some(tx))
            }
            TunnelKind::Local | TunnelKind::Dynamic => {
                let listener = TcpListener::bind((listen_host.as_str(), tunnel.listen_port))
                    .await
                    .map_err(|err| TunnelError::Listen {
                        address: listen_address.clone(),
                        message: listen_message(&err),
                    })?;
                (Side::Local(listener), None)
            }
        };
        let prompt = self.prompt_for(&tunnel);
        let mut ctx = self.ssh.context();
        ctx.prompt = prompt.as_ref();
        if let Some(timeout) = self.storage_timeout {
            ctx.storage_timeout = timeout;
        }
        let Chain {
            connection,
            jumps,
            warnings,
        } = connect_authenticated(target, trusted_fingerprint, storage, ctx, forwarded_tx).await?;
        let connection = Arc::new(connection);
        let warning = (!warnings.is_empty()).then(|| warnings.join("; "));

        let mut tasks = JoinSet::new();
        let mut remote_forward = None;
        match side {
            Side::Local(listener) => {
                tasks.spawn(serve_listener(listener, connection.clone(), tunnel.kind, dest));
            }
            Side::Remote(forwarded) => {
                let requested = u32::from(tunnel.listen_port);
                let allocated = connection
                    .tcpip_forward(listen_host.clone(), requested)
                    .await
                    .map_err(|err| TunnelError::Listen {
                        address: format!("{listen_address} on the SSH server"),
                        message: match err {
                            russh::Error::RequestDenied => {
                                "the server refused (port in use or forwarding disabled)".into()
                            }
                            other => other.to_string(),
                        },
                    })?;
                // Servers only report the port when they chose it (request for port 0).
                let port = if allocated == 0 { requested } else { allocated };
                remote_forward = Some((listen_host, port));
                tasks.spawn(serve_forwarded(forwarded, dest));
            }
        }
        tasks.spawn(watch_connection(
            self.shared.clone(),
            tunnel.id,
            generation,
            connection.clone(),
        ));
        Ok(Running {
            tasks,
            connection,
            jumps,
            remote_forward,
            warning,
        })
    }

    /// Stops the tunnel: closes its listener, its forwarded connections and its SSH connection.
    pub async fn stop(&self, id: &str) {
        let running = {
            let mut entries = lock(&self.shared.entries);
            let Some(entry) = entries.get_mut(id) else {
                return;
            };
            self.shared.halt(id, entry)
        };
        if let Some(running) = running {
            running.shutdown().await;
        }
    }

    /// Stops every starting or running tunnel, all at once (the app calls it on exit).
    pub async fn stop_all(&self) {
        let running: Vec<Running> = {
            let mut entries = lock(&self.shared.entries);
            entries
                .iter_mut()
                .filter(|(_, entry)| matches!(entry.status, TunnelStatus::Starting | TunnelStatus::Running { .. }))
                .filter_map(|(id, entry)| self.shared.halt(id, entry))
                .collect()
        };
        let mut shutdowns = JoinSet::new();
        for running in running {
            shutdowns.spawn(running.shutdown());
        }
        while shutdowns.join_next().await.is_some() {}
    }

    #[cfg(test)]
    fn status(&self, id: &str) -> TunnelStatus {
        lock(&self.shared.entries)
            .get(id)
            .map_or(TunnelStatus::Stopped, |entry| entry.status.clone())
    }

    /// Status of every tunnel started since launch.
    pub fn statuses(&self) -> Vec<StatusUpdate> {
        lock(&self.shared.entries)
            .iter()
            .map(|(id, entry)| StatusUpdate {
                id: id.clone(),
                status: entry.status.clone(),
            })
            .collect()
    }
}

/// A host key prompt whose "trust" only counts while its tunnel is still wanted.
struct WantedPrompt {
    prompt: SharedPrompt,
    still_wanted: StillWanted,
    tunnel: TunnelDefinition,
}

impl HostKeyPrompt for WantedPrompt {
    fn confirm(&self, request: TrustRequest) -> PromptFuture {
        let answer = self.prompt.confirm(request);
        let (still_wanted, tunnel) = (self.still_wanted.clone(), self.tunnel.clone());
        Box::pin(async move { answer.await && still_wanted(&tunnel) })
    }
}

/// `host:port`, with brackets around IPv6 addresses.
fn address_label(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn listen_message(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::AddrInUse => "the port is already in use".into(),
        std::io::ErrorKind::AddrNotAvailable => "the address is not available on this machine".into(),
        std::io::ErrorKind::PermissionDenied => "permission denied (ports below 1024 need privileges)".into(),
        _ => err.to_string(),
    }
}

/// Copies bytes both ways between a channel and a TCP stream until both sides close.
async fn pipe(channel: Channel<client::Msg>, mut tcp: TcpStream) {
    let mut stream = channel.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
}

/// Accepts local connections (local and dynamic tunnels) and forwards each one
/// through a `direct-tcpip` channel. Aborting it drops every open connection.
async fn serve_listener(listener: TcpListener, connection: Arc<Connection>, kind: TunnelKind, dest: (String, u16)) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((tcp, peer)) = accepted else {
                    // Usually out of file descriptors: back off instead of spinning.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                let connection = connection.clone();
                let dest = dest.clone();
                connections.spawn(async move {
                    // A failed connection only affects that client.
                    if kind == TunnelKind::Dynamic {
                        let _ = forward_socks(connection, tcp, peer).await;
                    } else {
                        let _ = forward_direct(connection, tcp, peer, dest).await;
                    }
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
}

async fn forward_direct(
    connection: Arc<Connection>,
    tcp: TcpStream,
    peer: SocketAddr,
    (host, port): (String, u16),
) -> Result<(), russh::Error> {
    let channel = connection
        .channel_open_direct_tcpip(host, port.into(), peer.ip().to_string(), peer.port().into())
        .await?;
    pipe(channel, tcp).await;
    Ok(())
}

async fn forward_socks(
    connection: Arc<Connection>,
    mut tcp: TcpStream,
    peer: SocketAddr,
) -> Result<(), socks::SocksError> {
    // On error (including a timeout) `tcp` is dropped, which closes the client connection.
    let target = socks::handshake(&mut tcp, SOCKS_HANDSHAKE_TIMEOUT).await?;
    let opened = connection
        .channel_open_direct_tcpip(
            target.host,
            target.port.into(),
            peer.ip().to_string(),
            peer.port().into(),
        )
        .await;
    let io = |err: std::io::Error| socks::SocksError::Io(err.to_string());
    match opened {
        Ok(channel) => {
            tcp.write_all(&socks::reply(socks::REPLY_SUCCEEDED)).await.map_err(io)?;
            pipe(channel, tcp).await;
        }
        Err(_) => {
            tcp.write_all(&socks::reply(socks::REPLY_GENERAL_FAILURE))
                .await
                .map_err(io)?;
        }
    }
    Ok(())
}

/// Connects every `forwarded-tcpip` channel of a remote tunnel to its local destination.
async fn serve_forwarded(mut forwarded: UnboundedReceiver<ForwardedChannel>, dest: (String, u16)) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            next = forwarded.recv() => {
                let Some(channel) = next else { break };
                let dest = dest.clone();
                connections.spawn(async move {
                    match TcpStream::connect((dest.0.as_str(), dest.1)).await {
                        Ok(tcp) => pipe(channel, tcp).await,
                        Err(_) => {
                            let _ = channel.close().await;
                        }
                    }
                });
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    // The connection ended; keep forwarding what is still open.
    while connections.join_next().await.is_some() {}
}

/// Reports a running tunnel as failed when its SSH connection closes.
async fn watch_connection(shared: Arc<Shared>, id: String, generation: u64, connection: Arc<Connection>) {
    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        if connection.is_closed() {
            shared.fail(&id, generation, "the SSH connection was closed".into());
            return;
        }
    }
}
