//! SSH shell sessions with OpenSSH-compatible host key verification, ported from PlexusTerm
//! v0.2.15 `src-tauri/src/ssh/mod.rs` (PRD SSH-1..SSH-6) without any UI types.
//!
//! Host keys are checked against a `known_hosts` file:
//! - a recorded matching key connects;
//! - a recorded *different* key is always rejected (possible MITM);
//! - an unknown host is put to the [`HostKeyPrompt`] when one is configured, and recorded only
//!   when it answers yes; without a prompt (or when it says no) the connection fails with
//!   [`SshError::UnknownHost`] carrying the fingerprint, and the caller may retry with
//!   `trusted_fingerprint` set to that exact value to accept and record it (v0.2.15's flow).
//!
//! A connection may go through up to [`MAX_JUMPS`] jump hosts (like `ssh -J`): each hop is
//! reached through a `direct-tcpip` channel of the previous one and applies the same
//! known_hosts policy and its own authentication. Errors of a jump host are wrapped in
//! [`SshError::Hop`], which names it.
//!
//! Password logins (typed or saved) first try public-key authentication like OpenSSH: the SSH
//! agent's keys, then the default identity files `id_ed25519`, `id_ecdsa` and `id_rsa` next to
//! known_hosts (`~/.ssh`), unencrypted only. The password is only sent when the server rejects
//! all of them (see [`try_default_keys`]).
//!
//! The shell's output is pushed into a [`term_engine::ByteRing`]; a full ring stops this crate
//! from taking messages off the channel, and russh's bounded channel queue then stops reading
//! the connection (PRD ST-3). Input and window changes come from [`ShellInput`] and
//! [`ShellResize`], the `output` writer and `on_resize` hook of a [`term_engine::Session`].

pub mod cwd;
pub mod platform;
#[cfg(any(test, feature = "test-support"))]
pub mod test_server;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use core_secrets::{NoStore, SavedPassword, SecretError, SharedSecrets, endpoint};
use russh::client::ChannelOpenHandle;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate, known_hosts, load_secret_key};
use russh::{Channel, ChannelMsg, ChannelOpenFailure, client};
use russh_sftp::client::SftpSession;
use term_engine::{ByteRing, Closed, TryPushError};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc, oneshot, watch};
use zeroize::Zeroizing;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long saving or forgetting a password may take after a login. A locked keyring can wait
/// for an unlock dialog indefinitely; the connection must not.
pub const STORAGE_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the connect timeout looks again while a host key prompt is open.
const PROMPT_POLL: Duration = Duration::from_millis(100);
/// Input chunks queued for the remote side before [`ShellInput`] blocks its caller.
const INPUT_QUEUE: usize = 16;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Every value behind these locks stays consistent, so a poisoned lock is still usable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What to connect to and the initial terminal size.
#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
    pub cols: u16,
    pub rows: u16,
    /// Fingerprint (`SHA256:...`) the user accepted for an unknown host.
    pub trusted_fingerprint: Option<String>,
    /// Jump hosts to go through, outermost first (see [`JumpHost`]).
    pub jumps: Vec<JumpHost>,
}

/// Most jump hosts a connection may go through.
pub const MAX_JUMPS: usize = 3;

/// A jump host on the way to the target, with everything that hop needs. `trusted_fingerprint`
/// and `storage` work like the target's own, for this hop only.
#[derive(Debug, Clone)]
pub struct JumpHost {
    /// Name shown in errors and prompts, e.g. the saved session name.
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
    pub trusted_fingerprint: Option<String>,
    pub storage: Option<PasswordStorage>,
}

/// How to log in. Passwords and passphrases are wiped from memory when dropped.
#[derive(Clone)]
pub enum SshAuth {
    Password {
        password: Zeroizing<String>,
    },
    Key {
        path: String,
        passphrase: Option<Zeroizing<String>>,
    },
    Agent,
    /// The password saved in the system keyring for this saved session.
    StoredPassword {
        session_id: String,
    },
}

/// What to do with the saved password of a session once a login succeeded.
#[derive(Debug, Clone)]
pub enum PasswordStorage {
    /// Save the typed password (`SshAuth::Password`) in the keyring.
    Remember { session_id: String, label: String },
    /// Delete the saved password.
    Forget { session_id: String },
}

