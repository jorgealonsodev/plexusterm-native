//! Minimal in-process SSH server used by the SSH client tests.
//!
//! Behaviour: accepts password `secret` or the configured public key, opens a
//! shell channel and echoes data back. Servers started with [`start_limited`]
//! disconnect after that many rejected authentication attempts (like OpenSSH's
//! `MaxAuthTries`). `size?` replies with the last window
//! size, and `exit` ends the channel with exit status 7.
//!
//! Servers started with [`start_with_sftp`] also serve the `sftp` subsystem
//! from a local directory, which appears to the client as `/`.
//!
//! Port forwarding works like OpenSSH: `direct-tcpip` channels connect to the
//! requested target, and `tcpip-forward` listens on the requested address and
//! opens a `forwarded-tcpip` channel back to the client for every connection.
//!
//! Exec requests fake a remote shell running one command (see [`MOSH_CONNECT_REPLY`]): a
//! command given a canned reply with [`TestServer::reply_to_exec`] prints it and exits 0,
//! `mosh-server ...` prints a `MOSH CONNECT` line, `sleep-forever` never answers,
//! `killed-by-signal` ends with an exit signal and no exit status, `true` exits quietly, and
//! anything else fails like a missing program (exit status 127). Exec channels the client has
//! not closed yet are counted by [`TestServer::open_exec_channels`].
//!
//! Built for this crate's tests and, with the `test-support` feature, for other crates' tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test harness: a failure here is a failed test"
)]

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::{PrivateKey, PublicKey};
use russh::server::{Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId, Pty};
use russh_sftp::protocol::{Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode};
use std::io::{Read, Seek, SeekFrom, Write};
use tokio::net::{TcpListener, TcpStream};

pub const PASSWORD: &str = "secret";

/// What the fake `mosh-server` prints, like the real one (port 60001, a 22-character key).
pub const MOSH_CONNECT_REPLY: &str =
    "\r\nMOSH CONNECT 60001 AAAAAAAAAAAAAAAAAAAAAA\r\n\r\nmosh-server (mosh 1.4.0) [build mosh 1.4.0]\r\n";

pub fn key_from_seed(seed: u8) -> PrivateKey {
    PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]))
}

pub struct TestServer {
    pub addr: SocketAddr,
    pub host_key: PublicKey,
    /// Password authentication attempts received, over every connection.
    pub password_attempts: Arc<AtomicUsize>,
    /// Public-key authentication requests received, over every connection.
    publickey_attempts: Arc<AtomicUsize>,
    /// Client connections currently open.
    open_connections: Arc<AtomicUsize>,
    /// Commands received in exec requests, over every connection.
    exec_commands: Arc<Mutex<Vec<String>>>,
    /// Canned stdout of exec commands, by exact command.
    exec_replies: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Exec channels the client has not closed yet, over every connection.
    open_exec_channels: Arc<AtomicUsize>,
}

impl TestServer {
    pub fn password_attempts(&self) -> usize {
        self.password_attempts.load(Ordering::SeqCst)
    }

    pub fn publickey_attempts(&self) -> usize {
        self.publickey_attempts.load(Ordering::SeqCst)
    }

    pub fn open_connections(&self) -> usize {
        self.open_connections.load(Ordering::SeqCst)
    }

    pub fn exec_commands(&self) -> Vec<String> {
        self.exec_commands.lock().unwrap().clone()
    }

    /// Makes the exec request `command` print `stdout` and exit with status 0.
    pub fn reply_to_exec(&self, command: &str, stdout: &str) {
        self.exec_replies
            .lock()
            .unwrap()
            .insert(command.to_owned(), stdout.as_bytes().to_vec());
    }

    pub fn open_exec_channels(&self) -> usize {
        self.open_exec_channels.load(Ordering::SeqCst)
    }
}

/// Counts a client connection as open until its handler is dropped.
struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct Handler {
    allowed_key: Option<PublicKey>,
    size: Arc<Mutex<(u32, u32)>>,
    sftp_root: Option<Arc<PathBuf>>,
    /// Channels of this connection that may still become an SFTP subsystem.
    channels: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    /// Channels served through their `Channel` object (SFTP, port forwards),
    /// which must not be echoed.
    stream_channels: Arc<Mutex<HashSet<ChannelId>>>,
    /// Remote forward listeners of this connection, by requested address and port.
    forwards: Arc<Mutex<HashMap<(String, u32), tokio::task::AbortHandle>>>,
    password_attempts: Arc<AtomicUsize>,
    publickey_attempts: Arc<AtomicUsize>,
    open_connections: Arc<AtomicUsize>,
    exec_commands: Arc<Mutex<Vec<String>>>,
    exec_replies: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    open_exec_channels: Arc<AtomicUsize>,
    /// Exec channels of this connection the client has not closed yet.
    exec_channels: Arc<Mutex<HashSet<ChannelId>>>,
    _connection: Option<Arc<ConnectionGuard>>,
}

