//! Ported from PlexusTerm v0.2.15 `src-tauri/src/ssh/mod.rs` (tests module): the terminal
//! sink is now the session's byte ring and its exit receiver.

use super::test_server::{self, PASSWORD, key_from_seed};
use super::*;
use core_secrets::{BlockingStore, MemoryStore, SecretStore};

/// A ring as big as a terminal session's.
fn ring() -> ByteRing {
    ByteRing::new(ByteRing::SESSION_CAPACITY)
}

/// Collects output until `needle` appears, the ring is closed and drained, or 5 s passed.
async fn wait_for(out: &ByteRing, needle: &str) -> String {
    let mut text = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match out.try_pop(64 * 1024) {
            Some(bytes) => {
                text.push_str(&String::from_utf8_lossy(&bytes));
                if !needle.is_empty() && text.contains(needle) {
                    break;
                }
            }
            None if out.is_closed() => break,
            None => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    text
}

/// The exit status of a session, or `None` when it did not end within 5 s.
async fn wait_exit(exit: oneshot::Receiver<Option<u32>>) -> Option<Option<u32>> {
    tokio::time::timeout(Duration::from_secs(5), exit).await.ok()?.ok()
}

fn request(port: u16, auth: SshAuth) -> ConnectRequest {
    ConnectRequest {
        host: "127.0.0.1".into(),
        port,
        username: "tester".into(),
        auth,
        cols: 80,
        rows: 24,
        trusted_fingerprint: None,
        jumps: vec![],
    }
}

fn password(pw: &str) -> SshAuth {
    SshAuth::Password {
        password: Zeroizing::new(pw.into()),
    }
}

fn fingerprint(key: &russh::keys::PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// A manager using `known_hosts` whose SSH agent is a missing socket next to it, so a
/// developer's or CI's `SSH_AUTH_SOCK` never changes a test; agent tests set their own.
pub(crate) fn manager(known_hosts: PathBuf) -> SshManager {
    let no_agent = known_hosts.with_file_name("no-agent.sock");
    SshManager::new(known_hosts).with_agent_socket(no_agent)
}

/// Manager whose known_hosts already trusts `key` for 127.0.0.1:`port`.
fn trusting(dir: &tempfile::TempDir, port: u16, key: &russh::keys::PublicKey) -> SshManager {
    let path = dir.path().join("known_hosts");
    russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", port, key, &path).unwrap();
    manager(path)
}

#[tokio::test]
async fn password_login_opens_an_interactive_shell() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    assert!(wait_for(&out, "welcome").await.contains("welcome"));

    mgr.write(id, b"ping").await.unwrap();
    let out = wait_for(&out, "ping").await;
    assert!(out.contains("ping"), "output was {out:?}");
}

#[tokio::test]
async fn wrong_password_is_rejected() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), password("nope")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::AuthFailed), "got {err:?}");
}

#[tokio::test]
async fn private_key_login_works() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("id_ed25519");
    let pem = client_key.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap();
    std::fs::write(&key_path, pem.as_bytes()).unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let auth = SshAuth::Key {
        path: key_path.to_string_lossy().into(),
        passphrase: None,
    };
    mgr.connect(request(server.addr.port(), auth), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn unknown_host_is_rejected_with_its_fingerprint_and_not_recorded() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    let mgr = manager(path.clone());
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap_err();
    match err {
        SshError::UnknownHost {
            fingerprint: fp,
            other_known_keys,
            ..
        } => {
            assert_eq!(fp, fingerprint(&server.host_key));
            assert!(other_known_keys.is_empty());
        }
        other => panic!("expected UnknownHost, got {other:?}"),
    }
    assert!(!path.exists(), "known_hosts must not be written without consent");
}

const ECDSA_HOST_KEY: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBLcc4aYgvq7F2Wz+WMIijfkYv83TLq6CCChf7Y/3RbrZLPu+it5GkrceW7eoKv62EINLq6qDsX34FW/ldLgLE1M=";

#[tokio::test]
async fn unknown_key_type_reports_the_key_types_already_known_for_the_host() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    let port = server.addr.port();
    std::fs::write(&path, format!("[127.0.0.1]:{port} {ECDSA_HOST_KEY}\n")).unwrap();
    let mgr = manager(path);
    let out = ring();

    let err = mgr
        .connect(request(port, password(PASSWORD)), out.clone())
        .await
        .unwrap_err();
    match err {
        SshError::UnknownHost { other_known_keys, .. } => {
            assert_eq!(other_known_keys, vec!["ecdsa-sha2-nistp256".to_string()])
        }
        other => panic!("expected UnknownHost, got {other:?}"),
    }
}

#[test]
fn debug_output_never_contains_secrets() {
    let pw = SshAuth::Password {
        password: Zeroizing::new("hunter2".into()),
    };
    assert!(!format!("{pw:?}").contains("hunter2"));
    let key = SshAuth::Key {
        path: "/k".into(),
        passphrase: Some(Zeroizing::new("s3cr3t".into())),
    };
    let dbg = format!("{key:?}");
    assert!(!dbg.contains("s3cr3t") && dbg.contains("/k"), "{dbg}");
    assert!(!format!("{:?}", request(22, pw)).contains("hunter2"));
}

