//! Session edit form (v0.2.15 `sessionForm.ts`, SES-1e, SES-1h): validation with the exact
//! v0.2.15 messages, default names, and the form of a saved session.

mod quick_connect;
mod serial;
mod telnet;

pub use quick_connect::{AuthMethod, QuickConnectForm, parse_quick_connect};
pub use serial::{COMMON_BAUD_RATES, CUSTOM_BAUD, SerialForm, form_from_settings, parse_serial_form};
pub use telnet::{TelnetForm, parse_telnet_form};

use proto_ssh::SshAuth;

use crate::library::{ChainError, normalize_folder, resolve_chain};
use crate::model::*;
use crate::shell::{join_args, shell_name_of, split_args};
use crate::target::LOCAL_SHELL_TITLE;
use crate::text::js_trim;

/// What the backend runs on the server when a Mosh session does not override it.
pub const DEFAULT_SERVER_COMMAND: &str = "mosh-server new -s -c 256 -l LANG=en_US.UTF-8";
/// Key path offered when a session has none yet.
pub const DEFAULT_KEY_PATH: &str = "~/.ssh/id_ed25519";

/// A validation error shown under the fields (SES-1h, SSH-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    HostRequired,
    UsernameRequired,
    PortRange,
    KeyPathRequired,
    MoshSessionRequired,
    TooManyJumps {
        max: usize,
    },
    JumpTwice,
    OwnJump,
    SerialPortRequired,
    BaudInvalid,
    DataBitsInvalid,
    StopBitsInvalid,
    /// The chain as it would be once saved is not usable.
    Chain(ChainError),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostRequired => f.write_str("Host is required"),
            Self::UsernameRequired => f.write_str("Username is required"),
            Self::PortRange => f.write_str("Port must be a number between 1 and 65535"),
            Self::KeyPathRequired => f.write_str("Private key path is required"),
            Self::MoshSessionRequired => f.write_str("Choose the saved SSH session Mosh logs in with"),
            Self::TooManyJumps { max } => write!(f, "At most {max} jump hosts are allowed"),
            Self::JumpTwice => f.write_str("A jump host is listed twice"),
            Self::OwnJump => f.write_str("A session cannot be its own jump host"),
            Self::SerialPortRequired => f.write_str("Serial port is required"),
            Self::BaudInvalid => f.write_str("Baud rate must be a positive whole number"),
            Self::DataBitsInvalid => f.write_str("Data bits must be 5, 6, 7 or 8"),
            Self::StopBitsInvalid => f.write_str("Stop bits must be 1 or 2"),
            Self::Chain(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FormError {}

/// Mosh fields as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoshForm {
    /// Saved SSH session whose login starts the server.
    pub ssh_session_id: String,
    /// Empty means [`DEFAULT_SERVER_COMMAND`].
    pub server_command: String,
}

/// A local terminal's shell ("" = system default), arguments as typed and starting directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalForm {
    pub program: String,
    pub args: String,
    pub cwd: String,
}

/// The edit dialog's fields; only those of `kind` are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionForm {
    pub kind: SessionKind,
    /// Empty means the default name of the kind (SES-1e).
    pub name: String,
    pub folder: String,
    pub ssh: QuickConnectForm,
    pub serial: SerialForm,
    pub telnet: TelnetForm,
    pub mosh: MoshForm,
    pub local: LocalForm,
    /// SSH only: ids of the saved sessions to go through, outermost first.
    pub jump_ids: Vec<String>,
}

fn empty_ssh() -> QuickConnectForm {
    QuickConnectForm {
        host: String::new(),
        port: String::new(),
        username: String::new(),
        method: AuthMethod::Password,
        password: String::new(),
        key_path: DEFAULT_KEY_PATH.into(),
        passphrase: String::new(),
    }
}

impl SessionForm {
    /// The form of a new session of `kind` in the root folder.
    pub fn new(kind: SessionKind) -> Self {
        Self {
            kind,
            name: String::new(),
            folder: String::new(),
            ssh: empty_ssh(),
            serial: SerialForm::default(),
            telnet: TelnetForm::default(),
            mosh: MoshForm::default(),
            local: LocalForm::default(),
            jump_ids: Vec::new(),
        }
    }
}

