//! Ported from PlexusTerm v0.2.15 `src-tauri/src/tunnels/tests.rs`: managers are built on an
//! `SshManager` (its known_hosts, saved passwords and a pinned, missing SSH agent), and statuses
//! are compared as values instead of their Tauri IPC JSON.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use core_secrets::{BlockingStore, MemoryStore, SecretStore, SharedSecrets};
use proto_ssh::test_server::{self, PASSWORD, TestServer};
use proto_ssh::{JumpHost, SshAuth};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use zeroize::Zeroizing;

use super::*;

/// An `SshManager` using `known_hosts` whose SSH agent is a missing socket next to it, so a
/// developer's `SSH_AUTH_SOCK` never changes a test.
fn ssh(known_hosts: PathBuf) -> SshManager {
    let no_agent = known_hosts.with_file_name("no-agent.sock");
    SshManager::new(known_hosts).with_agent_socket(no_agent)
}

/// A tunnel manager over `ssh(known_hosts)`.
fn tunnels(known_hosts: PathBuf, notify: impl Fn(StatusUpdate) + Send + Sync + 'static) -> TunnelManager {
    TunnelManager::new(Arc::new(ssh(known_hosts)), notify)
}

/// Accepts connections on 127.0.0.1 and echoes every byte back.
async fn echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut rd, mut wr) = tcp.split();
                let _ = tokio::io::copy(&mut rd, &mut wr).await;
            });
        }
    });
    port
}

/// A port that was free a moment ago.
async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// Keyring value of `password` saved for the test user on `server`.
fn secret_for(server: &TestServer, password: &str) -> String {
    serde_json::json!({
        "v": 1,
        "endpoint": format!("tester@127.0.0.1:{}", server.addr.port()),
        "password": password,
    })
    .to_string()
}

fn target(server: &TestServer) -> SshTarget {
    SshTarget {
        host: "127.0.0.1".into(),
        port: server.addr.port(),
        username: "tester".into(),
        auth: SshAuth::Password {
            password: Zeroizing::new(PASSWORD.into()),
        },
        jumps: vec![],
    }
}

fn tunnel(kind: TunnelKind, listen_port: u16, dest_port: u16) -> TunnelDefinition {
    TunnelDefinition {
        id: "t1".into(),
        name: "test".into(),
        kind,
        session_id: "s1".into(),
        listen_host: "127.0.0.1".into(),
        listen_port,
        dest_host: "127.0.0.1".into(),
        dest_port,
    }
}

/// Manager whose known_hosts trusts the test server; also returns its status updates.
fn manager(dir: &tempfile::TempDir, server: &TestServer) -> (TunnelManager, UnboundedReceiver<StatusUpdate>) {
    manager_with(dir, server, None)
}

/// Like `manager`, with `secrets` as the saved password store.
fn manager_with(
    dir: &tempfile::TempDir,
    server: &TestServer,
    secrets: Option<SharedSecrets>,
) -> (TunnelManager, UnboundedReceiver<StatusUpdate>) {
    let path = dir.path().join("known_hosts");
    russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", server.addr.port(), &server.host_key, &path).unwrap();
    let (tx, rx) = unbounded_channel();
    let mut ssh = ssh(path);
    if let Some(secrets) = secrets {
        ssh = ssh.with_secrets(secrets);
    }
    let mgr = TunnelManager::new(Arc::new(ssh), move |update| {
        let _ = tx.send(update);
    });
    (mgr, rx)
}

/// Sends `payload` and reads the same number of bytes back.
async fn round_trip(tcp: &mut TcpStream, payload: &[u8]) -> Vec<u8> {
    tcp.write_all(payload).await.unwrap();
    let mut back = vec![0; payload.len()];
    tokio::time::timeout(Duration::from_secs(5), tcp.read_exact(&mut back))
        .await
        .expect("echo timed out")
        .unwrap();
    back
}