#[tokio::test]
async fn large_paste_is_fully_echoed_without_stalling() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;

    let size = 4 * 1024 * 1024;
    mgr.write(id, &vec![b'a'; size]).await.unwrap();
    let mut echoed = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while echoed < size {
        match out.try_pop(64 * 1024) {
            Some(bytes) => echoed += bytes.iter().filter(|b| **b == b'a').count(),
            None if out.is_closed() => panic!("session ended after {echoed} bytes"),
            None if tokio::time::Instant::now() > deadline => {
                panic!("stalled after echoing {echoed} of {size} bytes")
            }
            None => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
}

/// Writes `key` unencrypted to `path` (mode 0600 on Unix, as ssh-add requires).
fn write_key(path: &Path, key: &russh::keys::PrivateKey) {
    let pem = key.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap();
    std::fs::write(path, pem.as_bytes()).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
}

/// A running `ssh-agent` listening on `socket`, killed when dropped (also when a test fails
/// or setting it up fails half-way).
#[cfg(unix)]
struct AgentProcess {
    child: std::process::Child,
    socket: PathBuf,
}

#[cfg(unix)]
impl Drop for AgentProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts an `ssh-agent` on `socket` and adds `key_file` to it; `Ok(None)` when ssh-agent is
/// not installed.
#[cfg(unix)]
async fn start_agent(socket: &Path, key_file: &Path) -> Result<Option<AgentProcess>, String> {
    if std::process::Command::new("ssh-agent").arg("-h").output().is_err() {
        return Ok(None);
    }
    let child = std::process::Command::new("ssh-agent")
        .args(["-D", "-a"])
        .arg(socket)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|err| format!("cannot start ssh-agent: {err}"))?;
    let mut agent = AgentProcess {
        child,
        socket: socket.to_path_buf(),
    };
    let mut waited = 0;
    while !socket.exists() {
        let exited = agent.child.try_wait().ok().flatten().is_some();
        if exited || waited == 50 {
            return Err(format!("ssh-agent did not create its socket {}", socket.display()));
        }
        waited += 1;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let added = std::process::Command::new("ssh-add")
        .arg(key_file)
        .env("SSH_AUTH_SOCK", socket)
        .output()
        .map_err(|err| format!("cannot run ssh-add: {err}"))?;
    if !added.status.success() {
        return Err(format!("ssh-add failed: {added:?}"));
    }
    Ok(Some(agent))
}

/// An `ssh-agent` holding `key`, listening in `dir`; `None` when ssh-agent is not installed.
#[cfg(unix)]
async fn spawn_agent(dir: &tempfile::TempDir, key: &russh::keys::PrivateKey) -> Option<AgentProcess> {
    // Outside `dir`, so it is never taken for a default identity file.
    let keys = tempfile::tempdir().unwrap();
    let key_path = keys.path().join("agent_key");
    write_key(&key_path, key);
    let agent = start_agent(&dir.path().join("agent.sock"), &key_path).await.unwrap();
    if agent.is_none() {
        eprintln!("skipping: ssh-agent not installed");
    }
    agent
}

/// Processes whose command line mentions `socket` (Linux `/proc`).
#[cfg(target_os = "linux")]
fn processes_using(socket: &Path) -> usize {
    let needle = socket.as_os_str().as_encoded_bytes();
    std::fs::read_dir("/proc")
        .unwrap()
        .filter_map(|entry| std::fs::read(entry.ok()?.path().join("cmdline")).ok())
        .filter(|cmdline| cmdline.windows(needle.len()).any(|window| window == needle))
        .count()
}

#[tokio::test]
#[cfg(target_os = "linux")]
async fn a_failed_ssh_add_leaves_no_ssh_agent_running() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("agent.sock");
    let result = start_agent(&socket, &dir.path().join("missing_key")).await;
    match result {
        Ok(None) => return,
        Ok(Some(_)) => panic!("ssh-add of a missing key succeeded"),
        Err(err) => assert!(err.contains("ssh-add failed"), "{err}"),
    }
    assert_eq!(processes_using(&socket), 0, "ssh-agent was left running");
}

#[tokio::test]
#[cfg(target_os = "linux")]
async fn an_agent_socket_that_never_appears_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    // ssh-agent cannot create a socket in a missing folder.
    let socket = dir.path().join("missing").join("agent.sock");
    let result = start_agent(&socket, &dir.path().join("missing_key")).await;
    match result {
        Ok(None) => return,
        Ok(Some(_)) => panic!("an agent without a socket was returned"),
        Err(err) => assert!(err.contains("did not create its socket"), "{err}"),
    }
    assert_eq!(processes_using(&socket), 0, "ssh-agent was left running");
}

/// Run by [`tests_ignore_the_ssh_agent_of_the_environment`] with `SSH_AUTH_SOCK` pointing at
/// an agent holding the key this server accepts; on its own it checks the plain case.
#[tokio::test]
async fn a_password_login_in_tests_offers_no_agent_key() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    mgr.connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.publickey_attempts(), 0, "an agent key was offered");
    assert_eq!(server.password_attempts(), 1);
}

/// A developer's or CI's `ssh-agent` must not change what the tests see.
#[tokio::test]
#[cfg(unix)]
async fn tests_ignore_the_ssh_agent_of_the_environment() {
    let client_key = key_from_seed(9);
    let dir = tempfile::tempdir().unwrap();
    let Some(agent) = spawn_agent(&dir, &client_key).await else {
        return;
    };
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tests::a_password_login_in_tests_offers_no_agent_key"])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    drop(agent);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success() && stdout.contains("1 passed"), "{output:?}");
}

#[tokio::test]
#[cfg(unix)]
async fn agent_login_uses_the_agent_identities() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    let Some(agent) = spawn_agent(&dir, &client_key).await else {
        return;
    };

    let mgr = trusting(&dir, server.addr.port(), &server.host_key).with_agent_socket(agent.socket.clone());
    let out = ring();
    let result = mgr
        .connect(request(server.addr.port(), SshAuth::Agent), out.clone())
        .await;
    drop(agent);
    result.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn trusting_the_fingerprint_connects_and_records_the_host() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(dir.path().join("known_hosts"));
    let port = server.addr.port();

    let mut req = request(port, password(PASSWORD));
    req.trusted_fingerprint = Some(fingerprint(&server.host_key));
    let out = ring();
    mgr.connect(req, out.clone()).await.unwrap();

    // Second connection needs no confirmation.
    let out = ring();
    mgr.connect(request(port, password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn trusting_a_different_fingerprint_is_still_rejected() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(dir.path().join("known_hosts"));

    let mut req = request(server.addr.port(), password(PASSWORD));
    req.trusted_fingerprint = Some("SHA256:not-the-real-one".into());
    let out = ring();
    let err = mgr.connect(req, out.clone()).await.unwrap_err();
    assert!(matches!(err, SshError::UnknownHost { .. }), "got {err:?}");
}

#[tokio::test]
async fn changed_host_key_is_rejected_even_when_trusted() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let old_key = key_from_seed(2).public_key().clone();
    let mgr = trusting(&dir, server.addr.port(), &old_key);

    let mut req = request(server.addr.port(), password(PASSWORD));
    req.trusted_fingerprint = Some(fingerprint(&server.host_key));
    let out = ring();
    let err = mgr.connect(req, out.clone()).await.unwrap_err();
    assert!(matches!(err, SshError::HostKeyChanged { .. }), "got {err:?}");
}

