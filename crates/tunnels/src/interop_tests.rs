//! Interoperability test against a real OpenSSH server, ported from PlexusTerm v0.2.15
//! `src-tauri/src/ssh/interop_tests.rs` (`openssh_local_tunnel_reaches_a_service_behind_the_server`).
//!
//! Ignored by default; a script starts a disposable OpenSSH container and runs it with:
//! - `PLEXUSTERM_INTEROP_PORT`: sshd port on 127.0.0.1
//! - `PLEXUSTERM_INTEROP_USER`: login user
//! - `PLEXUSTERM_INTEROP_KEY`: private key accepted by that user

use std::sync::Arc;
use std::time::Duration;

use proto_ssh::{SshAuth, SshError, SshManager, SshTarget};
use tokio::io::AsyncReadExt;

use super::{TunnelDefinition, TunnelError, TunnelKind, TunnelManager};

// Multi-threaded: the tunnel tasks run while the test awaits the TCP client.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a disposable OpenSSH server (see the module docs)"]
async fn openssh_local_tunnel_reaches_a_service_behind_the_server() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let (port, user, key): (u16, _, _) = (
        var("PLEXUSTERM_INTEROP_PORT").parse().unwrap(),
        var("PLEXUSTERM_INTEROP_USER"),
        var("PLEXUSTERM_INTEROP_KEY"),
    );
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = dir.path().join("known_hosts");
    let ssh = SshManager::new(known_hosts.clone()).with_agent_socket(dir.path().join("no-agent.sock"));
    let tunnels = TunnelManager::new(Arc::new(ssh), |_| {});

    let listen_port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        probe.local_addr().unwrap().port()
    };
    let tunnel = TunnelDefinition {
        id: "interop".into(),
        name: "interop".into(),
        kind: TunnelKind::Local,
        session_id: "interop".into(),
        listen_host: "127.0.0.1".into(),
        listen_port,
        // The container's own sshd, reachable only from inside the server.
        dest_host: "localhost".into(),
        dest_port: 22,
    };
    let session = SshTarget {
        host: "127.0.0.1".into(),
        port,
        username: user,
        auth: SshAuth::Key {
            path: key,
            passphrase: None,
        },
        jumps: vec![],
    };
    // Record the host key through the regular trust flow first.
    let fingerprint = match tunnels.start(tunnel.clone(), session.clone(), None).await {
        Err(TunnelError::Ssh(SshError::UnknownHost { fingerprint, .. })) => fingerprint,
        other => panic!("expected UnknownHost on first contact, got {other:?}"),
    };
    tunnels
        .start(tunnel, session, Some(fingerprint))
        .await
        .expect("tunnel starts");

    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", listen_port))
        .await
        .unwrap();
    let mut banner = vec![0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(10), tcp.read(&mut banner))
        .await
        .expect("banner within 10s")
        .unwrap();
    let banner = String::from_utf8_lossy(&banner[..n]);
    assert!(banner.starts_with("SSH-2.0-OpenSSH"), "got {banner:?}");

    tunnels.stop("interop").await;
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", listen_port))
            .await
            .is_err()
    );
}
