//! Port of v0.2.15 `src/features/sessions/library.test.ts`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use serde_json::Value;
use session_library::library::ssh_command_of;
use session_library::target::{LocalTarget, SessionTarget, direct_target, local_target, serial_target, ssh_target};
use session_library::*;

fn session() -> SshSession {
    SshSession {
        id: "s1".into(),
        name: "web".into(),
        folder: String::new(),
        host: "web.example.com".into(),
        port: 22,
        username: "deploy".into(),
        auth: SavedAuth::Agent,
        jump_ids: vec![],
    }
}

fn serial() -> SerialSession {
    SerialSession {
        id: "c1".into(),
        name: "switch console".into(),
        folder: "Lab".into(),
        settings: SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::None,
        },
    }
}

fn telnet() -> TelnetSession {
    TelnetSession {
        id: "t1".into(),
        name: "old switch".into(),
        folder: "Lab".into(),
        host: "switch.lan".into(),
        port: 23,
    }
}

fn mosh() -> MoshSession {
    MoshSession {
        id: "m1".into(),
        name: "web (mosh)".into(),
        folder: String::new(),
        ssh_session_id: "s1".into(),
        server_command: None,
    }
}

fn local() -> LocalSession {
    LocalSession {
        id: "l1".into(),
        name: "PowerShell admin".into(),
        folder: "Local".into(),
        program: Some("C:\\Program Files\\PowerShell\\7\\pwsh.exe".into()),
        args: vec!["-NoLogo".into()],
        cwd: Some("~\\src".into()),
    }
}

fn save(lib: &SessionLibrary, s: impl Into<LibrarySession>) -> SessionLibrary {
    lib.reduce(&LibraryAction::SaveSession(s.into()))
}

fn ids(lib: &SessionLibrary) -> Vec<&str> {
    lib.sessions.iter().map(LibrarySession::id).collect()
}

fn names<'a>(nodes: &'a [TreeNode<'_>]) -> Vec<&'a str> {
    nodes.iter().map(TreeNode::name).collect()
}

fn children<'a, 'b>(node: &'a TreeNode<'b>) -> &'a [TreeNode<'b>] {
    match node {
        TreeNode::Folder { children, .. } => children,
        TreeNode::Session { .. } => &[],
    }
}

/// `{ version: 1, folders: [], sessions: [entry] }` with `entry` changed by `edit`.
fn file_with(entry: impl Into<LibrarySession>, edit: impl FnOnce(&mut serde_json::Map<String, Value>)) -> String {
    let lib = SessionLibrary {
        folders: vec![],
        sessions: vec![entry.into()],
    };
    let mut json: Value = serde_json::from_str(&serialize_library(&lib)).unwrap();
    let entry = json["sessions"][0].as_object_mut().unwrap();
    edit(entry);
    json.to_string()
}

fn sessions_json(lib: &SessionLibrary) -> Vec<Value> {
    let json: Value = serde_json::from_str(&serialize_library(lib)).unwrap();
    json["sessions"].as_array().unwrap().clone()
}

// describe("normalizeFolder")

#[test]
fn normalize_folder_trims_segments_and_drops_empty_ones() {
    // it("trims segments and drops empty ones")
    assert_eq!(normalize_folder(" Prod / / DB/ "), "Prod/DB");
    assert_eq!(normalize_folder("   "), "");
}

// describe("libraryReducer")

#[test]
fn adds_a_session_and_registers_its_folder_and_ancestors() {
    let lib = save(
        &SessionLibrary::default(),
        SshSession {
            folder: "Prod/DB".into(),
            ..session()
        },
    );
    assert_eq!(lib.sessions.len(), 1);
    assert_eq!(lib.folders, ["Prod", "Prod/DB"]);
}