/// Hand-written so secrets never reach logs or panic messages.
impl std::fmt::Debug for SshAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password { .. } => f.write_str("Password { .. }"),
            Self::Key { path, passphrase } => f
                .debug_struct("Key")
                .field("path", path)
                .field("passphrase", &passphrase.as_ref().map(|_| "<redacted>"))
                .finish(),
            Self::Agent => f.write_str("Agent"),
            Self::StoredPassword { session_id } => f
                .debug_struct("StoredPassword")
                .field("session_id", session_id)
                .finish(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SshError {
    #[error("the authenticity of host {host} can't be established ({fingerprint})")]
    UnknownHost {
        host: String,
        fingerprint: String,
        /// Algorithms of keys already recorded for this host (e.g. after a server key-type change).
        other_known_keys: Vec<String>,
    },
    #[error("the host key for {host} has changed ({fingerprint}); it does not match line {line} of known_hosts")]
    HostKeyChanged {
        host: String,
        fingerprint: String,
        line: usize,
    },
    #[error("authentication failed")]
    AuthFailed,
    #[error("no password is saved for this session")]
    SecretMissing,
    #[error("unknown terminal session {id}")]
    UnknownSession { id: u32 },
    #[error("{message}")]
    Connection { message: String },
    /// A jump host failed: `hop` is its 1-based position and `error` what went wrong there.
    #[error("via {name} (jump host {hop}): {error}")]
    Hop {
        hop: usize,
        name: String,
        error: Box<SshError>,
    },
}

fn connection(err: impl std::fmt::Display) -> SshError {
    SshError::Connection {
        message: err.to_string(),
    }
}

/// Display form used by OpenSSH: `host` for port 22, `[host]:port` otherwise.
fn host_label(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

/// Where a jump host sits in its chain, for the host key prompt ("jump host 1 of 2").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpPosition {
    pub name: String,
    /// 1-based position, outermost first.
    pub hop: usize,
    /// Jump hosts in the chain.
    pub of: usize,
}

/// An unknown host key to show the user (PRD SSH-2a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRequest {
    /// `host` or `[host]:port`, as OpenSSH shows it.
    pub host: String,
    /// `SHA256:...`
    pub fingerprint: String,
    /// Algorithms of keys already recorded for this host; non-empty means "warn".
    pub other_known_keys: Vec<String>,
    /// Set when the host is a jump host rather than the target.
    pub jump: Option<JumpPosition>,
}

pub type PromptFuture = Pin<Box<dyn Future<Output = bool> + Send>>;

/// Asks the user whether to trust an unknown host key (the port the UI implements).
///
/// The connect timeout does not run while the prompt is open.
pub trait HostKeyPrompt: Send + Sync {
    /// Resolves to `true` to record the key in known_hosts and connect ("Trust and connect").
    fn confirm(&self, request: TrustRequest) -> PromptFuture;
}

impl<F, Fut> HostKeyPrompt for F
where
    F: Fn(TrustRequest) -> Fut + Send + Sync,
    Fut: Future<Output = bool> + Send + 'static,
{
    fn confirm(&self, request: TrustRequest) -> PromptFuture {
        Box::pin(self(request))
    }
}

pub type SharedPrompt = Arc<dyn HostKeyPrompt>;

/// Applies the known_hosts policy described in the module docs (without the prompt).
fn verify_host_key(
    known_hosts: &Path,
    host: &str,
    port: u16,
    key: &PublicKey,
    trusted_fingerprint: Option<&str>,
) -> Result<(), SshError> {
    let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
    match known_hosts::check_known_hosts_path(host, port, key, known_hosts) {
        Ok(true) => Ok(()),
        Ok(false) if trusted_fingerprint == Some(fingerprint.as_str()) => {
            known_hosts::learn_known_hosts_path(host, port, key, known_hosts).map_err(connection)
        }
        Ok(false) => {
            let mut other_known_keys: Vec<String> = known_hosts::known_host_keys_path(host, port, known_hosts)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, key)| key.algorithm().to_string())
                .collect();
            other_known_keys.dedup();
            Err(SshError::UnknownHost {
                host: host_label(host, port),
                fingerprint,
                other_known_keys,
            })
        }
        Err(russh::keys::Error::KeyChanged { line }) => Err(SshError::HostKeyChanged {
            host: host_label(host, port),
            fingerprint,
            line,
        }),
        Err(err) => Err(SshError::Connection {
            message: format!("cannot read known_hosts: {err}"),
        }),
    }
}

/// Time spent in host key prompts, which the connect timeout does not count.
#[derive(Default)]
struct PromptClock(Mutex<ClockState>);

#[derive(Default)]
struct ClockState {
    paused: Duration,
    since: Option<Instant>,
}

impl PromptClock {
    async fn paused<T>(&self, work: impl Future<Output = T>) -> T {
        lock(&self.0).since = Some(Instant::now());
        let out = work.await;
        let mut state = lock(&self.0);
        if let Some(since) = state.since.take() {
            state.paused += since.elapsed();
        }
        out
    }

    /// Time since `start` spent outside prompts, or `None` while a prompt is open.
    fn active_since(&self, start: Instant) -> Option<Duration> {
        let state = lock(&self.0);
        match state.since {
            Some(_) => None,
            None => Some(start.elapsed().saturating_sub(state.paused)),
        }
    }
}

