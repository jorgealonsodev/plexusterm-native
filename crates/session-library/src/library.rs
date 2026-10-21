//! The library and its actions (v0.2.15 `library.ts`): folders as paths, save, duplicate,
//! delete (with jump host cleanup), jump chains, search tree and the `ssh` command line.

use std::collections::HashSet;

use crate::model::*;

/// `sessions.json` in memory: `{version: 1, folders, sessions}` (the version is implied).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionLibrary {
    /// Slash-separated folder paths, in the order they were added.
    pub folders: Vec<String>,
    pub sessions: Vec<LibrarySession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryAction {
    /// Adds the session, or replaces the one with its id, and registers its folder.
    SaveSession(LibrarySession),
    /// Saves several sessions with a single write (e.g. an import).
    SaveSessions(Vec<LibrarySession>),
    /// Deletes the session and removes it from every chain of jump hosts.
    DeleteSession {
        id: String,
    },
    /// Copies a session right after it, named "<name> (copy)"; saved passwords are not copied.
    DuplicateSession {
        id: String,
        new_id: String,
    },
    AddFolder {
        path: String,
    },
    /// Deletes the folder, its subfolders and every session inside them.
    DeleteFolder {
        path: String,
    },
}

/// Why a chain of jump hosts cannot be used (SSH-3), with the v0.2.15 messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    OwnJump { name: String },
    Cycle { name: String },
    Missing { name: String },
    NotSsh { jump: String, name: String },
    Repeated { jump: String, name: String },
    TooMany { name: String, max: usize },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnJump { name } => write!(f, "\"{name}\" cannot be its own jump host."),
            Self::Cycle { name } => write!(f, "The jump hosts of \"{name}\" form a cycle."),
            Self::Missing { name } => {
                write!(
                    f,
                    "A jump host of \"{name}\" no longer exists; edit the session to choose another one."
                )
            }
            Self::NotSsh { jump, name } => write!(f, "Jump host \"{jump}\" of \"{name}\" is not an SSH session."),
            Self::Repeated { jump, name } => {
                write!(f, "\"{jump}\" appears more than once in the jump hosts of \"{name}\".")
            }
            Self::TooMany { name, max } => write!(f, "\"{name}\" goes through more than {max} jump hosts."),
        }
    }
}

impl std::error::Error for ChainError {}

/// A row of the sessions tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode<'a> {
    Folder {
        name: String,
        path: String,
        children: Vec<TreeNode<'a>>,
    },
    Session {
        name: String,
        session: &'a LibrarySession,
    },
}

impl TreeNode<'_> {
    pub fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } | Self::Session { name, .. } => name,
        }
    }
}