#[test]
fn duplicates_a_session_next_to_it_with_a_new_id_and_a_copy_name() {
    // it("duplicates a session next to it with a new id and a (copy) name")
    let lib = save(
        &save(
            &SessionLibrary::default(),
            SshSession {
                id: "1".into(),
                name: "web".into(),
                ..session()
            },
        ),
        SshSession {
            id: "2".into(),
            name: "db".into(),
            ..session()
        },
    );
    let next = lib.reduce(&LibraryAction::DuplicateSession {
        id: "1".into(),
        new_id: "9".into(),
    });
    let pairs: Vec<(&str, &str)> = next.sessions.iter().map(|s| (s.id(), s.name())).collect();
    assert_eq!(pairs, [("1", "web"), ("9", "web (copy)"), ("2", "db")]);
    let mut copy = next.sessions[1].clone();
    let LibrarySession::Ssh(inner) = &mut copy else {
        panic!("not ssh")
    };
    inner.id = "1".into();
    inner.name = "web".into();
    assert_eq!(copy, next.sessions[0]);
    assert_eq!(
        lib.reduce(&LibraryAction::DuplicateSession {
            id: "nope".into(),
            new_id: "9".into()
        }),
        lib
    );
}

#[test]
fn updates_an_existing_session_in_place_by_id() {
    let lib = save(
        &save(&SessionLibrary::default(), session()),
        SshSession {
            name: "web-1".into(),
            ..session()
        },
    );
    let names: Vec<&str> = lib.sessions.iter().map(LibrarySession::name).collect();
    assert_eq!(names, ["web-1"]);
}

#[test]
fn saves_several_sessions_in_one_action_like_saving_them_one_by_one() {
    let batch: Vec<LibrarySession> = vec![
        SshSession {
            id: "1".into(),
            folder: "M/A".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "2".into(),
            folder: "M".into(),
            ..session()
        }
        .into(),
    ];
    let start = save(
        &SessionLibrary::default(),
        SshSession {
            id: "0".into(),
            ..session()
        },
    );
    let lib = start.reduce(&LibraryAction::SaveSessions(batch.clone()));
    let one_by_one = batch.into_iter().fold(start, |acc, s| save(&acc, s));
    assert_eq!(lib, one_by_one);
    assert_eq!(lib.folders, ["M", "M/A"]);
}

#[test]
fn deletes_a_session() {
    let lib = save(&SessionLibrary::default(), session()).reduce(&LibraryAction::DeleteSession { id: "s1".into() });
    assert!(lib.sessions.is_empty());
}

#[test]
fn adds_folders_without_duplicates() {
    let lib = SessionLibrary::default().reduce(&LibraryAction::AddFolder { path: "A/B".into() });
    let lib = lib.reduce(&LibraryAction::AddFolder { path: " A ".into() });
    assert_eq!(lib.folders, ["A", "A/B"]);
}

#[test]
fn deleting_a_folder_removes_its_subfolders_and_sessions_only() {
    let mut lib = save(
        &SessionLibrary::default(),
        SshSession {
            id: "1".into(),
            folder: "A/B".into(),
            ..session()
        },
    );
    lib = save(
        &lib,
        SshSession {
            id: "2".into(),
            folder: "AB".into(),
            ..session()
        },
    );
    lib = save(
        &lib,
        SshSession {
            id: "3".into(),
            folder: "A".into(),
            ..session()
        },
    );
    lib = lib.reduce(&LibraryAction::DeleteFolder { path: "A".into() });
    assert_eq!(lib.folders, ["AB"]);
    assert_eq!(ids(&lib), ["2"]);
}

// describe("parseLibrary / serializeLibrary")

#[test]
fn treats_a_missing_file_as_an_empty_library() {
    assert_eq!(parse_library(None), Ok(SessionLibrary::default()));
}

#[test]
fn round_trips_through_json() {
    let lib = save(
        &SessionLibrary::default(),
        SshSession {
            folder: "X".into(),
            auth: SavedAuth::Key {
                path: "~/.ssh/id".into(),
            },
            ..session()
        },
    );
    assert_eq!(parse_library(Some(&serialize_library(&lib))), Ok(lib));
}

