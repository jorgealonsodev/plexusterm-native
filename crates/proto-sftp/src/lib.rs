//! File operations over the SFTP subsystem of an SSH session, ported from PlexusTerm v0.2.15
//! `src-tauri/src/sftp/mod.rs` (PRD SFTP-1..SFTP-3) without any UI types.
//!
//! Remote paths are POSIX strings as sent on the wire; transfers are streamed in chunks so large
//! files never sit in memory. The functions take the session's [`SftpSession`], which
//! [`proto_ssh::SshManager::sftp`] opens once per SSH session. [`edit`] keeps the temporary copies
//! opened in desktop programs and [`open`] the open/edit/compare actions of the browser's menus;
//! [`text`] holds the text limits and encodings the file viewer shares with the Local Explorer.

pub mod edit;
pub mod open;
pub mod text;

use std::path::{Path, PathBuf};

use proto_ssh::SshError;
use russh_sftp::client::SftpSession;

#[derive(Debug, thiserror::Error)]
pub enum SftpError {
    #[error(transparent)]
    Ssh(#[from] SshError),
    #[error("{message}")]
    Sftp { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Unix seconds.
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
    /// Owner ids (SFTP v3) and names (only when the server sends them).
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub user: Option<String>,
    pub group: Option<String>,
}

/// Read/write buffer for transfers; large enough to keep many SFTP requests in flight.
pub const TRANSFER_CHUNK: usize = 256 * 1024;

/// Suffix of the file a download writes before it is renamed to its target (SFTP-1b).
pub const PARTIAL_SUFFIX: &str = ".plexusterm-partial";

const S_IFMT: u32 = 0o170000;
const S_IFDIR: u32 = 0o040000;
const S_IFLNK: u32 = 0o120000;

/// Builds an error message with the operation and path, e.g. `cannot list /x: No such file`.
fn failure(action: &str, path: &str, err: impl Into<SftpCause>) -> SftpError {
    SftpError::Sftp {
        message: format!("cannot {action} {path}: {}", err.into().0),
    }
}

/// Human-readable cause of a failed SFTP or local I/O operation.
struct SftpCause(String);

impl From<russh_sftp::client::error::Error> for SftpCause {
    fn from(err: russh_sftp::client::error::Error) -> Self {
        use russh_sftp::client::error::Error;
        Self(match err {
            Error::Status(status) if !status.error_message.is_empty() => status.error_message,
            Error::Status(status) => status.status_code.to_string(),
            other => other.to_string(),
        })
    }
}

impl From<std::io::Error> for SftpCause {
    fn from(err: std::io::Error) -> Self {
        Self(err.to_string())
    }
}

/// Joins a POSIX directory and a name without doubling the separator.
fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn kind_of(permissions: Option<u32>) -> EntryKind {
    match permissions.unwrap_or(0) & S_IFMT {
        S_IFDIR => EntryKind::Dir,
        S_IFLNK => EntryKind::Symlink,
        _ => EntryKind::File,
    }
}

/// The remote working directory the server starts in (usually the user's home).
pub async fn home(sftp: &SftpSession) -> Result<String, SftpError> {
    sftp.canonicalize(".")
        .await
        .map_err(|err| failure("resolve", "the home folder", err))
}

/// Lists `path` without `.`/`..`: directories first, then by case-insensitive name.
pub async fn list(sftp: &SftpSession, path: &str) -> Result<Vec<Entry>, SftpError> {
    let dir = sftp.read_dir(path).await.map_err(|err| failure("list", path, err))?;
    let mut entries: Vec<Entry> = dir
        .filter(|entry| !matches!(entry.file_name().as_str(), "." | ".."))
        .map(|entry| {
            let meta = entry.metadata();
            let name = entry.file_name();
            Entry {
                path: join(path, &name),
                name,
                kind: kind_of(meta.permissions),
                size: meta.size.unwrap_or(0),
                modified: meta.mtime.map(u64::from),
                permissions: meta.permissions.map(|p| p & !S_IFMT),
                uid: meta.uid,
                gid: meta.gid,
                user: meta.user.clone(),
                group: meta.group.clone(),
            }
        })
        .collect();
    entries.sort_by_cached_key(|entry| {
        (
            entry.kind != EntryKind::Dir,
            entry.name.to_lowercase(),
            entry.name.clone(),
        )
    });
    Ok(entries)
}

/// The first [`text::MAX_ATTACH_BYTES`] of a remote text file, or a folder's listing, to attach
/// to an AI question.
pub async fn read_text(sftp: &SftpSession, path: &str) -> Result<text::TextFile, SftpError> {
    let meta = sftp.metadata(path).await.map_err(|err| failure("read", path, err))?;
    if meta.is_dir() {
        return Ok(text::listing_text(path, &list(sftp, path).await?));
    }
    let (bytes, truncated) = read_prefix(sftp, path, text::MAX_ATTACH_BYTES).await?;
    text::text_from(path, bytes, truncated).map_err(|message| SftpError::Sftp { message })
}

/// The first [`text::MAX_VIEW_BYTES`] of a remote text file, for the file viewer.
/// `encoding` is an encoding id; `None` detects a BOM and falls back to UTF-8.
pub async fn view_text(sftp: &SftpSession, path: &str, encoding: Option<&str>) -> Result<text::TextFile, SftpError> {
    let meta = sftp.metadata(path).await.map_err(|err| failure("read", path, err))?;
    if meta.is_dir() {
        return Err(SftpError::Sftp {
            message: format!("cannot view {path}: it is a folder"),
        });
    }
    let (bytes, truncated) = read_prefix(sftp, path, text::MAX_VIEW_BYTES).await?;
    text::decode_text("view", path, bytes, truncated, encoding).map_err(|message| SftpError::Sftp { message })
}

/// The bytes of a remote image a Markdown file shows, up to [`text::MAX_IMAGE_BYTES`].
pub async fn read_image(sftp: &SftpSession, path: &str) -> Result<Vec<u8>, SftpError> {
    let not_image = || SftpError::Sftp {
        message: text::not_an_image(path),
    };
    if !text::is_image_path(path) {
        return Err(not_image());
    }
    let meta = sftp.metadata(path).await.map_err(|err| failure("read", path, err))?;
    if meta.is_dir() {
        return Err(not_image());
    }
    let (bytes, truncated) = read_prefix(sftp, path, text::MAX_IMAGE_BYTES).await?;
    if truncated {
        return Err(SftpError::Sftp {
            message: text::image_too_large(path),
        });
    }
    Ok(bytes)
}

/// The first `max` bytes of a remote file, and whether it is longer.
async fn read_prefix(sftp: &SftpSession, path: &str, max: usize) -> Result<(Vec<u8>, bool), SftpError> {
    use tokio::io::AsyncReadExt;

    let file = sftp.open(path).await.map_err(|err| failure("read", path, err))?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|err| failure("read", path, err))?;
    let truncated = bytes.len() > max;
    bytes.truncate(max);
    Ok((bytes, truncated))
}

/// The file a download of `local` writes before renaming it: `<local>.plexusterm-partial`.
pub fn partial_path(local: &Path) -> PathBuf {
    let mut partial_name = local.file_name().unwrap_or_default().to_os_string();
    partial_name.push(PARTIAL_SUFFIX);
    local.with_file_name(partial_name)
}

/// Streams the remote file `remote` into the local file `local` (created or replaced).
/// A partially written local file is removed when the transfer fails.
pub async fn download(sftp: &SftpSession, remote: &str, local: &Path) -> Result<(), SftpError> {
    use tokio::io::AsyncWriteExt;

    let source = sftp
        .open(remote)
        .await
        .map_err(|err| failure("download", remote, err))?;
    let local_label = local.display().to_string();
    // Write next to the target and rename at the end, so a failed transfer never
    // destroys a file the user chose to replace.
    let partial = partial_path(local);
    let target = tokio::fs::File::create(&partial)
        .await
        .map_err(|err| failure("write", &local_label, err))?;
    let mut reader = tokio::io::BufReader::with_capacity(TRANSFER_CHUNK, source);
    let mut writer = tokio::io::BufWriter::with_capacity(TRANSFER_CHUNK, target);
    let copied = async {
        tokio::io::copy_buf(&mut reader, &mut writer).await?;
        writer.flush().await?;
        drop(writer);
        tokio::fs::rename(&partial, local).await
    }
    .await;
    if let Err(err) = copied {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(failure("download", remote, err));
    }
    Ok(())
}

/// Streams the local file `local` into `remote_dir`, keeping its name; returns the remote path.
pub async fn upload(sftp: &SftpSession, local: &Path, remote_dir: &str) -> Result<String, SftpError> {
    let local_label = local.display().to_string();
    let name = local
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SftpError::Sftp {
            message: format!("cannot upload {local_label}: not a file name"),
        })?;
    let remote = join(remote_dir, name);
    upload_file(sftp, local, &remote).await?;
    Ok(remote)
}