/// Trims every segment and drops empty ones: `" Prod / / DB/ "` is `"Prod/DB"`.
pub fn normalize_folder(path: &str) -> String {
    path.split('/')
        .map(crate::text::js_trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// `"A/B/C"` -> `["A", "A/B", "A/B/C"]`.
fn with_ancestors(path: &str) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = path.split('/').collect();
    (1..=parts.len())
        .map(|n| parts.iter().take(n).copied().collect::<Vec<_>>().join("/"))
        .collect()
}

fn is_inside(path: &str, folder: &str) -> bool {
    path == folder || path.strip_prefix(folder).is_some_and(|rest| rest.starts_with('/'))
}

fn add_folders(folders: &[String], path: &str) -> Vec<String> {
    let mut next = folders.to_vec();
    for folder in with_ancestors(path) {
        if !next.contains(&folder) {
            next.push(folder);
        }
    }
    next
}

/// Keeps only the saved shape of a session: a normalized folder, trimmed optional fields
/// (absent when blank) and local arguments only with a program, as v0.2.15's `sanitizeSession`.
pub(crate) fn sanitize(session: &LibrarySession) -> LibrarySession {
    let trimmed = |value: &Option<String>| {
        value
            .as_deref()
            .map(crate::text::js_trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let mut clean = session.clone();
    match &mut clean {
        LibrarySession::Ssh(s) => s.folder = normalize_folder(&s.folder),
        LibrarySession::Serial(s) => s.folder = normalize_folder(&s.folder),
        LibrarySession::Telnet(s) => s.folder = normalize_folder(&s.folder),
        LibrarySession::Mosh(s) => {
            s.folder = normalize_folder(&s.folder);
            s.server_command = trimmed(&s.server_command);
        }
        LibrarySession::Local(s) => {
            s.folder = normalize_folder(&s.folder);
            s.program = trimmed(&s.program);
            s.cwd = trimmed(&s.cwd);
            if s.program.is_none() {
                s.args.clear();
            }
        }
    }
    clean
}

/// Removes the sessions `remove` matches, and removes them from every chain of jump hosts.
fn remove_sessions(sessions: &[LibrarySession], remove: impl Fn(&LibrarySession) -> bool) -> Vec<LibrarySession> {
    let ids: HashSet<&str> = sessions.iter().filter(|s| remove(s)).map(LibrarySession::id).collect();
    sessions
        .iter()
        .filter(|s| !ids.contains(s.id()))
        .map(|s| match s {
            LibrarySession::Ssh(ssh) if ssh.jump_ids.iter().any(|id| ids.contains(id.as_str())) => {
                let mut kept = ssh.clone();
                kept.jump_ids.retain(|id| !ids.contains(id.as_str()));
                sanitize(&kept.into())
            }
            other => other.clone(),
        })
        .collect()
}

impl SessionLibrary {
    pub fn find(&self, id: &str) -> Option<&LibrarySession> {
        self.sessions.iter().find(|s| s.id() == id)
    }

    /// The library after `action` (v0.2.15 `libraryReducer`); `self` is left unchanged.
    pub fn reduce(&self, action: &LibraryAction) -> SessionLibrary {
        match action {
            LibraryAction::SaveSession(session) => {
                let session = sanitize(session);
                let folders = add_folders(&self.folders, session.folder());
                let mut sessions = self.sessions.clone();
                match sessions.iter_mut().find(|s| s.id() == session.id()) {
                    Some(slot) => *slot = session,
                    None => sessions.push(session),
                }
                SessionLibrary { folders, sessions }
            }
            LibraryAction::SaveSessions(batch) => batch.iter().fold(self.clone(), |acc, session| {
                acc.reduce(&LibraryAction::SaveSession(session.clone()))
            }),
            LibraryAction::DuplicateSession { id, new_id } => {
                let Some(index) = self.sessions.iter().position(|s| s.id() == id) else {
                    return self.clone();
                };
                let mut sessions = self.sessions.clone();
                let Some(original) = sessions.get(index) else {
                    return self.clone();
                };
                let mut copy = original.clone();
                copy.set_id_and_name(new_id.clone(), format!("{} (copy)", original.name()));
                sessions.insert(index + 1, copy);
                SessionLibrary {
                    folders: self.folders.clone(),
                    sessions,
                }
            }
            LibraryAction::DeleteSession { id } => SessionLibrary {
                folders: self.folders.clone(),
                sessions: remove_sessions(&self.sessions, |s| s.id() == id),
            },
            LibraryAction::AddFolder { path } => {
                let path = normalize_folder(path);
                if path.is_empty() {
                    return self.clone();
                }
                SessionLibrary {
                    folders: add_folders(&self.folders, &path),
                    sessions: self.sessions.clone(),
                }
            }
            LibraryAction::DeleteFolder { path } => {
                let path = normalize_folder(path);
                if path.is_empty() {
                    return self.clone();
                }
                SessionLibrary {
                    folders: self.folders.iter().filter(|f| !is_inside(f, &path)).cloned().collect(),
                    sessions: remove_sessions(&self.sessions, |s| is_inside(s.folder(), &path)),
                }
            }
        }
    }
}

/// Saved SSH sessions, other than those in `ids`, that go through one of `ids` as a jump host.
pub fn jump_users<'a, S: AsRef<str>>(library: &'a SessionLibrary, ids: &[S]) -> Vec<&'a SshSession> {
    let deleted: HashSet<&str> = ids.iter().map(AsRef::as_ref).collect();
    library
        .sessions
        .iter()
        .filter_map(LibrarySession::as_ssh)
        .filter(|s| !deleted.contains(s.id.as_str()) && s.jump_ids.iter().any(|id| deleted.contains(id.as_str())))
        .collect()
}

/// The saved SSH sessions `session` goes through, outermost first. A jump host's own jump
/// hosts come before it (like OpenSSH's ProxyJump). Missing or non-SSH jump sessions, cycles,
/// repeats and more than [`MAX_JUMPS`] hops are errors (SSH-3).
pub fn resolve_chain<'a>(
    sessions: &'a [LibrarySession],
    session: &SshSession,
) -> Result<Vec<&'a SshSession>, ChainError> {
    fn visit<'a>(
        sessions: &'a [LibrarySession],
        target: &SshSession,
        current: &SshSession,
        path: &mut Vec<String>,
        jumps: &mut Vec<&'a SshSession>,
    ) -> Result<(), ChainError> {
        for id in &current.jump_ids {
            if *id == current.id {
                return Err(ChainError::OwnJump {
                    name: current.name.clone(),
                });
            }
            if path.contains(id) {
                return Err(ChainError::Cycle {
                    name: target.name.clone(),
                });
            }
            let Some(jump) = sessions.iter().find(|s| s.id() == id) else {
                return Err(ChainError::Missing {
                    name: current.name.clone(),
                });
            };
            let Some(jump) = jump.as_ssh() else {
                return Err(ChainError::NotSsh {
                    jump: jump.name().to_string(),
                    name: current.name.clone(),
                });
            };
            path.push(id.clone());
            visit(sessions, target, jump, path, jumps)?;
            path.pop();
            if jumps.iter().any(|s| s.id == *id) {
                return Err(ChainError::Repeated {
                    jump: jump.name.clone(),
                    name: target.name.clone(),
                });
            }
            jumps.push(jump);
        }
        Ok(())
    }

    let mut jumps = Vec::new();
    visit(sessions, session, session, &mut vec![session.id.clone()], &mut jumps)?;
    if jumps.len() > MAX_JUMPS {
        return Err(ChainError::TooMany {
            name: session.name.clone(),
            max: MAX_JUMPS,
        });
    }
    Ok(jumps)
}