#[tokio::test]
async fn requests_the_initial_size_and_forwards_resizes() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    wait_for(&out, "welcome").await;
    mgr.write(id, b"size?").await.unwrap();
    assert!(wait_for(&out, "size=80x24").await.contains("size=80x24"));

    mgr.resize(id, 120, 40).unwrap();
    mgr.write(id, b"size?").await.unwrap();
    let out = wait_for(&out, "size=120x40").await;
    assert!(out.contains("size=120x40"), "output was {out:?}");
}

#[tokio::test]
async fn remote_exit_reports_the_exit_status() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let shell = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    let id = shell.id;
    wait_for(&out, "welcome").await;
    mgr.write(id, b"exit").await.unwrap();
    let exit = wait_exit(shell.exit).await;
    assert_eq!(exit, Some(Some(7)));
}

#[tokio::test]
async fn close_ends_the_session_and_forgets_it() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let shell = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    let id = shell.id;
    wait_for(&out, "welcome").await;
    mgr.close(id).unwrap();
    let exit = wait_exit(shell.exit).await;
    assert!(exit.is_some(), "session did not end after close");
    assert!(matches!(
        mgr.write(id, b"x").await,
        Err(SshError::UnknownSession { .. })
    ));
    assert!(matches!(mgr.resize(99, 1, 1), Err(SshError::UnknownSession { id: 99 })));
}

/// Manager trusting the test server whose keyring is `store`.
fn with_store(dir: &tempfile::TempDir, server: &test_server::TestServer, store: &Arc<MemoryStore>) -> SshManager {
    trusting(dir, server.addr.port(), &server.host_key).with_secrets(store.clone())
}

fn stored(session_id: &str) -> SshAuth {
    SshAuth::StoredPassword {
        session_id: session_id.into(),
    }
}

/// Keyring value binding `password` to `endpoint` (`user@host:port`), as `remember` writes it.
fn bound_secret(endpoint: &str, password: &str) -> String {
    serde_json::json!({"v": 1, "endpoint": endpoint, "password": password}).to_string()
}

/// Keyring value for the test user on the test server at `port`.
fn secret_for(port: u16, password: &str) -> String {
    bound_secret(&format!("tester@127.0.0.1:{port}"), password)
}

fn remember(session_id: &str) -> Option<PasswordStorage> {
    Some(PasswordStorage::Remember {
        session_id: session_id.into(),
        label: "PlexusTerm: tester@127.0.0.1:22".into(),
    })
}

#[tokio::test]
async fn stored_password_login_reads_the_secret_from_the_store() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store
        .set("s1", "label", &secret_for(server.addr.port(), PASSWORD))
        .unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    mgr.connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.password_attempts(), 1);
}

/// Stores `value(server port)` for "s1", connects with it and expects it to be
/// treated as missing without any password reaching the server.
async fn assert_secret_not_sent(value: impl Fn(u16) -> String) {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store.set("s1", "label", &value(server.addr.port())).unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::SecretMissing), "got {err:?}");
    assert_eq!(server.password_attempts(), 0, "the secret was sent");
}

#[tokio::test]
async fn a_secret_stored_for_another_port_is_never_sent() {
    assert_secret_not_sent(|port| bound_secret(&format!("tester@127.0.0.1:{}", port + 1), PASSWORD)).await;
}

#[tokio::test]
async fn a_secret_stored_for_another_user_is_never_sent() {
    assert_secret_not_sent(|port| bound_secret(&format!("root@127.0.0.1:{port}"), PASSWORD)).await;
}

#[tokio::test]
async fn a_secret_stored_for_another_host_is_never_sent() {
    assert_secret_not_sent(|port| bound_secret(&format!("tester@10.0.0.1:{port}"), PASSWORD)).await;
}

#[tokio::test]
async fn a_legacy_plain_text_secret_is_never_sent() {
    assert_secret_not_sent(|_| PASSWORD.to_string()).await;
}

#[tokio::test]
async fn an_unsupported_secret_format_is_never_sent() {
    assert_secret_not_sent(|port| {
        serde_json::json!({"v": 2, "endpoint": format!("tester@127.0.0.1:{port}"), "password": PASSWORD}).to_string()
    })
    .await;
}

#[test]
fn endpoints_are_normalized() {
    assert_eq!(endpoint(" tester ", " 127.0.0.1 ", 2222), "tester@127.0.0.1:2222");
}

#[tokio::test]
async fn stored_password_without_a_secret_is_secret_missing() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::SecretMissing), "got {err:?}");
}

#[tokio::test]
async fn a_wrong_stored_password_is_auth_failed_and_the_secret_is_kept() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let old = secret_for(server.addr.port(), "old-password");
    store.set("s1", "label", &old).unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::AuthFailed), "got {err:?}");
    assert_eq!(store.get("s1").unwrap(), Some(old));
}

#[tokio::test]
async fn remember_stores_the_typed_password_after_a_successful_login() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store.set("s1", "label", "old-password").unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    mgr.connect_with(
        request(server.addr.port(), password(PASSWORD)),
        remember("s1"),
        out.clone(),
    )
    .await
    .unwrap();
    let saved: serde_json::Value = serde_json::from_str(&store.get("s1").unwrap().unwrap()).unwrap();
    assert_eq!(
        saved,
        serde_json::json!({
            "v": 1,
            "endpoint": format!("tester@127.0.0.1:{}", server.addr.port()),
            "password": PASSWORD,
        })
    );
    assert_eq!(store.label("s1").as_deref(), Some("PlexusTerm: tester@127.0.0.1:22"));
}

#[tokio::test]
async fn remember_stores_nothing_when_the_login_fails() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    let err = mgr
        .connect_with(
            request(server.addr.port(), password("nope")),
            remember("s1"),
            out.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::AuthFailed), "got {err:?}");
    assert_eq!(store.get("s1").unwrap(), None);
}

#[tokio::test]
async fn remember_only_stores_typed_passwords() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store
        .set("other", "label", &secret_for(server.addr.port(), PASSWORD))
        .unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    mgr.connect_with(
        request(server.addr.port(), stored("other")),
        remember("s1"),
        out.clone(),
    )
    .await
    .unwrap();
    assert_eq!(store.get("s1").unwrap(), None);
}