#[tokio::test(flavor = "multi_thread")]
async fn local_forward_round_trips_bytes_and_reports_running() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, mut updates) = manager(&dir, &server);
    let (echo, listen) = (echo_server().await, free_port().await);

    mgr.start(tunnel(TunnelKind::Local, listen, echo), target(&server), None)
        .await
        .unwrap();
    assert_eq!(mgr.status("t1"), TunnelStatus::Running { warning: None });
    assert_eq!(updates.recv().await.unwrap().status, TunnelStatus::Starting);
    assert_eq!(
        updates.recv().await.unwrap().status,
        TunnelStatus::Running { warning: None }
    );

    // Two concurrent connections share the tunnel.
    let mut a = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    let mut b = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    let payload: Vec<u8> = (0..200_000).map(|i| (i % 251) as u8).collect();
    assert!(round_trip(&mut a, &payload).await == payload);
    assert_eq!(round_trip(&mut b, b"hello").await, b"hello");
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_closes_the_listener_and_open_connections() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let (echo, listen) = (echo_server().await, free_port().await);
    mgr.start(tunnel(TunnelKind::Local, listen, echo), target(&server), None)
        .await
        .unwrap();
    let mut open = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut open, b"x").await, b"x");

    mgr.stop("t1").await;
    assert_eq!(mgr.status("t1"), TunnelStatus::Stopped);
    assert!(TcpStream::connect(("127.0.0.1", listen)).await.is_err());
    let mut buf = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), open.read(&mut buf))
        .await
        .expect("open connection was not closed");
    assert!(matches!(read, Ok(0) | Err(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_forward_reaches_a_local_service_until_stopped() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let (echo, remote) = (echo_server().await, free_port().await);

    mgr.start(tunnel(TunnelKind::Remote, remote, echo), target(&server), None)
        .await
        .unwrap();
    // The test server listens on the requested address on behalf of the client.
    let mut tcp = TcpStream::connect(("127.0.0.1", remote)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"through the server").await, b"through the server");

    mgr.stop("t1").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(TcpStream::connect(("127.0.0.1", remote)).await.is_err());
}

/// Connects through a SOCKS5 proxy; returns the stream and the reply code.
async fn socks_connect(proxy: u16, request: &[u8]) -> (TcpStream, u8) {
    let mut tcp = TcpStream::connect(("127.0.0.1", proxy)).await.unwrap();
    tcp.write_all(&[5, 1, 0]).await.unwrap();
    let mut method = [0u8; 2];
    tcp.read_exact(&mut method).await.unwrap();
    assert_eq!(method, [5, 0]);
    tcp.write_all(request).await.unwrap();
    let mut reply = [0u8; 10];
    tcp.read_exact(&mut reply).await.unwrap();
    (tcp, reply[1])
}

#[tokio::test(flavor = "multi_thread")]
async fn dynamic_forward_connects_by_domain_and_ipv4() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let (echo, listen) = (echo_server().await, free_port().await);
    mgr.start(tunnel(TunnelKind::Dynamic, listen, 0), target(&server), None)
        .await
        .unwrap();
    let [hi, lo] = echo.to_be_bytes();

    let mut by_domain = vec![5, 1, 0, 3, 9];
    by_domain.extend_from_slice(b"localhost");
    by_domain.extend_from_slice(&[hi, lo]);
    let (mut tcp, code) = socks_connect(listen, &by_domain).await;
    assert_eq!(code, 0);
    assert_eq!(round_trip(&mut tcp, b"via domain").await, b"via domain");

    let (mut tcp, code) = socks_connect(listen, &[5, 1, 0, 1, 127, 0, 0, 1, hi, lo]).await;
    assert_eq!(code, 0);
    assert_eq!(round_trip(&mut tcp, b"via ipv4").await, b"via ipv4");
}

