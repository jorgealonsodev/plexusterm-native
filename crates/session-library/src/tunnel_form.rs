//! Tunnel editor (v0.2.15 `tunnelForm.ts`, TUN-1a).

use tunnels::{TunnelDefinition, TunnelKind};

use crate::text::{js_trim, port_of};
use crate::tunnel_library::kind_name;

pub const DEFAULT_LISTEN_HOST: &str = "127.0.0.1";

/// The editor's fields as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelForm {
    /// Empty means `<type> <listen port>→<dest host>:<dest port>` (`dynamic <listen port>`).
    pub name: String,
    pub kind: TunnelKind,
    pub session_id: String,
    /// Empty means [`DEFAULT_LISTEN_HOST`].
    pub listen_host: String,
    pub listen_port: String,
    pub dest_host: String,
    pub dest_port: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelFormError {
    ChooseSession,
    InvalidListenPort,
    DestHostRequired,
    InvalidDestPort,
}

impl std::fmt::Display for TunnelFormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ChooseSession => "Choose a saved SSH session",
            Self::InvalidListenPort => "Listen port must be a number between 1 and 65535",
            Self::DestHostRequired => "Destination host is required",
            Self::InvalidDestPort => "Destination port must be a number between 1 and 65535",
        })
    }
}

impl std::error::Error for TunnelFormError {}

/// Validates the editor into the tunnel saved under `id`; `session_ids` are the saved sessions
/// a tunnel may use. Dynamic tunnels have no destination ("" and 0).
pub fn parse_tunnel_form<S: AsRef<str>>(
    form: &TunnelForm,
    id: &str,
    session_ids: &[S],
) -> Result<TunnelDefinition, TunnelFormError> {
    if !session_ids.iter().any(|s| s.as_ref() == form.session_id) {
        return Err(TunnelFormError::ChooseSession);
    }
    let listen_port = port_of(js_trim(&form.listen_port)).ok_or(TunnelFormError::InvalidListenPort)?;
    let (dest_host, dest_port) = if form.kind == TunnelKind::Dynamic {
        (String::new(), 0)
    } else {
        let host = js_trim(&form.dest_host);
        if host.is_empty() {
            return Err(TunnelFormError::DestHostRequired);
        }
        (
            host.to_string(),
            port_of(js_trim(&form.dest_port)).ok_or(TunnelFormError::InvalidDestPort)?,
        )
    };
    let dest = if form.kind == TunnelKind::Dynamic {
        String::new()
    } else {
        format!("→{dest_host}:{dest_port}")
    };
    let name = js_trim(&form.name);
    let listen_host = js_trim(&form.listen_host);
    Ok(TunnelDefinition {
        id: id.into(),
        name: if name.is_empty() {
            format!("{} {listen_port}{dest}", kind_name(form.kind))
        } else {
            name.into()
        },
        kind: form.kind,
        session_id: form.session_id.clone(),
        listen_host: if listen_host.is_empty() {
            DEFAULT_LISTEN_HOST.into()
        } else {
            listen_host.into()
        },
        listen_port,
        dest_host,
        dest_port,
    })
}

pub fn form_from_tunnel(tunnel: &TunnelDefinition) -> TunnelForm {
    TunnelForm {
        name: tunnel.name.clone(),
        kind: tunnel.kind,
        session_id: tunnel.session_id.clone(),
        listen_host: tunnel.listen_host.clone(),
        listen_port: tunnel.listen_port.to_string(),
        dest_host: tunnel.dest_host.clone(),
        dest_port: if tunnel.dest_port == 0 {
            String::new()
        } else {
            tunnel.dest_port.to_string()
        },
    }
}