/// Validates the edit form into the session to save under `id`; secrets (password,
/// passphrase) are never kept. `library`, when given, lets Mosh check its SSH session and lets
/// the whole jump chain be checked (cycles through other sessions).
pub fn parse_session_form(
    form: &SessionForm,
    id: &str,
    library: Option<&[LibrarySession]>,
) -> Result<LibrarySession, FormError> {
    let folder = normalize_folder(&form.folder);
    let name = js_trim(&form.name);
    let named = |default: String| if name.is_empty() { default } else { name.to_string() };
    match form.kind {
        SessionKind::Serial => {
            let settings = parse_serial_form(&form.serial)?;
            Ok(SerialSession {
                id: id.into(),
                name: named(settings.path.clone()),
                folder,
                settings,
            }
            .into())
        }
        SessionKind::Telnet => {
            let (host, port) = parse_telnet_form(&form.telnet)?;
            Ok(TelnetSession {
                id: id.into(),
                name: named(host.clone()),
                folder,
                host,
                port,
            }
            .into())
        }
        SessionKind::Local => {
            let program = js_trim(&form.local.program);
            let args = if program.is_empty() {
                Vec::new()
            } else {
                split_args(&form.local.args)
            };
            let cwd = js_trim(&form.local.cwd);
            let default = if program.is_empty() {
                LOCAL_SHELL_TITLE.to_string()
            } else {
                shell_name_of(program)
            };
            Ok(LocalSession {
                id: id.into(),
                name: named(default),
                folder,
                program: (!program.is_empty()).then(|| program.to_string()),
                args,
                cwd: (!cwd.is_empty()).then(|| cwd.to_string()),
            }
            .into())
        }
        SessionKind::Mosh => {
            let ssh_session_id = form.mosh.ssh_session_id.clone();
            let ssh = library.and_then(|sessions| sessions.iter().find(|s| s.id() == ssh_session_id));
            if ssh_session_id.is_empty() || (library.is_some() && !ssh.is_some_and(LibrarySession::is_ssh)) {
                return Err(FormError::MoshSessionRequired);
            }
            let server_command = js_trim(&form.mosh.server_command);
            let default = ssh.map_or_else(|| "Mosh".to_string(), |s| format!("{} (mosh)", s.name()));
            Ok(MoshSession {
                id: id.into(),
                name: named(default),
                folder,
                ssh_session_id,
                server_command: (!server_command.is_empty()).then(|| server_command.to_string()),
            }
            .into())
        }
        SessionKind::Ssh => {
            let target = parse_quick_connect(&form.ssh)?;
            let jump_ids = &form.jump_ids;
            if jump_ids.len() > MAX_JUMPS {
                return Err(FormError::TooManyJumps { max: MAX_JUMPS });
            }
            if jump_ids
                .iter()
                .enumerate()
                .any(|(i, a)| jump_ids.iter().skip(i + 1).any(|b| a == b))
            {
                return Err(FormError::JumpTwice);
            }
            if jump_ids.iter().any(|j| j == id) {
                return Err(FormError::OwnJump);
            }
            let auth = match target.auth {
                SshAuth::Key { path, .. } => SavedAuth::Key { path },
                SshAuth::Agent => SavedAuth::Agent,
                SshAuth::Password { .. } | SshAuth::StoredPassword { .. } => SavedAuth::Password,
            };
            let session = SshSession {
                id: id.into(),
                name: named(format!("{}@{}", target.username, target.host)),
                folder,
                host: target.host,
                port: target.port,
                username: target.username,
                auth,
                jump_ids: jump_ids.clone(),
            };
            if let Some(sessions) = library.filter(|_| !jump_ids.is_empty()) {
                // Check the chain as it will be once saved, so loops through other sessions are caught now.
                let mut saved: Vec<LibrarySession> = sessions.iter().filter(|s| s.id() != id).cloned().collect();
                saved.push(session.clone().into());
                resolve_chain(&saved, &session).map_err(FormError::Chain)?;
            }
            Ok(session.into())
        }
    }
}

/// The edit form of a saved session. A name that was made from the address (left empty, such
/// as `user@host`) stays empty, so it follows a changed host instead of keeping the old address.
pub fn form_from_session(session: &LibrarySession, library: Option<&[LibrarySession]>) -> SessionForm {
    let form = form_with_name(session);
    let unnamed = SessionForm {
        name: String::new(),
        ..form.clone()
    };
    match parse_session_form(&unnamed, session.id(), library) {
        Ok(automatic) if automatic.name() == session.name() => unnamed,
        _ => form,
    }
}

fn form_with_name(session: &LibrarySession) -> SessionForm {
    let mut form = SessionForm::new(session.kind());
    form.name = session.name().to_string();
    form.folder = session.folder().to_string();
    match session {
        LibrarySession::Local(s) => {
            form.local = LocalForm {
                program: s.program.clone().unwrap_or_default(),
                args: join_args(&s.args),
                cwd: s.cwd.clone().unwrap_or_default(),
            };
        }
        LibrarySession::Serial(s) => form.serial = form_from_settings(&s.settings),
        LibrarySession::Telnet(s) => {
            form.telnet = TelnetForm {
                host: s.host.clone(),
                port: if s.port == 23 {
                    String::new()
                } else {
                    s.port.to_string()
                },
            };
        }
        LibrarySession::Mosh(s) => {
            form.mosh = MoshForm {
                ssh_session_id: s.ssh_session_id.clone(),
                server_command: s.server_command.clone().unwrap_or_default(),
            };
        }
        LibrarySession::Ssh(s) => {
            form.ssh = QuickConnectForm {
                host: s.host.clone(),
                port: if s.port == 22 {
                    String::new()
                } else {
                    s.port.to_string()
                },
                username: s.username.clone(),
                method: match s.auth {
                    SavedAuth::Password => AuthMethod::Password,
                    SavedAuth::Key { .. } => AuthMethod::Key,
                    SavedAuth::Agent => AuthMethod::Agent,
                },
                password: String::new(),
                key_path: match &s.auth {
                    SavedAuth::Key { path } => path.clone(),
                    _ => DEFAULT_KEY_PATH.into(),
                },
                passphrase: String::new(),
            };
            form.jump_ids = s.jump_ids.clone();
        }
    }
    form
}
