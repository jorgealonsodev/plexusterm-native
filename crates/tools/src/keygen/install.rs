//! Installs a generated or loaded public key on a server like `ssh-copy-id` (PRD TOOL-1a),
//! logging in through `proto-ssh` exactly as an SSH tab does.

use std::time::Duration;

use proto_ssh::{Chain, Connection, SshError, SshManager, SshTarget, connect_authenticated, disconnect_jumps};
use russh::ChannelMsg;

use super::{KeyStore, KeygenError};

/// How long the install command may take on the server.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(20);

/// Most output of the install command kept for error messages.
const MAX_OUTPUT: usize = 16 * 1024;

const ADDED: &str = "PLEXUSTERM_KEY_ADDED";
const PRESENT: &str = "PLEXUSTERM_KEY_PRESENT";

/// Public key types accepted in an `authorized_keys` line by [`install_command`].
const KEY_TYPES: [&str; 5] = [
    "ssh-ed25519",
    "ssh-rsa",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
];

/// Whether [`install`] added the key or found it already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Added,
    AlreadyPresent,
}

/// A public key line of one of [`KEY_TYPES`] safe to put in a shell command: the key
/// type, base64, and an optional comment without control characters.
fn validate_public_line(line: &str) -> Result<(&str, &str), String> {
    let invalid = || "not a supported public key line".to_string();
    if line.chars().any(char::is_control) {
        return Err(invalid());
    }
    let mut parts = line.splitn(3, ' ');
    let (kind, blob) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    let blob_ok = !blob.is_empty()
        && blob
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=');
    if !KEY_TYPES.contains(&kind) || !blob_ok {
        return Err(invalid());
    }
    Ok((kind, blob))
}

/// `text` as one single-quoted POSIX shell word.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// `text` as octal escapes (`\163\163\150...`) for `printf`: digits and backslashes only, so
/// no shell between sshd and `sh` (fish included) can reinterpret it.
fn octal_escaped(text: &str) -> String {
    text.bytes().map(|b| format!("\\{b:03o}")).collect()
}

/// The command that adds `line` to `~/.ssh/authorized_keys` unless the same key (type and
/// base64, whatever its comment) is already there, creating `~/.ssh` (0700) and the file
/// (0600). It runs under `sh` whatever the login shell is, and prints [`ADDED`] or [`PRESENT`].
///
/// sshd hands the command to the user's login shell first. The script therefore contains no
/// single quote and no backslash other than `\n` and octal escapes, and the user's text (the
/// comment) only travels octal-escaped.
pub fn install_command(line: &str) -> Result<String, String> {
    let (kind, blob) = validate_public_line(line)?;
    let script = format!(
        concat!(
            "umask 077; set -e; ",
            "mkdir -p \"$HOME/.ssh\"; chmod 700 \"$HOME/.ssh\"; ",
            "f=\"$HOME/.ssh/authorized_keys\"; touch \"$f\"; chmod 600 \"$f\"; ",
            "if grep -qF -- \"{kind} {blob}\" \"$f\"; then echo {present}; else ",
            "if [ -s \"$f\" ] && [ -n \"$(tail -c 1 \"$f\")\" ]; then echo >> \"$f\"; fi; ",
            "printf \"{line}\\n\" >> \"$f\"; echo {added}; fi"
        ),
        kind = kind,
        blob = blob,
        line = octal_escaped(line),
        present = PRESENT,
        added = ADDED,
    );
    debug_assert!(!script.contains('\''));
    Ok(format!("sh -c {}", sh_quote(&script)))
}

fn failure(message: impl Into<String>) -> SshError {
    SshError::Connection {
        message: message.into(),
    }
}

/// Reads what the install command printed.
fn install_outcome(output: &str, exit_status: Option<u32>) -> Result<InstallOutcome, SshError> {
    if output.contains(PRESENT) {
        return Ok(InstallOutcome::AlreadyPresent);
    }
    if output.contains(ADDED) {
        return Ok(InstallOutcome::Added);
    }
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let last = lines.get(lines.len().saturating_sub(3)..).unwrap_or_default();
    let tail: String = last.join(" / ").chars().take(300).collect();
    let status = exit_status.map_or(String::new(), |s| format!(" (exit status {s})"));
    Err(failure(if tail.is_empty() {
        format!("the key could not be installed{status}")
    } else {
        format!("the key could not be installed{status}: {tail}")
    }))
}