/// Output of a shell opened with a typed password and `storage`, up to the welcome banner.
async fn output_after_login(mgr: &SshManager, port: u16, storage: Option<PasswordStorage>) -> String {
    let out = ring();
    mgr.connect_with(request(port, password(PASSWORD)), storage, out.clone())
        .await
        .unwrap();
    wait_for(&out, "welcome").await
}

#[tokio::test]
async fn a_password_the_keyring_cannot_save_is_reported_in_the_terminal() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    // No keyring configured: saving fails.
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);

    let out = output_after_login(&mgr, server.addr.port(), remember("s1")).await;
    let warning = "\r\n\x1b[33mThe password could not be saved in the system keyring: \
                   the system keyring is not available\x1b[0m\r\n";
    assert!(out.starts_with(warning), "output was {out:?}");
    assert!(out.contains("welcome"), "output was {out:?}");
    assert!(!out.contains(PASSWORD), "output was {out:?}");
}

#[tokio::test]
async fn a_successful_save_writes_nothing_to_the_terminal() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let mgr = with_store(&dir, &server, &store);

    let out = output_after_login(&mgr, server.addr.port(), remember("s1")).await;
    assert!(!out.contains("keyring"), "output was {out:?}");
}

#[tokio::test]
async fn a_keyring_that_does_not_answer_does_not_hold_the_login_back() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (store, release) = BlockingStore::new();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key)
        .with_secrets(store)
        .with_storage_timeout(Duration::from_millis(100));

    for storage in [
        remember("s1"),
        Some(PasswordStorage::Forget {
            session_id: "s1".into(),
        }),
    ] {
        let out = tokio::time::timeout(
            Duration::from_secs(5),
            output_after_login(&mgr, server.addr.port(), storage),
        )
        .await
        .expect("the login waited for the keyring");
        assert!(out.contains("did not respond in time"), "output was {out:?}");
        assert!(out.contains("welcome"), "output was {out:?}");
    }
    drop(release);
}

#[tokio::test]
async fn forget_deletes_the_secret_only_after_a_successful_login() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    store.set("s1", "label", "old-password").unwrap();
    let mgr = with_store(&dir, &server, &store);
    let forget = || {
        Some(PasswordStorage::Forget {
            session_id: "s1".into(),
        })
    };

    let out = ring();
    mgr.connect_with(request(server.addr.port(), password("nope")), forget(), out.clone())
        .await
        .unwrap_err();
    assert!(store.get("s1").unwrap().is_some(), "deleted on failure");

    let out = ring();
    mgr.connect_with(request(server.addr.port(), password(PASSWORD)), forget(), out.clone())
        .await
        .unwrap();
    assert_eq!(store.get("s1").unwrap(), None);
}

#[test]
fn debug_of_stored_password_requests_never_contains_the_password() {
    let dbg = format!("{:?}", stored("s1"));
    assert!(dbg.contains("s1"), "{dbg}");
    let req = request(22, password("hunter2"));
    let dbg = format!("{:?} {:?}", req, remember("s1"));
    assert!(!dbg.contains("hunter2"), "{dbg}");
}

#[tokio::test]
async fn unreachable_host_is_a_connection_error() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(dir.path().join("known_hosts"));
    // Bind and drop to get a port with nothing listening.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let out = ring();
    let err = mgr
        .connect(request(port, password(PASSWORD)), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::Connection { .. }), "got {err:?}");
}

// --- Jump hosts (S14) ---

/// known_hosts trusting every server in `servers`.
fn trust_all(dir: &tempfile::TempDir, servers: &[&test_server::TestServer]) -> PathBuf {
    let path = dir.path().join("known_hosts");
    for server in servers {
        russh::keys::known_hosts::learn_known_hosts_path("127.0.0.1", server.addr.port(), &server.host_key, &path)
            .unwrap();
    }
    path
}

fn jump(name: &str, server: &test_server::TestServer, auth: SshAuth) -> JumpHost {
    JumpHost {
        name: name.into(),
        host: "127.0.0.1".into(),
        port: server.addr.port(),
        username: "tester".into(),
        auth,
        trusted_fingerprint: None,
        storage: None,
    }
}

/// Polls until `server` has `expected` open connections (5 s at most).
async fn wait_connections(server: &test_server::TestServer, expected: usize) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while server.open_connections() != expected && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    server.open_connections()
}

#[tokio::test]
async fn one_jump_host_reaches_the_final_shell() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];
    let out = ring();

    let id = mgr.connect(req, out.clone()).await.unwrap().id;
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    mgr.write(id, b"ping").await.unwrap();
    assert!(wait_for(&out, "ping").await.contains("ping"));
    assert_eq!(bastion.password_attempts(), 1);
    assert_eq!(target.password_attempts(), 1);
}

#[tokio::test]
async fn two_jump_hosts_reach_the_final_shell() {
    let first = test_server::start(2, None).await;
    let second = test_server::start(3, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&first, &second, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![
        jump("first", &first, password(PASSWORD)),
        jump("second", &second, password(PASSWORD)),
    ];
    let out = ring();

    let id = mgr.connect(req, out.clone()).await.unwrap().id;
    wait_for(&out, "welcome").await;
    mgr.write(id, b"through two hops").await.unwrap();
    let out = wait_for(&out, "through two hops").await;
    assert!(out.contains("through two hops"), "output was {out:?}");
    assert_eq!(second.password_attempts(), 1);
}

#[tokio::test]
async fn an_unknown_jump_host_names_the_hop_and_trusting_it_connects() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];

    let out = ring();
    let err = mgr.connect(req.clone(), out.clone()).await.unwrap_err();
    let SshError::Hop { hop: 1, name, error } = &err else {
        panic!("expected a jump host error, got {err:?}");
    };
    assert_eq!(name, "bastion");
    let SshError::UnknownHost {
        host, fingerprint: fp, ..
    } = &**error
    else {
        panic!("expected UnknownHost, got {error:?}");
    };
    assert_eq!(*host, format!("[127.0.0.1]:{}", bastion.addr.port()));
    assert_eq!(*fp, fingerprint(&bastion.host_key));
    assert_eq!(target.password_attempts(), 0);

    req.jumps[0].trusted_fingerprint = Some(fingerprint(&bastion.host_key));
    let out = ring();
    mgr.connect(req, out.clone()).await.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn an_unknown_target_behind_a_jump_host_is_not_wrapped() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];

    let out = ring();
    let err = mgr.connect(req.clone(), out.clone()).await.unwrap_err();
    assert!(matches!(err, SshError::UnknownHost { .. }), "got {err:?}");
    req.trusted_fingerprint = Some(fingerprint(&target.host_key));
    let out = ring();
    mgr.connect(req, out.clone()).await.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn a_wrong_jump_password_names_the_hop_and_never_reaches_the_target() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password("nope"))];
    let out = ring();

    let err = mgr.connect(req, out.clone()).await.unwrap_err();
    assert_eq!(err.to_string(), "via bastion (jump host 1): authentication failed");
    assert!(
        matches!(&err, SshError::Hop { hop: 1, name, error } if name == "bastion" && matches!(**error, SshError::AuthFailed)),
        "got {err:?}"
    );
    assert_eq!(target.password_attempts(), 0);
    assert_eq!(wait_connections(&bastion, 0).await, 0);
    assert_eq!(wait_connections(&target, 0).await, 0);
}

