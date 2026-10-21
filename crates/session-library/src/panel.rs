//! Sessions panel semantics (v0.2.15 `SessionsPanel.tsx`, `App.tsx`; SES-1a, SES-1g, SES-1i):
//! the context menus, row tooltips and the delete / new folder confirmations.

use crate::library::{LibraryAction, SessionLibrary, TreeNode, jump_users};
use crate::model::LibrarySession;
use crate::password_flow::removed_ssh_sessions;

/// An entry of a session or folder context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Connect,
    /// SSH sessions only (SES-1c).
    CopySshCommand,
    Edit,
    Duplicate,
    /// Danger.
    Delete,
    NewSessionHere,
    NewFolderHere,
    /// Opens every session shown under the folder, each in its own tab; disabled at 0.
    ConnectAll {
        count: usize,
    },
    /// Danger.
    DeleteFolder,
    Separator,
}

impl MenuItem {
    pub fn label(&self) -> String {
        match self {
            Self::Connect => "Connect".into(),
            Self::CopySshCommand => "Copy SSH command".into(),
            Self::Edit => "Edit…".into(),
            Self::Duplicate => "Duplicate".into(),
            Self::Delete => "Delete".into(),
            Self::NewSessionHere => "New session here…".into(),
            Self::NewFolderHere => "New folder here…".into(),
            Self::ConnectAll { count } => format!("Connect all ({count})"),
            Self::DeleteFolder => "Delete folder".into(),
            Self::Separator => String::new(),
        }
    }

    pub fn is_danger(&self) -> bool {
        matches!(self, Self::Delete | Self::DeleteFolder)
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::ConnectAll { count: 0 })
    }
}

/// Connect, Copy SSH command (SSH only), Edit…, Duplicate, separator, Delete; while the file
/// cannot be read only Connect and Copy SSH command.
pub fn session_menu(session: &LibrarySession, editable: bool) -> Vec<MenuItem> {
    let mut items = vec![MenuItem::Connect];
    if session.is_ssh() {
        items.push(MenuItem::CopySshCommand);
    }
    if editable {
        items.extend([
            MenuItem::Edit,
            MenuItem::Duplicate,
            MenuItem::Separator,
            MenuItem::Delete,
        ]);
    }
    items
}

/// New session here…, New folder here…, Connect all ({count}), separator, Delete folder (the
/// v0.2.15 order); while the file cannot be read only Connect all. `tree` is the tree as shown
/// (filtered), so the count is what the folder shows.
pub fn folder_menu(tree: &[TreeNode<'_>], path: &str, editable: bool) -> Vec<MenuItem> {
    let count = find_folder(tree, path).map_or(0, |folder| sessions_under(folder).len());
    let connect_all = MenuItem::ConnectAll { count };
    if !editable {
        return vec![connect_all];
    }
    vec![
        MenuItem::NewSessionHere,
        MenuItem::NewFolderHere,
        connect_all,
        MenuItem::Separator,
        MenuItem::DeleteFolder,
    ]
}

/// The folder node at `path`, searching the whole tree.
pub fn find_folder<'t, 'a>(nodes: &'t [TreeNode<'a>], path: &str) -> Option<&'t TreeNode<'a>> {
    nodes.iter().find_map(|node| match node {
        TreeNode::Folder { path: p, .. } if p == path => Some(node),
        TreeNode::Folder { children, .. } => find_folder(children, path),
        TreeNode::Session { .. } => None,
    })
}

/// Every session in a node and its subfolders, in tree order.
pub fn sessions_under<'a>(node: &TreeNode<'a>) -> Vec<&'a LibrarySession> {
    match node {
        TreeNode::Session { session, .. } => vec![*session],
        TreeNode::Folder { children, .. } => children.iter().flat_map(sessions_under).collect(),
    }
}

/// Tooltip of a session row: where it connects (SES-1g).
pub fn session_tooltip(session: &LibrarySession, library: &SessionLibrary) -> String {
    match session {
        LibrarySession::Serial(s) => format!("{} @ {}", s.settings.path, s.settings.baud),
        LibrarySession::Telnet(s) => format!("telnet {}:{}", s.host, s.port),
        LibrarySession::Local(s) => {
            let shell = s.program.as_deref().unwrap_or("System default shell");
            match s.cwd.as_deref() {
                Some(dir) if !dir.is_empty() => format!("{shell} in {dir}"),
                _ => shell.to_string(),
            }
        }
        LibrarySession::Mosh(s) => match library.find(&s.ssh_session_id) {
            Some(ssh) => format!("mosh, logging in with \"{}\"", ssh.name()),
            None => "mosh (its SSH session no longer exists)".into(),
        },
        LibrarySession::Ssh(s) => format!("{}@{}:{}", s.username, s.host, s.port),
    }
}

pub fn delete_session_question(name: &str) -> String {
    format!("Delete session \"{name}\"?")
}

pub fn delete_folder_question(path: &str) -> String {
    format!("Delete folder \"{path}\" and every session inside it?")
}

/// The confirmation for `action`: `question`, plus a note naming the sessions that use the
/// deleted ones as jump hosts (they are removed from those chains).
pub fn delete_confirmation(question: &str, library: &SessionLibrary, action: &LibraryAction) -> String {
    let ids: Vec<&str> = removed_ssh_sessions(library, action)
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    let users: Vec<String> = jump_users(library, &ids)
        .iter()
        .map(|s| format!("\"{}\"", s.name))
        .collect();
    if users.is_empty() {
        question.to_string()
    } else {
        format!(
            "{question}\n\nUsed as a jump host by {}; it will be removed from their jump hosts.",
            users.join(", ")
        )
    }
}

/// Message of the new folder prompt (confirm button **Create**).
pub fn new_folder_prompt(parent: &str) -> String {
    if parent.is_empty() {
        "New folder".into()
    } else {
        format!("New folder inside \"{parent}\"")
    }
}

/// The action for a typed folder name; nothing when it is blank. The path is normalized and
/// its missing ancestors are created when the action is applied.
pub fn new_folder_action(parent: &str, typed: &str) -> Option<LibraryAction> {
    if crate::text::js_trim(typed).is_empty() {
        return None;
    }
    let path = if parent.is_empty() {
        typed.to_string()
    } else {
        format!("{parent}/{typed}")
    };
    Some(LibraryAction::AddFolder { path })
}
