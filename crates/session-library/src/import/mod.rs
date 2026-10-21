//! Session imports with preview (v0.2.15 `iniImport.ts`, `sshConfigImport.ts`, SES-2): parse a
//! file into [`ParsedImport`], then [`plan_import`] counts duplicates and builds the sessions
//! to save in one write. Secrets are never read.

mod ini;
mod ssh_config;

pub use ini::{decode_ini, parse_sessions_ini};
pub use ssh_config::{SSH_CONFIG_FOLDER, decode_ssh_config, parse_ssh_config};

use std::collections::HashMap;
use std::path::Path;

use crate::library::{SessionLibrary, normalize_folder};
use crate::model::*;

/// Every imported session goes under this top-level folder.
pub const IMPORT_ROOT: &str = "Imported";

/// An SSH or Telnet session read from an import file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedSession {
    Ssh {
        name: String,
        /// Already placed under [`IMPORT_ROOT`].
        folder: String,
        host: String,
        port: u16,
        username: String,
        /// Password auth (asked at connect time) when absent.
        auth: Option<SavedAuth>,
        /// Names of the imported sessions to go through first, outermost first.
        jumps: Vec<String>,
    },
    Telnet {
        name: String,
        folder: String,
        host: String,
        port: u16,
    },
}

impl ImportedSession {
    pub fn name(&self) -> &str {
        match self {
            Self::Ssh { name, .. } | Self::Telnet { name, .. } => name,
        }
    }
}

/// Skipped lines per session type ("RDP", "type 7") or reason ("invalid", "unrecognized",
/// "include", "match", "proxyCommand", "jump"), in the order first seen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Skipped(Vec<(String, usize)>);

impl Skipped {
    pub fn add(&mut self, reason: &str) {
        match self.0.iter_mut().find(|(key, _)| key == reason) {
            Some((_, count)) => *count += 1,
            None => self.0.push((reason.to_string(), 1)),
        }
    }

    pub fn get(&self, reason: &str) -> usize {
        self.0
            .iter()
            .find(|(key, _)| key == reason)
            .map_or(0, |(_, count)| *count)
    }

    pub fn entries(&self) -> &[(String, usize)] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedImport {
    pub sessions: Vec<ImportedSession>,
    pub skipped: Skipped,
}

/// The preview: what would be saved, how many duplicates are skipped and what was not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    /// SSH and Telnet sessions, saved with one `SaveSessions` action.
    pub to_add: Vec<LibrarySession>,
    pub duplicates: usize,
    pub skipped: Skipped,
}

/// Kind, folder, name, host, port and username: equal ones are duplicates.
type Identity = (SessionKind, String, String, String, u16, String);

fn identity_of(kind: SessionKind, folder: &str, name: &str, host: &str, port: u16, username: &str) -> Identity {
    (
        kind,
        normalize_folder(folder),
        name.into(),
        host.into(),
        port,
        username.into(),
    )
}

fn name_key(folder: &str, name: &str) -> (String, String) {
    (normalize_folder(folder), name.to_string())
}