#[tokio::test]
async fn a_failing_second_jump_names_its_position_and_closes_the_first() {
    let first = test_server::start(2, None).await;
    let second = test_server::start(3, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&first, &second, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![
        jump("first", &first, password(PASSWORD)),
        jump("second", &second, password("nope")),
    ];
    let out = ring();

    let err = mgr.connect(req, out.clone()).await.unwrap_err();
    assert_eq!(err.to_string(), "via second (jump host 2): authentication failed");
    assert_eq!(wait_connections(&first, 0).await, 0);
}

#[tokio::test]
async fn stored_jump_passwords_are_bound_to_the_jump_endpoint() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let mgr = manager(trust_all(&dir, &[&bastion, &target])).with_secrets(store.clone());
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, stored("j1"))];

    // Saved for the target's endpoint, not the jump host's: never sent to the jump host.
    store
        .set("j1", "label", &secret_for(target.addr.port(), PASSWORD))
        .unwrap();
    let out = ring();
    let err = mgr.connect(req.clone(), out.clone()).await.unwrap_err();
    assert!(
        matches!(&err, SshError::Hop { hop: 1, error, .. } if matches!(**error, SshError::SecretMissing)),
        "got {err:?}"
    );
    assert_eq!(bastion.password_attempts(), 0);

    store
        .set("j1", "label", &secret_for(bastion.addr.port(), PASSWORD))
        .unwrap();
    let out = ring();
    mgr.connect(req, out.clone()).await.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(bastion.password_attempts(), 1);
}

#[tokio::test]
async fn remember_on_a_jump_host_saves_its_password_for_its_endpoint() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::shared();
    let mgr = manager(trust_all(&dir, &[&bastion, &target])).with_secrets(store.clone());
    let mut req = request(target.addr.port(), password(PASSWORD));
    let mut bastion_hop = jump("bastion", &bastion, password(PASSWORD));
    bastion_hop.storage = remember("j1");
    req.jumps = vec![bastion_hop];
    let out = ring();

    mgr.connect(req, out.clone()).await.unwrap();
    let saved: serde_json::Value = serde_json::from_str(&store.get("j1").unwrap().unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(&secret_for(bastion.addr.port(), PASSWORD)).unwrap();
    assert_eq!(saved, expected);
    assert_eq!(store.get("s1").unwrap(), None);
}

#[tokio::test]
async fn closing_a_chained_session_closes_every_hop() {
    let first = test_server::start(2, None).await;
    let second = test_server::start(3, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&first, &second, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![
        jump("first", &first, password(PASSWORD)),
        jump("second", &second, password(PASSWORD)),
    ];
    let out = ring();
    let shell = mgr.connect(req, out.clone()).await.unwrap();
    let id = shell.id;
    wait_for(&out, "welcome").await;
    assert_eq!(first.open_connections(), 1);
    assert_eq!(second.open_connections(), 1);

    mgr.close(id).unwrap();
    let exit = wait_exit(shell.exit).await;
    assert!(exit.is_some(), "session did not end after close");
    assert_eq!(wait_connections(&target, 0).await, 0, "target still open");
    assert_eq!(wait_connections(&second, 0).await, 0, "jump 2 still open");
    assert_eq!(wait_connections(&first, 0).await, 0, "jump 1 still open");
}

#[tokio::test]
async fn a_remote_exit_behind_jump_hosts_closes_every_hop() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];
    let out = ring();
    let shell = mgr.connect(req, out.clone()).await.unwrap();
    let id = shell.id;
    wait_for(&out, "welcome").await;

    mgr.write(id, b"exit").await.unwrap();
    let exit = wait_exit(shell.exit).await;
    assert_eq!(exit, Some(Some(7)));
    assert_eq!(wait_connections(&bastion, 0).await, 0, "jump still open");
}

#[tokio::test]
async fn sftp_works_over_a_chained_session() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("hello.txt"), b"hi").unwrap();
    let bastion = test_server::start(2, None).await;
    let target = test_server::start_with_sftp(root.path()).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion, &target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];
    let out = ring();
    let id = mgr.connect(req, out.clone()).await.unwrap().id;

    let sftp = tokio::time::timeout(Duration::from_secs(10), mgr.sftp(id))
        .await
        .expect("opening SFTP timed out")
        .unwrap();
    let names: Vec<String> = sftp
        .read_dir("/")
        .await
        .unwrap()
        .map(|entry| entry.file_name())
        .filter(|name| name != "." && name != "..")
        .collect();
    assert_eq!(names, vec!["hello.txt".to_string()]);
}

#[tokio::test]
async fn more_than_three_jump_hosts_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let target = test_server::start(1, None).await;
    let mgr = manager(trust_all(&dir, &[&target]));
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("j", &target, password(PASSWORD)); MAX_JUMPS + 1];
    let out = ring();

    let err = mgr.connect(req, out.clone()).await.unwrap_err();
    assert!(matches!(err, SshError::Connection { .. }), "got {err:?}");
    assert_eq!(target.open_connections(), 0);
}

#[test]
fn debug_of_jump_hosts_never_contains_their_passwords() {
    let mut req = request(22, password("hunter2"));
    req.jumps = vec![JumpHost {
        name: "b".into(),
        host: "b".into(),
        port: 22,
        username: "u".into(),
        auth: password("jumpsecret"),
        trusted_fingerprint: None,
        storage: remember("j1"),
    }];
    let dbg = format!("{req:?}");
    assert!(!dbg.contains("jumpsecret") && !dbg.contains("hunter2"), "{dbg}");
}