/// Starts a server with the given host key seed; `allowed_key` enables public-key auth.
pub async fn start(host_seed: u8, allowed_key: Option<PublicKey>) -> TestServer {
    start_inner(host_seed, allowed_key, None, 10).await
}

/// Like [`start`], but disconnects once `max_auth_attempts` attempts were rejected.
pub async fn start_limited(host_seed: u8, allowed_key: Option<PublicKey>, max_auth_attempts: usize) -> TestServer {
    start_inner(host_seed, allowed_key, None, max_auth_attempts).await
}

/// Starts a password-auth server (host key seed 1) that also serves SFTP from `root`.
pub async fn start_with_sftp(root: &Path) -> TestServer {
    start_inner(1, None, Some(Arc::new(root.to_path_buf())), 10).await
}

async fn start_inner(
    host_seed: u8,
    allowed_key: Option<PublicKey>,
    sftp_root: Option<Arc<PathBuf>>,
    max_auth_attempts: usize,
) -> TestServer {
    let host = key_from_seed(host_seed);
    let host_key = host.public_key().clone();
    let config = Arc::new(russh::server::Config {
        keys: vec![host],
        auth_rejection_time: std::time::Duration::from_millis(0),
        auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
        max_auth_attempts,
        ..Default::default()
    });
    let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let mut server = Handler {
        allowed_key,
        size: Arc::new(Mutex::new((0, 0))),
        sftp_root,
        channels: Arc::default(),
        stream_channels: Arc::default(),
        forwards: Arc::default(),
        password_attempts: Arc::default(),
        publickey_attempts: Arc::default(),
        open_connections: Arc::default(),
        exec_commands: Arc::default(),
        exec_replies: Arc::default(),
        open_exec_channels: Arc::default(),
        exec_channels: Arc::default(),
        _connection: None,
    };
    let password_attempts = server.password_attempts.clone();
    let publickey_attempts = server.publickey_attempts.clone();
    let open_connections = server.open_connections.clone();
    let exec_commands = server.exec_commands.clone();
    let exec_replies = server.exec_replies.clone();
    let open_exec_channels = server.open_exec_channels.clone();
    tokio::spawn(async move {
        let _ = server.run_on_socket(config, &socket).await;
    });
    TestServer {
        addr,
        host_key,
        password_attempts,
        publickey_attempts,
        open_connections,
        exec_commands,
        exec_replies,
        open_exec_channels,
    }
}

impl russh::server::Server for Handler {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.open_connections.fetch_add(1, Ordering::SeqCst);
        Self {
            channels: Arc::default(),
            stream_channels: Arc::default(),
            forwards: Arc::default(),
            exec_channels: Arc::default(),
            _connection: Some(Arc::new(ConnectionGuard(self.open_connections.clone()))),
            ..self.clone()
        }
    }
}