/// Runs `work` until it ends or `timeout` passed outside host key prompts.
async fn within_connect_timeout<T>(
    timeout: Duration,
    clock: &PromptClock,
    label: &str,
    work: impl Future<Output = Result<T, SshError>>,
) -> Result<T, SshError> {
    let start = Instant::now();
    let mut work = std::pin::pin!(work);
    loop {
        let wait = match clock.active_since(start) {
            None => PROMPT_POLL,
            Some(active) => match timeout.checked_sub(active) {
                Some(left) if !left.is_zero() => left,
                _ => {
                    return Err(SshError::Connection {
                        message: format!("connection to {label} timed out"),
                    });
                }
            },
        };
        tokio::select! {
            result = &mut work => return result,
            () = tokio::time::sleep(wait) => {}
        }
    }
}

/// A `forwarded-tcpip` channel opened by the server for a remote port forward.
pub type ForwardedChannel = Channel<client::Msg>;

pub struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    trusted_fingerprint: Option<String>,
    prompt: Option<SharedPrompt>,
    jump: Option<JumpPosition>,
    clock: Arc<PromptClock>,
    /// Why the host key was rejected; russh only reports a generic error.
    rejection: Arc<Mutex<Option<SshError>>>,
    /// Receives the server's `forwarded-tcpip` channels; without it they are refused.
    forwarded: Option<UnboundedSender<ForwardedChannel>>,
}

/// An authenticated SSH connection.
pub type Connection = client::Handle<ClientHandler>;

impl ClientHandler {
    async fn check_key(&self, key: &PublicKey) -> Result<(), SshError> {
        let verified = verify_host_key(
            &self.known_hosts,
            &self.host,
            self.port,
            key,
            self.trusted_fingerprint.as_deref(),
        );
        let (
            Err(SshError::UnknownHost {
                host,
                fingerprint,
                other_known_keys,
            }),
            Some(prompt),
        ) = (&verified, &self.prompt)
        else {
            return verified;
        };
        let request = TrustRequest {
            host: host.clone(),
            fingerprint: fingerprint.clone(),
            other_known_keys: other_known_keys.clone(),
            jump: self.jump.clone(),
        };
        if self.clock.paused(prompt.confirm(request)).await {
            known_hosts::learn_known_hosts_path(&self.host, self.port, key, &self.known_hosts).map_err(connection)
        } else {
            verified
        }
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let result = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => self.check_key(key).await,
            PublicKeyOrCertificate::Certificate(_) => Err(SshError::Connection {
                message: "SSH host certificates are not supported yet".into(),
            }),
        };
        match result {
            Ok(()) => Ok(true),
            Err(err) => {
                *lock(&self.rejection) = Some(err);
                Ok(false)
            }
        }
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // One connection per tunnel, so every forwarded channel belongs to it.
        match &self.forwarded {
            Some(tx) if tx.send(channel).is_ok() => reply.accept().await,
            _ => reply.reject(ChannelOpenFailure::AdministrativelyProhibited).await,
        }
        Ok(())
    }
}

/// Where and as whom to connect.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
    /// Jump hosts to go through, outermost first.
    pub jumps: Vec<JumpHost>,
}

/// How [`connect_authenticated`] reaches the server and the saved passwords.
#[derive(Clone, Copy)]
pub struct ConnectContext<'a> {
    pub known_hosts: &'a Path,
    pub agent_socket: Option<&'a Path>,
    pub secrets: &'a SharedSecrets,
    /// Bound on the keyring update after the login (see [`STORAGE_TIMEOUT`]).
    pub storage_timeout: Duration,
    /// Bound on reaching each hop, host key prompts excluded (15 s).
    pub connect_timeout: Duration,
    pub prompt: Option<&'a SharedPrompt>,
}

/// An authenticated connection and the jump host connections it goes through.
pub struct Chain {
    pub connection: Connection,
    /// Outermost first; they must stay open as long as `connection`.
    pub jumps: Vec<Connection>,
    /// User-facing keyring problems after the logins (see [`apply_storage`]).
    pub warnings: Vec<String>,
}

/// Ends jump host connections, innermost first.
pub async fn disconnect_jumps(jumps: &[Connection]) {
    for jump in jumps.iter().rev() {
        let _ = jump.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    }
}

/// One SSH server to log into, either directly or through the previous hop.
struct Hop {
    host: String,
    port: u16,
    username: String,
    auth: SshAuth,
    trusted_fingerprint: Option<String>,
    storage: Option<PasswordStorage>,
    jump: Option<JumpPosition>,
}

