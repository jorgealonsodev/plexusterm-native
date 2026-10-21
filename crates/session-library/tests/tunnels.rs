//! Port of v0.2.15 `src/features/tunnels/tunnels.test.ts` and `tunnelForm.test.ts` (TUN-1a).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use session_library::tunnel_form::*;
use session_library::tunnel_library::*;

fn tunnel() -> TunnelDefinition {
    TunnelDefinition {
        id: "t1".into(),
        name: "db".into(),
        kind: TunnelKind::Local,
        session_id: "s1".into(),
        listen_host: "127.0.0.1".into(),
        listen_port: 5433,
        dest_host: "db.internal".into(),
        dest_port: 5432,
    }
}

fn file(tunnels: serde_json::Value) -> String {
    serde_json::json!({ "version": 1, "tunnels": tunnels }).to_string()
}

fn as_json(t: &TunnelDefinition) -> serde_json::Value {
    serde_json::to_value(t).unwrap()
}

// describe("tunnelsReducer")

#[test]
fn adds_replaces_and_deletes_tunnels_by_id() {
    let list = TunnelList::default().reduce(&TunnelAction::Save(tunnel()));
    let list = list.reduce(&TunnelAction::Save(TunnelDefinition {
        id: "t2".into(),
        name: "web".into(),
        ..tunnel()
    }));
    let list = list.reduce(&TunnelAction::Save(TunnelDefinition {
        name: "renamed".into(),
        ..tunnel()
    }));
    let names: Vec<&str> = list.tunnels.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["renamed", "web"]);
    let list = list.reduce(&TunnelAction::Delete { id: "t1".into() });
    let ids: Vec<&str> = list.tunnels.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["t2"]);
}

// describe("parseTunnels / serializeTunnels")

#[test]
fn treats_a_missing_file_as_an_empty_list() {
    assert_eq!(parse_tunnels(None), Ok(TunnelList::default()));
}

#[test]
fn round_trips_and_drops_fields_that_are_not_part_of_the_saved_shape() {
    let mut entry = as_json(&tunnel());
    entry["password"] = "secret".into();
    let parsed = parse_tunnels(Some(&file(serde_json::json!([entry])))).unwrap();
    let json = serialize_tunnels(&parsed);
    assert!(!json.contains("secret"));
    assert_eq!(
        parse_tunnels(Some(&json)),
        Ok(TunnelList {
            tunnels: vec![tunnel()]
        })
    );
}