/// Streams the local file `local` into the remote file `remote`, creating or replacing it; a
/// failed upload leaves an existing `remote` as it was.
pub async fn upload_file(sftp: &SftpSession, local: &Path, remote: &str) -> Result<(), SftpError> {
    use russh_sftp::protocol::{FileAttributes, OpenFlags};
    use tokio::io::AsyncWriteExt;

    let local_label = local.display().to_string();
    let source = tokio::fs::File::open(local)
        .await
        .map_err(|err| failure("read", &local_label, err))?;
    // The bytes go to a new file next to the target first: a failed upload never touches the
    // original, and the swap at the end is a rename on the server.
    let temp = upload_temp_path(remote);
    let mut target = sftp
        .open_with_flags(&temp, OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE)
        .await
        .map_err(|err| failure("upload to", remote, err))?;
    let mut reader = tokio::io::BufReader::with_capacity(TRANSFER_CHUNK, source);
    // `shutdown` waits for every pending write to be acknowledged.
    let copied = match tokio::io::copy_buf(&mut reader, &mut target).await {
        Ok(_) => target.shutdown().await,
        Err(err) => Err(err),
    };
    drop(target);
    if let Err(err) = copied {
        let _ = sftp.remove_file(&temp).await;
        return Err(failure("upload to", remote, err));
    }
    let original = sftp.metadata(remote).await.ok();
    if let Some(permissions) = original.as_ref().and_then(|meta| meta.permissions) {
        // Keep the replaced file's mode; best effort, as the owner may differ.
        let mode = FileAttributes {
            permissions: Some(permissions & 0o7777),
            ..FileAttributes::empty()
        };
        let _ = sftp.set_metadata(&temp, mode).await;
    }
    replace_with(sftp, &temp, remote, original.is_some()).await
}