/// New SSH sessions keep the imported auth, or use password auth (asked at connect time); exact
/// duplicates of saved SSH/Telnet sessions, or of earlier lines, are counted and skipped. Jump
/// names become the ids of the sessions of the same folder (saved before or now); a chain with
/// an unknown name is dropped.
pub fn plan_import(library: &SessionLibrary, parsed: &ParsedImport, mut new_id: impl FnMut() -> String) -> ImportPlan {
    let mut seen: HashMap<Identity, String> = HashMap::new();
    // `new Map(entries)` keeps the last id of equal identities.
    for s in &library.sessions {
        let key = match s {
            LibrarySession::Ssh(s) => identity_of(SessionKind::Ssh, &s.folder, &s.name, &s.host, s.port, &s.username),
            LibrarySession::Telnet(s) => identity_of(SessionKind::Telnet, &s.folder, &s.name, &s.host, s.port, ""),
            _ => continue,
        };
        seen.insert(key, s.id().to_string());
    }

    let mut id_by_name: HashMap<(String, String), String> = HashMap::new();
    let mut to_add = Vec::new();
    // (index in to_add, jump name keys)
    let mut jumps_of: Vec<(usize, Vec<(String, String)>)> = Vec::new();
    let mut duplicates = 0;
    for session in &parsed.sessions {
        let key = match session {
            ImportedSession::Ssh {
                name,
                folder,
                host,
                port,
                username,
                ..
            } => identity_of(SessionKind::Ssh, folder, name, host, *port, username),
            ImportedSession::Telnet {
                name,
                folder,
                host,
                port,
            } => identity_of(SessionKind::Telnet, folder, name, host, *port, ""),
        };
        if let Some(existing) = seen.get(&key) {
            duplicates += 1;
            let (folder, name) = match session {
                ImportedSession::Ssh { folder, name, .. } | ImportedSession::Telnet { folder, name, .. } => {
                    (folder, name)
                }
            };
            id_by_name.insert(name_key(folder, name), existing.clone());
            continue;
        }
        let id = new_id();
        seen.insert(key, id.clone());
        match session {
            ImportedSession::Telnet {
                name,
                folder,
                host,
                port,
            } => {
                id_by_name.insert(name_key(folder, name), id.clone());
                to_add.push(LibrarySession::Telnet(TelnetSession {
                    id,
                    name: name.clone(),
                    folder: folder.clone(),
                    host: host.clone(),
                    port: *port,
                }));
            }
            ImportedSession::Ssh {
                name,
                folder,
                host,
                port,
                username,
                auth,
                jumps,
            } => {
                id_by_name.insert(name_key(folder, name), id.clone());
                if !jumps.is_empty() {
                    jumps_of.push((to_add.len(), jumps.iter().map(|j| name_key(folder, j)).collect()));
                }
                to_add.push(LibrarySession::Ssh(SshSession {
                    id,
                    name: name.clone(),
                    folder: folder.clone(),
                    host: host.clone(),
                    port: *port,
                    username: username.clone(),
                    auth: auth.clone().unwrap_or(SavedAuth::Password),
                    jump_ids: Vec::new(),
                }));
            }
        }
    }
    for (index, names) in jumps_of {
        let ids: Option<Vec<String>> = names.iter().map(|n| id_by_name.get(n).cloned()).collect();
        if let (Some(ids), Some(LibrarySession::Ssh(saved))) = (ids, to_add.get_mut(index)) {
            saved.jump_ids = ids;
        }
    }
    ImportPlan {
        to_add,
        duplicates,
        skipped: parsed.skipped.clone(),
    }
}

/// Readable label for a key of [`Skipped`], as listed under **Not imported:**.
pub fn skipped_label(key: &str) -> String {
    match key {
        "invalid" => "Sessions without a host".into(),
        "unrecognized" => "Unrecognized lines".into(),
        "Serial" => "Serial sessions".into(),
        "include" => "Include lines (not followed)".into(),
        "match" => "Match blocks (not applied)".into(),
        "proxyCommand" => "Hosts with a ProxyCommand (imported without it)".into(),
        "jump" => "Hosts whose ProxyJump is not a host of the file (imported without it)".into(),
        _ => match key.strip_prefix("type ").filter(|n| crate::text::all_digits(n)) {
            Some(n) => format!("type {n} sessions"),
            None => format!("{key} sessions"),
        },
    }
}

/// Reads a picked `.ini` file with the 5 MiB cap and parses its bookmarks.
pub fn read_ini_import(path: &Path) -> Result<ParsedImport, core_storage::StorageError> {
    let bytes = core_storage::read_bounded(path, core_storage::IMPORT_MAX_BYTES)?;
    Ok(parse_sessions_ini(&decode_ini(&bytes)))
}

/// Reads `<home>/.ssh/config` (also on Windows) with the 5 MiB cap and parses its hosts.
pub fn read_ssh_config_import(home: &Path) -> Result<ParsedImport, core_storage::StorageError> {
    let bytes = core_storage::read_bounded(&home.join(".ssh").join("config"), core_storage::IMPORT_MAX_BYTES)?;
    Ok(parse_ssh_config(&decode_ssh_config(&bytes)))
}
