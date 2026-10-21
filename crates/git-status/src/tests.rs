//! Ported from PlexusTerm v0.2.15 `src-tauri/src/git.rs` (its `tests` and `status_tests` modules),
//! plus the SSH variants against the in-process test server.

mod branch_tests {
    use crate::*;
    use std::fs;
    use std::path::Path;

    #[allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "a test helper: a failure here is a failed test"
    )]
    fn repo_with_head(head: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), head).unwrap();
        dir
    }

    #[test]
    fn reads_the_branch_of_a_repository() {
        let repo = repo_with_head("ref: refs/heads/main\n");
        assert_eq!(branch_of(repo.path()), Some("main".into()));
        let repo = repo_with_head("ref: refs/heads/feature/git-branch\n");
        assert_eq!(branch_of(repo.path()), Some("feature/git-branch".into()));
    }

    #[test]
    fn finds_the_repository_from_a_nested_folder() {
        let repo = repo_with_head("ref: refs/heads/develop\n");
        let nested = repo.path().join("src/features/deep");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(branch_of(&nested), Some("develop".into()));
    }

    #[test]
    fn a_detached_head_shows_the_short_commit() {
        let repo = repo_with_head("72e8845a1b2c3d4e5f60718293a4b5c6d7e8f901\n");
        assert_eq!(branch_of(repo.path()), Some("72e8845".into()));
        // SHA-256 repositories have 64-digit ids.
        let repo = repo_with_head(&format!("{}\n", "ab".repeat(32)));
        assert_eq!(branch_of(repo.path()), Some("abababa".into()));
    }

    #[test]
    fn follows_a_git_file_of_a_worktree_or_submodule() {
        let main = repo_with_head("ref: refs/heads/main\n");
        let gitdir = main.path().join(".git/worktrees/wt");
        fs::create_dir_all(&gitdir).unwrap();
        fs::write(gitdir.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
        let worktree = tempfile::tempdir().unwrap();
        fs::write(worktree.path().join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
        assert_eq!(branch_of(worktree.path()), Some("wt-branch".into()));

        // Submodules point with a relative path.
        let sub = main.path().join("vendor/lib");
        fs::create_dir_all(&sub).unwrap();
        let modules = main.path().join(".git/modules/lib");
        fs::create_dir_all(&modules).unwrap();
        fs::write(modules.join("HEAD"), "ref: refs/heads/stable\n").unwrap();
        fs::write(sub.join(".git"), "gitdir: ../../.git/modules/lib\n").unwrap();
        assert_eq!(branch_of(&sub), Some("stable".into()));
    }

    #[test]
    fn no_repository_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("a/b");
        fs::create_dir_all(&inner).unwrap();
        // The temporary folder itself is not inside a repository.
        assert_eq!(branch_of(&inner), None);
        assert_eq!(branch_of(Path::new("/definitely/not/here")), None);
    }

    #[test]
    fn an_unreadable_or_garbage_head_is_none_not_an_error() {
        assert_eq!(branch_of(repo_with_head("").path()), None);
        assert_eq!(branch_of(repo_with_head("garbage\n").path()), None);
        assert_eq!(branch_of(repo_with_head("ref: \n").path()), None);
        assert_eq!(
            branch_of(repo_with_head("ref: refs/heads/a\u{1b}[31mred\n").path()),
            None
        );
        assert_eq!(branch_of(repo_with_head(&"x".repeat(10_000)).path()), None);
        // HEAD missing, or a folder instead of a file.
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git/HEAD")).unwrap();
        assert_eq!(branch_of(repo.path()), None);
        // A .git file that points nowhere.
        let broken = tempfile::tempdir().unwrap();
        fs::write(broken.path().join(".git"), "nonsense").unwrap();
        assert_eq!(branch_of(broken.path()), None);
    }

    /// The `sh` script a remote command carries, decoded by the system's `base64 -d` (which also
    /// checks [`base64`] against a real decoder).
    #[allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "a test helper: a failure here is a failed test"
    )]
    pub(super) fn sh_script(command: &str) -> String {
        let payload = command
            .strip_prefix("echo ")
            .and_then(|rest| rest.strip_suffix(" | base64 -d | sh 2>/dev/null"))
            .unwrap_or_else(|| panic!("{command}"));
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("echo {payload} | base64 -d"))
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    #[test]
    fn the_remote_command_quotes_the_folder_and_always_succeeds() {
        let script = sh_script(&remote_command("/srv/it's here").unwrap());
        assert!(script.contains(r"-C '/srv/it'\''s here'"), "{script}");
        assert!(script.contains("symbolic-ref --short -q HEAD") && script.ends_with("exit 0"));
        assert_eq!(remote_command(""), None);
        assert_eq!(remote_command("/a\0b"), None);
    }

    /// The command really runs in `sh` and prints the branch, here with a real git when there is one.
    #[cfg(unix)]
    #[test]
    fn the_remote_command_runs_in_sh() {
        let outside = tempfile::tempdir().unwrap();
        let dir = outside.path().join("it's a repo; $(touch pwned)");
        fs::create_dir_all(&dir).unwrap();
        let run = |folder: &Path| {
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(remote_command(folder.to_str().unwrap()).unwrap())
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            parse_remote(&String::from_utf8(out.stdout).unwrap())
        };
        assert_eq!(run(&dir), None);
        assert!(!dir.join("pwned").exists() && !Path::new("pwned").exists());
        let has_git = std::process::Command::new("git").arg("--version").output().is_ok();
        if has_git {
            let init = std::process::Command::new("git")
                .args(["init", "-q", "-b", "topic"])
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(init.status.success(), "{init:?}");
            assert_eq!(run(&dir), Some("topic".into()));
        }
    }

    #[test]
    fn parses_the_remote_output() {
        assert_eq!(parse_remote("main\n"), Some("main".into()));
        assert_eq!(parse_remote("a1b2c3d\n"), Some("a1b2c3d".into()));
        assert_eq!(parse_remote(""), None);
        assert_eq!(parse_remote("\n"), None);
    }
}