impl russh::server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_password(&mut self, _user: &str, password: &str) -> Result<Auth, Self::Error> {
        self.password_attempts.fetch_add(1, Ordering::SeqCst);
        Ok(if password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, _user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        self.publickey_attempts.fetch_add(1, Ordering::SeqCst);
        Ok(match &self.allowed_key {
            Some(allowed) if allowed.key_data() == key.key_data() => Auth::Accept,
            _ => Auth::reject(),
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.sftp_root.is_some() {
            self.channels.lock().unwrap().insert(channel.id(), channel);
        }
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.stream_channels.lock().unwrap().insert(channel.id());
        reply.accept().await;
        let target = format!("{host_to_connect}:{port_to_connect}");
        tokio::spawn(async move {
            match TcpStream::connect(target).await {
                Ok(tcp) => pipe(channel, tcp).await,
                Err(_) => {
                    let _ = channel.close().await;
                }
            }
        });
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let Ok(listener) = TcpListener::bind(format!("{address}:{port}")).await else {
            return Ok(false);
        };
        if *port == 0 {
            *port = listener.local_addr().unwrap().port().into();
        }
        let (handle, stream_channels) = (session.handle(), self.stream_channels.clone());
        let (connected_address, connected_port) = (address.to_string(), *port);
        let task = tokio::spawn(async move {
            while let Ok((tcp, peer)) = listener.accept().await {
                let Ok(channel) = handle
                    .channel_open_forwarded_tcpip(
                        connected_address.clone(),
                        connected_port,
                        peer.ip().to_string(),
                        peer.port().into(),
                    )
                    .await
                else {
                    continue;
                };
                stream_channels.lock().unwrap().insert(channel.id());
                tokio::spawn(pipe(channel, tcp));
            }
        });
        self.forwards
            .lock()
            .unwrap()
            .insert((address.to_string(), *port), task.abort_handle());
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        address: &str,
        port: u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let task = self.forwards.lock().unwrap().remove(&(address.to_string(), port));
        Ok(task.map(|task| task.abort()).is_some())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let channel = self.channels.lock().unwrap().remove(&channel_id);
        match (name, channel, &self.sftp_root) {
            ("sftp", Some(channel), Some(root)) => {
                self.stream_channels.lock().unwrap().insert(channel_id);
                session.channel_success(channel_id)?;
                russh_sftp::server::run(channel.into_stream(), SftpFs::new(root.as_ref().clone())).await;
                Ok(())
            }
            _ => session.channel_failure(channel_id),
        }
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        cols: u32,
        rows: u32,
        _pw: u32,
        _ph: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // A shell channel is not read through its `Channel` object; keeping it
        // would block the session once its buffer fills up.
        self.channels.lock().unwrap().remove(&channel);
        *self.size.lock().unwrap() = (cols, rows);
        session.channel_success(channel)
    }

    async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, b"welcome\r\n".to_vec())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).into_owned();
        self.exec_commands.lock().unwrap().push(command.clone());
        if self.exec_channels.lock().unwrap().insert(channel) {
            self.open_exec_channels.fetch_add(1, Ordering::SeqCst);
        }
        // Exec channels are not echoed.
        self.stream_channels.lock().unwrap().insert(channel);
        self.channels.lock().unwrap().remove(&channel);
        session.channel_success(channel)?;
        let reply = self.exec_replies.lock().unwrap().get(&command).cloned();
        if let Some(stdout) = reply {
            session.data(channel, stdout)?;
            session.exit_status_request(channel, 0)?;
            session.eof(channel)?;
            return session.close(channel);
        }
        let program = command.split_whitespace().next().unwrap_or_default();
        let status = match program {
            "sleep-forever" => return Ok(()),
            "killed-by-signal" => {
                session.exit_signal_request(channel, russh::Sig::KILL, false, "", "en")?;
                session.eof(channel)?;
                return session.close(channel);
            }
            "mosh-server" => {
                session.data(channel, MOSH_CONNECT_REPLY.as_bytes().to_vec())?;
                0
            }
            "true" => 0,
            other => {
                let message = format!("sh: 1: {other}: not found\n");
                session.extended_data(channel, 1, message.into_bytes())?;
                127
            }
        };
        session.exit_status_request(channel, status)?;
        session.eof(channel)?;
        session.close(channel)
    }

    async fn channel_close(&mut self, channel: ChannelId, _session: &mut Session) -> Result<(), Self::Error> {
        if self.exec_channels.lock().unwrap().remove(&channel) {
            self.open_exec_channels.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        cols: u32,
        rows: u32,
        _pw: u32,
        _ph: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        *self.size.lock().unwrap() = (cols, rows);
        Ok(())
    }

    async fn data(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        if self.stream_channels.lock().unwrap().contains(&channel) {
            return Ok(());
        }
        match data {
            b"exit" => {
                session.exit_status_request(channel, 7)?;
                session.eof(channel)?;
                session.close(channel)
            }
            b"size?" => {
                let (cols, rows) = *self.size.lock().unwrap();
                session.data(channel, format!("size={cols}x{rows}\r\n").into_bytes())
            }
            other => session.data(channel, other.to_vec()),
        }
    }
}

/// Copies bytes both ways between a channel and a TCP stream until both sides close.
async fn pipe(channel: Channel<Msg>, mut tcp: TcpStream) {
    let mut stream = channel.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
}

enum OpenHandle {
    /// Directory entries not yet returned by `readdir`.
    Dir(Vec<File>),
    File(std::fs::File),
}

/// SFTP handler serving a local directory as `/`; paths escaping it are refused.
struct SftpFs {
    root: PathBuf,
    handles: HashMap<String, OpenHandle>,
    next_handle: u32,
}

