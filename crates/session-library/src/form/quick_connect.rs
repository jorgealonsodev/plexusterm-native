//! SSH fields (v0.2.15 `ssh/quickConnect.ts`).

use proto_ssh::SshAuth;
use zeroize::Zeroizing;

use super::FormError;
use crate::target::SshTarget;
use crate::text::{js_trim, port_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    Key,
    Agent,
}

/// The SSH fields as typed. Saved sessions never show the password or passphrase (SES-1h).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickConnectForm {
    /// Also accepts `user@host:port`, `[IPv6]:port` and a bare IPv6 address.
    pub host: String,
    /// Empty means 22.
    pub port: String,
    pub username: String,
    pub method: AuthMethod,
    pub password: String,
    pub key_path: String,
    pub passphrase: String,
}

/// `[IPv6]` or `[IPv6]:port`.
fn bracketed(host: &str) -> Option<(&str, &str)> {
    let rest = host.strip_prefix('[')?;
    let close = rest.find(']')?;
    let inner = rest.get(..close).filter(|i| !i.is_empty())?;
    let after = rest.get(close + 1..)?;
    if after.is_empty() {
        return Some((inner, ""));
    }
    let port = after.strip_prefix(':').filter(|p| crate::text::all_digits(p))?;
    Some((inner, port))
}

/// Validates the SSH fields in the v0.2.15 order: host, username, port, key path.
pub fn parse_quick_connect(form: &QuickConnectForm) -> Result<SshTarget, FormError> {
    let mut host = js_trim(&form.host).to_string();
    let mut username = js_trim(&form.username).to_string();
    let mut port_text = js_trim(&form.port).to_string();

    if let Some(at) = host.rfind('@') {
        if username.is_empty() {
            username = host.get(..at).unwrap_or("").to_string();
        }
        host = host.get(at + 1..).unwrap_or("").to_string();
    }
    if let Some((inner, port)) = bracketed(&host) {
        let (inner, port) = (inner.to_string(), port.to_string());
        host = inner;
        if port_text.is_empty() {
            port_text = port;
        }
    } else if host.matches(':').count() == 1 {
        // host:port; a bare IPv6 address (several colons) carries no port.
        let (name, port) = host.split_once(':').unwrap_or((&host, ""));
        let (name, port) = (name.to_string(), port.to_string());
        host = name;
        if port_text.is_empty() {
            port_text = port;
        }
    }

    if host.is_empty() {
        return Err(FormError::HostRequired);
    }
    if username.is_empty() {
        return Err(FormError::UsernameRequired);
    }
    let port = if port_text.is_empty() {
        22
    } else {
        port_of(&port_text).ok_or(FormError::PortRange)?
    };

    let auth = match form.method {
        AuthMethod::Password => SshAuth::Password {
            password: Zeroizing::new(form.password.clone()),
        },
        AuthMethod::Key => {
            let path = js_trim(&form.key_path);
            if path.is_empty() {
                return Err(FormError::KeyPathRequired);
            }
            let passphrase = (!form.passphrase.is_empty()).then(|| Zeroizing::new(form.passphrase.clone()));
            SshAuth::Key {
                path: path.to_string(),
                passphrase,
            }
        }
        AuthMethod::Agent => SshAuth::Agent,
    };
    Ok(SshTarget {
        host,
        port,
        username,
        auth,
        storage: None,
        jumps: Vec::new(),
    })
}