mod status_tests {
    use crate::FileStatus::*;
    use crate::*;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn has_git() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// Runs git in `dir` without the user's or system's configuration.
    #[allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "a test helper: a failure here is a failed test"
    )]
    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(["-c", "init.defaultBranch=main", "-c", "core.hooksPath=/dev/null"])
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    /// A repository with `a.txt`, `src/lib.rs` and `src/deep/x.rs` committed.
    #[allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "a test helper: a failure here is a failed test"
    )]
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q"]);
        fs::create_dir_all(p.join("src/deep")).unwrap();
        fs::write(p.join("a.txt"), "a\n").unwrap();
        fs::write(p.join("src/lib.rs"), "lib\n").unwrap();
        fs::write(p.join("src/deep/x.rs"), "x\n").unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn repo_of_names_the_repository_its_branch_and_whether_it_has_changes() {
        let dir = repo();
        let root = dir.path();
        git(root, &["checkout", "-q", "-b", "feature"]);
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        git(root, &["add", ".gitignore"]);
        git(root, &["commit", "-q", "-m", "ignore logs"]);
        // From a file deep inside, or from a folder: the repository's root.
        let info = repo_of(&root.join("src/deep/x.rs")).unwrap();
        assert_eq!(
            info,
            RepoInfo {
                root: root.to_string_lossy().into_owned(),
                branch: "feature".into(),
                dirty: false,
            }
        );
        assert_eq!(repo_of(&root.join("src")).unwrap().root, info.root);
        // Ignored files are not changes; an edited file is.
        fs::write(root.join("debug.log"), "x\n").unwrap();
        assert!(!repo_of(root).unwrap().dirty);
        fs::write(root.join("src/lib.rs"), "changed\n").unwrap();
        assert!(repo_of(&root.join("a.txt")).unwrap().dirty);
        // Outside any repository.
        let plain = tempfile::tempdir().unwrap();
        fs::write(plain.path().join("f.txt"), "").unwrap();
        assert_eq!(repo_of(&plain.path().join("f.txt")), None);
    }

    fn entry(path: &str, status: FileStatus) -> StatusEntry {
        StatusEntry {
            path: path.into(),
            status,
        }
    }

    fn sorted(mut entries: Vec<StatusEntry>) -> Vec<StatusEntry> {
        entries.sort_by(|a, b| a.path.cmp(&b.path).then((a.status as u8).cmp(&(b.status as u8))));
        entries
    }

    #[test]
    fn maps_the_xy_codes_like_vs_code() {
        assert_eq!(statuses_of_code(b'?', b'?'), vec![Untracked]);
        assert_eq!(statuses_of_code(b'!', b'!'), vec![Ignored]);
        assert_eq!(statuses_of_code(b'M', b' '), vec![IndexModified]);
        assert_eq!(statuses_of_code(b' ', b'M'), vec![Modified]);
        assert_eq!(statuses_of_code(b'M', b'M'), vec![IndexModified, Modified]);
        assert_eq!(statuses_of_code(b'A', b' '), vec![IndexAdded]);
        assert_eq!(statuses_of_code(b'A', b'D'), vec![IndexAdded, Deleted]);
        assert_eq!(statuses_of_code(b'D', b' '), vec![IndexDeleted]);
        assert_eq!(statuses_of_code(b'R', b' '), vec![IndexRenamed]);
        assert_eq!(statuses_of_code(b'C', b'M'), vec![IndexCopied, Modified]);
        assert_eq!(statuses_of_code(b'T', b' '), vec![IndexTypeChanged]);
        assert_eq!(statuses_of_code(b' ', b'T'), vec![TypeChanged]);
        assert_eq!(statuses_of_code(b' ', b'A'), vec![IntentToAdd]);
        assert_eq!(statuses_of_code(b' ', b'R'), vec![IntentToRename]);
        assert_eq!(statuses_of_code(b' ', b'D'), vec![Deleted]);
        assert_eq!(statuses_of_code(b'D', b'D'), vec![BothDeleted]);
        assert_eq!(statuses_of_code(b'A', b'U'), vec![AddedByUs]);
        assert_eq!(statuses_of_code(b'U', b'D'), vec![DeletedByThem]);
        assert_eq!(statuses_of_code(b'U', b'A'), vec![AddedByThem]);
        assert_eq!(statuses_of_code(b'D', b'U'), vec![DeletedByUs]);
        assert_eq!(statuses_of_code(b'A', b'A'), vec![BothAdded]);
        assert_eq!(statuses_of_code(b'U', b'U'), vec![BothModified]);
        assert_eq!(statuses_of_code(b' ', b' '), Vec::<FileStatus>::new());
    }

    #[test]
    fn parses_renames_prefixes_and_drops_a_cut_record() {
        let out =
            b"src/\0 M src/a.rs\0R  src/new.rs\0src/old.rs\0?? src/tmp/\0 M other/b.rs\0!! src/target/\0 M src/cu";
        assert_eq!(
            parse_status(out),
            vec![
                entry("a.rs", Modified),
                entry("new.rs", IndexRenamed),
                entry("tmp/", Untracked),
                entry("target/", Ignored),
            ]
        );
        // Inside an ignored folder: the listed folder itself is covered.
        assert_eq!(
            parse_status(b"node_modules/pkg/\0!! node_modules/\0"),
            vec![entry("", Ignored)]
        );
        assert_eq!(parse_status(b"src/\0?? src/\0"), vec![entry("", Untracked)]);
        // Nothing before the prefix separator (not a repository): nothing.
        assert_eq!(parse_status(b""), vec![]);
        assert_eq!(parse_status(b" M a.rs\0"), vec![]);
        // Garbage records are skipped.
        assert_eq!(parse_status(b"\0x\0M\0 M ok\0"), vec![entry("ok", Modified)]);
    }

    #[test]
    fn caps_the_number_of_entries() {
        let mut out = b"\0".to_vec();
        for i in 0..(MAX_STATUS_ENTRIES + 50) {
            out.extend_from_slice(format!("!! ignored/{i}\0").as_bytes());
        }
        assert_eq!(parse_status(&out).len(), MAX_STATUS_ENTRIES);
    }

    #[test]
    fn reads_modified_staged_untracked_ignored_deleted_and_renamed_files() {
        if !has_git() {
            return;
        }
        let repo = repo();
        let p = repo.path();
        fs::write(p.join("a.txt"), "changed\n").unwrap();
        fs::write(p.join("src/lib.rs"), "staged\n").unwrap();
        git(p, &["add", "src/lib.rs"]);
        fs::write(p.join("new.txt"), "new\n").unwrap();
        fs::write(p.join(".gitignore"), "build/\n*.log\n").unwrap();
        fs::create_dir_all(p.join("build/out")).unwrap();
        fs::write(p.join("build/out/big.bin"), "x").unwrap();
        fs::write(p.join("debug.log"), "x").unwrap();
        git(p, &["mv", "src/deep/x.rs", "src/deep/y.rs"]);
        fs::write(p.join("added.txt"), "added\n").unwrap();
        git(p, &["add", "added.txt"]);
        let got = sorted(status_of(p));
        assert_eq!(
            got,
            sorted(vec![
                entry(".gitignore", Untracked),
                entry("a.txt", Modified),
                entry("added.txt", IndexAdded),
                entry("build/", Ignored),
                entry("debug.log", Ignored),
                entry("new.txt", Untracked),
                entry("src/deep/y.rs", IndexRenamed),
                entry("src/lib.rs", IndexModified),
            ])
        );
        fs::remove_file(p.join("a.txt")).unwrap();
        assert!(status_of(p).contains(&entry("a.txt", Deleted)));
        git(p, &["rm", "-q", "--cached", "src/lib.rs"]);
        assert!(status_of(p).contains(&entry("src/lib.rs", IndexDeleted)));
    }

    #[test]
    fn a_nested_folder_gets_paths_relative_to_it_and_only_its_own() {
        if !has_git() {
            return;
        }
        let repo = repo();
        let p = repo.path();
        fs::write(p.join("a.txt"), "changed\n").unwrap();
        fs::write(p.join("src/deep/x.rs"), "changed\n").unwrap();
        fs::write(p.join("src/new.rs"), "new\n").unwrap();
        assert_eq!(
            sorted(status_of(&p.join("src"))),
            vec![entry("deep/x.rs", Modified), entry("new.rs", Untracked)]
        );
        // Inside an untracked folder, everything is untracked.
        fs::create_dir_all(p.join("fresh/inner")).unwrap();
        fs::write(p.join("fresh/inner/f.txt"), "f").unwrap();
        assert_eq!(status_of(&p.join("fresh/inner")), vec![entry("", Untracked)]);
    }

    #[test]
    fn reads_a_conflict() {
        if !has_git() {
            return;
        }
        let repo = repo();
        let p = repo.path();
        git(p, &["checkout", "-q", "-b", "other"]);
        fs::write(p.join("a.txt"), "other\n").unwrap();
        git(p, &["commit", "-q", "-am", "other"]);
        git(p, &["checkout", "-q", "main"]);
        fs::write(p.join("a.txt"), "main\n").unwrap();
        git(p, &["commit", "-q", "-am", "main"]);
        let merge = Command::new("git")
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "merge",
                "-q",
                "other",
            ])
            .current_dir(p)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(!merge.status.success());
        assert_eq!(status_of(p), vec![entry("a.txt", BothModified)]);
    }

    #[test]
    fn head_text_is_the_committed_text_of_a_tracked_file() {
        let dir = repo();
        let p = dir.path();
        fs::write(p.join("src/lib.rs"), "changed\n").unwrap();
        assert_eq!(head_text_of(&p.join("src/lib.rs")).as_deref(), Some("lib\n"));
        assert_eq!(head_text_of(&p.join("a.txt")).as_deref(), Some("a\n"));
        // Not committed: untracked, outside a repository, or no file at all.
        fs::write(p.join("src/new.rs"), "new\n").unwrap();
        assert_eq!(head_text_of(&p.join("src/new.rs")), None);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("f.txt"), "f").unwrap();
        assert_eq!(head_text_of(&outside.path().join("f.txt")), None);
        assert_eq!(head_text_of(Path::new("/definitely/not/here.txt")), None);
    }

    #[test]
    fn outside_a_repository_or_without_the_folder_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "f").unwrap();
        assert_eq!(status_of(dir.path()), vec![]);
        assert_eq!(status_of(Path::new("/definitely/not/here")), vec![]);
    }

    /// A repository's own `core.fsmonitor` command would run on every status read: a folder from
    /// elsewhere (a downloaded archive with a `.git`) could run anything just by being listed.
    #[test]
    fn status_never_runs_the_repository_fsmonitor_command() {
        let dir = repo();
        let root = dir.path();
        let marker = root.join("ran");
        git(
            root,
            &["config", "core.fsmonitor", &format!("touch '{}'", marker.display())],
        );
        fs::write(root.join("a.txt"), "changed\n").unwrap();
        assert!(!status_of(root).is_empty());
        assert!(repo_of(root).unwrap().dirty);
        assert!(!marker.exists(), "git ran the repository's fsmonitor command");
        let script = super::branch_tests::sh_script(&remote_status_command("/srv/app").unwrap());
        assert!(
            script.contains("git --no-optional-locks -c core.fsmonitor=false -C "),
            "{script}"
        );
    }

    #[test]
    fn the_remote_status_command_quotes_the_folder() {
        use super::branch_tests::sh_script;
        let script = sh_script(&remote_status_command("/srv/it's here").unwrap());
        assert!(script.contains(r"-C '/srv/it'\''s here'"), "{script}");
        assert!(script.contains("--porcelain=v1") && script.ends_with("exit 0"));
        assert_eq!(remote_status_command(""), None);
        assert_eq!(remote_status_command("/a\0b"), None);
    }

    /// What the login shell reads is only `echo <base64> | base64 -d | sh`: fish, bash, zsh and
    /// tcsh all read that the same way, whatever the folder name holds.
    #[test]
    fn the_login_shell_only_sees_base64_letters() {
        for folder in [
            "/tmp/x\\';touch PWN_A;echo '",
            "/tmp/x\\'(touch PWN_B)",
            "/a b/$(c)`d`",
            "x&calc&y",
        ] {
            for command in [remote_command(folder).unwrap(), remote_status_command(folder).unwrap()] {
                let payload = command
                    .strip_prefix("echo ")
                    .and_then(|rest| rest.strip_suffix(" | base64 -d | sh 2>/dev/null"))
                    .unwrap_or_else(|| panic!("{command}"));
                assert!(
                    payload.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)),
                    "{command}"
                );
            }
        }
    }

    /// The fish reproduction from the verifier: a folder name with `\'` runs nothing in fish.
    #[cfg(unix)]
    #[test]
    fn a_hostile_folder_name_runs_nothing_when_the_login_shell_is_fish() {
        let Ok(fish) = which_fish() else { return };
        let outside = tempfile::tempdir().unwrap();
        for folder in ["x\\';touch PWN_A;echo '", "x\\'(touch PWN_B)"] {
            for command in [remote_command(folder).unwrap(), remote_status_command(folder).unwrap()] {
                let out = Command::new(&fish)
                    .args(["--no-config", "-c", &command])
                    .current_dir(outside.path())
                    .output()
                    .unwrap();
                assert!(out.status.success(), "{out:?}");
            }
        }
        for leaked in ["PWN_A", "PWN_B"] {
            assert!(!outside.path().join(leaked).exists(), "{leaked} was created");
        }
    }

    #[cfg(unix)]
    fn which_fish() -> Result<std::path::PathBuf, ()> {
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|p| p.join("fish"))
                    .find(|p| p.is_file())
            })
            .ok_or(())
    }

    /// Run in `sh`, a hostile folder name runs nothing, and the output is the local one.
    #[cfg(unix)]
    #[test]
    fn the_remote_status_command_runs_in_sh_and_a_hostile_name_runs_nothing() {
        let outside = tempfile::tempdir().unwrap();
        let dir = outside.path().join("it's a repo; $(touch pwned) `touch pwned2`");
        fs::create_dir_all(&dir).unwrap();
        let run = |folder: &Path| {
            let out = Command::new("sh")
                .arg("-c")
                .arg(remote_status_command(folder.to_str().unwrap()).unwrap())
                .current_dir(outside.path())
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            parse_status(&out.stdout)
        };
        assert_eq!(run(&dir), vec![]);
        if has_git() {
            git(&dir, &["init", "-q"]);
            fs::create_dir_all(dir.join("sub")).unwrap();
            fs::write(dir.join("sub/new.txt"), "n").unwrap();
            fs::write(dir.join("top.txt"), "t").unwrap();
            assert_eq!(
                sorted(run(&dir)),
                vec![entry("sub/", Untracked), entry("top.txt", Untracked)]
            );
            // Inside the untracked folder, the whole folder is untracked.
            assert_eq!(run(&dir.join("sub")), vec![entry("", Untracked)]);
        }
        for leaked in ["pwned", "pwned2"] {
            assert!(!dir.join(leaked).exists() && !outside.path().join(leaked).exists());
        }
    }
}

