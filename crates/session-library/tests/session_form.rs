//! Port of v0.2.15 `src/features/sessions/sessionForm.test.ts`, plus the form parsers it
//! builds on: `ssh/quickConnect.test.ts` and the parsing cases of `serial/serialSettings.test.ts`
//! (SES-1e, SES-1h).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use proto_ssh::SshAuth;
use session_library::form::*;
use session_library::*;

fn form() -> SessionForm {
    SessionForm {
        kind: SessionKind::Ssh,
        name: "Web".into(),
        folder: " Prod / Web ".into(),
        ssh: QuickConnectForm {
            host: "web.example.com".into(),
            port: String::new(),
            username: "deploy".into(),
            method: AuthMethod::Password,
            password: String::new(),
            key_path: String::new(),
            passphrase: String::new(),
        },
        serial: SerialForm::default(),
        telnet: TelnetForm::default(),
        mosh: MoshForm::default(),
        local: LocalForm::default(),
        jump_ids: vec![],
    }
}

fn ssh_form(edit: impl FnOnce(&mut QuickConnectForm)) -> SessionForm {
    let mut f = form();
    edit(&mut f.ssh);
    f
}

fn ssh_of(result: Result<LibrarySession, FormError>) -> SshSession {
    match result.unwrap() {
        LibrarySession::Ssh(s) => s,
        other => panic!("not an SSH session: {other:?}"),
    }
}

fn error_of(result: Result<LibrarySession, FormError>) -> String {
    result.unwrap_err().to_string()
}