#[tokio::test(flavor = "multi_thread")]
async fn dynamic_forward_rejects_unsupported_commands_and_methods() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let listen = free_port().await;
    mgr.start(tunnel(TunnelKind::Dynamic, listen, 0), target(&server), None)
        .await
        .unwrap();

    // BIND (0x02) is not supported.
    let (_tcp, code) = socks_connect(listen, &[5, 2, 0, 1, 127, 0, 0, 1, 0, 80]).await;
    assert_eq!(code, 0x07);

    // Only username/password offered: no acceptable method.
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    tcp.write_all(&[5, 1, 2]).await.unwrap();
    let mut method = [0u8; 2];
    tcp.read_exact(&mut method).await.unwrap();
    assert_eq!(method, [5, 0xFF]);
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_while_starting_releases_the_listener_at_once() {
    // A server that accepts TCP but never speaks SSH keeps the tunnel in Starting.
    let blackhole = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ssh_port = blackhole.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((tcp, _)) = blackhole.accept().await {
            held.push(tcp);
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mgr = Arc::new(tunnels(dir.path().join("known_hosts"), |_| {}));
    let listen = free_port().await;
    let target = SshTarget {
        host: "127.0.0.1".into(),
        port: ssh_port,
        username: "tester".into(),
        auth: SshAuth::Agent,
        jumps: vec![],
    };

    let starting = {
        let (mgr, target) = (mgr.clone(), target.clone());
        tokio::spawn(async move { mgr.start(tunnel(TunnelKind::Local, listen, 9), target, None).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(matches!(mgr.status("t1"), TunnelStatus::Starting));

    mgr.stop("t1").await;
    let start_result = tokio::time::timeout(Duration::from_secs(2), starting)
        .await
        .expect("start must return promptly after stop")
        .unwrap();
    assert!(
        start_result.is_ok(),
        "a stopped start is not an error: {start_result:?}"
    );
    assert!(matches!(mgr.status("t1"), TunnelStatus::Stopped));
    assert!(
        TcpStream::connect(("127.0.0.1", listen)).await.is_err(),
        "the listener must be closed after stop"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn port_in_use_is_an_error_and_sets_the_error_status() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let busy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = busy.local_addr().unwrap().port();

    let err = mgr
        .start(tunnel(TunnelKind::Local, port, 9), target(&server), None)
        .await
        .unwrap_err();
    assert!(matches!(err, TunnelError::Listen { .. }), "got {err:?}");
    assert!(err.to_string().contains(&format!("127.0.0.1:{port}")), "message: {err}");
    match mgr.status("t1") {
        TunnelStatus::Error { message } => assert!(message.contains("in use"), "{message}"),
        other => panic!("expected an error status, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_host_reports_its_fingerprint_like_ssh_sessions() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = unbounded_channel();
    let mgr = tunnels(dir.path().join("known_hosts"), move |u| {
        let _ = tx.send(u);
    });
    let listen = free_port().await;

    let err = mgr
        .start(tunnel(TunnelKind::Local, listen, 9), target(&server), None)
        .await
        .unwrap_err();
    let expected = server.host_key.fingerprint(russh::keys::HashAlg::Sha256).to_string();
    match &err {
        TunnelError::Ssh(SshError::UnknownHost { fingerprint, .. }) => {
            assert_eq!(fingerprint, &expected)
        }
        other => panic!("expected UnknownHost, got {other:?}"),
    }
    // The listener is released after a failed start.
    assert!(TcpStream::connect(("127.0.0.1", listen)).await.is_err());

    // Trusting the fingerprint starts the tunnel.
    mgr.start(tunnel(TunnelKind::Local, listen, 9), target(&server), Some(expected))
        .await
        .unwrap();
    assert_eq!(mgr.status("t1"), TunnelStatus::Running { warning: None });
}

#[tokio::test(flavor = "multi_thread")]
async fn starting_a_running_tunnel_is_refused() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let (echo, listen) = (echo_server().await, free_port().await);
    let mgr = Arc::new(mgr);
    mgr.start(tunnel(TunnelKind::Local, listen, echo), target(&server), None)
        .await
        .unwrap();
    let err = mgr
        .start(tunnel(TunnelKind::Local, listen, echo), target(&server), None)
        .await
        .unwrap_err();
    assert!(matches!(err, TunnelError::AlreadyRunning { .. }), "got {err:?}");
    assert_eq!(mgr.status("t1"), TunnelStatus::Running { warning: None });
}

/// The definition keeps the field names of `tunnels.json`; v0.2.15's assertions on the status
/// and error IPC JSON are dropped (no IPC here).
#[test]
fn definitions_and_statuses_use_the_frontend_field_names() {
    let def: TunnelDefinition = serde_json::from_str(
        r#"{"id":"a","name":"n","type":"dynamic","sessionId":"s","listenHost":"127.0.0.1","listenPort":1080}"#,
    )
    .unwrap();
    assert_eq!(def.kind, TunnelKind::Dynamic);
    assert_eq!(def.dest_port, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stop_racing_the_running_notification_is_reported_last() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", server.addr.port(), &server.host_key, &path).unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (running_tx, running_rx) = std::sync::mpsc::channel();
    let mgr = {
        let seen = seen.clone();
        Arc::new(tunnels(path, move |update| {
            if matches!(update.status, TunnelStatus::Running { .. }) {
                // Let the test start a stop while this notification is in flight.
                let _ = running_tx.send(());
                std::thread::sleep(Duration::from_millis(300));
            }
            seen.lock().unwrap().push(update.status);
        }))
    };
    let (echo, listen) = (echo_server().await, free_port().await);

    let starting = {
        let (mgr, target) = (mgr.clone(), target(&server));
        tokio::spawn(async move { mgr.start(tunnel(TunnelKind::Local, listen, echo), target, None).await })
    };
    tokio::task::spawn_blocking(move || running_rx.recv_timeout(Duration::from_secs(10)))
        .await
        .unwrap()
        .expect("the tunnel never reported Running");
    tokio::time::timeout(Duration::from_secs(10), mgr.stop("t1"))
        .await
        .expect("stop hung");
    tokio::time::timeout(Duration::from_secs(10), starting)
        .await
        .expect("start hung")
        .unwrap()
        .unwrap();

    assert_eq!(mgr.status("t1"), TunnelStatus::Stopped);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(seen.last(), Some(&TunnelStatus::Stopped), "updates: {seen:?}");
}

/// TCP proxy to `upstream`; aborting the returned task cuts every proxied connection.
async fn cuttable_proxy(upstream: std::net::SocketAddr) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut links = tokio::task::JoinSet::new();
        while let Ok((mut client, _)) = listener.accept().await {
            links.spawn(async move {
                let mut server = TcpStream::connect(upstream).await.unwrap();
                let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
            });
        }
    });
    (port, task)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_ssh_connection_sets_the_error_status_and_releases_the_listener() {
    let server = test_server::start(1, None).await;
    let (proxy, cut) = cuttable_proxy(server.addr).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", proxy, &server.host_key, &path).unwrap();
    let (tx, mut updates) = unbounded_channel();
    let mgr = tunnels(path, move |update| {
        let _ = tx.send(update);
    });
    let (echo, listen) = (echo_server().await, free_port().await);
    let mut via_proxy = target(&server);
    via_proxy.port = proxy;
    mgr.start(tunnel(TunnelKind::Local, listen, echo), via_proxy, None)
        .await
        .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"up").await, b"up");

    cut.abort();
    let failed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let update = updates.recv().await.unwrap();
            if let TunnelStatus::Error { message } = update.status {
                return message;
            }
        }
    })
    .await
    .expect("no error status after the SSH connection dropped");
    assert!(failed.contains("closed"), "message: {failed}");
    assert!(matches!(mgr.status("t1"), TunnelStatus::Error { .. }));
    // The listener task closes just after the status is sent: give it a moment (Windows is slower).
    let released = tokio::time::timeout(Duration::from_secs(5), async {
        while TcpStream::connect(("127.0.0.1", listen)).await.is_ok() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        released.is_ok(),
        "the listener must be released after the connection dropped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stored_password_starts_the_tunnel() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store.set("s1", "label", &secret_for(&server, PASSWORD)).unwrap();
    let (mgr, _updates) = manager_with(&dir, &server, Some(store.clone()));
    let (echo, listen) = (echo_server().await, free_port().await);
    let mut session = target(&server);
    session.auth = SshAuth::StoredPassword {
        session_id: "s1".into(),
    };

    mgr.start(tunnel(TunnelKind::Local, listen, echo), session, None)
        .await
        .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"up").await, b"up");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tunnel_start_can_remember_the_typed_password() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let (mgr, _updates) = manager_with(&dir, &server, Some(store.clone()));
    let (echo, listen) = (echo_server().await, free_port().await);
    let remember = PasswordStorage::Remember {
        session_id: "s1".into(),
        label: "PlexusTerm: tester@127.0.0.1:22".into(),
    };

    mgr.start_with(
        tunnel(TunnelKind::Local, listen, echo),
        target(&server),
        None,
        Some(remember),
    )
    .await
    .unwrap();
    let saved: serde_json::Value = serde_json::from_str(&store.get("s1").unwrap().unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(&secret_for(&server, PASSWORD)).unwrap();
    assert_eq!(saved, expected);
    assert_eq!(mgr.status("t1"), TunnelStatus::Running { warning: None });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stored_password_for_another_endpoint_is_never_sent() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let other = serde_json::json!({"v": 1, "endpoint": "root@127.0.0.1:22", "password": PASSWORD});
    store.set("s1", "label", &other.to_string()).unwrap();
    let (mgr, _updates) = manager_with(&dir, &server, Some(store.clone()));
    let (echo, listen) = (echo_server().await, free_port().await);
    let mut session = target(&server);
    session.auth = SshAuth::StoredPassword {
        session_id: "s1".into(),
    };

    let err = mgr
        .start(tunnel(TunnelKind::Local, listen, echo), session, None)
        .await
        .unwrap_err();
    assert!(matches!(err, TunnelError::Ssh(SshError::SecretMissing)), "got {err:?}");
    assert_eq!(server.password_attempts(), 0, "the secret was sent");
}

fn remember_s1() -> Option<PasswordStorage> {
    Some(PasswordStorage::Remember {
        session_id: "s1".into(),
        label: "PlexusTerm: tester@127.0.0.1:22".into(),
    })
}

/// The status warning of a tunnel started with `remember_s1()` by `mgr`.
async fn warning_after_remember(server: &TestServer, mgr: &TunnelManager) -> String {
    let (echo, listen) = (echo_server().await, free_port().await);
    mgr.start_with(
        tunnel(TunnelKind::Local, listen, echo),
        target(server),
        None,
        remember_s1(),
    )
    .await
    .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"up").await, b"up");
    match mgr.status("t1") {
        TunnelStatus::Running { warning: Some(warning) } => warning,
        other => panic!("expected a running tunnel with a warning, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_password_the_keyring_cannot_save_is_a_warning_of_the_running_tunnel() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, mut updates) = manager(&dir, &server);
    // No keyring configured: saving fails.

    let warning = warning_after_remember(&server, &mgr).await;
    assert!(
        warning.starts_with("The password could not be saved in the system keyring:"),
        "{warning}"
    );
    assert!(!warning.contains(PASSWORD), "{warning}");
    assert_eq!(updates.recv().await.unwrap().status, TunnelStatus::Starting);
    let running = updates.recv().await.unwrap().status;
    assert_eq!(running, TunnelStatus::Running { warning: Some(warning) });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_keyring_that_does_not_answer_does_not_hold_the_tunnel_back() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (store, release) = BlockingStore::new();
    let (mgr, _updates) = manager_with(&dir, &server, Some(store));
    let mgr = mgr.with_storage_timeout(Duration::from_millis(100));

    let warning = tokio::time::timeout(Duration::from_secs(5), warning_after_remember(&server, &mgr))
        .await
        .expect("the tunnel waited for the keyring");
    assert!(warning.contains("did not respond in time"), "{warning}");
    drop(release);
}

/// `target(server)` reached through `bastion` (password login).
fn through(server: &TestServer, bastion: &TestServer, bastion_password: &str) -> SshTarget {
    SshTarget {
        jumps: vec![JumpHost {
            name: "bastion".into(),
            host: "127.0.0.1".into(),
            port: bastion.addr.port(),
            username: "tester".into(),
            auth: SshAuth::Password {
                password: Zeroizing::new(bastion_password.into()),
            },
            trusted_fingerprint: None,
            storage: None,
        }],
        ..target(server)
    }
}

/// Like `manager`, trusting `bastion` too.
fn chained_manager(dir: &tempfile::TempDir, server: &TestServer, bastion: &TestServer) -> TunnelManager {
    let (mgr, _updates) = manager(dir, server);
    russh::keys::known_hosts::learn_known_hosts_path(
        "127.0.0.1",
        bastion.addr.port(),
        &bastion.host_key,
        dir.path().join("known_hosts"),
    )
    .unwrap();
    mgr
}

async fn wait_connections(server: &TestServer, expected: usize) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while server.open_connections() != expected && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    server.open_connections()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_forward_through_a_jump_host_round_trips_and_stop_closes_every_hop() {
    let server = test_server::start(1, None).await;
    let bastion = test_server::start(2, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = chained_manager(&dir, &server, &bastion);
    let (echo, listen) = (echo_server().await, free_port().await);

    mgr.start(
        tunnel(TunnelKind::Local, listen, echo),
        through(&server, &bastion, PASSWORD),
        None,
    )
    .await
    .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"via jump").await, b"via jump");
    assert_eq!(bastion.open_connections(), 1);

    mgr.stop("t1").await;
    assert_eq!(wait_connections(&server, 0).await, 0, "target still open");
    assert_eq!(wait_connections(&bastion, 0).await, 0, "jump still open");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remote_forward_through_a_jump_host_reaches_a_local_service() {
    let server = test_server::start(1, None).await;
    let bastion = test_server::start(2, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = chained_manager(&dir, &server, &bastion);
    let (echo, remote) = (echo_server().await, free_port().await);

    mgr.start(
        tunnel(TunnelKind::Remote, remote, echo),
        through(&server, &bastion, PASSWORD),
        None,
    )
    .await
    .unwrap();
    let mut tcp = TcpStream::connect(("127.0.0.1", remote)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"back").await, b"back");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_jump_host_is_named_in_the_tunnel_error() {
    let server = test_server::start(1, None).await;
    let bastion = test_server::start(2, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = chained_manager(&dir, &server, &bastion);
    let listen = free_port().await;

    let err = mgr
        .start(
            tunnel(TunnelKind::Local, listen, 9),
            through(&server, &bastion, "nope"),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(
        mgr.status("t1"),
        TunnelStatus::Error {
            message: "via bastion (jump host 1): authentication failed".into()
        }
    );
    assert!(
        matches!(err, TunnelError::Ssh(SshError::Hop { hop: 1, .. })),
        "got {err:?}"
    );
    assert_eq!(server.password_attempts(), 0);
}

// Native additions (PRD TUN-1): stop all on exit, and no start for a tunnel changed while its
// host key prompt was open.

#[tokio::test(flavor = "multi_thread")]
async fn stop_all_stops_every_active_tunnel_and_closes_their_connections() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let echo = echo_server().await;
    let (first, second) = (free_port().await, free_port().await);
    mgr.start(tunnel(TunnelKind::Local, first, echo), target(&server), None)
        .await
        .unwrap();
    let mut other = tunnel(TunnelKind::Dynamic, second, 0);
    other.id = "t2".into();
    mgr.start(other, target(&server), None).await.unwrap();
    assert_eq!(wait_connections(&server, 2).await, 2);

    tokio::time::timeout(Duration::from_secs(10), mgr.stop_all())
        .await
        .expect("stop_all hung");

    assert_eq!(mgr.status("t1"), TunnelStatus::Stopped);
    assert_eq!(mgr.status("t2"), TunnelStatus::Stopped);
    assert!(TcpStream::connect(("127.0.0.1", first)).await.is_err());
    assert!(TcpStream::connect(("127.0.0.1", second)).await.is_err());
    assert_eq!(
        wait_connections(&server, 0).await,
        0,
        "a tunnel connection is still open"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_all_leaves_a_failed_tunnel_as_it_was() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _updates) = manager(&dir, &server);
    let busy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = busy.local_addr().unwrap().port();
    mgr.start(tunnel(TunnelKind::Local, port, 9), target(&server), None)
        .await
        .unwrap_err();

    mgr.stop_all().await;

    assert!(matches!(mgr.status("t1"), TunnelStatus::Error { .. }));
}

/// Where the test answers the open host key prompt.
type Answer = Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>;

/// A tunnel manager over an `SshManager` whose host key prompt waits for an answer sent through
/// the returned slot; the prompt reports each question on the returned receiver.
fn prompting(
    dir: &tempfile::TempDir,
    still_wanted: impl Fn(&TunnelDefinition) -> bool + Send + Sync + 'static,
) -> (TunnelManager, UnboundedReceiver<proto_ssh::TrustRequest>, Answer) {
    let (asked_tx, asked) = unbounded_channel();
    let answer: Answer = Arc::default();
    let pending = answer.clone();
    let prompt = move |request: proto_ssh::TrustRequest| {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *pending.lock().unwrap() = Some(tx);
        let _ = asked_tx.send(request);
        async move { rx.await.unwrap_or(false) }
    };
    let ssh = ssh(dir.path().join("known_hosts")).with_host_key_prompt(Arc::new(prompt));
    let mgr = TunnelManager::new(Arc::new(ssh), |_| {}).with_still_wanted(still_wanted);
    (mgr, asked, answer)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tunnel_deleted_while_its_host_key_prompt_is_open_is_not_started() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let deleted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (mgr, mut asked, answer) = {
        let deleted = deleted.clone();
        prompting(&dir, move |_| !deleted.load(std::sync::atomic::Ordering::SeqCst))
    };
    let mgr = Arc::new(mgr);
    let listen = free_port().await;
    let starting = {
        let (mgr, target) = (mgr.clone(), target(&server));
        tokio::spawn(async move { mgr.start(tunnel(TunnelKind::Local, listen, 9), target, None).await })
    };
    tokio::time::timeout(Duration::from_secs(5), asked.recv())
        .await
        .expect("the host key prompt never opened")
        .unwrap();

    // The user deletes the tunnel, then answers the still open prompt with "trust".
    deleted.store(true, std::sync::atomic::Ordering::SeqCst);
    answer.lock().unwrap().take().unwrap().send(true).unwrap();

    let err = tokio::time::timeout(Duration::from_secs(5), starting)
        .await
        .expect("start hung")
        .unwrap()
        .unwrap_err();
    assert!(
        matches!(err, TunnelError::Ssh(SshError::UnknownHost { .. })),
        "got {err:?}"
    );
    assert!(matches!(mgr.status("t1"), TunnelStatus::Error { .. }));
    assert!(
        TcpStream::connect(("127.0.0.1", listen)).await.is_err(),
        "listener still open"
    );
    assert!(
        !std::fs::read_to_string(dir.path().join("known_hosts"))
            .unwrap_or_default()
            .contains("127.0.0.1"),
        "the key was recorded for a tunnel nobody wants"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tunnel_still_saved_as_it_was_starts_after_its_host_key_is_trusted() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let saved = tunnel(TunnelKind::Local, 0, 0);
    let (mgr, mut asked, answer) = prompting(&dir, move |tunnel| tunnel.id == saved.id);
    let mgr = Arc::new(mgr);
    let (echo, listen) = (echo_server().await, free_port().await);
    let starting = {
        let (mgr, target) = (mgr.clone(), target(&server));
        tokio::spawn(async move { mgr.start(tunnel(TunnelKind::Local, listen, echo), target, None).await })
    };
    tokio::time::timeout(Duration::from_secs(5), asked.recv())
        .await
        .expect("the host key prompt never opened")
        .unwrap();
    answer.lock().unwrap().take().unwrap().send(true).unwrap();

    tokio::time::timeout(Duration::from_secs(5), starting)
        .await
        .expect("start hung")
        .unwrap()
        .unwrap();
    assert_eq!(mgr.status("t1"), TunnelStatus::Running { warning: None });
    let mut tcp = TcpStream::connect(("127.0.0.1", listen)).await.unwrap();
    assert_eq!(round_trip(&mut tcp, b"trusted").await, b"trusted");
}

#[test]
fn same_forward_ignores_the_name_only() {
    let a = tunnel(TunnelKind::Local, 8080, 80);
    let mut renamed = a.clone();
    renamed.name = "renamed".into();
    assert!(same_forward(&a, &renamed));
    let mut moved = a.clone();
    moved.dest_port = 81;
    assert!(!same_forward(&a, &moved));
    let mut other_session = a.clone();
    other_session.session_id = "s2".into();
    assert!(!same_forward(&a, &other_session));
}