/// A hidden name next to `remote` for an upload in progress.
fn upload_temp_path(remote: &str) -> String {
    let (dir, name) = match remote.rsplit_once('/') {
        Some(("", name)) => ("/", name),
        Some((dir, name)) => (dir, name),
        None => (".", remote),
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    join(dir, &format!(".{name}.plexus-upload-{}-{nonce}", std::process::id()))
}

/// Moves the finished `temp` to `remote`. SFTP v3 servers such as OpenSSH refuse to rename over
/// an existing file, so the original is first moved aside and restored if the swap fails.
async fn replace_with(sftp: &SftpSession, temp: &str, remote: &str, exists: bool) -> Result<(), SftpError> {
    let Err(err) = sftp.rename(temp, remote).await else {
        return Ok(());
    };
    if !exists {
        let _ = sftp.remove_file(temp).await;
        return Err(failure("upload to", remote, err));
    }
    let aside = format!("{temp}.old");
    if let Err(err) = sftp.rename(remote, &aside).await {
        let _ = sftp.remove_file(temp).await;
        return Err(failure("upload to", remote, err));
    }
    if let Err(err) = sftp.rename(temp, remote).await {
        let _ = sftp.rename(&aside, remote).await;
        let _ = sftp.remove_file(temp).await;
        return Err(failure("upload to", remote, err));
    }
    let _ = sftp.remove_file(&aside).await;
    Ok(())
}

pub async fn mkdir(sftp: &SftpSession, path: &str) -> Result<(), SftpError> {
    sftp.create_dir(path)
        .await
        .map_err(|err| failure("create folder", path, err))
}

/// Creates an empty file at `path`; an existing file is never replaced (exclusive create).
pub async fn create_file(sftp: &SftpSession, path: &str) -> Result<(), SftpError> {
    use russh_sftp::protocol::OpenFlags;
    let file = sftp
        .open_with_flags(path, OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE)
        .await
        .map_err(|err| failure("create file", path, err))?;
    drop(file);
    Ok(())
}

/// Removes a file, or a directory when `is_dir` (only if it is empty).
pub async fn remove(sftp: &SftpSession, path: &str, is_dir: bool) -> Result<(), SftpError> {
    let result = if is_dir {
        sftp.remove_dir(path).await
    } else {
        sftp.remove_file(path).await
    };
    result.map_err(|err| failure("delete", path, err))
}

pub async fn rename(sftp: &SftpSession, from: &str, to: &str) -> Result<(), SftpError> {
    sftp.rename(from, to).await.map_err(|err| failure("rename", from, err))
}

/// Moves `from` into the remote folder `dir`, keeping its name, and returns where it ended up.
/// Never replaces an existing entry, and never moves a folder into itself or one of its subfolders.
pub async fn move_into(sftp: &SftpSession, from: &str, dir: &str) -> Result<String, SftpError> {
    let refuse = |why: String| SftpError::Sftp {
        message: format!("cannot move {from}: {why}"),
    };
    let source = from.trim_end_matches('/');
    let target = match dir.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    };
    let Some((parent, name)) = source.rsplit_once('/').filter(|(_, name)| !name.is_empty()) else {
        return Err(refuse("not a file name".into()));
    };
    if target == source || target.starts_with(&format!("{source}/")) {
        return Err(refuse("a folder cannot be moved into itself".into()));
    }
    if (if parent.is_empty() { "/" } else { parent }) == target {
        return Err(refuse(format!("it is already in {target}")));
    }
    let to = join(target, name);
    // SFTP rename may replace an existing entry on some servers.
    if sftp.symlink_metadata(&to).await.is_ok() {
        return Err(refuse(format!("{to} already exists")));
    }
    sftp.rename(source, &to)
        .await
        .map_err(|err| failure("move", from, err))?;
    Ok(to)
}