/// Connects to `target` through its jump hosts (if any), applying the known_hosts policy and
/// authenticating at every hop, then applies `storage` to the target's saved password (each
/// jump host carries its own). Nothing is stored or deleted for a hop whose login failed.
///
/// A jump host failure is returned as [`SshError::Hop`] and closes the hops already opened.
/// Keyring problems never fail the login: they are returned as warnings.
pub async fn connect_authenticated(
    target: SshTarget,
    trusted_fingerprint: Option<String>,
    storage: Option<PasswordStorage>,
    ctx: ConnectContext<'_>,
    forwarded: Option<UnboundedSender<ForwardedChannel>>,
) -> Result<Chain, SshError> {
    if target.jumps.len() > MAX_JUMPS {
        return Err(SshError::Connection {
            message: format!("at most {MAX_JUMPS} jump hosts are supported"),
        });
    }
    let total = target.jumps.len();
    let mut jumps: Vec<Connection> = Vec::with_capacity(total);
    let mut warnings = Vec::new();
    for (index, jump) in target.jumps.into_iter().enumerate() {
        let (hop, name) = (index + 1, jump.name);
        let next = Hop {
            host: jump.host,
            port: jump.port,
            username: jump.username,
            auth: jump.auth,
            trusted_fingerprint: jump.trusted_fingerprint,
            storage: jump.storage,
            jump: Some(JumpPosition {
                name: name.clone(),
                hop,
                of: total,
            }),
        };
        match connect_hop(next, jumps.last(), ctx, None).await {
            Ok((connection, warning)) => {
                jumps.push(connection);
                warnings.extend(warning.map(|w| format!("via {name} (jump host {hop}): {w}")));
            }
            Err(err) => {
                disconnect_jumps(&jumps).await;
                return Err(SshError::Hop {
                    hop,
                    name,
                    error: Box::new(err),
                });
            }
        }
    }
    let last = Hop {
        host: target.host,
        port: target.port,
        username: target.username,
        auth: target.auth,
        trusted_fingerprint,
        storage,
        jump: None,
    };
    match connect_hop(last, jumps.last(), ctx, forwarded).await {
        Ok((connection, warning)) => {
            warnings.extend(warning);
            Ok(Chain {
                connection,
                jumps,
                warnings,
            })
        }
        Err(err) => {
            disconnect_jumps(&jumps).await;
            Err(err)
        }
    }
}

/// Connects to one hop (over a `direct-tcpip` channel of `via` when given), applies the
/// known_hosts policy, authenticates and then applies the hop's `storage`.
///
/// A password login first tries the default keys; when the server drops the connection while
/// they are tried (too many authentication failures), the hop is reconnected once and only the
/// password is tried.
async fn connect_hop(
    hop: Hop,
    via: Option<&Connection>,
    ctx: ConnectContext<'_>,
    forwarded: Option<UnboundedSender<ForwardedChannel>>,
) -> Result<(Connection, Option<String>), SshError> {
    let mut handle = open_hop(&hop, via, ctx, forwarded.clone()).await?;
    let endpoint = endpoint(&hop.username, &hop.host, hop.port);

    if matches!(hop.auth, SshAuth::Password { .. } | SshAuth::StoredPassword { .. }) {
        let identity_dir = ctx.known_hosts.parent();
        match try_default_keys(&mut handle, &hop.username, ctx.agent_socket, identity_dir).await {
            // The typed password was never checked, so it is not saved.
            KeysOutcome::Accepted => {
                let warning = apply_storage(ctx.secrets, hop.storage, None, endpoint, ctx.storage_timeout).await;
                return Ok((handle, warning));
            }
            KeysOutcome::Rejected => {}
            KeysOutcome::Disconnected => {
                handle = open_hop(&hop, via, ctx, forwarded).await?;
            }
        }
    }

    let typed_password = match &hop.auth {
        SshAuth::Password { password } => Some(password.clone()),
        _ => None,
    };
    authenticate(
        &mut handle,
        &hop.username,
        hop.auth,
        ctx.agent_socket,
        ctx.secrets,
        &endpoint,
    )
    .await?;
    let warning = apply_storage(ctx.secrets, hop.storage, typed_password, endpoint, ctx.storage_timeout).await;
    Ok((handle, warning))
}

/// Opens the SSH connection to `hop` (over a `direct-tcpip` channel of `via` when given) and
/// applies the known_hosts policy, without authenticating.
async fn open_hop(
    hop: &Hop,
    via: Option<&Connection>,
    ctx: ConnectContext<'_>,
    forwarded: Option<UnboundedSender<ForwardedChannel>>,
) -> Result<Connection, SshError> {
    let rejection = Arc::new(Mutex::new(None));
    let clock = Arc::new(PromptClock::default());
    let handler = ClientHandler {
        host: hop.host.clone(),
        port: hop.port,
        known_hosts: ctx.known_hosts.to_path_buf(),
        trusted_fingerprint: hop.trusted_fingerprint.clone(),
        prompt: ctx.prompt.cloned(),
        jump: hop.jump.clone(),
        clock: clock.clone(),
        rejection: rejection.clone(),
        forwarded,
    };
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        ..Default::default()
    });
    let label = host_label(&hop.host, hop.port);

    let connecting = async {
        let connected = match via {
            None => client::connect(config, (hop.host.as_str(), hop.port), handler).await,
            Some(jump) => {
                let channel = jump
                    .channel_open_direct_tcpip(hop.host.clone(), hop.port.into(), "127.0.0.1", 0)
                    .await
                    .map_err(|err| SshError::Connection {
                        message: format!("the jump host cannot reach {label}: {err}"),
                    })?;
                client::connect_stream(config, channel.into_stream(), handler).await
            }
        };
        connected.map_err(|err| lock(&rejection).take().unwrap_or_else(|| connection(err)))
    };
    within_connect_timeout(ctx.connect_timeout, &clock, &label, connecting).await
}

