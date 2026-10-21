//! The working folder of the remote shell of an SSH session, for shells that do not report it
//! with OSC 7. One POSIX `sh` command, run over the session's existing connection, walks up from
//! itself to the per-connection `sshd` (or `dropbear`) process and prints the folder of that
//! process's child holding a terminal: the shell of the session's pty. Linux `/proc` only;
//! elsewhere it prints nothing.

use crate::{SshError, SshManager};

/// The script of [`SHELL_CWD_COMMAND`], as a macro so it can be spliced into a constant.
macro_rules! shell_cwd_script {
    () => {
        "p=$$
while :; do
  read -r c < /proc/$p/comm || exit 0
  case $c in sshd*|dropbear*) break;; esac
  read -r s < /proc/$p/stat || exit 0
  set -- ${s##*) }
  p=$2
  [ \"$p\" -gt 1 ] || exit 0
done
kids=$(cat /proc/$p/task/*/children 2>/dev/null)
[ -n \"$kids\" ] || kids=$(cd /proc && echo [0-9]*)
for k in $kids; do
  read -r s < /proc/$k/stat || continue
  set -- ${s##*) }
  [ \"$2\" = \"$p\" ] && [ \"$5\" != 0 ] && cd /proc/$k/cwd && pwd -P && exit 0
done
exit 0"
    };
}

/// Uses only shell builtins (`read`, `set`, `cd`, `pwd`) apart from one `cat`, so polling it
/// stays cheap. Wrapped in `sh -c` so it works whatever the user's login shell is; it holds no
/// single quote or backslash, so the quoting is the same in bash, zsh and fish. A `stat` line is
/// read after its `) `, as the command name before it may contain spaces: then field 2 is the
/// parent pid and field 5 the controlling terminal (0 for none).
pub const SHELL_CWD_COMMAND: &str = concat!("sh -c '", shell_cwd_script!(), "' 2>/dev/null");

/// The folder printed by [`SHELL_CWD_COMMAND`], or `None` when it printed none.
pub fn parse(output: &str) -> Option<String> {
    let path = output.strip_suffix('\n').unwrap_or(output);
    path.starts_with('/').then(|| path.to_string())
}

/// The working folder of session `id`'s shell, or `None` when the server cannot tell.
pub async fn shell_cwd(ssh: &SshManager, id: u32) -> Result<Option<String>, SshError> {
    Ok(parse(&ssh.exec(id, SHELL_CWD_COMMAND).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_printed_folder() {
        assert_eq!(parse("/home/u/src\n"), Some("/home/u/src".into()));
        assert_eq!(parse("/srv/my dir\n"), Some("/srv/my dir".into()));
    }

    #[test]
    fn no_folder_is_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("\n"), None);
        assert_eq!(parse("sh: not found\n"), None);
    }

    /// Run here, where no ancestor is sshd, the script ends without an error and prints at
    /// most a folder (an absolute path) when the tests themselves run over SSH.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_script_runs_in_sh_and_prints_at_most_a_folder() {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(shell_cwd_script!())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.is_empty() || parse(&stdout).is_some(), "{stdout:?}");
    }

    #[tokio::test]
    async fn shell_cwd_runs_the_command_over_the_session() {
        use crate::test_server::{self, PASSWORD};
        use crate::{ConnectRequest, SshAuth};

        let server = test_server::start(1, None).await;
        let dir = tempfile::tempdir().unwrap();
        let known_hosts = dir.path().join("known_hosts");
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            server.addr.port(),
            &server.host_key,
            &known_hosts,
        )
        .unwrap();
        let ssh = SshManager::new(known_hosts);
        let id = ssh
            .connect(
                ConnectRequest {
                    host: "127.0.0.1".into(),
                    port: server.addr.port(),
                    username: "tester".into(),
                    auth: SshAuth::Password {
                        password: PASSWORD.to_owned().into(),
                    },
                    cols: 80,
                    rows: 24,
                    trusted_fingerprint: None,
                    jumps: vec![],
                },
                term_engine::ByteRing::new(term_engine::ByteRing::SESSION_CAPACITY),
            )
            .await
            .unwrap()
            .id;

        // The fake server has no `sh`, so the command fails.
        assert!(shell_cwd(&ssh, id).await.is_err());
        assert_eq!(server.exec_commands(), vec![SHELL_CWD_COMMAND.to_string()]);
    }
}
