//! Logging in through jump hosts (v0.2.15 `jumpHosts.ts`, SSH-3, SSH-6a): prompt labels, the
//! login of every hop, and the target that goes through them.

use proto_ssh::{JumpHost, PasswordStorage, SshAuth};
use zeroize::Zeroizing;

use crate::model::{SavedAuth, SshSession};
use crate::password_flow::storage_for;
use crate::target::{SshTarget, ssh_target};

/// How one hop of a chain logs in.
#[derive(Debug, Clone)]
pub struct HopLogin {
    pub auth: SshAuth,
    pub storage: Option<PasswordStorage>,
}

/// A password typed in the prompt.
#[derive(Debug, Clone)]
pub struct PasswordAnswer {
    pub password: Zeroizing<String>,
    /// `None` while the keyring state is unknown: the saved password is left as it is.
    pub remember: Option<bool>,
}

/// Prompt label for hop `index` of `hops` (jump hosts first, the target last):
/// `bastion — jump host 1 of 2` for jump hosts, `user@host` for the target.
pub fn hop_label(hops: &[SshSession], index: usize) -> String {
    let jumps = hops.len().saturating_sub(1);
    match hops.get(index) {
        Some(hop) if index < jumps => format!("{} — jump host {} of {jumps}", hop.name, index + 1),
        Some(hop) => format!("{}@{}", hop.username, hop.host),
        None => String::new(),
    }
}

fn saved_login(hop: &SshSession) -> HopLogin {
    let auth = match hop.auth {
        SavedAuth::Password => SshAuth::StoredPassword {
            session_id: hop.id.clone(),
        },
        _ => ssh_target(hop, None).auth,
    };
    HopLogin { auth, storage: None }
}

/// Every hop's login without asking: password hops use the saved-password slot, so the backend
/// tries the agent and default keys first and reports `SecretMissing` when the password is needed.
pub fn saved_logins(hops: &[SshSession]) -> Vec<HopLogin> {
    hops.iter().map(saved_login).collect()
}

/// Collects how every hop logs in, in order: password hops use the password saved in the
/// keyring when there is one, otherwise `ask_password(index)` asks for it, one prompt at a time.
/// `None` when a prompt is cancelled (no later hop is asked).
pub async fn collect_logins(
    hops: &[SshSession],
    mut secret_exists: impl AsyncFnMut(&str) -> bool,
    mut ask_password: impl AsyncFnMut(usize) -> Option<PasswordAnswer>,
) -> Option<Vec<HopLogin>> {
    let mut logins = Vec::with_capacity(hops.len());
    for (index, hop) in hops.iter().enumerate() {
        if hop.auth != SavedAuth::Password || secret_exists(&hop.id).await {
            logins.push(saved_login(hop));
        } else {
            let answer = ask_password(index).await?;
            let storage = storage_for(hop, answer.remember, false);
            logins.push(HopLogin {
                auth: SshAuth::Password {
                    password: answer.password,
                },
                storage,
            });
        }
    }
    Some(logins)
}

/// One jump host of a chain, with its own login.
pub fn jump_target(session: &SshSession, login: HopLogin) -> JumpHost {
    JumpHost {
        name: session.name.clone(),
        host: session.host.clone(),
        port: session.port,
        username: session.username.clone(),
        auth: login.auth,
        trusted_fingerprint: None,
        storage: login.storage,
    }
}

/// SSH target for the last of `hops`, going through the others with their own logins; `None`
/// when there are no hops or not one login per hop.
pub fn chain_target(hops: &[SshSession], logins: Vec<HopLogin>) -> Option<SshTarget> {
    if hops.is_empty() || hops.len() != logins.len() {
        return None;
    }
    let mut logins = logins;
    let last = logins.pop()?;
    let (target, jump_hops) = hops.split_last()?;
    let jumps = jump_hops
        .iter()
        .zip(logins)
        .map(|(hop, login)| jump_target(hop, login))
        .collect();
    Some(SshTarget {
        auth: last.auth,
        storage: last.storage,
        jumps,
        ..ssh_target(target, None)
    })
}
