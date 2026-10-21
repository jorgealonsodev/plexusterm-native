//! Saved-password rules (v0.2.15 `passwordFlow.ts`, SSH-6a): which login uses the keyring,
//! what a login with a typed password saves or forgets, and which edits forget it.

use std::collections::HashSet;

use proto_ssh::{PasswordStorage, SshAuth, SshError};

use crate::library::{LibraryAction, SessionLibrary};
use crate::model::*;
use crate::target::{SshTarget, ssh_target};
use crate::text::js_trim;

/// Label of the keyring item, as shown by keyring managers: `PlexusTerm: user@host:port`.
pub fn secret_label(username: &str, host: &str, port: u16) -> String {
    format!("PlexusTerm: {username}@{host}:{port}")
}

/// Target that connects with the password saved in the keyring; the UI never sees it.
pub fn stored_password_target(session: &SshSession) -> SshTarget {
    SshTarget {
        auth: SshAuth::StoredPassword {
            session_id: session.id.clone(),
        },
        ..ssh_target(session, None)
    }
}

/// Target that connects with a password typed in the prompt.
pub fn typed_password_target(session: &SshSession, password: &str, storage: Option<PasswordStorage>) -> SshTarget {
    SshTarget {
        storage,
        ..ssh_target(session, Some(password))
    }
}

/// What to do with the saved password after a login with a typed one: ticked saves it,
/// unticked forgets a password saved before; `None` (keyring state still unknown) changes nothing.
pub fn storage_for(session: &SshSession, remember: Option<bool>, had_secret: bool) -> Option<PasswordStorage> {
    match remember? {
        true => Some(PasswordStorage::Remember {
            session_id: session.id.clone(),
            label: secret_label(&session.username, &session.host, session.port),
        }),
        false => had_secret.then(|| PasswordStorage::Forget {
            session_id: session.id.clone(),
        }),
    }
}

/// True when a login with the saved password failed in a way the prompt can fix.
pub fn should_ask_again(auth: &SshAuth, err: &SshError) -> bool {
    matches!(auth, SshAuth::StoredPassword { .. }) && matches!(err, SshError::SecretMissing | SshError::AuthFailed)
}

/// True when the server rejected the saved password; false when none could be used, which is
/// asked like a first login (unticked, with nothing to forget).
pub fn saved_password_rejected(err: &SshError) -> bool {
    matches!(err, SshError::AuthFailed)
}

/// Saved SSH sessions that `action` removes from `library` (their saved passwords go with them).
pub fn removed_ssh_sessions<'a>(library: &'a SessionLibrary, action: &LibraryAction) -> Vec<&'a SshSession> {
    let next = library.reduce(action);
    let kept: HashSet<&str> = next.sessions.iter().map(LibrarySession::id).collect();
    library
        .sessions
        .iter()
        .filter_map(LibrarySession::as_ssh)
        .filter(|s| !kept.contains(s.id.as_str()))
        .collect()
}

/// `user@host:port`, normalized like the endpoint the backend binds a saved password to.
fn endpoint_of(session: &SshSession) -> String {
    format!(
        "{}@{}:{}",
        js_trim(&session.username),
        js_trim(&session.host),
        session.port
    )
}

/// Id whose saved password must be forgotten when `before` is edited into `after`: password
/// auth was dropped, the session became another kind, or the password would now go to another
/// host, port or user.
pub fn secret_to_forget_on_edit(before: &SshSession, after: &LibrarySession) -> Option<String> {
    if before.auth != SavedAuth::Password {
        return None;
    }
    let same_endpoint = after
        .as_ssh()
        .is_some_and(|next| next.auth == SavedAuth::Password && endpoint_of(next) == endpoint_of(before));
    (!same_endpoint).then(|| before.id.clone())
}
