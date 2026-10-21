//! Interoperability test against a real OpenSSH server, ported from PlexusTerm v0.2.15
//! `src-tauri/src/ssh/interop_tests.rs` (`openssh_sftp_round_trip`, which used these helpers),
//! extended to the operations whose replies differ between servers (exclusive create, rename
//! into a taken name, the changed-on-disk check and chmod).
//!
//! Ignored by default; a script starts a disposable OpenSSH container and runs it with:
//! - `PLEXUSTERM_INTEROP_PORT`: sshd port on 127.0.0.1
//! - `PLEXUSTERM_INTEROP_USER`: login user
//! - `PLEXUSTERM_INTEROP_KEY`: private key accepted by that user

use proto_sftp as sftp;
use proto_ssh::{ConnectRequest, SshAuth, SshError, SshManager};
use term_engine::ByteRing;

// Multi-threaded: the SFTP calls run while the session tasks keep running.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a disposable OpenSSH server (see the module docs)"]
async fn openssh_sftp_round_trip() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let request = |trusted_fingerprint| ConnectRequest {
        host: "127.0.0.1".into(),
        port: var("PLEXUSTERM_INTEROP_PORT").parse().unwrap(),
        username: var("PLEXUSTERM_INTEROP_USER"),
        auth: SshAuth::Key {
            path: var("PLEXUSTERM_INTEROP_KEY"),
            passphrase: None,
        },
        cols: 80,
        rows: 24,
        trusted_fingerprint,
        jumps: vec![],
    };
    let ring = || ByteRing::new(ByteRing::SESSION_CAPACITY);
    let dir = tempfile::tempdir().unwrap();
    let mgr = SshManager::new(dir.path().join("known_hosts")).with_agent_socket(dir.path().join("no-agent.sock"));
    let fingerprint = match mgr.connect_with(request(None), None, ring()).await {
        Err(SshError::UnknownHost { fingerprint, .. }) => fingerprint,
        other => panic!("expected UnknownHost on first contact, got {:?}", other.map(|_| ())),
    };
    let id = mgr
        .connect_with(request(Some(fingerprint)), None, ring())
        .await
        .unwrap()
        .id;
    let session = mgr.sftp(id).await.unwrap();

    let home = sftp::home(&session).await.unwrap();
    let work = format!("{}/plexusterm-interop", home.trim_end_matches('/'));
    for leftover in ["payload.bin", "sub/payload.bin", "notes.txt"] {
        let _ = sftp::remove(&session, &format!("{work}/{leftover}"), false).await;
    }
    let _ = sftp::remove(&session, &format!("{work}/sub"), true).await;
    let _ = sftp::remove(&session, &work, true).await;
    sftp::mkdir(&session, &work).await.unwrap();

    let payload: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i * 31 % 251) as u8).collect();
    let local = dir.path().join("payload.bin");
    std::fs::write(&local, &payload).unwrap();
    let remote = sftp::upload(&session, &local, &work).await.unwrap();

    let names: Vec<_> = sftp::list(&session, &work)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["payload.bin"]);

    let back = dir.path().join("back.bin");
    sftp::download(&session, &remote, &back).await.unwrap();
    assert!(std::fs::read(&back).unwrap() == payload, "downloaded bytes differ");
    assert!(!sftp::partial_path(&back).exists());

    // Exclusive create, the changed-on-disk check, chmod and a refused move.
    let notes = format!("{work}/notes.txt");
    sftp::create_file(&session, &notes).await.unwrap();
    assert!(
        sftp::create_file(&session, &notes).await.is_err(),
        "an existing file was replaced"
    );
    sftp::write_text(&session, &notes, "", "first", None, None)
        .await
        .unwrap();
    let err = sftp::write_text(&session, &notes, "stale", "second", None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("changed on disk"), "{err}");
    assert_eq!(sftp::view_text(&session, &notes, None).await.unwrap().content, "first");
    sftp::chmod(&session, &notes, 0o640).await.unwrap();
    let listed = sftp::list(&session, &work).await.unwrap();
    let mode = listed
        .iter()
        .find(|e| e.name == "notes.txt")
        .and_then(|e| e.permissions);
    assert_eq!(mode.map(|m| m & 0o7777), Some(0o640));

    let sub = format!("{work}/sub");
    sftp::mkdir(&session, &sub).await.unwrap();
    sftp::upload(&session, &local, &sub).await.unwrap();
    let err = sftp::move_into(&session, &remote, &sub).await.unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    let err = sftp::remove(&session, &sub, true).await.unwrap_err();
    assert!(
        matches!(err, sftp::SftpError::Sftp { .. }),
        "a non-empty folder was removed"
    );

    for file in [format!("{sub}/payload.bin"), remote, notes] {
        sftp::remove(&session, &file, false).await.unwrap();
    }
    sftp::remove(&session, &sub, true).await.unwrap();
    sftp::remove(&session, &work, true).await.unwrap();
    mgr.close(id).unwrap();
}
