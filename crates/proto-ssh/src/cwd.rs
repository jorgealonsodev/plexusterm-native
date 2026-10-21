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