/// Default identity files tried by password logins, in OpenSSH's order.
const DEFAULT_IDENTITIES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// Most keys a password login tries before the password. OpenSSH servers allow 6
/// authentication attempts by default, so the password always keeps one.
const MAX_DEFAULT_KEY_ATTEMPTS: usize = 5;

/// What trying the default keys before a password led to.
#[derive(Debug, PartialEq, Eq)]
enum KeysOutcome {
    Accepted,
    /// Every key was rejected (or there was none): the password comes next.
    Rejected,
    /// The server closed the connection while keys were tried.
    Disconnected,
}

/// Tries the SSH agent's keys, then the unencrypted default identity files in `identity_dir`
/// that the agent did not already offer, at most [`MAX_DEFAULT_KEY_ATTEMPTS`] in all. Nothing
/// here prompts or fails the login: an unavailable agent, a missing, unreadable or encrypted
/// file, or a key that cannot sign is skipped.
async fn try_default_keys(
    handle: &mut Connection,
    user: &str,
    agent_socket: Option<&Path>,
    identity_dir: Option<&Path>,
) -> KeysOutcome {
    let hash = match handle.best_supported_rsa_hash().await {
        Ok(hash) => hash.flatten(),
        Err(_) if handle.is_closed() => return KeysOutcome::Disconnected,
        Err(_) => None,
    };
    let mut attempts = 0;
    let mut offered: Vec<PublicKey> = Vec::new();

    // Tests only use an explicit agent socket, never the developer's own agent.
    if (agent_socket.is_some() || !(cfg!(test) || cfg!(feature = "test-support")))
        && let Ok(mut agent) = platform::connect_agent(agent_socket).await
    {
        let identities = agent.request_identities().await.unwrap_or_default();
        for identity in identities {
            if attempts == MAX_DEFAULT_KEY_ATTEMPTS {
                return KeysOutcome::Rejected;
            }
            let key = identity.public_key().into_owned();
            attempts += 1;
            offered.push(key.clone());
            match handle.authenticate_publickey_with(user, key, hash, &mut agent).await {
                Ok(result) if result.success() => return KeysOutcome::Accepted,
                _ if handle.is_closed() => return KeysOutcome::Disconnected,
                _ => {}
            }
        }
    }

    let Some(dir) = identity_dir else {
        return KeysOutcome::Rejected;
    };
    for name in DEFAULT_IDENTITIES {
        if attempts == MAX_DEFAULT_KEY_ATTEMPTS {
            break;
        }
        // Missing, unreadable and passphrase-protected files are skipped without asking.
        let Ok(key) = load_secret_key(dir.join(name), None) else {
            continue;
        };
        if offered
            .iter()
            .any(|known| known.key_data() == key.public_key().key_data())
        {
            continue;
        }
        attempts += 1;
        match handle
            .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await
        {
            Ok(result) if result.success() => return KeysOutcome::Accepted,
            _ if handle.is_closed() => return KeysOutcome::Disconnected,
            _ => {}
        }
    }
    KeysOutcome::Rejected
}

/// Saves or forgets the session's password after a successful login. A keyring failure does
/// not fail the login: it becomes the returned warning, which never contains the password.
async fn apply_storage(
    secrets: &SharedSecrets,
    storage: Option<PasswordStorage>,
    typed_password: Option<Zeroizing<String>>,
    endpoint: String,
    timeout: Duration,
) -> Option<String> {
    let (failure, result) = match (storage, typed_password) {
        (Some(PasswordStorage::Remember { session_id, label }), Some(password)) => {
            let value = Zeroizing::new(SavedPassword::encode(endpoint, password.as_str().to_owned()));
            drop(password);
            let update = core_secrets::run(secrets, move |s| s.set(&session_id, &label, &value));
            (
                "The password could not be saved in the system keyring",
                bounded(timeout, update).await,
            )
        }
        (Some(PasswordStorage::Forget { session_id }), _) => {
            let update = core_secrets::run(secrets, move |s| s.delete(&session_id));
            (
                "The saved password could not be removed from the system keyring",
                bounded(timeout, update).await,
            )
        }
        _ => return None,
    };
    result.err().map(|reason| format!("{failure}: {reason}"))
}

