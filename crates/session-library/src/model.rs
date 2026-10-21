//! The saved session kinds of `sessions.json`, field for field as v0.2.15 writes them
//! (`src/features/sessions/library.ts`, `types.ts`).
//!
//! Field order is the key order v0.2.15 writes (`kind`, `id`, `name`, `folder`, then the
//! kind's own fields), so a file saved here is byte-identical to the one v0.2.15 saves.

use serde::Serialize;

/// Most jump hosts a connection may go through (SSH-3).
pub const MAX_JUMPS: usize = proto_ssh::MAX_JUMPS;

/// Saved auth never contains secrets: passwords are asked at connect time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum SavedAuth {
    Password,
    Key { path: String },
    Agent,
}

/// A saved SSH session. Entries without `kind` (files written before serial sessions existed)
/// load as SSH and are written back with `"kind": "ssh"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSession {
    pub id: String,
    pub name: String,
    /// Slash-separated folder path; "" is the root.
    pub folder: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SavedAuth,
    /// Saved SSH sessions to go through first, outermost first; left out of the file when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jump_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialFlow {
    None,
    Software,
    Hardware,
}

/// Line settings of a serial console (e.g. 115200 8N1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialSettings {
    /// Device node or port name, e.g. /dev/ttyUSB0 or COM3.
    pub path: String,
    /// A positive JavaScript safe integer, as v0.2.15 accepts.
    pub baud: u64,
    /// 5, 6, 7 or 8.
    pub data_bits: u8,
    pub parity: SerialParity,
    /// 1 or 2.
    pub stop_bits: u8,
    pub flow: SerialFlow,
}

/// A saved serial console; no secrets are involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerialSession {
    pub id: String,
    pub name: String,
    pub folder: String,
    #[serde(flatten)]
    pub settings: SerialSettings,
}

/// A saved Telnet connection; no secrets are involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TelnetSession {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub host: String,
    pub port: u16,
}

/// A saved Mosh session: it logs in with the saved SSH session `ssh_session_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoshSession {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub ssh_session_id: String,
    /// Replaces the default `mosh-server` command; absent when not overridden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_command: Option<String>,
}

/// A saved local terminal: its shell, arguments and starting directory; absent ones keep the
/// defaults (the shell from Settings or the system's, no arguments, the home directory).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalSession {
    pub id: String,
    pub name: String,
    pub folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Starting directory; a leading `~` is the home directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// Any entry of the session library (SES-1: kinds `ssh`, `serial`, `telnet`, `mosh`, `local`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LibrarySession {
    Ssh(SshSession),
    Serial(SerialSession),
    Telnet(TelnetSession),
    Mosh(MoshSession),
    Local(LocalSession),
}

/// The kind of a session, as written in its `kind` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {
    Ssh,
    Serial,
    Telnet,
    Mosh,
    Local,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Serial => "serial",
            Self::Telnet => "telnet",
            Self::Mosh => "mosh",
            Self::Local => "local",
        }
    }
}

impl LibrarySession {
    pub fn kind(&self) -> SessionKind {
        match self {
            Self::Ssh(_) => SessionKind::Ssh,
            Self::Serial(_) => SessionKind::Serial,
            Self::Telnet(_) => SessionKind::Telnet,
            Self::Mosh(_) => SessionKind::Mosh,
            Self::Local(_) => SessionKind::Local,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Ssh(s) => &s.id,
            Self::Serial(s) => &s.id,
            Self::Telnet(s) => &s.id,
            Self::Mosh(s) => &s.id,
            Self::Local(s) => &s.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Ssh(s) => &s.name,
            Self::Serial(s) => &s.name,
            Self::Telnet(s) => &s.name,
            Self::Mosh(s) => &s.name,
            Self::Local(s) => &s.name,
        }
    }

    pub fn folder(&self) -> &str {
        match self {
            Self::Ssh(s) => &s.folder,
            Self::Serial(s) => &s.folder,
            Self::Telnet(s) => &s.folder,
            Self::Mosh(s) => &s.folder,
            Self::Local(s) => &s.folder,
        }
    }

    /// The SSH session, when this is one; Telnet, Mosh, serial and local sessions are not
    /// (so they are never jump hosts).
    pub fn as_ssh(&self) -> Option<&SshSession> {
        match self {
            Self::Ssh(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_ssh(&self) -> bool {
        matches!(self, Self::Ssh(_))
    }

    pub(crate) fn set_id_and_name(&mut self, id: String, name: String) {
        let (slot_id, slot_name) = match self {
            Self::Ssh(s) => (&mut s.id, &mut s.name),
            Self::Serial(s) => (&mut s.id, &mut s.name),
            Self::Telnet(s) => (&mut s.id, &mut s.name),
            Self::Mosh(s) => (&mut s.id, &mut s.name),
            Self::Local(s) => (&mut s.id, &mut s.name),
        };
        *slot_id = id;
        *slot_name = name;
    }

    /// The text search matches besides the name: the host, the serial device or the shell
    /// (nothing for Mosh), as v0.2.15's `buildTree`.
    pub(crate) fn address(&self) -> &str {
        match self {
            Self::Ssh(s) => &s.host,
            Self::Telnet(s) => &s.host,
            Self::Serial(s) => &s.settings.path,
            Self::Mosh(_) => "",
            Self::Local(s) => s.program.as_deref().unwrap_or(""),
        }
    }
}

impl From<SshSession> for LibrarySession {
    fn from(s: SshSession) -> Self {
        Self::Ssh(s)
    }
}

impl From<SerialSession> for LibrarySession {
    fn from(s: SerialSession) -> Self {
        Self::Serial(s)
    }
}

impl From<TelnetSession> for LibrarySession {
    fn from(s: TelnetSession) -> Self {
        Self::Telnet(s)
    }
}

impl From<MoshSession> for LibrarySession {
    fn from(s: MoshSession) -> Self {
        Self::Mosh(s)
    }
}

impl From<LocalSession> for LibrarySession {
    fn from(s: LocalSession) -> Self {
        Self::Local(s)
    }
}
