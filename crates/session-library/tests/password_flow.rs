//! Port of v0.2.15 `src/features/sessions/passwordFlow.test.ts` (SSH-6a).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use proto_ssh::{PasswordStorage, SshAuth, SshError};
use session_library::password_flow::*;
use session_library::*;
use zeroize::Zeroizing;

fn web() -> SshSession {
    SshSession {
        id: "s1".into(),
        name: "web".into(),
        folder: "Prod".into(),
        host: "10.0.0.1".into(),
        port: 2222,
        username: "u".into(),
        auth: SavedAuth::Password,
        jump_ids: vec![],
    }
}

fn storage_text(storage: &Option<PasswordStorage>) -> String {
    match storage {
        None => "-".into(),
        Some(PasswordStorage::Remember { session_id, label }) => format!("remember:{session_id}:{label}"),
        Some(PasswordStorage::Forget { session_id }) => format!("forget:{session_id}"),
    }
}

fn auth_text(auth: &SshAuth) -> String {
    match auth {
        SshAuth::Password { password } => format!("password:{}", password.as_str()),
        SshAuth::Key { path, .. } => format!("key:{path}"),
        SshAuth::Agent => "agent".into(),
        SshAuth::StoredPassword { session_id } => format!("stored:{session_id}"),
    }
}

#[test]
fn secret_label_names_the_session_as_plexusterm_user_at_host_port() {
    // it("names the session as PlexusTerm: user@host:port")
    let w = web();
    assert_eq!(
        secret_label(&w.username, &w.host, w.port),
        "PlexusTerm: u@10.0.0.1:2222"
    );
}

#[test]
fn stored_password_target_connects_with_a_reference_to_the_saved_password_never_the_password_itself() {
    let target = stored_password_target(&web());
    assert_eq!(
        (
            target.host.as_str(),
            target.port,
            target.username.as_str(),
            auth_text(&target.auth)
        ),
        ("10.0.0.1", 2222, "u", "stored:s1".to_string())
    );
    assert!(target.storage.is_none() && target.jumps.is_empty());
}

#[test]
fn typed_password_target_connects_with_the_typed_password_and_carries_the_storage_request() {
    let storage = PasswordStorage::Remember {
        session_id: "s1".into(),
        label: "L".into(),
    };
    let target = typed_password_target(&web(), "pw", Some(storage));
    assert_eq!(auth_text(&target.auth), "password:pw");
    assert_eq!(storage_text(&target.storage), "remember:s1:L");
    assert_eq!(
        (target.host.as_str(), target.port, target.username.as_str()),
        ("10.0.0.1", 2222, "u")
    );
}

#[test]
fn typed_password_target_omits_storage_when_there_is_nothing_to_save_or_forget() {
    assert!(typed_password_target(&web(), "pw", None).storage.is_none());
}

#[test]
fn storage_for_leaves_a_saved_password_alone_when_the_keyring_state_is_still_unknown() {
    assert!(storage_for(&web(), None, true).is_none());
}

#[test]
fn storage_for_remembers_the_password_when_the_checkbox_is_ticked() {
    assert_eq!(
        storage_text(&storage_for(&web(), Some(true), false)),
        "remember:s1:PlexusTerm: u@10.0.0.1:2222"
    );
    assert!(matches!(
        storage_for(&web(), Some(true), true),
        Some(PasswordStorage::Remember { .. })
    ));
}

#[test]
fn storage_for_forgets_a_saved_password_when_the_checkbox_is_unticked() {
    assert_eq!(storage_text(&storage_for(&web(), Some(false), true)), "forget:s1");
}

#[test]
fn storage_for_does_nothing_when_unticked_and_nothing_was_saved() {
    assert!(storage_for(&web(), Some(false), false).is_none());
}

fn stored() -> SshAuth {
    SshAuth::StoredPassword {
        session_id: "s1".into(),
    }
}

#[test]
fn should_ask_again_asks_for_the_password_when_the_saved_one_is_missing_or_rejected() {
    assert!(should_ask_again(&stored(), &SshError::SecretMissing));
    assert!(should_ask_again(&stored(), &SshError::AuthFailed));
}

#[test]
fn should_ask_again_does_not_ask_again_for_other_failures() {
    assert!(!should_ask_again(
        &stored(),
        &SshError::Connection {
            message: "refused".into()
        }
    ));
    let unknown = SshError::UnknownHost {
        host: "h".into(),
        fingerprint: "f".into(),
        other_known_keys: vec![],
    };
    assert!(!should_ask_again(&stored(), &unknown));
}

