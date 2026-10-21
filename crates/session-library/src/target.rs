//! Connection targets and tab titles (v0.2.15 `types.ts`, `library.ts::toTarget`, SES-1e).
//!
//! These mirror the targets v0.2.15 sends to its backend; SSH auth, keyring requests and jump
//! hosts reuse the `proto-ssh` types, so a target maps straight onto a connection.

use proto_ssh::{JumpHost, PasswordStorage, SshAuth};
use zeroize::Zeroizing;

use crate::model::*;
use crate::shell::shell_name_of;

/// Title of a local tab without a program, and the default name of such a session.
pub const LOCAL_SHELL_TITLE: &str = "Local shell";

/// An SSH login: where, as whom, how, and through which jump hosts.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
    /// Applied after a successful login with a typed password.
    pub storage: Option<PasswordStorage>,
    /// Jump hosts to go through, outermost first.
    pub jumps: Vec<JumpHost>,
}

/// A terminal on this machine; absent fields keep the defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalTarget {
    pub program: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

/// A Mosh session: `ssh` is the login that starts `mosh-server`.
#[derive(Debug, Clone)]
pub struct MoshTarget {
    pub ssh: SshTarget,
    pub server_command: Option<String>,
}

/// A tab shared from another PlexusTerm (SHARE-13); never saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedTarget {
    pub url: String,
    pub relay_url: String,
    pub relay_host: String,
}

#[derive(Debug, Clone)]
pub enum SessionTarget {
    Local(LocalTarget),
    Ssh(SshTarget),
    Serial(SerialSettings),
    Telnet { host: String, port: u16 },
    Mosh(MoshTarget),
    Shared(SharedTarget),
}

/// The SSH login of a target: its own for SSH, the one that starts the server for Mosh.
pub fn ssh_login_of(target: &SessionTarget) -> Option<&SshTarget> {
    match target {
        SessionTarget::Ssh(ssh) => Some(ssh),
        SessionTarget::Mosh(mosh) => Some(&mosh.ssh),
        _ => None,
    }
}

/// Tab title (SES-1e): `user@host[:port]`, `telnet host[:port]`, `mosh user@host`,
/// `<device> @ <baud>`, the shell name or "Local shell", `Shared · {host}`.
pub fn target_title(target: &SessionTarget) -> String {
    match target {
        SessionTarget::Local(local) => match local.program.as_deref() {
            Some(program) if !crate::text::js_trim(program).is_empty() => shell_name_of(program),
            _ => LOCAL_SHELL_TITLE.to_string(),
        },
        SessionTarget::Serial(serial) => format!("{} @ {}", serial.path, serial.baud),
        SessionTarget::Telnet { host, port } => {
            if *port == 23 {
                format!("telnet {host}")
            } else {
                format!("telnet {host}:{port}")
            }
        }
        SessionTarget::Mosh(mosh) => format!("mosh {}@{}", mosh.ssh.username, mosh.ssh.host),
        SessionTarget::Ssh(ssh) => {
            if ssh.port == 22 {
                format!("{}@{}", ssh.username, ssh.host)
            } else {
                format!("{}@{}:{}", ssh.username, ssh.host, ssh.port)
            }
        }
        SessionTarget::Shared(shared) => format!("Shared · {}", shared.relay_host),
    }
}

/// The SSH target of a saved session; `password` is the one typed at connect time and is used
/// only for password sessions (an empty one when none was typed).
pub fn ssh_target(session: &SshSession, password: Option<&str>) -> SshTarget {
    let auth = match &session.auth {
        SavedAuth::Password => SshAuth::Password {
            password: Zeroizing::new(password.unwrap_or("").to_string()),
        },
        SavedAuth::Key { path } => SshAuth::Key {
            path: path.clone(),
            passphrase: None,
        },
        SavedAuth::Agent => SshAuth::Agent,
    };
    SshTarget {
        host: session.host.clone(),
        port: session.port,
        username: session.username.clone(),
        auth,
        storage: None,
        jumps: Vec::new(),
    }
}

pub fn serial_target(session: &SerialSession) -> SerialSettings {
    session.settings.clone()
}

pub fn local_target(session: &LocalSession) -> LocalTarget {
    LocalTarget {
        program: session.program.clone(),
        args: session.args.clone(),
        cwd: session.cwd.clone(),
    }
}

/// The target of a session that connects by itself; Mosh sessions need their SSH login
/// resolved first, so they have none.
pub fn direct_target(session: &LibrarySession) -> Option<SessionTarget> {
    match session {
        LibrarySession::Ssh(s) => Some(SessionTarget::Ssh(ssh_target(s, None))),
        LibrarySession::Serial(s) => Some(SessionTarget::Serial(serial_target(s))),
        LibrarySession::Telnet(s) => Some(SessionTarget::Telnet {
            host: s.host.clone(),
            port: s.port,
        }),
        LibrarySession::Local(s) => Some(SessionTarget::Local(local_target(s))),
        LibrarySession::Mosh(_) => None,
    }
}
