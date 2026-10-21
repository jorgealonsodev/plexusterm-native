//! Port of v0.2.15 `src/features/sessions/sshConfigImport.test.ts` (SES-2).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use std::collections::BTreeMap;

use session_library::import::*;
use session_library::*;

fn counter() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("id{n}")
    }
}

fn counts(skipped: &Skipped) -> BTreeMap<String, usize> {
    skipped.entries().iter().cloned().collect()
}

fn map(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// (name, host, port, username, auth, jumps) of an imported SSH session.
type Fields<'a> = (&'a str, &'a str, u16, &'a str, &'a Option<SavedAuth>, &'a [String]);

fn fields(session: &ImportedSession) -> Fields<'_> {
    match session {
        ImportedSession::Ssh {
            name,
            host,
            port,
            username,
            auth,
            jumps,
            ..
        } => (name, host, *port, username, auth, jumps),
        ImportedSession::Telnet { .. } => panic!("not SSH"),
    }
}

fn key(path: &str) -> Option<SavedAuth> {
    Some(SavedAuth::Key { path: path.into() })
}

// describe("parseSshConfig")

#[test]
fn reads_each_concrete_host_with_its_hostname_user_port_and_identityfile() {
    // it("reads each concrete Host with its HostName, User, Port and IdentityFile")
    let parsed = parse_ssh_config(
        &[
            "# my servers",
            "Host web",
            "  HostName 10.0.0.5",
            "  User deploy",
            "  Port 2222",
            "  IdentityFile ~/.ssh/id_web",
            "",
            "Host db",
            "  HostName=db.internal",
        ]
        .join("\n"),
    );
    assert_eq!(
        parsed.sessions,
        [
            ImportedSession::Ssh {
                name: "web".into(),
                folder: SSH_CONFIG_FOLDER.into(),
                host: "10.0.0.5".into(),
                port: 2222,
                username: "deploy".into(),
                auth: key("~/.ssh/id_web"),
                jumps: vec![],
            },
            ImportedSession::Ssh {
                name: "db".into(),
                folder: SSH_CONFIG_FOLDER.into(),
                host: "db.internal".into(),
                port: 22,
                username: String::new(),
                auth: None,
                jumps: vec![],
            },
        ]
    );
    assert!(parsed.skipped.is_empty());
    assert_eq!(SSH_CONFIG_FOLDER, "Imported/SSH config");
}

#[test]
fn uses_the_alias_as_the_host_when_there_is_no_hostname_and_expands_percent_h() {
    // it("uses the alias as the host when there is no HostName, and expands %h")
    let parsed = parse_ssh_config("Host box\nHost *.lan\n  HostName %h.example.org\nHost nas.lan");
    let pairs: Vec<(&str, &str)> = parsed.sessions.iter().map(|s| (fields(s).0, fields(s).1)).collect();
    assert_eq!(pairs, [("box", "box"), ("nas.lan", "nas.lan.example.org")]);
}

#[test]
fn applies_wildcard_blocks_and_global_options_keeping_the_first_value_of_each_option_like_ssh() {
    // it("applies wildcard blocks and global options, keeping the first value of each option like ssh")
    let parsed = parse_ssh_config(
        &[
            "User everyone",
            "Host app",
            "  Port 2200",
            "Host *",
            "  User fallback",
            "  Port 22",
            "  IdentityFile ~/.ssh/default",
        ]
        .join("\n"),
    );
    assert_eq!(parsed.sessions.len(), 1);
    let (name, _, port, username, auth, _) = fields(&parsed.sessions[0]);
    assert_eq!(
        (name, username, port, auth),
        ("app", "everyone", 2200, &key("~/.ssh/default"))
    );
}

#[test]
fn imports_every_alias_of_a_multi_pattern_host_but_skips_wildcard_and_negated_patterns() {
    let parsed = parse_ssh_config("Host a b !c d?\n  HostName shared.example\nHost * !b\n  User u");
    let pairs: Vec<(&str, &str)> = parsed.sessions.iter().map(|s| (fields(s).0, fields(s).3)).collect();
    assert_eq!(pairs, [("a", "u"), ("b", "")]);
}

#[test]
fn keeps_a_quoted_value_with_spaces_and_is_case_insensitive_about_keywords() {
    let parsed = parse_ssh_config("HOST win\n  hostname 10.1.1.1\n  identityfile \"C:/Users/Me/My Keys/id\"");
    let (_, host, _, _, auth, _) = fields(&parsed.sessions[0]);
    assert_eq!((host, auth), ("10.1.1.1", &key("C:/Users/Me/My Keys/id")));
}

#[test]
fn replaces_percent_d_in_identityfile_with_the_home_folder() {
    let parsed = parse_ssh_config("Host h\n  IdentityFile %d/.ssh/id_h");
    assert_eq!(fields(&parsed.sessions[0]).4, &key("~/.ssh/id_h"));
}

#[test]
fn falls_back_to_port_22_for_an_invalid_port() {
    assert_eq!(fields(&parse_ssh_config("Host h\n  Port 99999").sessions[0]).2, 22);
}

#[test]
fn links_proxyjump_hops_to_the_imported_hosts_with_those_names() {
    let parsed = parse_ssh_config("Host bastion\n  HostName 1.2.3.4\nHost inner\n  ProxyJump bastion");
    let (name, _, _, _, _, jumps) = fields(&parsed.sessions[1]);
    assert_eq!((name, jumps), ("inner", &["bastion".to_string()][..]));
    assert!(parsed.skipped.is_empty());
}