/// Runs a keyring update for at most `timeout`; the error is a reason for the user.
async fn bounded(timeout: Duration, update: impl Future<Output = Result<(), SecretError>>) -> Result<(), String> {
    // On timeout the blocking keyring call keeps running in the background and may still
    // complete once the keyring is unlocked; the connection goes on regardless.
    match tokio::time::timeout(timeout, update).await {
        Ok(result) => result.map_err(|err| err.to_string()),
        Err(_) => Err("the system keyring did not respond in time (it may be locked)".into()),
    }
}

async fn authenticate(
    handle: &mut Connection,
    user: &str,
    auth: SshAuth,
    agent_socket: Option<&Path>,
    secrets: &SharedSecrets,
    endpoint: &str,
) -> Result<(), SshError> {
    let accepted = match auth {
        SshAuth::Password { password } => handle
            .authenticate_password(user, password.as_str())
            .await
            .map_err(connection)?
            .success(),
        SshAuth::StoredPassword { session_id } => {
            // An unreadable keyring, or a secret saved for another endpoint (the session was
            // edited) or in an older format, is treated like a missing secret: the user is
            // asked instead and the secret is never sent.
            let value = core_secrets::run(secrets, move |s| s.get(&session_id))
                .await
                .ok()
                .flatten()
                .map(Zeroizing::new);
            let password = value
                .and_then(|value| SavedPassword::password_for(&value, endpoint))
                .map(Zeroizing::new)
                .ok_or(SshError::SecretMissing)?;
            handle
                .authenticate_password(user, password.as_str())
                .await
                .map_err(connection)?
                .success()
        }
        SshAuth::Key { path, passphrase } => {
            let key = load_secret_key(platform::expand_home(&path), passphrase.as_ref().map(|p| p.as_str())).map_err(
                |err| SshError::Connection {
                    message: format!("cannot load private key {path}: {err}"),
                },
            )?;
            let hash = handle.best_supported_rsa_hash().await.map_err(connection)?.flatten();
            handle
                .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await
                .map_err(connection)?
                .success()
        }
        SshAuth::Agent => {
            let mut agent = platform::connect_agent(agent_socket)
                .await
                .map_err(|err| SshError::Connection {
                    message: format!("SSH agent unavailable: {err}"),
                })?;
            let identities = agent.request_identities().await.map_err(connection)?;
            let hash = handle.best_supported_rsa_hash().await.map_err(connection)?.flatten();
            let mut accepted = false;
            for identity in identities {
                let key = identity.public_key().into_owned();
                // One identity failing to sign (e.g. a removed hardware key) must not stop the others.
                if let Ok(result) = handle.authenticate_publickey_with(user, key, hash, &mut agent).await
                    && result.success()
                {
                    accepted = true;
                    break;
                }
            }
            accepted
        }
    };
    if accepted { Ok(()) } else { Err(SshError::AuthFailed) }
}

/// Size and lifetime of a shell, shared by the manager, [`ShellResize`] and the session tasks.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Control {
    size: (u16, u16),
    closed: bool,
}

/// Input of a shell: the `output` writer of a [`term_engine::Session`].
///
/// `write` waits while the remote side is not taking input (bounded queue), so it must run on
/// a plain thread (term-engine's writer thread), never inside an async task.
pub struct ShellInput(mpsc::Sender<Vec<u8>>);

impl Write for ShellInput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.0
            .blocking_send(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the SSH session has ended"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Window changes of a shell: the `on_resize` hook of a [`term_engine::Session`]. Never blocks;
/// the latest size wins and is sent before input queued after it.
#[derive(Clone)]
pub struct ShellResize(Arc<watch::Sender<Control>>);

impl ShellResize {
    pub fn resize(&self, cols: u16, rows: u16) {
        self.0.send_modify(|control| control.size = (cols, rows));
    }

    /// This handle as a [`term_engine::SessionHooks::on_resize`].
    pub fn into_hook(self) -> Arc<dyn Fn(u16, u16) + Send + Sync> {
        Arc::new(move |cols, rows| self.resize(cols, rows))
    }
}

/// A shell opened by [`SshManager::connect_with`].
pub struct Shell {
    pub id: u32,
    pub input: ShellInput,
    pub resize: ShellResize,
    /// The remote exit status (`None` when the server sent none) once the session ended; the
    /// output ring is closed right after.
    pub exit: oneshot::Receiver<Option<u32>>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell").field("id", &self.id).finish_non_exhaustive()
    }
}

/// The SFTP subsystem of a session, opened on first use.
type SftpSlot = Arc<tokio::sync::Mutex<Option<Arc<SftpSession>>>>;

