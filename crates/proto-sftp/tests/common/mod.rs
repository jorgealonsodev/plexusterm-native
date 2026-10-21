//! Connects to proto-ssh's in-process SSH test server, which serves SFTP from a local folder.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers shared by several test crates, each using a part of them"
)]

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use proto_ssh::test_server::{self, PASSWORD, TestServer};
use proto_ssh::{ConnectRequest, SshAuth, SshManager};
use russh_sftp::client::SftpSession;
use term_engine::ByteRing;
use zeroize::Zeroizing;

/// A connected session: keep it alive for as long as the test uses the session.
pub struct Connected {
    pub mgr: SshManager,
    pub id: u32,
    _server: TestServer,
    _output: ByteRing,
    _dir: tempfile::TempDir,
}

/// Connects a manager to an SFTP-enabled test server rooted at `root`. The manager's SSH agent is
/// a missing socket, so a developer's `SSH_AUTH_SOCK` never changes a test.
pub async fn connect(root: &Path) -> Connected {
    let server = test_server::start_with_sftp(root).await;
    let dir = tempfile::tempdir().unwrap();
    let known_hosts = dir.path().join("known_hosts");
    let port = server.addr.port();
    russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", port, &server.host_key, &known_hosts).unwrap();
    let mgr = SshManager::new(known_hosts).with_agent_socket(dir.path().join("no-agent.sock"));
    let request = ConnectRequest {
        host: "127.0.0.1".into(),
        port,
        username: "tester".into(),
        auth: SshAuth::Password {
            password: Zeroizing::new(PASSWORD.into()),
        },
        cols: 80,
        rows: 24,
        trusted_fingerprint: None,
        jumps: vec![],
    };
    let output = ByteRing::new(ByteRing::SESSION_CAPACITY);
    let id = mgr.connect_with(request, None, output.clone()).await.unwrap().id;
    Connected {
        mgr,
        id,
        _server: server,
        _output: output,
        _dir: dir,
    }
}

/// The SFTP session of a new connection to a test server rooted at `root`.
pub async fn session(root: &Path) -> (Arc<SftpSession>, Connected) {
    let connected = connect(root).await;
    let sftp = tokio::time::timeout(Duration::from_secs(10), connected.mgr.sftp(connected.id))
        .await
        .expect("opening SFTP timed out")
        .unwrap();
    (sftp, connected)
}

/// Waits up to 10 s for `done`, checking every 20 ms.
pub async fn eventually(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    done()
}