#[test]
fn reports_corrupt_files_as_errors_instead_of_emptying_them() {
    assert!(matches!(
        parse_tunnels(Some("{oops")),
        Err(TunnelFileError::InvalidJson(_))
    ));
    assert_eq!(
        parse_tunnels(Some(r#"{"version":2,"tunnels":[]}"#)),
        Err(TunnelFileError::UnsupportedVersion)
    );
    assert_eq!(
        parse_tunnels(Some(r#"{"version":1,"tunnels":[{"id":"x"}]}"#)),
        Err(TunnelFileError::InvalidTunnels)
    );
    let mut bad_type = as_json(&tunnel());
    bad_type["type"] = "other".into();
    assert_eq!(
        parse_tunnels(Some(&file(serde_json::json!([bad_type])))),
        Err(TunnelFileError::InvalidTunnels)
    );
    let mut bad_port = as_json(&tunnel());
    bad_port["listenPort"] = 70000.into();
    assert_eq!(
        parse_tunnels(Some(&file(serde_json::json!([bad_port])))),
        Err(TunnelFileError::InvalidTunnels)
    );
}

#[test]
fn file_errors_carry_the_v0_2_15_messages() {
    assert!(
        parse_tunnels(Some("{oops"))
            .unwrap_err()
            .to_string()
            .starts_with("tunnels file is not valid JSON: ")
    );
    assert_eq!(
        TunnelFileError::UnsupportedVersion.to_string(),
        "unsupported tunnels file version"
    );
    assert_eq!(
        TunnelFileError::InvalidTunnels.to_string(),
        "tunnels file has invalid tunnels"
    );
}

#[test]
fn accepts_dynamic_tunnels_without_a_destination() {
    let dynamic = TunnelDefinition {
        kind: TunnelKind::Dynamic,
        dest_host: String::new(),
        dest_port: 0,
        ..tunnel()
    };
    let parsed = parse_tunnels(Some(&file(serde_json::json!([as_json(&dynamic)]))));
    assert_eq!(parsed, Ok(TunnelList { tunnels: vec![dynamic] }));
}

#[test]
fn writes_the_v0_2_15_shape() {
    // DATA-1a: `{version:1, tunnels}` written by `JSON.stringify(_, null, 2)`.
    let json = serialize_tunnels(&TunnelList {
        tunnels: vec![tunnel()],
    });
    assert_eq!(
        json,
        "{\n  \"version\": 1,\n  \"tunnels\": [\n    {\n      \"id\": \"t1\",\n      \"name\": \"db\",\n      \"type\": \"local\",\n      \"sessionId\": \"s1\",\n      \"listenHost\": \"127.0.0.1\",\n      \"listenPort\": 5433,\n      \"destHost\": \"db.internal\",\n      \"destPort\": 5432\n    }\n  ]\n}"
    );
}

#[test]
fn loads_and_saves_tunnels_json_in_the_config_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = tunnels_path(dir.path());
    assert!(path.ends_with("tunnels.json"));
    assert_eq!(load_tunnels(&path), Ok(TunnelList::default()));
    let list = TunnelList {
        tunnels: vec![tunnel()],
    };
    save_tunnels(&path, &list).unwrap();
    assert_eq!(load_tunnels(&path), Ok(list));
    std::fs::write(&path, "{oops").unwrap();
    assert!(matches!(load_tunnels(&path), Err(TunnelFileError::InvalidJson(_))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{oops");
}

// describe("describeTunnel")

#[test]
fn summarizes_each_type() {
    assert_eq!(describe_tunnel(&tunnel()), "L 127.0.0.1:5433 → db.internal:5432");
    let remote = TunnelDefinition {
        kind: TunnelKind::Remote,
        listen_port: 8080,
        dest_host: "localhost".into(),
        dest_port: 3000,
        ..tunnel()
    };
    assert_eq!(describe_tunnel(&remote), "R 127.0.0.1:8080 → localhost:3000");
    let dynamic = TunnelDefinition {
        kind: TunnelKind::Dynamic,
        listen_port: 1080,
        dest_host: String::new(),
        dest_port: 0,
        ..tunnel()
    };
    assert_eq!(describe_tunnel(&dynamic), "D 127.0.0.1:1080 (SOCKS5)");
    assert_eq!(
        describe_tunnel(&TunnelDefinition {
            listen_host: "::1".into(),
            ..tunnel()
        }),
        "L [::1]:5433 → db.internal:5432"
    );
}

#[test]
fn labels_the_tunnel_types() {
    assert_eq!(kind_label(TunnelKind::Local), "Local (-L)");
    assert_eq!(kind_label(TunnelKind::Remote), "Remote (-R)");
    assert_eq!(kind_label(TunnelKind::Dynamic), "Dynamic SOCKS5 (-D)");
}

// describe("isLoopback")

#[test]
fn accepts_only_loopback_listen_addresses() {
    assert!(
        ["127.0.0.1", "localhost", "::1", " LOCALHOST "]
            .into_iter()
            .all(is_loopback)
    );
    assert!(!["0.0.0.0", "192.168.1.5", "::", ""].into_iter().any(is_loopback));
}

#[test]
fn covers_the_whole_127_0_0_0_8_range_and_bracketed_1_but_not_look_alikes() {
    // it("covers the whole 127.0.0.0/8 range and bracketed ::1, but not look-alikes")
    assert!(
        ["127.0.0.2", "127.255.255.254", "[::1]", "0:0:0:0:0:0:0:1"]
            .into_iter()
            .all(is_loopback)
    );
    assert!(
        !["127.0.0.256", "127.1", "1127.0.0.1", "127.0.0.1.evil.com", "::2"]
            .into_iter()
            .any(is_loopback)
    );
}

// tunnelForm.test.ts

fn form() -> TunnelForm {
    TunnelForm {
        name: String::new(),
        kind: TunnelKind::Local,
        session_id: "s1".into(),
        listen_host: String::new(),
        listen_port: "5433".into(),
        dest_host: "db.internal".into(),
        dest_port: "5432".into(),
    }
}

const SESSIONS: [&str; 2] = ["s1", "s2"];

fn error(form: &TunnelForm) -> String {
    parse_tunnel_form(form, "t1", &SESSIONS).unwrap_err().to_string()
}

#[test]
fn defaults_the_name_and_the_listen_host() {
    assert_eq!(
        parse_tunnel_form(&form(), "t1", &SESSIONS),
        Ok(TunnelDefinition {
            id: "t1".into(),
            name: "local 5433→db.internal:5432".into(),
            kind: TunnelKind::Local,
            session_id: "s1".into(),
            listen_host: "127.0.0.1".into(),
            listen_port: 5433,
            dest_host: "db.internal".into(),
            dest_port: 5432,
        })
    );
}

#[test]
fn keeps_a_typed_name_and_listen_host_trimmed() {
    let typed = TunnelForm {
        name: " DB ".into(),
        listen_host: " 0.0.0.0 ".into(),
        ..form()
    };
    let tunnel = parse_tunnel_form(&typed, "t1", &SESSIONS).unwrap();
    assert_eq!((tunnel.name.as_str(), tunnel.listen_host.as_str()), ("DB", "0.0.0.0"));
}

#[test]
fn requires_ports_from_1_to_65535() {
    for listen_port in ["", "0", "65536", "80a", "-1"] {
        let f = TunnelForm {
            listen_port: listen_port.into(),
            ..form()
        };
        assert_eq!(
            error(&f),
            "Listen port must be a number between 1 and 65535",
            "{listen_port:?}"
        );
    }
    let f = TunnelForm {
        dest_port: "0".into(),
        ..form()
    };
    assert_eq!(error(&f), "Destination port must be a number between 1 and 65535");
}

#[test]
fn requires_a_destination_except_for_dynamic_tunnels() {
    assert_eq!(
        error(&TunnelForm {
            dest_host: " ".into(),
            ..form()
        }),
        "Destination host is required"
    );
    let dynamic = TunnelForm {
        kind: TunnelKind::Dynamic,
        dest_host: String::new(),
        dest_port: String::new(),
        ..form()
    };
    let tunnel = parse_tunnel_form(&dynamic, "t1", &SESSIONS).unwrap();
    assert_eq!(
        (tunnel.name.as_str(), tunnel.dest_host.as_str(), tunnel.dest_port),
        ("dynamic 5433", "", 0)
    );
}

#[test]
fn requires_an_existing_saved_session() {
    for session_id in ["", "gone"] {
        assert_eq!(
            error(&TunnelForm {
                session_id: session_id.into(),
                ..form()
            }),
            "Choose a saved SSH session"
        );
    }
}

#[test]
fn round_trips_through_the_form() {
    let parsed = parse_tunnel_form(
        &TunnelForm {
            name: "DB".into(),
            ..form()
        },
        "t1",
        &SESSIONS,
    )
    .unwrap();
    assert_eq!(
        parse_tunnel_form(&form_from_tunnel(&parsed), "t1", &SESSIONS),
        Ok(parsed)
    );
}

#[test]
fn leaves_the_destination_empty_for_dynamic_tunnels() {
    let parsed = parse_tunnel_form(
        &TunnelForm {
            kind: TunnelKind::Dynamic,
            ..form()
        },
        "t1",
        &SESSIONS,
    )
    .unwrap();
    let back = form_from_tunnel(&parsed);
    assert_eq!((back.dest_host.as_str(), back.dest_port.as_str()), ("", ""));
}
