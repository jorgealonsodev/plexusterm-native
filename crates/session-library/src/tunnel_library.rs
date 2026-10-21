//! `tunnels.json` and the tunnel list (v0.2.15 `tunnels.ts`, TUN-1a, DATA-1a). Entries are
//! the `tunnels` crate's [`TunnelDefinition`], whose serde shape is the file's.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

pub use tunnels::{TunnelDefinition, TunnelKind};

use crate::file::integer;

/// `tunnels.json` in memory: `{version: 1, tunnels}` (the version is implied).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelList {
    pub tunnels: Vec<TunnelDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelAction {
    /// Adds the tunnel, or replaces the one with its id.
    Save(TunnelDefinition),
    Delete {
        id: String,
    },
}

/// Why `tunnels.json` cannot be used; the panel shows it and disables editing (TUN-1a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelFileError {
    InvalidJson(String),
    UnsupportedVersion,
    InvalidTunnels,
    /// The file exists but could not be read.
    Storage(String),
}

impl std::fmt::Display for TunnelFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "tunnels file is not valid JSON: {error}"),
            Self::UnsupportedVersion => f.write_str("unsupported tunnels file version"),
            Self::InvalidTunnels => f.write_str("tunnels file has invalid tunnels"),
            Self::Storage(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for TunnelFileError {}

/// The `type` value, also used in default names: `local`, `remote`, `dynamic`.
pub fn kind_name(kind: TunnelKind) -> &'static str {
    match kind {
        TunnelKind::Local => "local",
        TunnelKind::Remote => "remote",
        TunnelKind::Dynamic => "dynamic",
    }
}

/// The type select label.
pub fn kind_label(kind: TunnelKind) -> &'static str {
    match kind {
        TunnelKind::Local => "Local (-L)",
        TunnelKind::Remote => "Remote (-R)",
        TunnelKind::Dynamic => "Dynamic SOCKS5 (-D)",
    }
}

impl TunnelList {
    /// The list after `action` (v0.2.15 `tunnelsReducer`).
    pub fn reduce(&self, action: &TunnelAction) -> TunnelList {
        let mut tunnels = self.tunnels.clone();
        match action {
            TunnelAction::Save(tunnel) => match tunnels.iter_mut().find(|t| t.id == tunnel.id) {
                Some(slot) => *slot = tunnel.clone(),
                None => tunnels.push(tunnel.clone()),
            },
            TunnelAction::Delete { id } => tunnels.retain(|t| t.id != *id),
        }
        TunnelList { tunnels }
    }
}

fn port(entry: &Map<String, Value>, key: &str, min: i128) -> Option<u16> {
    integer(entry.get(key)?)
        .filter(|n| (min..=65535).contains(n))
        .and_then(|n| u16::try_from(n).ok())
}

/// One entry, validated like v0.2.15's `isTunnel`; other fields are dropped.
fn tunnel(value: &Value) -> Option<TunnelDefinition> {
    let entry = value.as_object()?;
    let text = |key: &str| entry.get(key)?.as_str().map(str::to_string);
    let kind = match entry.get("type")?.as_str()? {
        "local" => TunnelKind::Local,
        "remote" => TunnelKind::Remote,
        "dynamic" => TunnelKind::Dynamic,
        _ => return None,
    };
    Some(TunnelDefinition {
        id: text("id")?,
        name: text("name")?,
        kind,
        session_id: text("sessionId")?,
        listen_host: text("listenHost")?,
        listen_port: port(entry, "listenPort", 1)?,
        dest_host: text("destHost")?,
        dest_port: port(entry, "destPort", 0)?,
    })
}

/// A missing file (`None`) is an empty list; anything unreadable is an error, never silently emptied.
pub fn parse_tunnels(json: Option<&str>) -> Result<TunnelList, TunnelFileError> {
    let Some(json) = json else {
        return Ok(TunnelList::default());
    };
    let data: Value = serde_json::from_str(json).map_err(|err| TunnelFileError::InvalidJson(err.to_string()))?;
    if data.get("version").and_then(integer) != Some(1) {
        return Err(TunnelFileError::UnsupportedVersion);
    }
    match data.get("tunnels") {
        Some(Value::Array(items)) => items
            .iter()
            .map(tunnel)
            .collect::<Option<Vec<_>>>()
            .map(|tunnels| TunnelList { tunnels })
            .ok_or(TunnelFileError::InvalidTunnels),
        _ => Err(TunnelFileError::InvalidTunnels),
    }
}

#[derive(Serialize)]
struct TunnelFile<'a> {
    version: u8,
    tunnels: &'a [TunnelDefinition],
}

/// The file contents, written like v0.2.15 (`JSON.stringify(_, null, 2)`).
pub fn serialize_tunnels(list: &TunnelList) -> String {
    // Serializing plain strings and numbers into a String cannot fail.
    serde_json::to_string_pretty(&TunnelFile {
        version: 1,
        tunnels: &list.tunnels,
    })
    .unwrap_or_default()
}

/// `<config dir>/tunnels.json`.
pub fn tunnels_path(config_dir: &Path) -> PathBuf {
    config_dir.join(core_storage::TUNNELS_FILE)
}

pub fn load_tunnels(path: &Path) -> Result<TunnelList, TunnelFileError> {
    let contents = core_storage::load(path).map_err(|err| TunnelFileError::Storage(err.to_string()))?;
    parse_tunnels(contents.as_deref())
}

/// Atomically replaces `tunnels.json`; callers must not save while it could not be read.
pub fn save_tunnels(path: &Path, list: &TunnelList) -> Result<(), core_storage::StorageError> {
    core_storage::save(path, &serialize_tunnels(list))
}

fn host_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// One-line summary, e.g. `L 127.0.0.1:5433 → db:5432`.
pub fn describe_tunnel(tunnel: &TunnelDefinition) -> String {
    let listen = host_port(&tunnel.listen_host, tunnel.listen_port);
    match tunnel.kind {
        TunnelKind::Local => format!("L {listen} → {}", host_port(&tunnel.dest_host, tunnel.dest_port)),
        TunnelKind::Remote => format!("R {listen} → {}", host_port(&tunnel.dest_host, tunnel.dest_port)),
        TunnelKind::Dynamic => format!("D {listen} (SOCKS5)"),
    }
}

/// Listen addresses only reachable from this machine: `localhost`, `::1` and 127.0.0.0/8.
pub fn is_loopback(host: &str) -> bool {
    let lower = crate::text::js_trim(host).to_lowercase();
    let h = lower
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(&lower);
    if matches!(h, "localhost" | "::1" | "0:0:0:0:0:0:0:1") {
        return true;
    }
    let Some(rest) = h.strip_prefix("127.") else {
        return false;
    };
    let octets: Vec<&str> = rest.split('.').collect();
    octets.len() == 3
        && octets.iter().all(|o| {
            (1..=3).contains(&o.len()) && crate::text::all_digits(o) && o.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}