/// The SSH variants run their command over the session's connection (v0.2.15 `lib.rs`
/// `ssh_git_branch`, `ssh_git_status`), here against the in-process test server.
mod ssh_tests {
    use crate::FileStatus::*;
    use crate::*;
    use proto_ssh::test_server::{self, PASSWORD, TestServer};
    use proto_ssh::{ConnectRequest, SshAuth, SshManager};
    use term_engine::ByteRing;
    use zeroize::Zeroizing;

    /// A password session on `server`, with a known_hosts in `dir` that trusts it and a missing
    /// SSH agent, so a developer's `SSH_AUTH_SOCK` never changes the test.
    #[allow(clippy::unwrap_used, reason = "a test helper: a failure here is a failed test")]
    async fn session(server: &TestServer, dir: &tempfile::TempDir) -> (SshManager, u32) {
        let known_hosts = dir.path().join("known_hosts");
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            server.addr.port(),
            &server.host_key,
            &known_hosts,
        )
        .unwrap();
        let ssh = SshManager::new(known_hosts).with_agent_socket(dir.path().join("no-agent.sock"));
        let request = ConnectRequest {
            host: "127.0.0.1".into(),
            port: server.addr.port(),
            username: "tester".into(),
            auth: SshAuth::Password {
                password: Zeroizing::new(PASSWORD.into()),
            },
            cols: 80,
            rows: 24,
            trusted_fingerprint: None,
            jumps: vec![],
        };
        let shell = ssh
            .connect_with(request, None, ByteRing::new(ByteRing::SESSION_CAPACITY))
            .await
            .unwrap();
        (ssh, shell.id)
    }

    #[tokio::test]
    async fn the_ssh_branch_is_read_with_the_remote_command_over_the_session() {
        let server = test_server::start(1, None).await;
        let dir = tempfile::tempdir().unwrap();
        let (ssh, id) = session(&server, &dir).await;
        let command = remote_command("/srv/app").unwrap();
        server.reply_to_exec(&command, "feature/x\n");
        assert_eq!(ssh_git_branch(&ssh, id, "/srv/app").await, Some("feature/x".into()));
        assert_eq!(server.exec_commands(), vec![command]);
        // No second login: the command ran on the shell's connection.
        assert_eq!(server.open_connections(), 1);
        // Nothing printed (outside a repository), a failing command, a folder that cannot be passed,
        // or a session that is not there: no branch.
        server.reply_to_exec(&remote_command("/tmp").unwrap(), "");
        assert_eq!(ssh_git_branch(&ssh, id, "/tmp").await, None);
        assert_eq!(ssh_git_branch(&ssh, id, "/no/reply").await, None);
        assert_eq!(ssh_git_branch(&ssh, id, "").await, None);
        assert_eq!(ssh_git_branch(&ssh, id + 1, "/srv/app").await, None);
    }

    #[tokio::test]
    async fn the_ssh_status_is_parsed_like_the_local_one() {
        let server = test_server::start(1, None).await;
        let dir = tempfile::tempdir().unwrap();
        let (ssh, id) = session(&server, &dir).await;
        let command = remote_status_command("/srv/app/src").unwrap();
        server.reply_to_exec(&command, "src/\0 M src/a.rs\0?? src/tmp/\0 M other/b.rs\0");
        assert_eq!(
            ssh_git_status(&ssh, id, "/srv/app/src").await,
            vec![
                StatusEntry {
                    path: "a.rs".into(),
                    status: Modified
                },
                StatusEntry {
                    path: "tmp/".into(),
                    status: Untracked
                },
            ]
        );
        assert_eq!(server.exec_commands(), vec![command]);
        assert_eq!(ssh_git_status(&ssh, id, "/no/reply").await, vec![]);
        assert_eq!(ssh_git_status(&ssh, id, "/a\0b").await, vec![]);
    }
}