#[test]
fn reports_corrupt_or_unsupported_files_instead_of_returning_an_empty_library() {
    assert!(matches!(
        parse_library(Some("{oops")),
        Err(LibraryError::InvalidJson(_))
    ));
    assert_eq!(
        parse_library(Some(r#"{"version":2,"folders":[],"sessions":[]}"#)),
        Err(LibraryError::UnsupportedVersion)
    );
    assert_eq!(
        parse_library(Some(r#"{"version":1,"folders":[],"sessions":[{"id":"x"}]}"#)),
        Err(LibraryError::InvalidSessions)
    );
}

#[test]
fn file_errors_carry_the_v0_2_15_messages() {
    // SES-1i: the messages shown in the panel.
    assert!(
        parse_library(Some("{oops"))
            .unwrap_err()
            .to_string()
            .starts_with("sessions file is not valid JSON: ")
    );
    assert_eq!(
        LibraryError::UnsupportedVersion.to_string(),
        "unsupported sessions file version"
    );
    assert_eq!(
        parse_library(Some(r#"{"version":1,"folders":[1],"sessions":[]}"#))
            .unwrap_err()
            .to_string(),
        "sessions file has invalid folders"
    );
    assert_eq!(
        LibraryError::InvalidSessions.to_string(),
        "sessions file has invalid sessions"
    );
    // An entry with a kind other than the five is invalid: the whole file is refused.
    assert_eq!(
        parse_library(Some(
            r#"{"version":1,"folders":[],"sessions":[{"kind":"rdp","id":"x","name":"x","folder":""}]}"#
        )),
        Err(LibraryError::InvalidSessions)
    );
}

#[test]
fn loads_files_written_before_serial_sessions_existed_no_kind_as_ssh_sessions() {
    // it("loads files written before serial sessions existed (no kind) as SSH sessions")
    let legacy = r#"{"version":1,"folders":["Prod"],"sessions":[{"id":"old","name":"web","folder":"Prod","host":"web.example.com","port":22,"username":"deploy","auth":{"method":"key","path":"~/.ssh/id"}}]}"#;
    let lib = parse_library(Some(legacy)).unwrap();
    assert_eq!(
        lib.sessions,
        [LibrarySession::Ssh(SshSession {
            id: "old".into(),
            name: "web".into(),
            folder: "Prod".into(),
            host: "web.example.com".into(),
            port: 22,
            username: "deploy".into(),
            auth: SavedAuth::Key {
                path: "~/.ssh/id".into()
            },
            jump_ids: vec![],
        })]
    );
    assert_eq!(sessions_json(&lib)[0]["kind"], "ssh");
}

#[test]
fn round_trips_serial_sessions_and_writes_their_kind() {
    let mut console = serial();
    console.settings.parity = SerialParity::Even;
    console.settings.stop_bits = 2;
    console.settings.flow = SerialFlow::Software;
    let lib = save(&save(&SessionLibrary::default(), session()), console);
    let kinds: Vec<Value> = sessions_json(&lib).iter().map(|s| s["kind"].clone()).collect();
    assert_eq!(kinds, ["ssh", "serial"]);
    assert_eq!(parse_library(Some(&serialize_library(&lib))), Ok(lib));
}

#[test]
fn rejects_serial_sessions_with_invalid_fields() {
    let bad: Vec<(&str, Value)> = vec![
        ("path", 5.into()),
        ("baud", 0.into()),
        ("baud", 9600.5.into()),
        ("dataBits", 9.into()),
        ("parity", "mark".into()),
        ("stopBits", 3.into()),
        ("flow", "rts".into()),
        ("kind", "telnet".into()),
    ];
    for (key, value) in bad {
        let file = file_with(serial(), |entry| {
            entry.insert(key.into(), value.clone());
        });
        assert_eq!(
            parse_library(Some(&file)),
            Err(LibraryError::InvalidSessions),
            "{key}: {value}"
        );
    }
}

#[test]
fn round_trips_telnet_and_mosh_sessions_and_writes_their_kind() {
    let entries: Vec<LibrarySession> = vec![
        session().into(),
        TelnetSession { port: 2323, ..telnet() }.into(),
        MoshSession {
            server_command: Some("mosh-server new -p 60010".into()),
            ..mosh()
        }
        .into(),
        MoshSession {
            id: "m2".into(),
            ..mosh()
        }
        .into(),
    ];
    let lib = entries
        .into_iter()
        .fold(SessionLibrary::default(), |acc, s| save(&acc, s));
    let json = sessions_json(&lib);
    let kinds: Vec<Value> = json.iter().map(|s| s["kind"].clone()).collect();
    assert_eq!(kinds, ["ssh", "telnet", "mosh", "mosh"]);
    assert_eq!(
        json[3],
        serde_json::json!({ "kind": "mosh", "id": "m2", "name": "web (mosh)", "folder": "", "sshSessionId": "s1" })
    );
    assert_eq!(parse_library(Some(&serialize_library(&lib))), Ok(lib));
}

#[test]
fn drops_an_empty_mosh_server_command_and_stray_fields() {
    // Stray fields cannot exist in the Rust model; a stray `password` in the file is dropped on load.
    let file = file_with(
        MoshSession {
            server_command: Some("  ".into()),
            ..mosh()
        },
        |entry| {
            entry.insert("password".into(), "hunter2".into());
        },
    );
    let json = serialize_library(&parse_library(Some(&file)).unwrap());
    assert!(!json.contains("hunter2"));
    let saved = save(
        &SessionLibrary::default(),
        MoshSession {
            server_command: Some("  ".into()),
            ..mosh()
        },
    );
    assert!(sessions_json(&saved)[0].get("serverCommand").is_none());
}

#[test]
fn rejects_telnet_and_mosh_sessions_with_invalid_fields() {
    let cases: Vec<(LibrarySession, &str, Value)> = vec![
        (telnet().into(), "host", 5.into()),
        (telnet().into(), "port", 0.into()),
        (telnet().into(), "port", 70000.into()),
        (telnet().into(), "port", 23.5.into()),
        (mosh().into(), "sshSessionId", 1.into()),
        (mosh().into(), "serverCommand", 3.into()),
    ];
    for (entry, key, value) in cases {
        let file = file_with(entry, |e| {
            e.insert(key.into(), value.clone());
        });
        assert_eq!(
            parse_library(Some(&file)),
            Err(LibraryError::InvalidSessions),
            "{key}: {value}"
        );
    }
}

#[test]
fn treats_only_ssh_sessions_as_ssh_telnet_and_mosh_are_not_jump_hosts() {
    // it("treats only SSH sessions as SSH (Telnet and Mosh are not jump hosts)")
    let all: Vec<LibrarySession> = vec![session().into(), serial().into(), telnet().into(), mosh().into()];
    assert_eq!(
        all.iter().map(LibrarySession::is_ssh).collect::<Vec<_>>(),
        [true, false, false, false]
    );
    let target = SshSession {
        id: "s2".into(),
        jump_ids: vec!["t1".into()],
        ..session()
    };
    let lib = save(&save(&SessionLibrary::default(), telnet()), target.clone());
    assert_eq!(
        resolve_chain(&lib.sessions, &target).unwrap_err().to_string(),
        r#"Jump host "old switch" of "web" is not an SSH session."#
    );
}

#[test]
fn never_serializes_passwords() {
    // A password in the file is dropped on load and never written back.
    let file = file_with(
        SshSession {
            auth: SavedAuth::Password,
            ..session()
        },
        |entry| {
            entry.insert(
                "auth".into(),
                serde_json::json!({ "method": "password", "password": "hunter2" }),
            );
        },
    );
    let json = serialize_library(&parse_library(Some(&file)).unwrap());
    assert!(!json.contains("hunter2"));
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["sessions"][0]["auth"],
        serde_json::json!({ "method": "password" })
    );
}

// describe("buildTree")

fn tree_library() -> SessionLibrary {
    let mut router = serial();
    router.id = "5".into();
    router.name = "router".into();
    router.folder = "Prod".into();
    router.settings.path = "/dev/ttyACM3".into();
    let entries: Vec<LibrarySession> = vec![
        SshSession {
            id: "1".into(),
            name: "zeta".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "2".into(),
            name: "alpha".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "3".into(),
            name: "db".into(),
            folder: "Prod".into(),
            host: "10.0.0.9".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "4".into(),
            name: "cache".into(),
            folder: "Prod/Infra".into(),
            ..session()
        }
        .into(),
        router.into(),
    ];
    let start = SessionLibrary::default().reduce(&LibraryAction::AddFolder { path: "Empty".into() });
    entries.into_iter().fold(start, |acc, s| save(&acc, s))
}

#[test]
fn nests_folders_first_then_sessions_each_sorted_by_name() {
    let lib = tree_library();
    let tree = build_tree(&lib, "");
    assert_eq!(names(&tree), ["Empty", "Prod", "alpha", "zeta"]);
    assert_eq!(names(children(&tree[1])), ["Infra", "db", "router"]);
}

#[test]
fn filters_by_name_or_host_and_keeps_only_folders_with_matches() {
    let lib = tree_library();
    assert_eq!(names(&build_tree(&lib, "10.0.0")), ["Prod"]);
    let tree = build_tree(&lib, "CACHE");
    assert_eq!(names(children(&tree[0])), ["Infra"]);
}

#[test]
fn matches_serial_sessions_by_device_path() {
    let lib = tree_library();
    let tree = build_tree(&lib, "ttyacm3");
    assert_eq!(names(children(&tree[0])), ["router"]);
}

#[test]
fn sorts_names_case_insensitively() {
    // SES-1g: each group sorted by name case-insensitively.
    let entries: Vec<LibrarySession> = vec![
        SshSession {
            id: "1".into(),
            name: "beta".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "2".into(),
            name: "Alpha".into(),
            ..session()
        }
        .into(),
        SshSession {
            id: "3".into(),
            name: "alpha2".into(),
            ..session()
        }
        .into(),
    ];
    let lib = entries
        .into_iter()
        .fold(SessionLibrary::default(), |acc, s| save(&acc, s));
    assert_eq!(names(&build_tree(&lib, "")), ["Alpha", "alpha2", "beta"]);
}

// describe("toTarget")

fn auth_text(auth: &proto_ssh::SshAuth) -> String {
    match auth {
        proto_ssh::SshAuth::Password { password } => format!("password:{}", password.as_str()),
        proto_ssh::SshAuth::Key { path, passphrase } => {
            format!("key:{path}:{}", passphrase.as_ref().map(|p| p.as_str()).unwrap_or("-"))
        }
        proto_ssh::SshAuth::Agent => "agent".into(),
        proto_ssh::SshAuth::StoredPassword { session_id } => format!("stored:{session_id}"),
    }
}

#[test]
fn adds_the_password_typed_at_connect_time_only_for_password_sessions() {
    let target = ssh_target(
        &SshSession {
            auth: SavedAuth::Password,
            ..session()
        },
        Some("pw"),
    );
    assert_eq!(auth_text(&target.auth), "password:pw");
    let target = ssh_target(
        &SshSession {
            auth: SavedAuth::Key { path: "/k".into() },
            ..session()
        },
        None,
    );
    assert_eq!(auth_text(&target.auth), "key:/k:-");
    let target = ssh_target(&session(), None);
    assert_eq!(
        (
            target.host.as_str(),
            target.port,
            target.username.as_str(),
            auth_text(&target.auth)
        ),
        ("web.example.com", 22, "deploy", "agent".to_string())
    );
    assert!(target.storage.is_none() && target.jumps.is_empty());
}

#[test]
fn turns_a_telnet_session_into_a_telnet_target() {
    let target = direct_target(&TelnetSession { port: 2323, ..telnet() }.into());
    assert!(matches!(target, Some(SessionTarget::Telnet { host, port: 2323 }) if host == "switch.lan"));
}

#[test]
fn turns_a_serial_session_into_a_serial_target() {
    let mut console = serial();
    console.settings.flow = SerialFlow::Hardware;
    assert_eq!(
        serial_target(&console),
        SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::Hardware,
        }
    );
}

#[test]
fn mosh_sessions_have_no_direct_target() {
    // Mosh sessions need their SSH login resolved first (library.ts `DirectSession`).
    assert!(direct_target(&mosh().into()).is_none());
}

// describe("saved local sessions (local shells S4)")

#[test]
fn round_trips_a_local_session_and_writes_its_kind() {
    let lib = save(&SessionLibrary::default(), local());
    assert_eq!(
        sessions_json(&lib)[0],
        serde_json::json!({
            "kind": "local",
            "id": "l1",
            "name": "PowerShell admin",
            "folder": "Local",
            "program": "C:\\Program Files\\PowerShell\\7\\pwsh.exe",
            "args": ["-NoLogo"],
            "cwd": "~\\src",
        })
    );
    assert_eq!(parse_library(Some(&serialize_library(&lib))), Ok(lib));
}

#[test]
fn keeps_a_local_session_with_the_system_default_shell_as_just_its_name_and_folder() {
    let lib = save(
        &SessionLibrary::default(),
        LocalSession {
            program: Some("  ".into()),
            args: vec![],
            cwd: Some(" ".into()),
            ..local()
        },
    );
    assert_eq!(
        sessions_json(&lib)[0],
        serde_json::json!({ "kind": "local", "id": "l1", "name": "PowerShell admin", "folder": "Local" })
    );
}

#[test]
fn local_arguments_are_kept_only_with_a_program() {
    // SES-1h: args are kept only when a program is set.
    let lib = save(
        &SessionLibrary::default(),
        LocalSession {
            program: None,
            ..local()
        },
    );
    assert!(sessions_json(&lib)[0].get("args").is_none());
}

#[test]
fn rejects_local_sessions_with_invalid_fields() {
    let cases: Vec<(&str, Option<Value>)> = vec![
        ("program", Some(7.into())),
        ("args", Some("-NoLogo".into())),
        ("args", Some(serde_json::json!([1]))),
        ("cwd", Some(false.into())),
        ("name", None),
    ];
    for (key, value) in cases {
        let file = file_with(local(), |entry| match &value {
            Some(v) => {
                entry.insert(key.into(), v.clone());
            }
            None => {
                entry.remove(key);
            }
        });
        assert_eq!(
            parse_library(Some(&file)),
            Err(LibraryError::InvalidSessions),
            "{key}: {value:?}"
        );
    }
}

#[test]
fn finds_local_sessions_by_name_or_program_and_opens_them_as_local_targets() {
    let lib = save(&save(&SessionLibrary::default(), local()), session());
    let found = |filter: &str| -> Vec<String> {
        build_tree(&lib, filter)
            .iter()
            .flat_map(|n| match n {
                TreeNode::Folder { children, .. } => children.iter().map(|c| c.name().to_string()).collect(),
                TreeNode::Session { name, .. } => vec![name.clone()],
            })
            .collect()
    };
    assert_eq!(found("admin"), ["PowerShell admin"]);
    assert_eq!(found("pwsh"), ["PowerShell admin"]);
    assert_eq!(
        local_target(&local()),
        LocalTarget {
            program: Some("C:\\Program Files\\PowerShell\\7\\pwsh.exe".into()),
            args: vec!["-NoLogo".into()],
            cwd: Some("~\\src".into()),
        }
    );
    assert_eq!(
        local_target(&LocalSession {
            program: None,
            args: vec![],
            cwd: None,
            ..local()
        }),
        LocalTarget::default()
    );
    assert!(!LibrarySession::from(local()).is_ssh());
}

// SES-1c: Copy SSH command (library.ts `sshCommandOf`; the v0.2.15 test lives in SessionsPanel.test.tsx).

/// A user name starting with `-` would be read by ssh as an option (`-oProxyCommand=...`).
#[test]
fn a_copied_ssh_command_never_lets_the_destination_read_as_an_option() {
    let hostile = SshSession {
        username: "-oProxyCommand=touch /tmp/x".into(),
        ..session()
    };
    assert_eq!(
        ssh_command_of(&hostile),
        "ssh -p 22 -- '-oProxyCommand=touch /tmp/x@web.example.com'"
    );
}

#[test]
fn copies_an_ssh_session_as_an_ssh_command_shell_quoted_without_jump_hosts() {
    assert_eq!(ssh_command_of(&session()), "ssh -p 22 deploy@web.example.com");
    let keyed = SshSession {
        port: 2222,
        auth: SavedAuth::Key {
            path: "/home/me/my keys/id".into(),
        },
        jump_ids: vec!["b".into()],
        ..session()
    };
    assert_eq!(
        ssh_command_of(&keyed),
        "ssh -p 2222 -i '/home/me/my keys/id' deploy@web.example.com"
    );
    let quote = SshSession {
        username: "o'neil".into(),
        ..session()
    };
    assert_eq!(ssh_command_of(&quote), r#"ssh -p 22 'o'\''neil@web.example.com'"#);
}
