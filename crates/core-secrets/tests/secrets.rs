//! Ported from PlexusTerm v0.2.15 `src-tauri/src/secrets.rs` and the saved-password tests of
//! `ssh/mod.rs` (PRD DATA-3).
#![allow(clippy::unwrap_used, reason = "test code")]

use std::collections::HashMap;

use core_secrets::{
    KeyringStore, MemoryStore, NoStore, SERVICE, SavedPassword, SecretError, SecretStore, SharedSecrets, endpoint,
    entry_modifiers, run,
};

#[test]
fn every_secret_lives_under_the_v0215_service() {
    assert_eq!(SERVICE, "dev.plexusterm.app");
}

#[tokio::test]
async fn run_uses_the_blocking_pool_on_a_current_thread_runtime() {
    let store: SharedSecrets = MemoryStore::shared();
    run(&store, |s| s.set("s1", "label", "pw")).await.unwrap();
    let got = run(&store, |s| s.get("s1")).await.unwrap();
    assert_eq!(got.as_deref(), Some("pw"));
    run(&store, |s| s.delete("s1")).await.unwrap();
    run(&store, |s| s.delete("s1")).await.unwrap();
    assert_eq!(run(&store, |s| s.get("s1")).await.unwrap(), None);
}

#[test]
fn only_secret_service_platforms_get_a_label_modifier() {
    let label = |os| entry_modifiers(os, "PlexusTerm AI key (OpenCode Go)");
    for os in ["linux", "freebsd", "openbsd"] {
        assert_eq!(
            label(os),
            Some(HashMap::from([("label", "PlexusTerm AI key (OpenCode Go)")])),
            "{os}"
        );
    }
    // The Windows and Apple stores reject unknown modifiers ("unknown key").
    for os in ["windows", "macos", "ios", "android"] {
        assert_eq!(label(os), None, "{os}");
    }
}

#[test]
fn no_store_is_unavailable_and_never_stores() {
    assert!(!NoStore.available());
    assert!(matches!(NoStore.set("s1", "l", "pw"), Err(SecretError::Unavailable)));
    assert_eq!(NoStore.get("s1").unwrap(), None);
}

#[test]
fn keyring_errors_never_carry_secret_bytes() {
    let err = core_secrets::keyring_error(keyring_core::Error::BadEncoding(b"hunter2".to_vec()));
    let shown = format!("{err} {err:?}");
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(matches!(err, SecretError::Keyring { .. }));
}

#[test]
fn a_saved_password_is_bound_to_its_endpoint() {
    let value = SavedPassword::encode(endpoint(" tester ", " 127.0.0.1 ", 22), "pw".into());
    assert_eq!(
        value, r#"{"v":1,"endpoint":"tester@127.0.0.1:22","password":"pw"}"#,
        "the v0.2.15 format"
    );
    assert_eq!(
        SavedPassword::password_for(&value, "tester@127.0.0.1:22").as_deref(),
        Some("pw")
    );
    assert_eq!(
        SavedPassword::password_for(&value, "tester@127.0.0.1:2222"),
        None,
        "never sent to another endpoint"
    );
}

#[test]
fn other_versions_and_plain_text_values_are_no_saved_password() {
    let endpoint = "tester@127.0.0.1:22";
    let v2 = r#"{"v":2,"endpoint":"tester@127.0.0.1:22","password":"pw"}"#;
    assert_eq!(SavedPassword::password_for(v2, endpoint), None);
    assert_eq!(
        SavedPassword::password_for("pw", endpoint),
        None,
        "plain text from older builds"
    );
}

/// Writes to the real keyring under a throwaway service and removes what it created.
/// Run with `cargo test -p core-secrets -- --ignored real_keyring`.
#[test]
#[ignore = "touches the user's real keyring"]
fn real_keyring_round_trip() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let store = KeyringStore::with_service(&format!("dev.plexusterm.test.{nonce}"));
    assert!(store.available(), "no Secret Service on this machine");
    store.set("s1", "PlexusTerm test", "pw").unwrap();
    let got = store.get("s1");
    store.delete("s1").unwrap();
    assert_eq!(got.unwrap().as_deref(), Some("pw"));
    assert_eq!(store.get("s1").unwrap(), None);
}
