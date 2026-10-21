//! Port of v0.2.15 `src/features/sessions/jumpHosts.test.ts` (SSH-3).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use std::cell::RefCell;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use proto_ssh::{PasswordStorage, SshAuth};
use serde_json::Value;
use session_library::jump_hosts::*;
use session_library::library::jump_users;
use session_library::target::SshTarget;
use session_library::*;
use zeroize::Zeroizing;

fn ssh(id: &str) -> SshSession {
    SshSession {
        id: id.into(),
        name: id.into(),
        folder: String::new(),
        host: format!("{id}.example.com"),
        port: 22,
        username: "u".into(),
        auth: SavedAuth::Agent,
        jump_ids: vec![],
    }
}

fn jumps(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn lib(sessions: Vec<LibrarySession>) -> SessionLibrary {
    SessionLibrary {
        folders: vec![],
        sessions,
    }
}

fn chain_ids(result: Result<Vec<&SshSession>, ChainError>) -> Vec<String> {
    result.unwrap().iter().map(|s| s.id.clone()).collect()
}

/// Runs a future whose awaits are all ready at once (the test callbacks never wait).
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the test future waited"),
    }
}

fn auth_text(auth: &SshAuth) -> String {
    match auth {
        SshAuth::Password { password } => format!("password:{}", password.as_str()),
        SshAuth::Key { path, passphrase } => {
            format!("key:{path}:{}", passphrase.as_ref().map(|p| p.as_str()).unwrap_or("-"))
        }
        SshAuth::Agent => "agent".into(),
        SshAuth::StoredPassword { session_id } => format!("stored:{session_id}"),
    }
}

fn storage_text(storage: &Option<PasswordStorage>) -> String {
    match storage {
        None => "-".into(),
        Some(PasswordStorage::Remember { session_id, label }) => format!("remember:{session_id}:{label}"),
        Some(PasswordStorage::Forget { session_id }) => format!("forget:{session_id}"),
    }
}

fn login_text(login: &HopLogin) -> String {
    format!("{} {}", auth_text(&login.auth), storage_text(&login.storage))
}

fn target_text(t: &SshTarget) -> String {
    let jumps: Vec<String> = t
        .jumps
        .iter()
        .map(|j| {
            format!(
                "[{} {}:{} {} {} {} {:?}]",
                j.name,
                j.host,
                j.port,
                j.username,
                auth_text(&j.auth),
                storage_text(&j.storage),
                j.trusted_fingerprint
            )
        })
        .collect();
    format!(
        "{}@{}:{} {} {} jumps={}",
        t.username,
        t.host,
        t.port,
        auth_text(&t.auth),
        storage_text(&t.storage),
        jumps.join("")
    )
}

// describe("saved jump hosts")

#[test]
fn saved_jump_hosts_parse_serialize_and_survive_a_save() {
    // it("parse, serialize and survive a save")
    let target = SshSession {
        jump_ids: jumps(&["a", "b"]),
        ..ssh("t")
    };
    let file = serialize_library(&lib(vec![ssh("a").into(), ssh("b").into(), target.clone().into()]));
    let parsed = parse_library(Some(&file)).unwrap();
    assert_eq!(parsed.sessions[2], LibrarySession::Ssh(target.clone()));
    let saved = SessionLibrary::default().reduce(&LibraryAction::SaveSession(target.clone().into()));
    assert_eq!(saved.sessions[0], LibrarySession::Ssh(target));
}

#[test]
fn saved_jump_hosts_are_left_out_when_empty_so_older_files_load_and_save_unchanged() {
    // it("are left out when empty, so older files load and save unchanged")
    let old = r#"{"version":1,"folders":[],"sessions":[{"id":"s","name":"s","folder":"","host":"h","port":22,"username":"u","auth":{"method":"agent"}}]}"#;
    let parsed = parse_library(Some(old)).unwrap();
    let json: Value = serde_json::from_str(&serialize_library(&parsed)).unwrap();
    assert!(json["sessions"][0].get("jumpIds").is_none());
    let saved = SessionLibrary::default().reduce(&LibraryAction::SaveSession(ssh("x").into()));
    let json: Value = serde_json::from_str(&serialize_library(&saved)).unwrap();
    assert!(json["sessions"][0].get("jumpIds").is_none());
}