/// Runs `command` on `connection` and returns its output and exit status.
async fn run(connection: &Connection, command: &str, timeout: Duration) -> Result<(String, Option<u32>), SshError> {
    let mut channel = connection
        .channel_open_session()
        .await
        .map_err(|err| failure(format!("cannot run the install command: {err}")))?;
    channel
        .exec(true, command)
        .await
        .map_err(|err| failure(format!("cannot run the install command: {err}")))?;
    let mut output = Vec::new();
    let mut exit_status = None;
    let wait = async {
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    if output.len() < MAX_OUTPUT {
                        output.extend_from_slice(&data);
                    }
                }
                ChannelMsg::ExitStatus { exit_status: s } => exit_status = Some(s),
                ChannelMsg::Close => break,
                _ => {}
            }
        }
    };
    tokio::time::timeout(timeout, wait)
        .await
        .map_err(|_| failure(format!("the server did not answer within {} s", timeout.as_secs())))?;
    Ok((String::from_utf8_lossy(&output).into_owned(), exit_status))
}

/// Logs into `target` (known_hosts, jump hosts and saved passwords as for an SSH tab,
/// never changing saved passwords) and adds key `id` to its `authorized_keys`. Only the
/// public key line travels; the private key never leaves `store`.
pub async fn install(
    store: &KeyStore,
    id: u32,
    ssh: &SshManager,
    target: SshTarget,
    trusted_fingerprint: Option<String>,
) -> Result<InstallOutcome, SshError> {
    let public_key = store
        .with(id, |key| key.public_key.clone())
        .ok_or_else(|| failure(KeygenError::UnknownKey.to_string()))?;
    let command = install_command(&public_key).map_err(failure)?;
    let Chain { connection, jumps, .. } =
        connect_authenticated(target, trusted_fingerprint, None, ssh.context(), None).await?;
    let result = run(&connection, &command, INSTALL_TIMEOUT).await;
    let _ = connection.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    disconnect_jumps(&jumps).await;
    let (output, exit_status) = result?;
    install_outcome(&output, exit_status)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::{KeyType, generate};
    use super::*;
    use proto_ssh::SshAuth;
    use proto_ssh::test_server::{self, PASSWORD};

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_install_command_only_accepts_supported_public_key_lines() {
        for bad in [
            "",
            "ssh-dss AAAA x",
            "ecdsa-sha2-nistp999 AAAA x",
            "sk-ssh-ed25519@openssh.com AAAA x",
            "ssh-ed25519",
            "ssh-rsa",
            "ssh-ed25519 AAAA;rm -rf ~",
            "ssh-rsa AAAA$(id)",
            "ecdsa-sha2-nistp256 AAAA`id`",
            "ssh-ed25519 AAAA c\nrm -rf ~",
        ] {
            assert!(install_command(bad).is_err(), "{bad:?}");
        }
        for kind in KEY_TYPES {
            assert!(
                install_command(&format!("{kind} AAAA it's 'quoted' $(id)")).is_ok(),
                "{kind}"
            );
        }
    }

    /// Runs the install command for `line` under a local `sh` with `home` as `$HOME`.
    #[cfg(unix)]
    fn run_locally(line: &str, home: &Path) -> String {
        let command = install_command(line).unwrap();
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .env("HOME", home)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn the_install_command_appends_the_key_once_with_safe_modes() {
        let home = tempfile::tempdir().unwrap();
        let line = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB/x+y= it's me $(touch pwned)";
        assert_eq!(run_locally(line, home.path()).trim(), ADDED);
        let file = home.path().join(".ssh/authorized_keys");
        assert_eq!(mode(&home.path().join(".ssh")), 0o700);
        assert_eq!(mode(&file), 0o600);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("{line}\n"));
        assert!(!home.path().join("pwned").exists());
        // Again, even with another comment: already there, nothing added.
        assert_eq!(run_locally(line, home.path()).trim(), PRESENT);
        let renamed = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB/x+y= other";
        assert_eq!(run_locally(renamed, home.path()).trim(), PRESENT);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("{line}\n"));
    }

    /// sshd hands the command to the user's login shell, which parses it before `sh -c`
    /// runs: the key line must arrive intact whatever that shell is (fish treats `\'` and
    /// `\\` as escapes even inside single quotes).
    #[cfg(unix)]
    #[test]
    fn the_install_command_survives_every_login_shell() {
        let comments = [
            "jorge's laptop",
            r"a\'b\\c",
            r#"x'"'"'y; touch pwned #"#,
            "plain",
            "$(id) `id` %s %n ~ * !!",
        ];
        for shell in ["sh", "dash", "bash", "zsh", "fish"] {
            if std::process::Command::new(shell)
                .arg("-c")
                .arg("true")
                .output()
                .is_err()
            {
                eprintln!("{shell} not installed, skipped");
                continue;
            }
            for comment in comments {
                let home = tempfile::tempdir().unwrap();
                let line = format!("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB/x+y= {comment}");
                let output = std::process::Command::new(shell)
                    .arg("-c")
                    .arg(install_command(&line).unwrap())
                    .env("HOME", home.path())
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{shell} {comment:?}: {output:?}");
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout).trim(),
                    ADDED,
                    "{shell} {comment:?}"
                );
                let written = std::fs::read_to_string(home.path().join(".ssh/authorized_keys")).unwrap();
                assert_eq!(written, format!("{line}\n"), "{shell} {comment:?}");
                assert!(!home.path().join("pwned").exists(), "{shell} {comment:?}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_install_command_keeps_existing_keys_and_fixes_modes() {
        let home = tempfile::tempdir().unwrap();
        let ssh_dir = home.path().join(".ssh");
        std::fs::create_dir(&ssh_dir).unwrap();
        let file = ssh_dir.join("authorized_keys");
        // An existing last line without a newline must not be joined with the new key.
        std::fs::write(&file, "ssh-ed25519 AAAAold old").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let line = "ssh-ed25519 AAAAnew new";
        assert_eq!(run_locally(line, home.path()).trim(), ADDED);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "ssh-ed25519 AAAAold old\nssh-ed25519 AAAAnew new\n"
        );
        assert_eq!(mode(&ssh_dir), 0o700);
        assert_eq!(mode(&file), 0o600);
    }

    #[test]
    fn reads_the_install_outcome() {
        assert_eq!(
            install_outcome("PLEXUSTERM_KEY_ADDED\n", Some(0)).unwrap(),
            InstallOutcome::Added
        );
        assert_eq!(
            install_outcome("PLEXUSTERM_KEY_PRESENT\n", Some(0)).unwrap(),
            InstallOutcome::AlreadyPresent
        );
        let err = install_outcome("chmod: Read-only file system\n", Some(1)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "the key could not be installed (exit status 1): chmod: Read-only file system"
        );
    }

    #[tokio::test]
    async fn install_logs_in_and_runs_the_install_command() {
        let server = test_server::start(1, None).await;
        let dir = tempfile::tempdir().unwrap();
        // A missing agent socket, so the environment's SSH agent never changes the login.
        let ssh = SshManager::new(dir.path().join("known_hosts")).with_agent_socket(dir.path().join("no-agent"));
        let store = KeyStore::default();
        let key = generate(&store, KeyType::Ed25519, None, "me").unwrap();
        let target = SshTarget {
            host: "127.0.0.1".into(),
            port: server.addr.port(),
            username: "tester".into(),
            auth: SshAuth::Password {
                password: PASSWORD.to_string().into(),
            },
            jumps: vec![],
        };
        // An unknown host is reported like for an SSH tab, before anything runs.
        let err = install(&store, key.id, &ssh, target.clone(), None).await.unwrap_err();
        let SshError::UnknownHost { fingerprint, .. } = err else {
            panic!("{err:?}")
        };
        assert!(server.exec_commands().is_empty());
        // Trusted: the command runs (the fake server has no `sh`, so it fails like one without it).
        let err = install(&store, key.id, &ssh, target, Some(fingerprint))
            .await
            .unwrap_err();
        assert_eq!(server.exec_commands(), vec![install_command(&key.public_key).unwrap()]);
        assert!(
            err.to_string()
                .starts_with("the key could not be installed (exit status 127)"),
            "{err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_install_command_appends_rsa_and_ecdsa_keys() {
        let store = KeyStore::default();
        for kind in [KeyType::Rsa2048, KeyType::EcdsaP384] {
            let home = tempfile::tempdir().unwrap();
            let key = generate(&store, kind, None, "it's me").unwrap();
            assert_eq!(run_locally(&key.public_key, home.path()).trim(), ADDED);
            assert_eq!(
                std::fs::read_to_string(home.path().join(".ssh/authorized_keys")).unwrap(),
                format!("{}\n", key.public_key)
            );
            assert_eq!(run_locally(&key.public_key, home.path()).trim(), PRESENT);
        }
    }
}