impl SftpFs {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            handles: HashMap::new(),
            next_handle: 0,
        }
    }

    /// Normalizes a client path to an absolute virtual path (`/`-rooted).
    fn virtual_path(path: &str) -> Result<String, StatusCode> {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop().ok_or(StatusCode::PermissionDenied)?;
                }
                other => parts.push(other),
            }
        }
        Ok(format!("/{}", parts.join("/")))
    }

    fn real_path(&self, path: &str) -> Result<PathBuf, StatusCode> {
        let virt = Self::virtual_path(path)?;
        Ok(self.root.join(virt.trim_start_matches('/')))
    }

    fn add_handle(&mut self, handle: OpenHandle) -> String {
        self.next_handle += 1;
        let key = self.next_handle.to_string();
        self.handles.insert(key.clone(), handle);
        key
    }
}

fn io_status(err: std::io::Error) -> StatusCode {
    match err.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".into(),
        language_tag: "en-US".into(),
    }
}

impl russh_sftp::server::Handler for SftpFs {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy(Self::virtual_path(&path)?)],
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let dir = self.real_path(&path)?;
        let mut files = vec![
            File::new(".", FileAttributes::from(&dir.metadata().map_err(io_status)?)),
            File::new("..", FileAttributes::dummy()),
        ];
        for entry in std::fs::read_dir(&dir).map_err(io_status)? {
            let entry = entry.map_err(io_status)?;
            let meta = std::fs::symlink_metadata(entry.path()).map_err(io_status)?;
            files.push(File::new(
                entry.file_name().to_string_lossy(),
                FileAttributes::from(&meta),
            ));
        }
        let handle = self.add_handle(OpenHandle::Dir(files));
        Ok(Handle { id, handle })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        match self.handles.get_mut(&handle) {
            Some(OpenHandle::Dir(files)) if !files.is_empty() => Ok(Name {
                id,
                files: std::mem::take(files),
            }),
            Some(OpenHandle::Dir(_)) => Err(StatusCode::Eof),
            _ => Err(StatusCode::Failure),
        }
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = self.real_path(&filename)?;
        let file = std::fs::OpenOptions::from(pflags).open(path).map_err(io_status)?;
        let handle = self.add_handle(OpenHandle::File(file));
        Ok(Handle { id, handle })
    }

    async fn read(&mut self, id: u32, handle: String, offset: u64, len: u32) -> Result<Data, Self::Error> {
        let Some(OpenHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        let mut data = vec![0; len as usize];
        file.seek(SeekFrom::Start(offset)).map_err(io_status)?;
        let read = file.read(&mut data).map_err(io_status)?;
        if read == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(read);
        Ok(Data { id, data })
    }

    async fn write(&mut self, id: u32, handle: String, offset: u64, data: Vec<u8>) -> Result<Status, Self::Error> {
        let Some(OpenHandle::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset)).map_err(io_status)?;
        file.write_all(&data).map_err(io_status)?;
        Ok(ok(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok(id))
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = self.real_path(&path)?.metadata().map_err(io_status)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&meta),
        })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = std::fs::symlink_metadata(self.real_path(&path)?).map_err(io_status)?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&meta),
        })
    }

    async fn setstat(&mut self, id: u32, path: String, attrs: FileAttributes) -> Result<Status, Self::Error> {
        let path = self.real_path(&path)?;
        // Like a real server: a missing path is an error on every platform.
        std::fs::metadata(&path).map_err(io_status)?;
        #[cfg(unix)]
        if let Some(mode) = attrs.permissions {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode & 0o7777)).map_err(io_status)?;
        }
        #[cfg(not(unix))]
        let _ = (path, attrs);
        Ok(ok(id))
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let Some(OpenHandle::File(file)) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&file.metadata().map_err(io_status)?),
        })
    }

    async fn mkdir(&mut self, id: u32, path: String, _attrs: FileAttributes) -> Result<Status, Self::Error> {
        std::fs::create_dir(self.real_path(&path)?).map_err(io_status)?;
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        std::fs::remove_dir(self.real_path(&path)?).map_err(io_status)?;
        Ok(ok(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        std::fs::remove_file(self.real_path(&filename)?).map_err(io_status)?;
        Ok(ok(id))
    }

    async fn rename(&mut self, id: u32, oldpath: String, newpath: String) -> Result<Status, Self::Error> {
        std::fs::rename(self.real_path(&oldpath)?, self.real_path(&newpath)?).map_err(io_status)?;
        Ok(ok(id))
    }
}