#[test]
fn counts_proxyjump_hops_that_are_not_imported_hosts_and_imports_the_host_without_them() {
    // it("counts ProxyJump hops that are not imported hosts, and imports the host without them")
    let parsed = parse_ssh_config("Host inner\n  ProxyJump admin@gw.example.com:2222");
    assert!(fields(&parsed.sessions[0]).5.is_empty());
    assert_eq!(counts(&parsed.skipped), map(&[("jump", 1)]));
}

#[test]
fn refuses_proxyjump_chains_longer_than_3_or_through_the_host_itself() {
    // SES-2: hops that are aliases of the file, at most 3, not itself.
    let long = parse_ssh_config("Host a\nHost b\nHost c\nHost d\nHost t\n  ProxyJump a,b,c,d");
    assert_eq!(counts(&long.skipped), map(&[("jump", 1)]));
    let own = parse_ssh_config("Host t\n  ProxyJump t");
    assert_eq!(counts(&own.skipped), map(&[("jump", 1)]));
    let three = parse_ssh_config("Host a\nHost b\nHost c\nHost t\n  ProxyJump a,b,c");
    assert_eq!(fields(&three.sessions[3]).5, ["a", "b", "c"]);
    // Only the first argument of an option is read, as v0.2.15's `firstArgument`.
    let spaced = parse_ssh_config("Host a\nHost b\nHost t\n  ProxyJump a, b");
    assert_eq!(fields(&spaced.sessions[2]).5, ["a"]);
}

#[test]
fn ignores_proxyjump_none() {
    assert!(parse_ssh_config("Host h\n  ProxyJump none").skipped.is_empty());
}

#[test]
fn counts_include_and_match_lines_and_proxycommand_hosts_which_are_not_followed() {
    // it("counts Include and Match lines and ProxyCommand hosts, which are not followed")
    let parsed = parse_ssh_config(
        &[
            "Include ~/.orbstack/ssh/config",
            "Host h",
            "  ProxyCommand nc -X 5 %h %p",
            "Match host x",
            "  User nobody",
        ]
        .join("\n"),
    );
    let names: Vec<&str> = parsed.sessions.iter().map(ImportedSession::name).collect();
    assert_eq!(names, ["h"]);
    assert_eq!(fields(&parsed.sessions[0]).3, "");
    assert_eq!(
        counts(&parsed.skipped),
        map(&[("include", 1), ("match", 1), ("proxyCommand", 1)])
    );
}

#[test]
fn returns_nothing_for_an_empty_file() {
    assert_eq!(parse_ssh_config(""), ParsedImport::default());
}

// describe("planImport with an ssh config")

#[test]
fn keeps_the_key_auth_and_turns_proxyjump_names_into_the_ids_of_the_saved_sessions() {
    let parsed = parse_ssh_config("Host bastion\n  IdentityFile ~/.ssh/b\nHost inner\n  ProxyJump bastion");
    let plan = plan_import(&SessionLibrary::default(), &parsed, counter());
    assert_eq!(
        plan.to_add,
        [
            LibrarySession::Ssh(SshSession {
                id: "id1".into(),
                name: "bastion".into(),
                folder: SSH_CONFIG_FOLDER.into(),
                host: "bastion".into(),
                port: 22,
                username: String::new(),
                auth: SavedAuth::Key {
                    path: "~/.ssh/b".into()
                },
                jump_ids: vec![],
            }),
            LibrarySession::Ssh(SshSession {
                id: "id2".into(),
                name: "inner".into(),
                folder: SSH_CONFIG_FOLDER.into(),
                host: "inner".into(),
                port: 22,
                username: String::new(),
                auth: SavedAuth::Password,
                jump_ids: vec!["id1".into()],
            }),
        ]
    );
}

#[test]
fn links_a_jump_to_the_session_saved_by_an_earlier_import_when_that_host_is_a_duplicate() {
    let saved = SshSession {
        id: "old".into(),
        name: "bastion".into(),
        folder: SSH_CONFIG_FOLDER.into(),
        host: "bastion".into(),
        port: 22,
        username: String::new(),
        auth: SavedAuth::Password,
        jump_ids: vec![],
    };
    let library = SessionLibrary {
        folders: vec![],
        sessions: vec![saved.into()],
    };
    let plan = plan_import(
        &library,
        &parse_ssh_config("Host bastion\nHost inner\n  ProxyJump bastion"),
        counter(),
    );
    assert_eq!(plan.duplicates, 1);
    let added: Vec<(&str, Vec<String>)> = plan
        .to_add
        .iter()
        .filter_map(LibrarySession::as_ssh)
        .map(|s| (s.name.as_str(), s.jump_ids.clone()))
        .collect();
    assert_eq!(added, [("inner", vec!["old".to_string()])]);
}

// Reading `<home>/.ssh/config` (SessionsPanel.tsx: bounded read, UTF-8 with the BOM dropped).

#[test]
fn reads_the_ssh_config_of_a_home_folder() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".ssh")).unwrap();
    std::fs::write(
        home.path().join(".ssh/config"),
        "\u{feff}Host web\n  HostName 10.0.0.5\n",
    )
    .unwrap();
    let parsed = read_ssh_config_import(home.path()).unwrap();
    assert_eq!(fields(&parsed.sessions[0]).1, "10.0.0.5");
    assert_eq!(decode_ssh_config(b"Host \xff"), "Host \u{fffd}");
}