#[cfg(unix)]
mod hostile_repository_tests {
    use std::process::Command;

    fn git(dir: &std::path::Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A downloaded archive with a `.git` whose config declares a filter that runs a command.
    fn hostile_repo(marker: &std::path::Path, process: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        git(d, &["init", "-q"]);
        git(d, &["config", "user.email", "t@example.com"]);
        git(d, &["config", "user.name", "t"]);
        std::fs::write(d.join("file.txt"), "content\n").unwrap();
        git(d, &["add", "file.txt"]);
        git(d, &["commit", "-qm", "init"]);
        let command = format!("touch '{}'; cat", marker.display());
        let key = if process {
            "filter.evil.process"
        } else {
            "filter.evil.clean"
        };
        git(d, &["config", key, &command]);
        std::fs::write(d.join(".gitattributes"), "* filter=evil\n").unwrap();
        // A freshly extracted file never matches the index's stat data.
        std::fs::write(d.join("file.txt"), "content\n").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(d.join("file.txt"))
            .unwrap()
            .set_modified(later)
            .unwrap();
        dir
    }

    #[test]
    fn listing_a_hostile_repository_never_runs_its_clean_filter() {
        let scratch = tempfile::tempdir().unwrap();
        let marker = scratch.path().join("ran");
        let repo = hostile_repo(&marker, false);
        let _ = crate::status_of(repo.path());
        let _ = crate::repo_of(repo.path());
        assert!(!marker.exists(), "the repository's clean filter ran");
    }

    #[test]
    fn listing_a_hostile_repository_never_runs_its_process_filter() {
        let scratch = tempfile::tempdir().unwrap();
        let marker = scratch.path().join("ran");
        let repo = hostile_repo(&marker, true);
        let _ = crate::status_of(repo.path());
        assert!(!marker.exists(), "the repository's process filter ran");
    }
}
