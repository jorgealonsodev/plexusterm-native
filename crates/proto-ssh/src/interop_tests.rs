//! Interoperability tests against a real OpenSSH server, ported from PlexusTerm v0.2.15
//! `src-tauri/src/ssh/interop_tests.rs`.
//!
//! Ignored by default (shell, SFTP and a jump host); a script starts a disposable OpenSSH
//! container and runs them with:
//! - `PLEXUSTERM_INTEROP_PORT`: sshd port on 127.0.0.1
//! - `PLEXUSTERM_INTEROP_USER`: login user
//! - `PLEXUSTERM_INTEROP_KEY`: private key accepted by that user
//!
//! v0.2.15's local tunnel test moves to the tunnels crate, which owns `TunnelManager`.

use std::time::{Duration, Instant};

use term_engine::ByteRing;

use super::{ConnectRequest, JumpHost, Shell, SshAuth, SshError, SshManager};

struct Target {
    port: u16,
    user: String,
    key: String,
}

fn target() -> Target {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    Target {
        port: var("PLEXUSTERM_INTEROP_PORT").parse().unwrap(),
        user: var("PLEXUSTERM_INTEROP_USER"),
        key: var("PLEXUSTERM_INTEROP_KEY"),
    }
}

fn request(t: &Target, trusted_fingerprint: Option<String>) -> ConnectRequest {
    ConnectRequest {
        host: "127.0.0.1".into(),
        port: t.port,
        username: t.user.clone(),
        auth: SshAuth::Key {
            path: t.key.clone(),
            passphrase: None,
        },
        cols: 80,
        rows: 24,
        trusted_fingerprint,
        jumps: vec![],
    }
}

fn ring() -> ByteRing {
    ByteRing::new(ByteRing::SESSION_CAPACITY)
}

/// Collects output until `needle` appears or 10 s passed. Raw bytes are kept and decoded
/// once, so a UTF-8 character split across two pops is not mangled.
fn wait_for(out: &ByteRing, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let needle = needle.as_bytes();
    let mut bytes = Vec::new();
    while Instant::now() < deadline && !bytes.windows(needle.len()).any(|window| window == needle) {
        match out.try_pop(64 * 1024) {
            Some(chunk) => bytes.extend_from_slice(&chunk),
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Trusts the server through the real unknown-host flow and returns a connected session.
async fn connect(t: &Target, mgr: &SshManager) -> (Shell, ByteRing) {
    let fingerprint = match mgr.connect(request(t, None), ring()).await {
        Err(SshError::UnknownHost { fingerprint, .. }) => fingerprint,
        other => panic!("expected UnknownHost on first contact, got {:?}", other.map(|_| ())),
    };
    let out = ring();
    let shell = mgr
        .connect(request(t, Some(fingerprint)), out.clone())
        .await
        .expect("connect after trusting the host key");
    (shell, out)
}

// Multi-threaded: `wait_for` blocks while the session tasks keep running.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a disposable OpenSSH server (PLEXUSTERM_INTEROP_*)"]
async fn openssh_shell_resize_and_known_hosts() {
    let t = target();
    let dir = tempfile::tempdir().unwrap();
    let mgr = super::tests::manager(dir.path().join("known_hosts"));
    let (shell, out) = connect(&t, &mgr).await;
    let id = shell.id;

    mgr.write(id, b"echo plexus-$((40+2))\n").await.unwrap();
    let text = wait_for(&out, "plexus-42");
    assert!(text.contains("plexus-42"), "shell output: {text:?}");

    mgr.resize(id, 132, 43).unwrap();
    mgr.write(id, b"stty size\n").await.unwrap();
    let text = wait_for(&out, "43 132");
    assert!(text.contains("43 132"), "shell output: {text:?}");
    mgr.close(id).unwrap();

    // The key is now recorded: a second connection needs no confirmation.
    let id = mgr.connect(request(&t, None), ring()).await.unwrap().id;
    mgr.close(id).unwrap();
}

// Multi-threaded: the SFTP calls run while the session tasks keep running.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a disposable OpenSSH server (PLEXUSTERM_INTEROP_*)"]
async fn openssh_sftp_round_trip() {
    let t = target();
    let dir = tempfile::tempdir().unwrap();
    let mgr = super::tests::manager(dir.path().join("known_hosts"));
    let (shell, _out) = connect(&t, &mgr).await;
    let session = mgr.sftp(shell.id).await.unwrap();

    // The SFTP crate's helpers (upload, download, remove) are not ported yet: the raw client.
    let home = session.canonicalize(".").await.unwrap();
    let work = format!("{}/plexusterm-interop", home.trim_end_matches('/'));
    let remote = format!("{work}/payload.bin");
    let _ = session.remove_file(&remote).await;
    let _ = session.remove_dir(&work).await;
    session.create_dir(&work).await.unwrap();

    let payload: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i * 31 % 251) as u8).collect();
    // `SftpSession::write` opens without CREATE, so a new file is created first.
    let mut file = session.create(&remote).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut file, &payload).await.unwrap();
    file.close().await.unwrap();

    let names: Vec<String> = session
        .read_dir(&work)
        .await
        .unwrap()
        .map(|entry| entry.file_name())
        .filter(|name| name != "." && name != "..")
        .collect();
    assert_eq!(names, vec!["payload.bin"]);

    let back = session.read(&remote).await.unwrap();
    assert!(back == payload, "downloaded bytes differ");

    session.remove_file(&remote).await.unwrap();
    session.remove_dir(&work).await.unwrap();
    mgr.close(shell.id).unwrap();
}