#[test]
fn saved_jump_hosts_reject_a_file_whose_jump_hosts_are_not_a_list_of_ids() {
    // it("reject a file whose jump hosts are not a list of ids")
    for bad in [
        serde_json::json!("a"),
        serde_json::json!([1]),
        serde_json::json!([null]),
        serde_json::json!({}),
    ] {
        let mut file: Value = serde_json::from_str(&serialize_library(&lib(vec![ssh("t").into()]))).unwrap();
        file["sessions"][0]["jumpIds"] = bad;
        let error = parse_library(Some(&file.to_string())).unwrap_err();
        assert_eq!(error.to_string(), "sessions file has invalid sessions");
    }
}

// describe("resolveChain")

#[test]
fn resolve_chain_returns_the_jump_sessions_in_order() {
    let t = SshSession {
        jump_ids: jumps(&["b", "a"]),
        ..ssh("t")
    };
    let library = lib(vec![ssh("a").into(), ssh("b").into(), t.clone().into()]);
    assert_eq!(chain_ids(resolve_chain(&library.sessions, &t)), ["b", "a"]);
}

#[test]
fn resolve_chain_is_empty_for_sessions_without_jump_hosts() {
    let library = lib(vec![ssh("t").into()]);
    assert!(resolve_chain(&library.sessions, &ssh("t")).unwrap().is_empty());
}

#[test]
fn resolve_chain_goes_through_the_jump_hosts_of_a_jump_host_first() {
    let inner = SshSession {
        jump_ids: jumps(&["outer"]),
        ..ssh("inner")
    };
    let t = SshSession {
        jump_ids: jumps(&["inner"]),
        ..ssh("t")
    };
    let library = lib(vec![ssh("outer").into(), inner.into(), t.clone().into()]);
    assert_eq!(chain_ids(resolve_chain(&library.sessions, &t)), ["outer", "inner"]);
}

#[test]
fn resolve_chain_rejects_a_session_that_is_its_own_jump_host() {
    let t = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["t"]),
        ..ssh("t")
    };
    let library = lib(vec![t.clone().into()]);
    assert_eq!(
        resolve_chain(&library.sessions, &t).unwrap_err().to_string(),
        r#""web" cannot be its own jump host."#
    );
}

#[test]
fn resolve_chain_rejects_cycles() {
    let a = SshSession {
        jump_ids: jumps(&["b"]),
        ..ssh("a")
    };
    let b = SshSession {
        jump_ids: jumps(&["a"]),
        ..ssh("b")
    };
    let t = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["a"]),
        ..ssh("t")
    };
    let library = lib(vec![a.into(), b.into(), t.clone().into()]);
    let cycle = r#"The jump hosts of "web" form a cycle."#;
    assert_eq!(resolve_chain(&library.sessions, &t).unwrap_err().to_string(), cycle);
    let back = SshSession {
        jump_ids: jumps(&["t"]),
        ..ssh("a")
    };
    let library = lib(vec![back.into(), t.clone().into()]);
    assert_eq!(resolve_chain(&library.sessions, &t).unwrap_err().to_string(), cycle);
}

#[test]
fn resolve_chain_rejects_a_session_used_twice_in_the_same_chain() {
    let b = SshSession {
        jump_ids: jumps(&["a"]),
        ..ssh("b")
    };
    let t = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["a", "b"]),
        ..ssh("t")
    };
    let library = lib(vec![
        SshSession {
            name: "bastion".into(),
            ..ssh("a")
        }
        .into(),
        b.into(),
        t.clone().into(),
    ]);
    assert_eq!(
        resolve_chain(&library.sessions, &t).unwrap_err().to_string(),
        r#""bastion" appears more than once in the jump hosts of "web"."#
    );
}

