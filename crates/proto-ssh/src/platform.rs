//! The operating-system helpers SSH logins need (PRD PLAT-1), ported from PlexusTerm v0.2.15
//! `src-tauri/src/platform.rs`: home directory, `~` expansion of key paths, the `known_hosts`
//! file and the SSH agent endpoint.
//!
//! The decisions are pure functions taking an [`Os`] and an environment lookup, so the Windows
//! rules are unit-tested on Linux too; the public wrappers apply them to this machine.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The platform families PlexusTerm distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Unix,
    Windows,
}

/// The family this binary was built for.
pub const CURRENT: Os = if cfg!(windows) { Os::Windows } else { Os::Unix };

/// Reads one environment variable.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// Windows OpenSSH agent pipe, used when `SSH_AUTH_SOCK` is not set.
pub const WINDOWS_AGENT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";

fn env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// Reads one environment variable, treating an empty value as unset (like OpenSSH).
fn non_empty(env: EnvLookup, name: &str) -> Option<OsString> {
    env(name).filter(|value| !value.is_empty())
}

/// The user's home directory: `$HOME` on Unix, `%USERPROFILE%` on Windows. An empty value is
/// no home, so nothing is ever resolved against the working folder.
pub fn home_dir_in(os: Os, env: EnvLookup) -> Option<PathBuf> {
    let var = match os {
        Os::Unix => "HOME",
        Os::Windows => "USERPROFILE",
    };
    non_empty(env, var).map(PathBuf::from)
}

pub fn home_dir() -> Option<PathBuf> {
    home_dir_in(CURRENT, &env)
}

