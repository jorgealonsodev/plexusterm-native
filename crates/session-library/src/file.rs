//! `sessions.json` (DATA-1a): validated like v0.2.15's `parseLibrary` and written like its
//! `serializeLibrary` (`JSON.stringify(_, null, 2)`), so a v0.2.15 file round-trips byte for byte.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::library::{SessionLibrary, sanitize};
use crate::model::*;

/// Why `sessions.json` cannot be used; the panel shows it and disables editing (SES-1a, SES-1i).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// The parser's own description follows the v0.2.15 prefix (it differs from the browser's).
    InvalidJson(String),
    UnsupportedVersion,
    InvalidFolders,
    InvalidSessions,
    /// The file exists but could not be read.
    Storage(String),
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "sessions file is not valid JSON: {error}"),
            Self::UnsupportedVersion => f.write_str("unsupported sessions file version"),
            Self::InvalidFolders => f.write_str("sessions file has invalid folders"),
            Self::InvalidSessions => f.write_str("sessions file has invalid sessions"),
            Self::Storage(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for LibraryError {}

/// A JSON number that `Number.isInteger` accepts (22 and 22.0 alike).
pub(crate) fn integer(value: &Value) -> Option<i128> {
    if let Some(n) = value.as_i64() {
        return Some(i128::from(n));
    }
    if let Some(n) = value.as_u64() {
        return Some(i128::from(n));
    }
    let f = value.as_f64()?;
    // Larger floats are integers in JavaScript too, but no field accepts them.
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15 {
        #[allow(clippy::cast_possible_truncation, reason = "checked to be a small whole number")]
        return Some(f as i128);
    }
    None
}

/// `Number.isSafeInteger(v) && v > 0`.
fn positive_safe_integer(value: &Value) -> Option<u64> {
    integer(value)
        .filter(|n| (1..=9_007_199_254_740_991).contains(n))
        .and_then(|n| u64::try_from(n).ok())
}

fn string(entry: &Map<String, Value>, key: &str) -> Option<String> {
    entry.get(key)?.as_str().map(str::to_string)
}

/// Absent is fine (`undefined`), present must be a string (`null` is not).
fn optional_string(entry: &Map<String, Value>, key: &str) -> Result<Option<String>, ()> {
    match entry.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(()),
    }
}

/// Absent is an empty list; present must be a list of strings.
fn optional_strings(entry: &Map<String, Value>, key: &str) -> Result<Vec<String>, ()> {
    match entry.get(key) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string).ok_or(()))
            .collect(),
        Some(_) => Err(()),
    }
}

fn port(value: Option<&Value>, min: i128) -> Option<u16> {
    integer(value?)
        .filter(|n| (min..=65535).contains(n))
        .and_then(|n| u16::try_from(n).ok())
}

fn saved_auth(value: Option<&Value>) -> Option<SavedAuth> {
    let auth = value?.as_object()?;
    match auth.get("method")?.as_str()? {
        "key" => Some(SavedAuth::Key {
            path: string(auth, "path")?,
        }),
        "password" => Some(SavedAuth::Password),
        "agent" => Some(SavedAuth::Agent),
        _ => None,
    }
}

fn one_of<T: Copy>(value: Option<&Value>, choices: &[(&str, T)]) -> Option<T> {
    let text = value?.as_str()?;
    choices.iter().find(|(name, _)| *name == text).map(|(_, v)| *v)
}

fn small_choice(value: Option<&Value>, choices: &[u8]) -> Option<u8> {
    let n = integer(value?)?;
    choices.iter().copied().find(|c| i128::from(*c) == n)
}