#[test]
fn resolve_chain_rejects_deleted_and_non_ssh_jump_sessions() {
    let t = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["gone"]),
        ..ssh("t")
    };
    let library = lib(vec![t.clone().into()]);
    assert_eq!(
        resolve_chain(&library.sessions, &t).unwrap_err().to_string(),
        r#"A jump host of "web" no longer exists; edit the session to choose another one."#
    );
    let console = SerialSession {
        id: "c".into(),
        name: "console".into(),
        folder: String::new(),
        settings: SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::None,
        },
    };
    let u = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["c"]),
        ..ssh("u")
    };
    let library = lib(vec![console.into(), u.clone().into()]);
    assert_eq!(
        resolve_chain(&library.sessions, &u).unwrap_err().to_string(),
        r#"Jump host "console" of "web" is not an SSH session."#
    );
}

#[test]
fn resolve_chain_rejects_more_than_3_jump_hosts_in_total() {
    let d = SshSession {
        jump_ids: jumps(&["a", "b"]),
        ..ssh("d")
    };
    let t = SshSession {
        name: "web".into(),
        jump_ids: jumps(&["c", "d"]),
        ..ssh("t")
    };
    let library = lib(vec![
        ssh("a").into(),
        ssh("b").into(),
        ssh("c").into(),
        d.into(),
        t.clone().into(),
    ]);
    assert_eq!(
        resolve_chain(&library.sessions, &t).unwrap_err().to_string(),
        r#""web" goes through more than 3 jump hosts."#
    );
}

// describe("deleting a jump host")

fn delete_library() -> SessionLibrary {
    lib(vec![
        ssh("a").into(),
        SshSession {
            folder: "Gw".into(),
            ..ssh("b")
        }
        .into(),
        SshSession {
            jump_ids: jumps(&["a", "b"]),
            ..ssh("t")
        }
        .into(),
        SshSession {
            jump_ids: jumps(&["b"]),
            ..ssh("u")
        }
        .into(),
    ])
}

#[test]
fn deleting_a_jump_host_lists_the_sessions_that_use_it() {
    let library = delete_library();
    let users = |ids: &[&str]| -> Vec<String> { jump_users(&library, ids).iter().map(|s| s.id.clone()).collect() };
    assert_eq!(users(&["b"]), ["t", "u"]);
    assert!(users(&["t"]).is_empty());
    // Sessions deleted together are not reported as users.
    assert_eq!(users(&["b", "u"]), ["t"]);
}

#[test]
fn deleting_a_jump_host_removes_it_from_every_chain() {
    let next = delete_library().reduce(&LibraryAction::DeleteSession { id: "a".into() });
    let ids: Vec<&str> = next.sessions.iter().map(LibrarySession::id).collect();
    assert_eq!(ids, ["b", "t", "u"]);
    assert_eq!(
        next.find("t"),
        Some(
            &SshSession {
                jump_ids: jumps(&["b"]),
                ..ssh("t")
            }
            .into()
        )
    );
}

#[test]
fn deleting_a_jump_host_removes_the_jump_hosts_of_a_deleted_folder_from_every_chain() {
    let next = delete_library().reduce(&LibraryAction::DeleteFolder { path: "Gw".into() });
    assert_eq!(
        next.find("t"),
        Some(
            &SshSession {
                jump_ids: jumps(&["a"]),
                ..ssh("t")
            }
            .into()
        )
    );
    assert_eq!(
        next.find("u")
            .and_then(LibrarySession::as_ssh)
            .map(|s| s.jump_ids.len()),
        Some(0)
    );
}

// describe("hopLabel")

fn hops() -> Vec<SshSession> {
    vec![
        ssh("bastion"),
        ssh("inner"),
        SshSession {
            username: "deploy".into(),
            host: "10.0.0.1".into(),
            ..ssh("web")
        },
    ]
}

#[test]
fn hop_label_names_jump_hosts_by_position() {
    assert_eq!(hop_label(&hops(), 0), "bastion — jump host 1 of 2");
    assert_eq!(hop_label(&hops(), 1), "inner — jump host 2 of 2");
}

#[test]
fn hop_label_names_the_target_as_user_at_host() {
    assert_eq!(hop_label(&hops(), 2), "deploy@10.0.0.1");
    assert_eq!(hop_label(&hops()[2..], 0), "deploy@10.0.0.1");
}