#[tokio::test]
async fn exec_returns_the_stdout_of_a_command_over_the_existing_connection() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    wait_for(&out, "welcome").await;

    let stdout = mgr.exec(id, "mosh-server new").await.unwrap();

    assert_eq!(stdout, test_server::MOSH_CONNECT_REPLY);
    assert_eq!(server.exec_commands(), vec!["mosh-server new".to_string()]);
    // No second login: the command ran on the shell's connection.
    assert_eq!(server.open_connections(), 1);
    // The shell keeps working afterwards.
    mgr.write(id, b"ping").await.unwrap();
    assert!(wait_for(&out, "ping").await.contains("ping"));
}

#[tokio::test]
async fn exec_reports_a_failing_command_with_its_stderr() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    wait_for(&out, "welcome").await;

    let err = mgr.exec(id, "nope").await.unwrap_err();

    let message = err.to_string();
    assert!(message.contains("exit status 127"), "{message}");
    assert!(message.contains("nope: not found"), "{message}");
}

/// A password session on `server` and the manager holding it.
async fn exec_session(server: &test_server::TestServer, dir: &tempfile::TempDir) -> (SshManager, u32) {
    let mgr = trusting(dir, server.addr.port(), &server.host_key);
    let out = ring();
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    wait_for(&out, "welcome").await;
    (mgr, id)
}

/// Waits up to 5 s for `server` to have no exec channel open.
async fn wait_exec_channels_closed(server: &test_server::TestServer) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while server.open_exec_channels() > 0 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    server.open_exec_channels()
}

#[tokio::test]
async fn exec_closes_its_channel_when_the_command_times_out() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, id) = exec_session(&server, &dir).await;
    let mgr = mgr.with_exec_timeout(Duration::from_millis(200));

    let err = mgr.exec(id, "sleep-forever").await.unwrap_err();

    assert!(err.to_string().contains("did not finish"), "{err}");
    // OpenSSH allows 10 channels per connection: a timed-out command must not keep one.
    assert_eq!(
        wait_exec_channels_closed(&server).await,
        0,
        "the exec channel was left open"
    );
}

/// After its deadline, `exec` must return even when the connection cannot take the channel
/// close: here the session's own output ring is full, so russh's read loop is stuck delivering
/// shell output and its 10-command queue is filled by other requests.
#[tokio::test(flavor = "multi_thread")]
async fn an_exec_timeout_returns_even_when_the_close_cannot_be_sent() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key).with_exec_timeout(Duration::from_millis(1500));
    // Nobody drains this ring: once it is full the shell reader stops taking messages.
    let out = ByteRing::new(64);
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    let mgr = Arc::new(mgr);
    let handle = mgr.entry(id, |entry| entry.handle.clone()).unwrap();

    let exec = {
        let mgr = mgr.clone();
        tokio::spawn(async move { mgr.exec(id, "sleep-forever").await })
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while server.open_exec_channels() == 0 {
        assert!(tokio::time::Instant::now() < deadline, "the exec channel never opened");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Echoed input fills the ring, then the shell channel's queue, which stops the read loop.
    let writer = {
        let mgr = mgr.clone();
        tokio::spawn(async move {
            for _ in 0..1000 {
                if mgr.write(id, b"x").await.is_err() {
                    return;
                }
            }
        })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Requests that never get through fill the connection's command queue.
    let mut stuck = Vec::new();
    for _ in 0..50 {
        let handle = handle.clone();
        stuck.push(tokio::spawn(async move {
            let _ = handle.channel_open_session().await;
        }));
    }

    let result = tokio::time::timeout(Duration::from_secs(8), exec).await;
    // Unblock the stuck reader (it waits on the blocking pool) so the runtime can shut down.
    out.close();
    writer.abort();
    for task in stuck {
        task.abort();
    }
    let err = result
        .expect("exec did not return after its deadline")
        .unwrap()
        .unwrap_err();
    assert!(err.to_string().contains("did not finish"), "{err}");
}

#[tokio::test]
async fn exec_reports_a_command_killed_by_a_signal_as_an_error() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, id) = exec_session(&server, &dir).await;

    let err = mgr.exec(id, "killed-by-signal").await.unwrap_err();

    let message = err.to_string();
    assert!(message.contains("signal KILL"), "{message}");
}

#[tokio::test]
async fn exec_returns_canned_stdout_and_a_quiet_command_returns_nothing() {
    let server = test_server::start(1, None).await;
    server.reply_to_exec("hello", "hi\n");
    let dir = tempfile::tempdir().unwrap();
    let (mgr, id) = exec_session(&server, &dir).await;

    assert_eq!(mgr.exec(id, "hello").await.unwrap(), "hi\n");
    assert_eq!(mgr.exec(id, "true").await.unwrap(), "");
}

#[test]
fn without_a_home_folder_there_is_no_known_hosts_file_and_no_manager() {
    let err = SshManager::for_known_hosts(None)
        .err()
        .expect("no manager without a home");
    assert!(matches!(err, SshError::NoHome), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Cannot find your home folder, so the known hosts file is unknown."
    );
    let path = PathBuf::from("/home/me/.ssh/known_hosts");
    let mgr = SshManager::for_known_hosts(Some(path.clone())).unwrap();
    assert_eq!(mgr.context().known_hosts, path);
}

#[tokio::test]
async fn exec_on_an_unknown_session_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(dir.path().join("known_hosts"));
    let err = mgr.exec(42, "true").await.unwrap_err();
    assert!(matches!(err, SshError::UnknownSession { id: 42 }), "{err:?}");
}

// Password logins try the agent and the default identity files first (S1).

#[tokio::test]
async fn a_password_login_uses_an_accepted_default_key_without_sending_the_password() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &client_key);
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    mgr.connect(request(server.addr.port(), password("nope")), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.password_attempts(), 0, "the password was sent");
}

#[tokio::test]
async fn a_stored_password_login_uses_an_accepted_default_key_without_reading_the_keyring() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_rsa"), &client_key);
    // No saved password: the login must not need one.
    let mgr = with_store(&dir, &server, &MemoryStore::shared());
    let out = ring();

    mgr.connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.password_attempts(), 0);
}