struct SessionEntry {
    input: mpsc::Sender<Vec<u8>>,
    control: Arc<watch::Sender<Control>>,
    handle: Arc<Connection>,
    sftp: SftpSlot,
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

pub struct SshManager {
    known_hosts: PathBuf,
    /// Overrides `SSH_AUTH_SOCK` (used by tests).
    agent_socket: Option<PathBuf>,
    secrets: SharedSecrets,
    storage_timeout: Duration,
    connect_timeout: Duration,
    prompt: Option<SharedPrompt>,
    next_id: AtomicU32,
    sessions: Mutex<HashMap<u32, SessionEntry>>,
}

impl SshManager {
    pub fn new(known_hosts: PathBuf) -> Self {
        Self {
            known_hosts,
            agent_socket: None,
            secrets: Arc::new(NoStore),
            storage_timeout: STORAGE_TIMEOUT,
            connect_timeout: CONNECT_TIMEOUT,
            prompt: None,
            next_id: AtomicU32::new(0),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Uses `secrets` for stored passwords (none are available otherwise).
    pub fn with_secrets(mut self, secrets: SharedSecrets) -> Self {
        self.secrets = secrets;
        self
    }

    /// Asks `prompt` about unknown host keys instead of failing with [`SshError::UnknownHost`].
    pub fn with_host_key_prompt(mut self, prompt: SharedPrompt) -> Self {
        self.prompt = Some(prompt);
        self
    }

    #[cfg(test)]
    fn with_storage_timeout(mut self, timeout: Duration) -> Self {
        self.storage_timeout = timeout;
        self
    }

    #[cfg(test)]
    fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    #[cfg(all(test, unix))]
    fn with_agent_socket(mut self, socket: PathBuf) -> Self {
        self.agent_socket = Some(socket);
        self
    }

    /// How this manager reaches servers and saved passwords, for logins made by other session
    /// kinds (Mosh starts its server over SSH; tunnels open their own connection).
    pub fn context(&self) -> ConnectContext<'_> {
        ConnectContext {
            known_hosts: &self.known_hosts,
            agent_socket: self.agent_socket.as_deref(),
            secrets: &self.secrets,
            storage_timeout: self.storage_timeout,
            connect_timeout: self.connect_timeout,
            prompt: self.prompt.as_ref(),
        }
    }

    /// Uses the current user's `~/.ssh/known_hosts`, like OpenSSH.
    pub fn for_current_user() -> Self {
        Self::new(platform::known_hosts_path())
    }

    /// Connects without touching saved passwords (tests; the app uses `connect_with`).
    #[cfg(test)]
    pub async fn connect(&self, req: ConnectRequest, output: ByteRing) -> Result<Shell, SshError> {
        self.connect_with(req, None, output).await
    }

    /// Opens an interactive shell whose output goes to `output`, then saves or forgets the
    /// session's password as `storage` asks once the login succeeded. Keyring warnings are the
    /// first thing written to `output`, in yellow. `output` is closed when the session ends.
    pub async fn connect_with(
        &self,
        req: ConnectRequest,
        storage: Option<PasswordStorage>,
        output: ByteRing,
    ) -> Result<Shell, SshError> {
        let target = SshTarget {
            host: req.host,
            port: req.port,
            username: req.username,
            auth: req.auth,
            jumps: req.jumps,
        };
        let ctx = self.context();
        let Chain {
            connection: handle,
            jumps,
            warnings,
        } = connect_authenticated(target, req.trusted_fingerprint, storage, ctx, None).await?;
        let jumps = Arc::new(jumps);

        let channel = handle.channel_open_session().await.map_err(connection)?;
        channel
            .request_pty(false, "xterm-256color", req.cols.into(), req.rows.into(), 0, 0, &[])
            .await
            .map_err(connection)?;
        channel.request_shell(false).await.map_err(connection)?;
        // Shown before any remote output, which only the reader task below forwards.
        for warning in warnings {
            let _ = push(&output, format!("\r\n\x1b[33m{warning}\x1b[0m\r\n").as_bytes()).await;
        }

        let (input, mut inputs) = mpsc::channel::<Vec<u8>>(INPUT_QUEUE);
        let control = Arc::new(
            watch::channel(Control {
                size: (req.cols, req.rows),
                closed: false,
            })
            .0,
        );
        let mut controls = control.subscribe();
        let resize = ShellResize(control.clone());
        let (exit_tx, exit) = oneshot::channel();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (mut reader, writer) = channel.split();
        let handle = Arc::new(handle);
        lock(&self.sessions).insert(
            id,
            SessionEntry {
                input: input.clone(),
                control: control.clone(),
                handle: handle.clone(),
                sftp: SftpSlot::default(),
            },
        );

        // Writes run in their own task: a large paste can wait for the remote window while the
        // reader keeps draining output (otherwise both stall).
        let (writer_handle, writer_jumps) = (handle.clone(), jumps.clone());
        let mut applied = (req.cols, req.rows);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // A resize goes out before input queued after it.
                    biased;
                    changed = controls.changed() => {
                        let now = *controls.borrow_and_update();
                        if changed.is_err() || now.closed {
                            break;
                        }
                        if now.size != applied {
                            applied = now.size;
                            let _ = writer.window_change(now.size.0.into(), now.size.1.into(), 0, 0).await;
                        }
                    }
                    data = inputs.recv() => match data {
                        Some(data) if writer.data(data.as_slice()).await.is_ok() => {}
                        _ => break,
                    },
                }
            }
            // Closed by the user (or every handle was dropped): end the whole connection,
            // which also ends the reader below.
            let _ = writer.close().await;
            let _ = writer_handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
            disconnect_jumps(&writer_jumps).await;
        });

        tokio::spawn(async move {
            let mut exit_code = None;
            while let Some(message) = reader.wait().await {
                match message {
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                        // Nobody reads this terminal any more: end the session.
                        if push(&output, &data).await.is_err() {
                            break;
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            control.send_modify(|control| control.closed = true);
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
            disconnect_jumps(&jumps).await;
            let _ = exit_tx.send(exit_code);
            output.close();
        });

        Ok(Shell {
            id,
            input: ShellInput(input),
            resize,
            exit,
        })
    }

    fn entry<T>(&self, id: u32, get: impl FnOnce(&SessionEntry) -> T) -> Result<T, SshError> {
        lock(&self.sessions)
            .get(&id)
            .map(get)
            .ok_or(SshError::UnknownSession { id })
    }

    /// Queues `data` for session `id`, waiting while the remote side is not taking input.
    pub async fn write(&self, id: u32, data: &[u8]) -> Result<(), SshError> {
        let input = self.entry(id, |entry| entry.input.clone())?;
        input.send(data.to_vec()).await.map_err(|_| SshError::Connection {
            message: "the SSH session has ended".into(),
        })
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), SshError> {
        let control = self.entry(id, |entry| entry.control.clone())?;
        if control.borrow().closed {
            return Err(SshError::Connection {
                message: "the SSH session has ended".into(),
            });
        }
        ShellResize(control).resize(cols, rows);
        Ok(())
    }

    pub fn close(&self, id: u32) -> Result<(), SshError> {
        let entry = lock(&self.sessions)
            .remove(&id)
            .ok_or(SshError::UnknownSession { id })?;
        // The session tasks may already have ended; nothing else to clean up then.
        entry.control.send_modify(|control| control.closed = true);
        Ok(())
    }

    /// Returns the SFTP subsystem of session `id`, opening it over the same SSH connection on
    /// first use.
    pub async fn sftp(&self, id: u32) -> Result<Arc<SftpSession>, SshError> {
        let (handle, slot) = self.entry(id, |entry| (entry.handle.clone(), entry.sftp.clone()))?;
        // Held across the open so concurrent callers share a single channel.
        let mut slot = slot.lock().await;
        if let Some(sftp) = slot.as_ref() {
            return Ok(sftp.clone());
        }
        let unavailable = |err: &dyn std::fmt::Display| SshError::Connection {
            message: format!("cannot start SFTP: {err}"),
        };
        let channel = handle.channel_open_session().await.map_err(|err| unavailable(&err))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|err| unavailable(&err))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map(Arc::new)
            .map_err(|err| unavailable(&err))?;
        *slot = Some(sftp.clone());
        Ok(sftp)
    }

    /// Runs `command` on session `id` over a new exec channel of its existing connection (no
    /// new login) and returns its stdout. Output beyond [`MAX_EXEC_OUTPUT`] is dropped; a
    /// non-zero exit status or no answer within [`EXEC_TIMEOUT`] is an error.
    pub async fn exec(&self, id: u32, command: &str) -> Result<String, SshError> {
        let handle = self.entry(id, |entry| entry.handle.clone())?;
        let run = async {
            let mut channel = handle.channel_open_session().await.map_err(connection)?;
            channel.exec(true, command).await.map_err(connection)?;
            let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
            let mut exit_status = None;
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Data { data } => append_capped(&mut stdout, &data),
                    ChannelMsg::ExtendedData { data, .. } => append_capped(&mut stderr, &data),
                    ChannelMsg::ExitStatus { exit_status: s } => exit_status = Some(s),
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            match exit_status {
                Some(0) | None => Ok(String::from_utf8_lossy(&stdout).into_owned()),
                Some(status) => Err(SshError::Connection {
                    message: format!(
                        "remote command failed (exit status {status}): {}",
                        String::from_utf8_lossy(&stderr).trim()
                    ),
                }),
            }
        };
        tokio::time::timeout(EXEC_TIMEOUT, run).await.unwrap_or_else(|_| {
            Err(SshError::Connection {
                message: format!("remote command did not finish within {} s", EXEC_TIMEOUT.as_secs()),
            })
        })
    }
}

/// How long [`SshManager::exec`] waits for a command to finish.
const EXEC_TIMEOUT: Duration = Duration::from_secs(10);
/// Bytes of stdout (and, separately, stderr) [`SshManager::exec`] keeps.
const MAX_EXEC_OUTPUT: usize = 256 * 1024;

fn append_capped(buffer: &mut Vec<u8>, data: &[u8]) {
    let room = MAX_EXEC_OUTPUT.saturating_sub(buffer.len());
    buffer.extend(data.iter().take(room));
}