/// Case-insensitive name order (v0.2.15 uses `localeCompare` with base sensitivity; accents
/// and punctuation are ordered by code point here). Equal names keep their order.
fn by_name(a: &TreeNode<'_>, b: &TreeNode<'_>) -> std::cmp::Ordering {
    a.name().to_lowercase().cmp(&b.name().to_lowercase())
}

/// Folders first, then sessions, both sorted by name; a filter keeps the sessions whose name or
/// address (host, serial device, shell) contains it, and the folders leading to them (SES-1, SES-1g).
pub fn build_tree<'a>(library: &'a SessionLibrary, filter: &str) -> Vec<TreeNode<'a>> {
    let needle = crate::text::js_trim(filter).to_lowercase();
    let matches = |s: &LibrarySession| {
        needle.is_empty() || s.name().to_lowercase().contains(&needle) || s.address().to_lowercase().contains(&needle)
    };

    fn parent_of(folder: &str) -> &str {
        folder.rfind('/').and_then(|i| folder.get(..i)).unwrap_or("")
    }

    fn build<'a>(
        library: &'a SessionLibrary,
        parent: &str,
        needle: &str,
        matches: &dyn Fn(&LibrarySession) -> bool,
    ) -> Vec<TreeNode<'a>> {
        let mut folders: Vec<TreeNode<'a>> = library
            .folders
            .iter()
            .filter(|f| parent_of(f) == parent)
            .map(|path| TreeNode::Folder {
                name: path
                    .rfind('/')
                    .and_then(|i| path.get(i + 1..))
                    .unwrap_or(path)
                    .to_string(),
                path: path.clone(),
                children: build(library, path, needle, matches),
            })
            .filter(|node| {
                needle.is_empty() || matches!(node, TreeNode::Folder { children, .. } if !children.is_empty())
            })
            .collect();
        folders.sort_by(by_name);
        let mut sessions: Vec<TreeNode<'a>> = library
            .sessions
            .iter()
            .filter(|s| s.folder() == parent && matches(s))
            .map(|session| TreeNode::Session {
                name: session.name().to_string(),
                session,
            })
            .collect();
        sessions.sort_by(by_name);
        folders.extend(sessions);
        folders
    }

    build(library, "", &needle, &matches)
}

/// A word for a POSIX shell: as is when it is plain, else single-quoted.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./~-".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The `ssh` command that opens a saved SSH session: its port, its own key if it has one,
/// `user@host`; no jump hosts (SES-1c).
pub fn ssh_command_of(session: &SshSession) -> String {
    let key = match &session.auth {
        SavedAuth::Key { path } => format!(" -i {}", shell_word(path)),
        _ => String::new(),
    };
    format!(
        "ssh -p {}{key} {}",
        session.port,
        shell_word(&format!("{}@{}", session.username, session.host))
    )
}