#[test]
fn should_ask_again_never_loops_on_a_typed_password() {
    let typed = SshAuth::Password {
        password: Zeroizing::new("pw".into()),
    };
    assert!(!should_ask_again(&typed, &SshError::AuthFailed));
}

#[test]
fn saved_password_rejected_is_true_when_the_server_rejected_the_saved_password() {
    assert!(saved_password_rejected(&SshError::AuthFailed));
}

#[test]
fn saved_password_rejected_is_false_when_no_saved_password_could_be_used_asked_like_a_first_login() {
    // it("is false when no saved password could be used (asked like a first login)")
    assert!(!saved_password_rejected(&SshError::SecretMissing));
}

// describe("removedSshSessions")

fn removal_library() -> SessionLibrary {
    let serial = SerialSession {
        id: "c1".into(),
        name: "console".into(),
        folder: "Prod".into(),
        settings: SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 115200,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::None,
        },
    };
    let db = SshSession {
        id: "s2".into(),
        name: "db".into(),
        folder: "Dev".into(),
        ..web()
    };
    SessionLibrary::default().reduce(&LibraryAction::SaveSessions(vec![
        web().into(),
        db.into(),
        serial.into(),
    ]))
}

fn removed(action: LibraryAction) -> Vec<String> {
    let library = removal_library();
    removed_ssh_sessions(&library, &action)
        .iter()
        .map(|s| s.id.clone())
        .collect()
}

#[test]
fn removed_ssh_sessions_lists_the_deleted_ssh_session() {
    assert_eq!(removed(LibraryAction::DeleteSession { id: "s1".into() }), ["s1"]);
}

#[test]
fn removed_ssh_sessions_lists_every_ssh_session_inside_a_deleted_folder_but_no_serial_session() {
    assert_eq!(removed(LibraryAction::DeleteFolder { path: "Prod".into() }), ["s1"]);
}

#[test]
fn removed_ssh_sessions_lists_nothing_for_actions_that_delete_no_session() {
    assert!(removed(LibraryAction::AddFolder { path: "X".into() }).is_empty());
}

// describe("secretToForgetOnEdit")

#[test]
fn forgets_the_saved_password_when_a_session_stops_using_password_authentication() {
    let after = SshSession {
        auth: SavedAuth::Agent,
        ..web()
    };
    assert_eq!(secret_to_forget_on_edit(&web(), &after.into()).as_deref(), Some("s1"));
}

#[test]
fn forgets_it_when_the_host_port_or_username_changes() {
    for after in [
        SshSession {
            host: "10.0.0.2".into(),
            ..web()
        },
        SshSession { port: 22, ..web() },
        SshSession {
            username: "root".into(),
            ..web()
        },
    ] {
        assert_eq!(secret_to_forget_on_edit(&web(), &after.into()).as_deref(), Some("s1"));
    }
}

#[test]
fn forgets_it_when_the_session_becomes_another_kind() {
    // passwordFlow.ts accepts `{ kind: "serial" }` as the edited session.
    let telnet = TelnetSession {
        id: "s1".into(),
        name: "web".into(),
        folder: "Prod".into(),
        host: "10.0.0.1".into(),
        port: 23,
    };
    assert_eq!(secret_to_forget_on_edit(&web(), &telnet.into()).as_deref(), Some("s1"));
}

#[test]
fn keeps_it_while_the_session_still_uses_a_password_for_the_same_endpoint_and_ignores_other_sessions() {
    // it("keeps it while the session still uses a password for the same endpoint, and ignores other sessions")
    let renamed = SshSession {
        name: "renamed".into(),
        folder: "Other".into(),
        ..web()
    };
    assert_eq!(secret_to_forget_on_edit(&web(), &renamed.into()), None);
    let spaced = SshSession {
        host: " 10.0.0.1 ".into(),
        ..web()
    };
    assert_eq!(secret_to_forget_on_edit(&web(), &spaced.into()), None);
    let key = SshSession {
        auth: SavedAuth::Key { path: "/k".into() },
        ..web()
    };
    let agent = SshSession {
        auth: SavedAuth::Agent,
        ..key.clone()
    };
    assert_eq!(secret_to_forget_on_edit(&key, &agent.into()), None);
}
