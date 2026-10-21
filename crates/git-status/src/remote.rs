//! The git branch and status of a folder on the server of an SSH session, read with one read-only
//! `git` command over its existing connection (v0.2.15 `src-tauri/src/lib.rs` `ssh_git_branch`,
//! `ssh_git_status`).

use proto_ssh::SshManager;

use crate::{StatusEntry, parse_remote, parse_status, remote_command, remote_status_command};

/// The branch of the remote `folder` of SSH session `id` (its short commit when detached); `None`
/// outside a repository, without git, for a folder that cannot be passed, or when the command
/// fails (including a closed session or the exec timeout).
pub async fn ssh_git_branch(ssh: &SshManager, id: u32, folder: &str) -> Option<String> {
    let command = remote_command(folder)?;
    ssh.exec(id, &command)
        .await
        .ok()
        .and_then(|output| parse_remote(&output))
}

/// The statuses of the files under the remote `folder` of SSH session `id`, relative to it; empty
/// when they cannot be read (see [`ssh_git_branch`]).
pub async fn ssh_git_status(ssh: &SshManager, id: u32, folder: &str) -> Vec<StatusEntry> {
    let Some(command) = remote_status_command(folder) else {
        return Vec::new();
    };
    ssh.exec(id, &command)
        .await
        .map(|output| parse_status(output.as_bytes()))
        .unwrap_or_default()
}