/// One entry of `sessions`, validated like v0.2.15's `isLibrarySession`; entries without
/// `kind` are SSH. Fields outside the saved shape (e.g. a password) are dropped.
fn library_session(value: &Value) -> Option<LibrarySession> {
    let entry = value.as_object()?;
    let id = string(entry, "id")?;
    let name = string(entry, "name")?;
    let folder = string(entry, "folder")?;
    let kind = match entry.get("kind") {
        None => "ssh",
        Some(kind) => kind.as_str()?,
    };
    let session = match kind {
        "ssh" => LibrarySession::Ssh(SshSession {
            id,
            name,
            folder,
            host: string(entry, "host")?,
            // v0.2.15 accepts any integer here; ports outside 0..=65535 are refused.
            port: port(entry.get("port"), 0)?,
            username: string(entry, "username")?,
            auth: saved_auth(entry.get("auth"))?,
            jump_ids: optional_strings(entry, "jumpIds").ok()?,
        }),
        "serial" => LibrarySession::Serial(SerialSession {
            id,
            name,
            folder,
            settings: SerialSettings {
                path: string(entry, "path")?,
                baud: positive_safe_integer(entry.get("baud")?)?,
                data_bits: small_choice(entry.get("dataBits"), &[5, 6, 7, 8])?,
                parity: one_of(
                    entry.get("parity"),
                    &[
                        ("none", SerialParity::None),
                        ("odd", SerialParity::Odd),
                        ("even", SerialParity::Even),
                    ],
                )?,
                stop_bits: small_choice(entry.get("stopBits"), &[1, 2])?,
                flow: one_of(
                    entry.get("flow"),
                    &[
                        ("none", SerialFlow::None),
                        ("software", SerialFlow::Software),
                        ("hardware", SerialFlow::Hardware),
                    ],
                )?,
            },
        }),
        "telnet" => LibrarySession::Telnet(TelnetSession {
            id,
            name,
            folder,
            host: string(entry, "host")?,
            port: port(entry.get("port"), 1)?,
        }),
        "mosh" => LibrarySession::Mosh(MoshSession {
            id,
            name,
            folder,
            ssh_session_id: string(entry, "sshSessionId")?,
            server_command: optional_string(entry, "serverCommand").ok()?,
        }),
        "local" => LibrarySession::Local(LocalSession {
            id,
            name,
            folder,
            program: optional_string(entry, "program").ok()?,
            args: optional_strings(entry, "args").ok()?,
            cwd: optional_string(entry, "cwd").ok()?,
        }),
        _ => return None,
    };
    Some(sanitize(&session))
}

/// A missing file (`None`) is an empty library; anything unreadable is an error, never
/// silently emptied (DATA-1a, SES-1i).
pub fn parse_library(json: Option<&str>) -> Result<SessionLibrary, LibraryError> {
    let Some(json) = json else {
        return Ok(SessionLibrary::default());
    };
    let data: Value = serde_json::from_str(json).map_err(|err| LibraryError::InvalidJson(err.to_string()))?;
    if data.get("version").and_then(integer) != Some(1) {
        return Err(LibraryError::UnsupportedVersion);
    }
    let folders = match data.get("folders") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|f| f.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or(LibraryError::InvalidFolders)?,
        _ => return Err(LibraryError::InvalidFolders),
    };
    let sessions = match data.get("sessions") {
        Some(Value::Array(items)) => items
            .iter()
            .map(library_session)
            .collect::<Option<Vec<_>>>()
            .ok_or(LibraryError::InvalidSessions)?,
        _ => return Err(LibraryError::InvalidSessions),
    };
    Ok(SessionLibrary { folders, sessions })
}

#[derive(Serialize)]
struct LibraryFile<'a> {
    version: u8,
    folders: &'a [String],
    sessions: Vec<LibrarySession>,
}

/// The file contents: `{version: 1, folders, sessions}` with two-space indentation, every
/// session in its saved shape and with its `kind`.
pub fn serialize_library(library: &SessionLibrary) -> String {
    let file = LibraryFile {
        version: 1,
        folders: &library.folders,
        sessions: library.sessions.iter().map(sanitize).collect(),
    };
    // Serializing plain strings, numbers and lists into a String cannot fail.
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

/// `<config dir>/sessions.json`.
pub fn library_path(config_dir: &Path) -> PathBuf {
    config_dir.join(core_storage::SESSIONS_FILE)
}

/// Reads and validates `sessions.json`; a missing file is an empty library.
pub fn load_library(path: &Path) -> Result<SessionLibrary, LibraryError> {
    let contents = core_storage::load(path).map_err(|err| LibraryError::Storage(err.to_string()))?;
    parse_library(contents.as_deref())
}

/// Atomically replaces `sessions.json` (private to the user). Callers must not save while the
/// file could not be read, so it is never overwritten (SES-1a).
pub fn save_library(path: &Path, library: &SessionLibrary) -> Result<(), core_storage::StorageError> {
    core_storage::save(path, &serialize_library(library))
}