// Multi-threaded: `wait_for` blocks while the session tasks keep running.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a disposable OpenSSH server (PLEXUSTERM_INTEROP_*)"]
async fn openssh_through_a_jump_host() {
    // The container is its own jump host: from inside it, `localhost:22` is the same sshd,
    // reached through a real OpenSSH `direct-tcpip` channel.
    let t = target();
    let dir = tempfile::tempdir().unwrap();
    let mgr = super::tests::manager(dir.path().join("known_hosts"));
    let auth = SshAuth::Key {
        path: t.key.clone(),
        passphrase: None,
    };
    let mut req = request(&t, None);
    req.host = "localhost".into();
    req.port = 22;
    req.jumps = vec![JumpHost {
        name: "bastion".into(),
        host: "127.0.0.1".into(),
        port: t.port,
        username: t.user.clone(),
        auth,
        trusted_fingerprint: None,
        storage: None,
    }];

    // First the jump host, then the target behind it, are unknown.
    let jump_fingerprint = match mgr.connect(req.clone(), ring()).await {
        Err(SshError::Hop { hop: 1, error, .. }) => match *error {
            SshError::UnknownHost { fingerprint, .. } => fingerprint,
            other => panic!("expected UnknownHost on the jump host, got {other:?}"),
        },
        other => panic!("expected a jump host error, got {:?}", other.map(|_| ())),
    };
    req.jumps[0].trusted_fingerprint = Some(jump_fingerprint);
    let target_fingerprint = match mgr.connect(req.clone(), ring()).await {
        Err(SshError::UnknownHost { host, fingerprint, .. }) => {
            assert_eq!(host, "localhost");
            fingerprint
        }
        other => panic!("expected UnknownHost on the target, got {:?}", other.map(|_| ())),
    };
    req.trusted_fingerprint = Some(target_fingerprint);
    let out = ring();
    let id = mgr
        .connect(req, out.clone())
        .await
        .expect("connect through the jump host")
        .id;

    mgr.write(id, b"echo jump-$((40+2))\n").await.unwrap();
    let text = wait_for(&out, "jump-42");
    assert!(text.contains("jump-42"), "shell output: {text:?}");
    mgr.close(id).unwrap();
}