fn web_ssh() -> SshSession {
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

// describe("parseSessionForm")

#[test]
fn builds_a_saved_session_with_a_normalized_folder_and_no_password() {
    assert_eq!(
        parse_session_form(&form(), "id-1", None),
        Ok(LibrarySession::Ssh(SshSession {
            id: "id-1".into(),
            name: "Web".into(),
            folder: "Prod/Web".into(),
            host: "web.example.com".into(),
            port: 22,
            username: "deploy".into(),
            auth: SavedAuth::Password,
            jump_ids: vec![],
        }))
    );
}

#[test]
fn defaults_the_name_to_user_at_host() {
    let mut f = ssh_form(|s| s.host = "h:2200".into());
    f.name = "  ".into();
    let s = ssh_of(parse_session_form(&f, "x", None));
    assert_eq!((s.name.as_str(), s.port), ("deploy@h", 2200));
}

#[test]
fn keeps_the_key_path_but_not_the_passphrase() {
    let f = ssh_form(|s| {
        s.method = AuthMethod::Key;
        s.key_path = "~/.ssh/k".into();
        s.passphrase = "secret".into();
    });
    let result = parse_session_form(&f, "x", None);
    assert!(!format!("{result:?}").contains("secret"));
    assert_eq!(
        ssh_of(result).auth,
        SavedAuth::Key {
            path: "~/.ssh/k".into()
        }
    );
}

#[test]
fn reports_validation_errors() {
    assert_eq!(
        error_of(parse_session_form(&ssh_form(|s| s.host = String::new()), "x", None)),
        "Host is required"
    );
}

#[test]
fn reports_ssh_validation_errors_in_order() {
    // SES-1h: `Host is required`, `Username is required`, `Port must be ...`, `Private key path is required`.
    let username = ssh_form(|s| s.username = String::new());
    assert_eq!(
        error_of(parse_session_form(&username, "x", None)),
        "Username is required"
    );
    let port = ssh_form(|s| s.port = "0".into());
    assert_eq!(
        error_of(parse_session_form(&port, "x", None)),
        "Port must be a number between 1 and 65535"
    );
    let key = ssh_form(|s| {
        s.method = AuthMethod::Key;
        s.key_path = "  ".into();
    });
    assert_eq!(
        error_of(parse_session_form(&key, "x", None)),
        "Private key path is required"
    );
}

// describe("parseSessionForm (serial)")

fn serial_form(edit: impl FnOnce(&mut SerialForm)) -> SessionForm {
    let mut f = form();
    f.kind = SessionKind::Serial;
    f.name = String::new();
    f.ssh.host = String::new();
    f.serial.path = "/dev/ttyUSB1".into();
    edit(&mut f.serial);
    f
}

#[test]
fn builds_a_serial_session_named_after_the_device_by_default() {
    assert_eq!(
        parse_session_form(&serial_form(|s| s.baud = "9600".into()), "c-1", None),
        Ok(LibrarySession::Serial(SerialSession {
            id: "c-1".into(),
            name: "/dev/ttyUSB1".into(),
            folder: "Prod/Web".into(),
            settings: SerialSettings {
                path: "/dev/ttyUSB1".into(),
                baud: 9600,
                data_bits: 8,
                parity: SerialParity::None,
                stop_bits: 1,
                flow: SerialFlow::None,
            },
        }))
    );
}

#[test]
fn reports_serial_validation_errors() {
    assert_eq!(
        error_of(parse_session_form(&serial_form(|s| s.path = String::new()), "x", None)),
        "Serial port is required"
    );
}

#[test]
fn round_trips_a_serial_session() {
    let f = serial_form(|s| {
        s.baud = "custom".into();
        s.custom_baud = "250000".into();
        s.parity = SerialParity::Odd;
    });
    let parsed = parse_session_form(&f, "c-2", None).unwrap();
    assert_eq!(
        parse_session_form(&form_from_session(&parsed, None), "c-2", None),
        Ok(parsed)
    );
}

// describe("parseSessionForm (Telnet and Mosh)")

fn library() -> Vec<LibrarySession> {
    vec![web_ssh().into()]
}

fn telnet_form(host: &str, port: &str) -> SessionForm {
    let mut f = form();
    f.kind = SessionKind::Telnet;
    f.telnet = TelnetForm {
        host: host.into(),
        port: port.into(),
    };
    f
}

fn mosh_form(ssh_session_id: &str, server_command: &str) -> SessionForm {
    let mut f = form();
    f.kind = SessionKind::Mosh;
    f.mosh = MoshForm {
        ssh_session_id: ssh_session_id.into(),
        server_command: server_command.into(),
    };
    f
}

#[test]
fn builds_a_telnet_session_on_port_23_by_default_named_after_the_host() {
    let mut f = telnet_form(" switch.lan ", "");
    f.name = String::new();
    assert_eq!(
        parse_session_form(&f, "t-1", None),
        Ok(LibrarySession::Telnet(TelnetSession {
            id: "t-1".into(),
            name: "switch.lan".into(),
            folder: "Prod/Web".into(),
            host: "switch.lan".into(),
            port: 23,
        }))
    );
}

#[test]
fn reports_telnet_validation_errors() {
    assert_eq!(
        error_of(parse_session_form(&telnet_form(" ", ""), "t", None)),
        "Host is required"
    );
    assert_eq!(
        error_of(parse_session_form(&telnet_form("h", "99999"), "t", None)),
        "Port must be a number between 1 and 65535"
    );
}

#[test]
fn builds_a_mosh_session_over_a_saved_ssh_session_keeping_a_custom_server_command() {
    let lib = library();
    let mut f = mosh_form("s1", " mosh-server new -p 60010 ");
    f.name = String::new();
    assert_eq!(
        parse_session_form(&f, "m-1", Some(&lib)),
        Ok(LibrarySession::Mosh(MoshSession {
            id: "m-1".into(),
            name: "web (mosh)".into(),
            folder: "Prod/Web".into(),
            ssh_session_id: "s1".into(),
            server_command: Some("mosh-server new -p 60010".into()),
        }))
    );
    let plain = parse_session_form(&mosh_form("s1", ""), "m-1", Some(&lib)).unwrap();
    assert!(matches!(
        plain,
        LibrarySession::Mosh(MoshSession {
            server_command: None,
            ..
        })
    ));
}

#[test]
fn names_a_mosh_session_mosh_when_its_ssh_session_cannot_be_resolved() {
    // SES-1e: "Mosh" when the SSH session cannot be resolved (no library given).
    let mut f = mosh_form("s1", "");
    f.name = String::new();
    assert_eq!(parse_session_form(&f, "m-1", None).unwrap().name(), "Mosh");
}

#[test]
fn requires_an_existing_ssh_session_for_mosh() {
    let lib = library();
    let error = "Choose the saved SSH session Mosh logs in with";
    assert_eq!(error_of(parse_session_form(&mosh_form("", ""), "m", Some(&lib))), error);
    assert_eq!(
        error_of(parse_session_form(&mosh_form("gone", ""), "m", Some(&lib))),
        error
    );
}

#[test]
fn round_trips_telnet_and_mosh_sessions() {
    let lib = library();
    for f in [telnet_form("h", "2323"), mosh_form("s1", "mosh-server new")] {
        let parsed = parse_session_form(&f, "x", Some(&lib)).unwrap();
        assert_eq!(
            parse_session_form(&form_from_session(&parsed, Some(&lib)), "x", Some(&lib)),
            Ok(parsed)
        );
    }
}

// describe("formFromSession")

#[test]
fn form_from_session_round_trips_a_saved_session() {
    let f = ssh_form(|s| {
        s.method = AuthMethod::Key;
        s.key_path = "/k".into();
        s.port = "2222".into();
    });
    let parsed = parse_session_form(&f, "id-9", None).unwrap();
    assert_eq!(
        parse_session_form(&form_from_session(&parsed, None), "id-9", None),
        Ok(parsed)
    );
}

#[test]
fn leaves_the_name_empty_when_it_was_made_from_the_address_so_it_follows_a_new_host() {
    // it("leaves the name empty when it was made from the address, so it follows a new host")
    let mut f = ssh_form(|s| {
        s.username = "portainer-sys".into();
        s.host = "100.103.237.68".into();
    });
    f.name = String::new();
    let saved = parse_session_form(&f, "id-1", None).unwrap();
    assert_eq!(saved.name(), "portainer-sys@100.103.237.68");
    let mut edit = form_from_session(&saved, None);
    assert_eq!(edit.name, "");
    edit.ssh.host = "192.168.25.115".into();
    let moved = ssh_of(parse_session_form(&edit, "id-1", None));
    assert_eq!(
        (moved.name.as_str(), moved.host.as_str()),
        ("portainer-sys@192.168.25.115", "192.168.25.115")
    );
}

#[test]
fn keeps_a_name_the_user_chose_even_one_that_looks_like_an_address() {
    let mut f = form();
    f.name = "root@backup".into();
    let saved = parse_session_form(&f, "id-1", None).unwrap();
    assert_eq!(form_from_session(&saved, None).name, "root@backup");
    let mut t = telnet_form("switch-1", "");
    t.name = String::new();
    let telnet = parse_session_form(&t, "t-1", None).unwrap();
    assert_eq!(form_from_session(&telnet, None).name, "");
}

#[test]
fn fills_the_ssh_form_with_defaults_for_the_hidden_fields() {
    // SES-1h: port shown empty when 22; the key path defaults to ~/.ssh/id_ed25519.
    let edit = form_from_session(&web_ssh().into(), None);
    assert_eq!(
        (edit.ssh.port.as_str(), edit.ssh.method, edit.ssh.key_path.as_str()),
        ("", AuthMethod::Agent, "~/.ssh/id_ed25519")
    );
    let telnet = form_from_session(
        &TelnetSession {
            id: "t".into(),
            name: "x".into(),
            folder: String::new(),
            host: "h".into(),
            port: 23,
        }
        .into(),
        None,
    );
    assert_eq!(telnet.telnet.port, "");
}

// describe("parseSessionForm (jump hosts)")

fn with_jumps(ids: &[&str]) -> SessionForm {
    let mut f = form();
    f.jump_ids = ids.iter().map(|s| s.to_string()).collect();
    f
}

#[test]
fn keeps_the_ordered_jump_hosts_of_an_ssh_session() {
    let result = parse_session_form(&with_jumps(&["b", "a"]), "id-1", None).unwrap();
    assert_eq!(
        result.as_ssh().map(|s| s.jump_ids.clone()),
        Some(vec!["b".to_string(), "a".to_string()])
    );
    assert_eq!(
        parse_session_form(&form_from_session(&result, None), "id-1", None),
        Ok(result)
    );
}

#[test]
fn leaves_jump_ids_out_when_there_are_none() {
    let result = parse_session_form(&with_jumps(&[]), "id-1", None).unwrap();
    assert!(result.as_ssh().is_some_and(|s| s.jump_ids.is_empty()));
}

#[test]
fn rejects_a_chain_that_loops_back_through_another_sessions_jump_hosts() {
    // it("rejects a chain that loops back through another session's jump hosts")
    let other = SshSession {
        id: "b".into(),
        name: "bastion".into(),
        folder: String::new(),
        host: "bastion".into(),
        port: 22,
        username: "u".into(),
        auth: SavedAuth::Agent,
        jump_ids: vec!["x".into()],
    };
    let sessions = vec![LibrarySession::from(other.clone())];
    let error = error_of(parse_session_form(&with_jumps(&["b"]), "x", Some(&sessions)));
    assert!(error.contains("cycle"), "{error}");
    let plain = vec![LibrarySession::from(SshSession {
        jump_ids: vec![],
        ..other
    })];
    assert!(parse_session_form(&with_jumps(&["b"]), "x", Some(&plain)).is_ok());
}

#[test]
fn rejects_more_than_3_repeated_or_self_jump_hosts() {
    assert_eq!(
        error_of(parse_session_form(&with_jumps(&["a", "b", "c", "d"]), "x", None)),
        "At most 3 jump hosts are allowed"
    );
    assert_eq!(
        error_of(parse_session_form(&with_jumps(&["a", "a"]), "x", None)),
        "A jump host is listed twice"
    );
    assert_eq!(
        error_of(parse_session_form(&with_jumps(&["x"]), "x", None)),
        "A session cannot be its own jump host"
    );
}

// describe("local session form (local shells S4)")

fn local_form(program: &str, args: &str, cwd: &str) -> SessionForm {
    let mut f = form();
    f.kind = SessionKind::Local;
    f.local = LocalForm {
        program: program.into(),
        args: args.into(),
        cwd: cwd.into(),
    };
    f
}

#[test]
fn saves_the_shell_its_split_arguments_and_the_starting_directory() {
    let mut f = local_form(" /usr/bin/fish ", "--login -C 'cd /srv'", " ~/src ");
    f.name = " ".into();
    assert_eq!(
        parse_session_form(&f, "l-1", None),
        Ok(LibrarySession::Local(LocalSession {
            id: "l-1".into(),
            name: "fish".into(),
            folder: "Prod/Web".into(),
            program: Some("/usr/bin/fish".into()),
            args: vec!["--login".into(), "-C".into(), "cd /srv".into()],
            cwd: Some("~/src".into()),
        }))
    );
}

#[test]
fn saves_the_system_default_shell_without_program_or_arguments_named_local_shell() {
    let mut f = local_form("", "-x", "");
    f.name = String::new();
    f.folder = String::new();
    assert_eq!(
        parse_session_form(&f, "l-2", None),
        Ok(LibrarySession::Local(LocalSession {
            id: "l-2".into(),
            name: "Local shell".into(),
            folder: String::new(),
            program: None,
            args: vec![],
            cwd: None,
        }))
    );
}

#[test]
fn fills_the_form_back_quoting_arguments_that_have_spaces() {
    let session = LocalSession {
        id: "l".into(),
        name: "WSL".into(),
        folder: String::new(),
        program: Some("wsl.exe".into()),
        args: vec!["-d".into(), "Ubuntu 24.04".into(), String::new()],
        cwd: Some("~".into()),
    };
    let f = form_from_session(&session.into(), None);
    assert_eq!(f.kind, SessionKind::Local);
    assert_eq!(
        f.local,
        LocalForm {
            program: "wsl.exe".into(),
            args: r#"-d "Ubuntu 24.04" """#.into(),
            cwd: "~".into()
        }
    );
    let parsed = parse_session_form(&f, "l", None).unwrap();
    assert!(
        matches!(parsed, LibrarySession::Local(LocalSession { ref args, .. }) if args == &["-d", "Ubuntu 24.04", ""])
    );
}

// ssh/quickConnect.test.ts

fn quick() -> QuickConnectForm {
    QuickConnectForm {
        host: "example.com".into(),
        port: String::new(),
        username: "alice".into(),
        method: AuthMethod::Password,
        password: "pw".into(),
        key_path: String::new(),
        passphrase: String::new(),
    }
}

fn quick_with(edit: impl FnOnce(&mut QuickConnectForm)) -> QuickConnectForm {
    let mut f = quick();
    edit(&mut f);
    f
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

#[test]
fn quick_connect_builds_a_password_target_with_the_default_port() {
    let t = parse_quick_connect(&quick()).unwrap();
    assert_eq!(
        (t.host.as_str(), t.port, t.username.as_str(), auth_text(&t.auth)),
        ("example.com", 22, "alice", "password:pw".to_string())
    );
}

#[test]
fn quick_connect_accepts_user_at_host_port_in_the_host_field() {
    let t = parse_quick_connect(&quick_with(|f| {
        f.host = " bob@10.0.0.5:2222 ".into();
        f.username = String::new();
    }))
    .unwrap();
    assert_eq!(
        (t.host.as_str(), t.port, t.username.as_str()),
        ("10.0.0.5", 2222, "bob")
    );
}

#[test]
fn quick_connect_parses_ipv6_addresses() {
    // it.each(...)("parses the IPv6 address %s")
    for (input, host, port) in [
        ("::1", "::1", 22),
        ("[::1]", "::1", 22),
        ("[fe80::1%eth0]:2222", "fe80::1%eth0", 2222),
        ("bob@[2001:db8::7]:2200", "2001:db8::7", 2200),
    ] {
        let t = parse_quick_connect(&quick_with(|f| f.host = input.into())).unwrap();
        assert_eq!((t.host.as_str(), t.port), (host, port), "{input}");
    }
}

#[test]
fn quick_connect_an_explicit_port_field_wins_over_the_host_field() {
    let t = parse_quick_connect(&quick_with(|f| {
        f.host = "h:2222".into();
        f.port = "2200".into();
    }))
    .unwrap();
    assert_eq!((t.host.as_str(), t.port), ("h", 2200));
}

#[test]
fn quick_connect_builds_key_and_agent_targets() {
    let key = parse_quick_connect(&quick_with(|f| {
        f.method = AuthMethod::Key;
        f.key_path = "~/.ssh/id_ed25519".into();
    }))
    .unwrap();
    assert_eq!(auth_text(&key.auth), "key:~/.ssh/id_ed25519:-");
    let agent = parse_quick_connect(&quick_with(|f| f.method = AuthMethod::Agent)).unwrap();
    assert_eq!(auth_text(&agent.auth), "agent");
}

#[test]
fn quick_connect_keeps_a_key_passphrase_only_when_given() {
    let t = parse_quick_connect(&quick_with(|f| {
        f.method = AuthMethod::Key;
        f.key_path = "/k".into();
        f.passphrase = "s3".into();
    }))
    .unwrap();
    assert_eq!(auth_text(&t.auth), "key:/k:s3");
}

#[test]
fn quick_connect_rejects_invalid_input() {
    // it.each(...)("rejects invalid input %o")
    let cases: Vec<(QuickConnectForm, &str)> = vec![
        (quick_with(|f| f.host = String::new()), "Host is required"),
        (quick_with(|f| f.username = String::new()), "Username is required"),
        (
            quick_with(|f| f.port = "70000".into()),
            "Port must be a number between 1 and 65535",
        ),
        (
            quick_with(|f| f.port = "abc".into()),
            "Port must be a number between 1 and 65535",
        ),
        (
            quick_with(|f| {
                f.method = AuthMethod::Key;
                f.key_path = String::new();
            }),
            "Private key path is required",
        ),
    ];
    for (f, error) in cases {
        assert_eq!(parse_quick_connect(&f).unwrap_err().to_string(), error);
    }
}

// serial/serialSettings.test.ts (parsing; describeSerialError belongs to the serial backend)

fn serial_settings_form(edit: impl FnOnce(&mut SerialForm)) -> SerialForm {
    let mut f = SerialForm {
        path: "/dev/ttyUSB0".into(),
        ..SerialForm::default()
    };
    edit(&mut f);
    f
}

#[test]
fn default_serial_form_defaults_to_115200_8n1_without_flow_control() {
    let f = SerialForm::default();
    assert_eq!(
        (
            f.baud.as_str(),
            f.data_bits.as_str(),
            f.parity,
            f.stop_bits.as_str(),
            f.flow
        ),
        ("115200", "8", SerialParity::None, "1", SerialFlow::None)
    );
}

#[test]
fn offers_the_common_baud_rates() {
    assert_eq!(
        COMMON_BAUD_RATES,
        [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]
    );
}

#[test]
fn parse_serial_form_builds_serial_settings() {
    let f = serial_settings_form(|f| {
        f.path = " /dev/ttyACM0 ".into();
        f.baud = "9600".into();
        f.parity = SerialParity::Even;
        f.stop_bits = "2".into();
    });
    assert_eq!(
        parse_serial_form(&f),
        Ok(SerialSettings {
            path: "/dev/ttyACM0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::Even,
            stop_bits: 2,
            flow: SerialFlow::None,
        })
    );
}

#[test]
fn parse_serial_form_accepts_a_custom_baud_rate() {
    let f = serial_settings_form(|f| {
        f.baud = CUSTOM_BAUD.into();
        f.custom_baud = "250000".into();
    });
    assert_eq!(parse_serial_form(&f).map(|s| s.baud), Ok(250000));
}

#[test]
fn parse_serial_form_reports_validation_errors() {
    let message = |f: SerialForm| parse_serial_form(&f).unwrap_err().to_string();
    assert_eq!(
        message(serial_settings_form(|f| f.path = "  ".into())),
        "Serial port is required"
    );
    let abc = serial_settings_form(|f| {
        f.baud = CUSTOM_BAUD.into();
        f.custom_baud = "abc".into();
    });
    assert_eq!(message(abc), "Baud rate must be a positive whole number");
    let zero = serial_settings_form(|f| {
        f.baud = CUSTOM_BAUD.into();
        f.custom_baud = "0".into();
    });
    assert!(parse_serial_form(&zero).is_err());
    assert_eq!(
        message(serial_settings_form(|f| f.data_bits = "9".into())),
        "Data bits must be 5, 6, 7 or 8"
    );
    assert_eq!(
        message(serial_settings_form(|f| f.stop_bits = "3".into())),
        "Stop bits must be 1 or 2"
    );
}

#[test]
fn parse_serial_form_round_trips_through_form_from_settings_including_custom_baud_rates() {
    for baud in ["115200", CUSTOM_BAUD] {
        let f = serial_settings_form(|f| {
            f.baud = baud.into();
            f.custom_baud = "250000".into();
            f.data_bits = "7".into();
            f.flow = SerialFlow::Hardware;
        });
        let parsed = parse_serial_form(&f).unwrap();
        assert_eq!(parse_serial_form(&form_from_settings(&parsed)), Ok(parsed));
    }
}