/// Replaces the text of the existing remote file `path` with `content`, but only while it still
/// holds `expected` (the text that was opened), so changes made by someone else are never lost.
/// `encoding` is the id to write with (`None`: UTF-8); `expected_encoding` the one `expected` was
/// decoded with (`None`: the same as `encoding`).
pub async fn write_text(
    sftp: &SftpSession,
    path: &str,
    expected: &str,
    content: &str,
    encoding: Option<&str>,
    expected_encoding: Option<&str>,
) -> Result<(), SftpError> {
    use russh_sftp::protocol::OpenFlags;
    use tokio::io::AsyncWriteExt;

    let sftp_error = |message| SftpError::Sftp { message };
    // Encoded first: characters the encoding cannot hold leave the file untouched.
    let bytes = text::encode_text(path, content, encoding).map_err(sftp_error)?;

    let (current, truncated) = read_prefix(sftp, path, text::MAX_VIEW_BYTES)
        .await
        .map_err(|err| match err {
            SftpError::Sftp { message } => SftpError::Sftp {
                message: message.replacen("cannot read", "cannot save", 1),
            },
            other => other,
        })?;
    text::check_unchanged(path, current, truncated, expected, expected_encoding.or(encoding)).map_err(sftp_error)?;
    let mut file = sftp
        .open_with_flags(path, OpenFlags::WRITE | OpenFlags::TRUNCATE)
        .await
        .map_err(|err| failure("save", path, err))?;
    file.write_all(&bytes).await.map_err(|err| failure("save", path, err))?;
    // Waits for every pending write to be acknowledged.
    file.shutdown().await.map_err(|err| failure("save", path, err))
}

/// Highest mode `chmod` accepts: permission bits plus setuid, setgid and sticky.
pub const MAX_MODE: u32 = 0o7777;

/// Sets the permission bits of `path` (`mode` must be at most `0o7777`).
pub async fn chmod(sftp: &SftpSession, path: &str, mode: u32) -> Result<(), SftpError> {
    const ACTION: &str = "change permissions of";
    if mode > MAX_MODE {
        return Err(SftpError::Sftp {
            message: format!("cannot {ACTION} {path}: invalid mode {mode:#o}"),
        });
    }
    let attrs = russh_sftp::protocol::FileAttributes {
        permissions: Some(mode),
        ..Default::default()
    };
    sftp.set_metadata(path, attrs)
        .await
        .map_err(|err| failure(ACTION, path, err))
}

/// Downloads `remote` into its private temporary copy for `purpose` in session `id`
/// and returns the copy's path.
pub async fn download_copy(
    sftp: &SftpSession,
    temp: &edit::TempFiles,
    id: u32,
    purpose: &str,
    remote: &str,
) -> Result<PathBuf, SftpError> {
    let local = temp
        .local_path(id, purpose, remote)
        .map_err(|err| failure("download", remote, err))?;
    download(sftp, remote, &local).await?;
    Ok(local)
}
