//! The open, edit-and-upload and compare actions of the SFTP browser's context menu
//! (SFTP-1a, SFTP-2, SFTP-3), ported from PlexusTerm v0.2.15 `src-tauri/src/sftp/commands.rs`.
//!
//! v0.2.15 ran them as Tauri commands and emitted `sftp-edit-saved` when an edited copy was
//! saved; here each action takes the [`SshManager`] and the [`TempFiles`] it used as state, and
//! [`edit`] calls back with an [`EditSaved`] instead of the event. The app asks once per file
//! before the first re-upload (SFTP-2), then calls [`edit_upload`], and calls [`discard_copies`]
//! when the SSH session closes (SEC-e).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use proto_ssh::SshManager;

use crate::edit::{self, TempFiles};
use crate::{SftpError, download_copy, upload_file};

/// Purpose of the copies opened for editing (see [`TempFiles::local_path`]).
pub const EDIT: &str = "edit";
/// Purpose of the copies opened in a diff tool.
pub const COMPARE: &str = "compare";

/// Which program opens the temporary copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWith {
    /// The system's default program for the file.
    Default,
    /// The text editor chosen in Settings (`command`), or the system's default one.
    TextEditor { command: Option<String> },
    /// A program chosen by the user.
    Program { path: String },
}

/// A temporary copy opened for editing was saved (v0.2.15 event `sftp-edit-saved`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSaved {
    pub id: u32,
    pub remote: String,
    pub local: PathBuf,
}

fn open_failure(local: &Path, err: impl std::fmt::Display) -> SftpError {
    SftpError::Sftp {
        message: format!("cannot open {}: {err}", local.display()),
    }
}

/// The system's default program for `local`; it must exist (as v0.2.15's opener checked).
fn open_default(local: &Path) -> Result<(), SftpError> {
    local
        .metadata()
        .and_then(|_| ::open::that_detached(local))
        .map_err(|err| open_failure(local, err))
}

/// Opens `local` with the program `with` names, without waiting for it.
pub fn launch(with: &OpenWith, local: &Path) -> Result<(), SftpError> {
    match with {
        OpenWith::Default => open_default(local),
        OpenWith::Program { path: program } => {
            ::open::with_detached(local, program.as_str()).map_err(|err| open_failure(local, err))
        }
        // The chosen editor runs directly, never through a shell.
        OpenWith::TextEditor { command: Some(command) } if !command.trim().is_empty() => {
            match edit::chosen_editor_command(command, local) {
                Some(args) => edit::spawn_detached(&args).map_err(|err| open_failure(local, err)),
                None => Err(open_failure(local, "no text editor command")),
            }
        }
        // Without a known text editor, fall back to the default program.
        OpenWith::TextEditor { .. } => match edit::text_editor_command(local) {
            Some(command) => edit::spawn_detached(&command).map_err(|err| open_failure(local, err)),
            None => open_default(local),
        },
    }
}

/// Downloads `remote` to a private temporary copy, opens it and watches it: every save calls
/// `on_save` (every [`edit::WATCH_INTERVAL`]) until the file is opened again or the session's
/// copies are discarded. Returns the local path.
pub async fn edit(
    ssh: &SshManager,
    temp: &TempFiles,
    id: u32,
    remote: &str,
    with: &OpenWith,
    on_save: impl Fn(EditSaved) + Send + 'static,
) -> Result<PathBuf, SftpError> {
    let sftp = ssh.sftp(id).await?;
    let local = download_copy(&sftp, temp, id, EDIT, remote).await?;
    let saved = EditSaved {
        id,
        remote: remote.to_string(),
        local: local.clone(),
    };
    temp.watch(id, EDIT, remote, edit::WATCH_INTERVAL, move || on_save(saved.clone()));
    launch(with, &local)?;
    Ok(local)
}

/// Uploads the edited temporary copy of `remote` back to `remote`.
pub async fn edit_upload(ssh: &SshManager, temp: &TempFiles, id: u32, remote: &str) -> Result<(), SftpError> {
    let local = temp.existing(id, EDIT, remote).ok_or_else(|| SftpError::Sftp {
        message: format!("cannot upload to {remote}: it is not open for editing"),
    })?;
    upload_file(&*ssh.sftp(id).await?, &local, remote).await
}

/// The local file to compare with: an absolute path (as the file dialog gives), so it can never
/// be read by the diff tool as one of its options.
pub fn compare_target(local: &str) -> Result<PathBuf, SftpError> {
    let path = PathBuf::from(local);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(SftpError::Sftp {
            message: format!("cannot compare with \"{local}\": choose a local file"),
        })
    }
}

/// Opens a temporary copy of `remote` and the local file `local` in the first installed diff
/// tool on the `PATH` (Meld, KDiff3, Kompare, then `code --diff`).
pub async fn compare(ssh: &SshManager, temp: &TempFiles, id: u32, remote: &str, local: &str) -> Result<(), SftpError> {
    compare_using(ssh, temp, id, remote, local, std::env::var_os("PATH")).await
}

/// [`compare`] looking for the diff tool in the `PATH`-style list `path`.
pub async fn compare_using(
    ssh: &SshManager,
    temp: &TempFiles,
    id: u32,
    remote: &str,
    local: &str,
    path: Option<OsString>,
) -> Result<(), SftpError> {
    let local = compare_target(local)?;
    let (tool, args) = edit::find_diff_tool(path).ok_or_else(|| SftpError::Sftp {
        message: "No diff tool found: install Meld, KDiff3, Kompare or Visual Studio Code.".into(),
    })?;
    let copy = download_copy(&*ssh.sftp(id).await?, temp, id, COMPARE, remote).await?;
    let mut command: Vec<OsString> = vec![tool.into_os_string()];
    command.extend(args.iter().map(OsString::from));
    command.push(copy.clone().into_os_string());
    command.push(local.into_os_string());
    edit::spawn_detached(&command).map_err(|err| open_failure(&copy, err))
}

/// Deletes the temporary copies of session `id` (call when the SSH session ends).
pub fn discard_copies(temp: &TempFiles, id: u32) {
    temp.remove(id);
}