/// Expands a leading `~/` (also `~\` on Windows) to `home`, as users type key paths like
/// `~/.ssh/id_ed25519`. Other paths, or any path without a home, are returned unchanged.
pub fn expand_home_in(os: Os, path: &str, home: Option<&Path>) -> PathBuf {
    let rest = path.strip_prefix("~/").or_else(|| match os {
        Os::Windows => path.strip_prefix("~\\"),
        Os::Unix => None,
    });
    match (rest, home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

pub fn expand_home(path: &str) -> PathBuf {
    expand_home_in(CURRENT, path, home_dir().as_deref())
}

/// The user's OpenSSH `known_hosts` file (`~/.ssh/known_hosts`), or `None` without a home.
pub fn known_hosts_path_in(os: Os, env: EnvLookup) -> Option<PathBuf> {
    home_dir_in(os, env).map(|home| home.join(".ssh").join("known_hosts"))
}

/// The current user's OpenSSH `known_hosts` file (`~/.ssh/known_hosts`), or `None` without a
/// home.
pub fn known_hosts_path() -> Option<PathBuf> {
    known_hosts_path_in(CURRENT, &env)
}

/// The SSH agent to use: `SSH_AUTH_SOCK` when set and not empty; on Windows the OpenSSH agent
/// pipe otherwise.
pub fn agent_endpoint_in(os: Os, env: EnvLookup) -> Option<PathBuf> {
    non_empty(env, "SSH_AUTH_SOCK").map(PathBuf::from).or(match os {
        Os::Windows => Some(PathBuf::from(WINDOWS_AGENT_PIPE)),
        Os::Unix => None,
    })
}

/// The SSH agent to use: `explicit` when given, otherwise as [`agent_endpoint_in`].
pub fn agent_endpoint(explicit: Option<&Path>, os: Os, env: EnvLookup) -> Option<PathBuf> {
    explicit.map(Path::to_path_buf).or_else(|| agent_endpoint_in(os, env))
}

/// A connected SSH agent.
pub type Agent =
    russh::keys::agent::client::AgentClient<Box<dyn russh::keys::agent::client::AgentStream + Send + Unpin>>;

/// Connects to the SSH agent at `endpoint` (see [`agent_endpoint_in`]); `None` uses this
/// machine's default.
pub async fn connect_agent(endpoint: Option<&Path>) -> Result<Agent, russh::keys::Error> {
    use russh::keys::agent::client::AgentClient;
    let endpoint = agent_endpoint(endpoint, CURRENT, &env);
    #[cfg(unix)]
    let agent = match endpoint {
        Some(socket) => AgentClient::connect_uds(socket).await?.dynamic(),
        // The "SSH_AUTH_SOCK not set" error of OpenSSH-style clients.
        None => return Err(russh::keys::Error::EnvVar("SSH_AUTH_SOCK")),
    };
    #[cfg(windows)]
    let agent = AgentClient::connect_named_pipe(endpoint.unwrap_or_else(|| WINDOWS_AGENT_PIPE.into()))
        .await?
        .dynamic();
    Ok(agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let pairs: Vec<(String, String)> = pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
        move |name| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| OsString::from(v))
    }

    #[test]
    fn home_dir_comes_from_home_on_unix_and_userprofile_on_windows() {
        let env = vars(&[("HOME", "/home/me"), ("USERPROFILE", r"C:\Users\me")]);
        assert_eq!(home_dir_in(Os::Unix, &env), Some(PathBuf::from("/home/me")));
        assert_eq!(home_dir_in(Os::Windows, &env), Some(PathBuf::from(r"C:\Users\me")));
        assert_eq!(home_dir_in(Os::Windows, &vars(&[("HOME", "/h")])), None);
    }

    #[test]
    fn expands_tilde_slash_on_both_and_tilde_backslash_only_on_windows() {
        let unix_home = Path::new("/home/me");
        assert_eq!(
            expand_home_in(Os::Unix, "~/.ssh/id", Some(unix_home)),
            unix_home.join(".ssh/id")
        );
        assert_eq!(
            expand_home_in(Os::Unix, r"~\.ssh\id", Some(unix_home)),
            PathBuf::from(r"~\.ssh\id")
        );
        assert_eq!(
            expand_home_in(Os::Unix, "/abs/id", Some(unix_home)),
            PathBuf::from("/abs/id")
        );
        assert_eq!(
            expand_home_in(Os::Unix, "rel~/id", Some(unix_home)),
            PathBuf::from("rel~/id")
        );
        assert_eq!(expand_home_in(Os::Unix, "~/id", None), PathBuf::from("~/id"));

        let win_home = Path::new(r"C:\Users\me");
        assert_eq!(
            expand_home_in(Os::Windows, r"~\.ssh\id", Some(win_home)),
            win_home.join(r".ssh\id")
        );
        assert_eq!(
            expand_home_in(Os::Windows, "~/.ssh/id", Some(win_home)),
            win_home.join(".ssh/id")
        );
        assert_eq!(
            expand_home_in(Os::Windows, r"D:\keys\id", Some(win_home)),
            PathBuf::from(r"D:\keys\id")
        );
    }

    #[test]
    fn agent_endpoint_uses_ssh_auth_sock_then_the_windows_pipe() {
        let sock = vars(&[("SSH_AUTH_SOCK", "/run/agent.sock")]);
        assert_eq!(
            agent_endpoint_in(Os::Unix, &sock),
            Some(PathBuf::from("/run/agent.sock"))
        );
        assert_eq!(agent_endpoint_in(Os::Unix, &vars(&[])), None);
        assert_eq!(
            agent_endpoint_in(Os::Windows, &sock),
            Some(PathBuf::from("/run/agent.sock"))
        );
        assert_eq!(
            agent_endpoint_in(Os::Windows, &vars(&[])),
            Some(PathBuf::from(r"\\.\pipe\openssh-ssh-agent"))
        );
    }

    #[test]
    fn expand_home_uses_this_machines_home() {
        let home = home_dir().expect("home directory");
        assert_eq!(expand_home("~/.ssh/id"), home.join(".ssh/id"));
        assert_eq!(expand_home("rel~/id"), PathBuf::from("rel~/id"));
    }

    #[test]
    fn an_empty_home_is_no_home() {
        assert_eq!(home_dir_in(Os::Unix, &vars(&[("HOME", "")])), None);
        assert_eq!(home_dir_in(Os::Windows, &vars(&[("USERPROFILE", "")])), None);
        // Without a home, `~/x` is left as typed instead of becoming the relative `x`.
        let home = home_dir_in(Os::Unix, &vars(&[("HOME", "")]));
        assert_eq!(expand_home_in(Os::Unix, "~/x", home.as_deref()), PathBuf::from("~/x"));
    }

    #[test]
    fn known_hosts_is_in_the_home_ssh_folder_and_unknown_without_a_home() {
        assert_eq!(
            known_hosts_path_in(Os::Unix, &vars(&[("HOME", "/home/me")])),
            Some(PathBuf::from("/home/me/.ssh/known_hosts"))
        );
        assert_eq!(
            known_hosts_path_in(Os::Windows, &vars(&[("USERPROFILE", r"C:\Users\me")])),
            Some(Path::new(r"C:\Users\me").join(".ssh").join("known_hosts"))
        );
        // Never a path relative to the working folder.
        assert_eq!(known_hosts_path_in(Os::Unix, &vars(&[])), None);
        assert_eq!(known_hosts_path_in(Os::Unix, &vars(&[("HOME", "")])), None);
        assert_eq!(known_hosts_path_in(Os::Windows, &vars(&[("USERPROFILE", "")])), None);
    }

    #[test]
    fn an_empty_ssh_auth_sock_is_unset() {
        let empty = vars(&[("SSH_AUTH_SOCK", "")]);
        assert_eq!(agent_endpoint_in(Os::Unix, &empty), None);
        assert_eq!(
            agent_endpoint_in(Os::Windows, &empty),
            Some(PathBuf::from(WINDOWS_AGENT_PIPE))
        );
    }

    #[test]
    fn an_explicit_agent_endpoint_overrides_the_environment() {
        let sock = vars(&[("SSH_AUTH_SOCK", "/run/agent.sock")]);
        let explicit = Path::new("/tmp/test-agent.sock");
        for os in [Os::Unix, Os::Windows] {
            assert_eq!(agent_endpoint(Some(explicit), os, &sock), Some(explicit.to_path_buf()));
            assert_eq!(agent_endpoint(None, os, &sock), Some(PathBuf::from("/run/agent.sock")));
        }
        assert_eq!(agent_endpoint(None, Os::Unix, &vars(&[])), None);
    }

    #[tokio::test]
    async fn connecting_to_a_missing_agent_socket_fails() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-agent.sock");
        assert!(connect_agent(Some(&missing)).await.is_err());
    }

    #[test]
    fn current_matches_the_build_target() {
        assert_eq!(CURRENT == Os::Windows, cfg!(windows));
    }
}