// describe("collectLogins")

fn bastion() -> SshSession {
    SshSession {
        auth: SavedAuth::Password,
        ..ssh("bastion")
    }
}

fn saved() -> SshSession {
    SshSession {
        auth: SavedAuth::Password,
        ..ssh("saved")
    }
}

fn keyed() -> SshSession {
    SshSession {
        auth: SavedAuth::Key {
            path: "~/.ssh/id".into(),
        },
        ..ssh("keyed")
    }
}

#[test]
fn collect_logins_uses_stored_passwords_asks_for_the_others_one_by_one_and_keeps_key_agent_auth() {
    // it("uses stored passwords, asks for the others one by one and keeps key/agent auth")
    let asked = RefCell::new(Vec::new());
    let logins = block_on(collect_logins(
        &[bastion(), saved(), keyed()],
        async |id: &str| id == "saved",
        async |index: usize| {
            asked.borrow_mut().push(index);
            Some(PasswordAnswer {
                password: Zeroizing::new("pw".into()),
                remember: Some(true),
            })
        },
    ))
    .unwrap();
    assert_eq!(*asked.borrow(), [0]);
    let texts: Vec<String> = logins.iter().map(login_text).collect();
    assert_eq!(
        texts,
        [
            "password:pw remember:bastion:PlexusTerm: u@bastion.example.com:22",
            "stored:saved -",
            "key:~/.ssh/id:- -",
        ]
    );
}

#[test]
fn without_prompts_gives_every_password_hop_its_saved_password_slot_so_the_backend_tries_keys_first() {
    // it("without prompts, gives every password hop its saved password slot so the backend tries keys first")
    let texts: Vec<String> = saved_logins(&[bastion(), saved(), keyed()])
        .iter()
        .map(login_text)
        .collect();
    assert_eq!(texts, ["stored:bastion -", "stored:saved -", "key:~/.ssh/id:- -"]);
}

#[test]
fn collect_logins_stops_at_the_first_cancelled_prompt() {
    let calls = RefCell::new(0);
    let logins = block_on(collect_logins(
        &[
            bastion(),
            SshSession {
                auth: SavedAuth::Password,
                ..ssh("t")
            },
        ],
        async |_: &str| false,
        async |_: usize| {
            *calls.borrow_mut() += 1;
            None
        },
    ));
    assert!(logins.is_none());
    assert_eq!(*calls.borrow(), 1);
}

// describe("chainTarget")

#[test]
fn chain_target_puts_the_jump_hosts_with_their_own_logins_in_front_of_the_target() {
    // it("puts the jump hosts, with their own logins, in front of the target")
    let hops = [
        SshSession {
            port: 2222,
            ..ssh("bastion")
        },
        SshSession {
            auth: SavedAuth::Password,
            ..ssh("web")
        },
    ];
    let logins = vec![
        HopLogin {
            auth: SshAuth::StoredPassword {
                session_id: "bastion".into(),
            },
            storage: None,
        },
        HopLogin {
            auth: SshAuth::Password {
                password: Zeroizing::new("pw".into()),
            },
            storage: Some(PasswordStorage::Forget {
                session_id: "web".into(),
            }),
        },
    ];
    let target = chain_target(&hops, logins).unwrap();
    assert_eq!(
        target_text(&target),
        "u@web.example.com:22 password:pw forget:web jumps=[bastion bastion.example.com:2222 u stored:bastion - None]"
    );
}

#[test]
fn chain_target_adds_no_jumps_for_a_single_hop() {
    let target = chain_target(
        &[ssh("web")],
        vec![HopLogin {
            auth: SshAuth::Agent,
            storage: None,
        }],
    )
    .unwrap();
    assert!(target.jumps.is_empty());
}

#[test]
fn chain_target_refuses_logins_that_do_not_match_the_hops() {
    // Rust-only: v0.2.15 would read past the end of the list.
    assert!(chain_target(&[], vec![]).is_none());
    assert!(
        chain_target(
            &[ssh("a"), ssh("b")],
            vec![HopLogin {
                auth: SshAuth::Agent,
                storage: None
            }]
        )
        .is_none()
    );
}