#[tokio::test]
#[cfg(unix)]
async fn a_password_login_uses_an_accepted_agent_key_without_sending_the_password() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    let Some(agent) = spawn_agent(&dir, &client_key).await else {
        return;
    };
    let mgr = trusting(&dir, server.addr.port(), &server.host_key).with_agent_socket(agent.socket.clone());
    let out = ring();

    let result = mgr
        .connect(request(server.addr.port(), password("nope")), out.clone())
        .await;
    drop(agent);
    result.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.password_attempts(), 0, "the password was sent");
}

#[tokio::test]
async fn an_encrypted_default_key_is_skipped_and_the_password_logs_in() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    let encrypted = client_key
        .encrypt(
            &mut russh::keys::ssh_key::rand_core::UnwrapErr(rand::rngs::SysRng),
            "passphrase",
        )
        .unwrap();
    let pem = encrypted.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap();
    std::fs::write(dir.path().join("id_ed25519"), pem.as_bytes()).unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    mgr.connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.publickey_attempts(), 0, "the encrypted key was offered");
    assert_eq!(server.password_attempts(), 1);
}

#[tokio::test]
async fn rejected_default_keys_fall_back_to_the_stored_password() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &key_from_seed(20));
    write_key(&dir.path().join("id_ecdsa"), &key_from_seed(21));
    let store = MemoryStore::shared();
    store
        .set("s1", "label", &secret_for(server.addr.port(), PASSWORD))
        .unwrap();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    mgr.connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.publickey_attempts(), 2);
    assert_eq!(server.password_attempts(), 1);
}

#[tokio::test]
async fn rejected_default_keys_without_a_stored_password_are_secret_missing() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &key_from_seed(20));
    let mgr = with_store(&dir, &server, &MemoryStore::shared());
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), stored("s1")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::SecretMissing), "got {err:?}");
    assert_eq!(server.publickey_attempts(), 1, "the default key was not tried first");
    assert_eq!(server.password_attempts(), 0);
}

#[tokio::test]
async fn rejected_default_keys_then_a_wrong_password_is_auth_failed() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &key_from_seed(20));
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let err = mgr
        .connect(request(server.addr.port(), password("nope")), out.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::AuthFailed), "got {err:?}");
    assert_eq!(server.password_attempts(), 1);
}

#[tokio::test]
async fn the_password_still_gets_a_chance_when_the_server_drops_after_too_many_keys() {
    // Disconnects at the third attempt, like OpenSSH with `MaxAuthTries 2`.
    let server = test_server::start_limited(1, None, 2).await;
    let dir = tempfile::tempdir().unwrap();
    for (name, seed) in [("id_ed25519", 20), ("id_ecdsa", 21), ("id_rsa", 22)] {
        write_key(&dir.path().join(name), &key_from_seed(seed));
    }
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    mgr.connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(server.password_attempts(), 1);
}

#[tokio::test]
async fn remember_saves_nothing_when_a_default_key_logged_in() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &client_key);
    let store = MemoryStore::shared();
    let mgr = with_store(&dir, &server, &store);
    let out = ring();

    mgr.connect_with(
        request(server.addr.port(), password("unverified")),
        remember("s1"),
        out.clone(),
    )
    .await
    .unwrap();
    assert_eq!(store.get("s1").unwrap(), None, "an unverified password was saved");
}

#[tokio::test]
async fn a_key_file_login_does_not_try_the_default_keys() {
    let client_key = key_from_seed(9);
    let server = test_server::start(1, Some(client_key.public_key().clone())).await;
    let dir = tempfile::tempdir().unwrap();
    write_key(&dir.path().join("id_ed25519"), &key_from_seed(20));
    let keys = tempfile::tempdir().unwrap();
    let key_path = keys.path().join("chosen");
    write_key(&key_path, &client_key);
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();

    let auth = SshAuth::Key {
        path: key_path.to_string_lossy().into(),
        passphrase: None,
    };
    mgr.connect(request(server.addr.port(), auth), out.clone())
        .await
        .unwrap();
    assert_eq!(server.publickey_attempts(), 1);
}

#[tokio::test]
async fn a_password_jump_host_logs_in_with_a_default_key() {
    let client_key = key_from_seed(9);
    let bastion = test_server::start(2, Some(client_key.public_key().clone())).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(trust_all(&dir, &[&bastion, &target]));
    write_key(&dir.path().join("id_ed25519"), &client_key);
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password("nope"))];
    let out = ring();

    mgr.connect(req, out.clone()).await.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(bastion.password_attempts(), 0, "the jump password was sent");
    assert_eq!(target.publickey_attempts(), 1);
    assert_eq!(target.password_attempts(), 1);
}

// --- Native port: the host key prompt and the term-engine session (T3a) ---

/// A prompt answering `answer` (after `delay`) and recording every request it was shown.
fn prompt(answer: bool, delay: Duration) -> (SharedPrompt, Arc<Mutex<Vec<TrustRequest>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    let prompt = move |request: TrustRequest| {
        lock(&seen).push(request);
        async move {
            tokio::time::sleep(delay).await;
            answer
        }
    };
    (Arc::new(prompt), asked)
}

#[tokio::test]
async fn an_unknown_host_trusted_in_the_prompt_connects_and_is_recorded() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    let (yes, asked) = prompt(true, Duration::ZERO);
    let mgr = manager(path.clone()).with_host_key_prompt(yes);
    let port = server.addr.port();

    let out = ring();
    mgr.connect(request(port, password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(
        *lock(&asked),
        vec![TrustRequest {
            host: format!("[127.0.0.1]:{port}"),
            fingerprint: fingerprint(&server.host_key),
            other_known_keys: vec![],
            jump: None,
        }]
    );

    // Recorded: a manager without a prompt now connects too.
    let out = ring();
    let plain = manager(path);
    let _shell = plain
        .connect(request(port, password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    assert_eq!(lock(&asked).len(), 1);
}

#[tokio::test]
async fn an_unknown_host_declined_in_the_prompt_is_unknown_host_and_not_recorded() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known_hosts");
    let (no, asked) = prompt(false, Duration::ZERO);
    let mgr = manager(path.clone()).with_host_key_prompt(no);

    let err = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), ring())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::UnknownHost { .. }), "got {err:?}");
    assert_eq!(lock(&asked).len(), 1);
    assert!(!path.exists(), "known_hosts must not be written without consent");
    assert_eq!(server.password_attempts(), 0);
}

#[tokio::test]
async fn a_changed_host_key_is_refused_without_asking() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (yes, asked) = prompt(true, Duration::ZERO);
    let mgr = trusting(&dir, server.addr.port(), &key_from_seed(2).public_key().clone()).with_host_key_prompt(yes);

    let err = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), ring())
        .await
        .unwrap_err();
    assert!(matches!(err, SshError::HostKeyChanged { .. }), "got {err:?}");
    assert!(lock(&asked).is_empty());
}

#[tokio::test]
async fn the_prompt_names_the_jump_host_and_its_position() {
    let bastion = test_server::start(2, None).await;
    let target = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (yes, asked) = prompt(true, Duration::ZERO);
    let mgr = manager(trust_all(&dir, &[&target])).with_host_key_prompt(yes);
    let mut req = request(target.addr.port(), password(PASSWORD));
    req.jumps = vec![jump("bastion", &bastion, password(PASSWORD))];

    let out = ring();
    mgr.connect(req, out.clone()).await.unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
    let asked = lock(&asked);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].fingerprint, fingerprint(&bastion.host_key));
    assert_eq!(
        asked[0].jump,
        Some(JumpPosition {
            name: "bastion".into(),
            hop: 1,
            of: 1
        })
    );
}

#[tokio::test]
async fn time_spent_in_the_prompt_does_not_count_toward_the_connect_timeout() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let (slow_yes, _) = prompt(true, Duration::from_millis(800));
    let mgr = manager(dir.path().join("known_hosts"))
        .with_host_key_prompt(slow_yes)
        .with_connect_timeout(Duration::from_millis(300));

    let out = ring();
    mgr.connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));
}

#[tokio::test]
async fn a_server_that_never_answers_times_out() {
    // Accepts the TCP connection but never sends an SSH banner.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let _held = tokio::spawn(async move {
        let mut open = Vec::new();
        while let Ok((tcp, _)) = listener.accept().await {
            open.push(tcp);
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mgr = manager(dir.path().join("known_hosts")).with_connect_timeout(Duration::from_millis(300));

    let err = tokio::time::timeout(
        Duration::from_secs(5),
        mgr.connect(request(port, password(PASSWORD)), ring()),
    )
    .await
    .expect("the connect timeout did not fire")
    .unwrap_err();
    assert_eq!(err.to_string(), format!("connection to [127.0.0.1]:{port} timed out"));
}

/// Every visible row of `session`'s screen, one per line.
fn screen(session: &term_engine::Session) -> String {
    let frame = session.full_frame();
    (0..usize::from(frame.rows))
        .map(|row| frame.text_of_row(row))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn wait_screen(session: &term_engine::Session, needle: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        // A snapshot also applies pending resizes (term-engine ST-6).
        let _ = session.snapshot();
        let text = screen(session);
        if text.contains(needle) || tokio::time::Instant::now() > deadline {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_term_engine_session_runs_over_the_shell() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let output = ring();
    let shell = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), output.clone())
        .await
        .unwrap();
    let hooks = term_engine::SessionHooks {
        on_event: Arc::new(|_| {}),
        on_resize: shell.resize.clone().into_hook(),
    };
    let config = term_engine::SessionConfig {
        columns: 80,
        rows: 24,
        scrollback: 100,
    };
    let session = term_engine::Session::start(config, output, Box::new(shell.input), hooks);

    assert!(wait_screen(&session, "welcome").await.contains("welcome"));
    session.write(b"ping").unwrap();
    assert!(wait_screen(&session, "ping").await.contains("ping"));

    session.request_resize(100, 30);
    let _ = session.snapshot();
    session.write(b"size?").unwrap();
    let text = wait_screen(&session, "size=100x30").await;
    assert!(text.contains("size=100x30"), "screen was {text:?}");

    session.write(b"exit").unwrap();
    assert_eq!(wait_exit(shell.exit).await, Some(Some(7)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_output_ring_holds_the_shell_back_without_losing_output() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    // Far smaller than the echoed data, so the reader must wait for room many times.
    let out = ByteRing::new(1024);
    let id = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap()
        .id;
    assert!(wait_for(&out, "welcome").await.contains("welcome"));

    let size = 512 * 1024;
    mgr.write(id, &vec![b'b'; size]).await.unwrap();
    let mut echoed = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while echoed < size {
        assert!(out.len() <= out.capacity());
        match out.try_pop(256) {
            Some(bytes) => echoed += bytes.iter().filter(|b| **b == b'b').count(),
            None if out.is_closed() => panic!("session ended after {echoed} bytes"),
            None if tokio::time::Instant::now() > deadline => panic!("stalled after {echoed} of {size} bytes"),
            None => tokio::time::sleep(Duration::from_millis(1)).await,
        }
    }
    assert_eq!(echoed, size);
    // The shell still answers afterwards.
    mgr.write(id, b"ping").await.unwrap();
    assert!(wait_for(&out, "ping").await.contains("ping"));
}

#[tokio::test]
async fn closing_the_output_ring_ends_the_connection() {
    let server = test_server::start(1, None).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = trusting(&dir, server.addr.port(), &server.host_key);
    let out = ring();
    let shell = mgr
        .connect(request(server.addr.port(), password(PASSWORD)), out.clone())
        .await
        .unwrap();
    assert!(wait_for(&out, "welcome").await.contains("welcome"));

    // Nobody reads the terminal any more (its session was dropped).
    out.close();
    mgr.write(shell.id, b"more output").await.unwrap();
    assert!(wait_exit(shell.exit).await.is_some(), "the session did not end");
    assert_eq!(wait_connections(&server, 0).await, 0, "the connection is still open");
}

#[test]
fn the_exec_timeout_message_keeps_the_v0215_text() {
    assert_eq!(
        crate::exec_timeout_message(crate::EXEC_TIMEOUT),
        format!(
            "remote command did not finish within {} s",
            crate::EXEC_TIMEOUT.as_secs()
        )
    );
    assert_eq!(
        crate::exec_timeout_message(Duration::from_secs(10)),
        "remote command did not finish within 10 s"
    );
}
