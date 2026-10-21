//! Port of v0.2.15 `src/features/sessions/iniImport.test.ts` (SES-2).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use std::collections::BTreeMap;

use session_library::import::*;
use session_library::*;

const SAMPLE: &str = concat!(
    "[Misc]\r\n",
    "SessionName=ignored=#109#0%misc.example.com%22%root%\r\n",
    "\r\n",
    "[Bookmarks]\r\n",
    "SubRep=\r\n",
    "ImgNum=42\r\n",
    "web=#109#0%web.example.com%2222%deploy%%-1%-1%%%22%%0%0%0%%%-1%0%0%0%%1080%%0%0%1#Font%10%0%0%-1%15#0# #-1\r\n",
    "bare=#109#0%bare.example.com%%%%-1\r\n",
    "telnet box=#98#1%tel.example.com%23%%\r\n",
    "desktop=#91#4%rdp.example.com%3389%admin%\r\n",
    "rdp=#1#0%rdp2.example.com%3389%\r\n",
    "garbage=this is not a session\r\n",
    "\r\n",
    "[Bookmarks_1]\r\n",
    "SubRep=Prod\\DB\r\n",
    "ImgNum=41\r\n",
    "db1=#109#0%10.0.0.5%abc% admin %\r\n",
    "nohost=#109#0%  %22%root%\r\n",
    "vnc=#2#0%vnc.example.com%5900%\r\n",
    "serial=#131#0%COM1%\r\n",
    "wsl=#105#0%Ubuntu%\r\n",
    "ftp=#4#0%ftp.example.com%21%\r\n",
    "\r\n",
    "[Passwords]\r\n",
    "root@web.example.com=SECRET_PASSWORD_HASH",
);

fn ssh(name: &str, folder: &str, host: &str, port: u16, username: &str) -> ImportedSession {
    ImportedSession::Ssh {
        name: name.into(),
        folder: folder.into(),
        host: host.into(),
        port,
        username: username.into(),
        auth: None,
        jumps: vec![],
    }
}

fn telnet(name: &str, folder: &str, host: &str, port: u16) -> ImportedSession {
    ImportedSession::Telnet {
        name: name.into(),
        folder: folder.into(),
        host: host.into(),
        port,
    }
}

fn counts(skipped: &Skipped) -> BTreeMap<String, usize> {
    skipped.entries().iter().cloned().collect()
}

fn map(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

// describe("decodeIni")

#[test]
fn decodes_utf8_and_strips_the_bom() {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice("[Bookmarks]\nSeñal=x".as_bytes());
    assert_eq!(decode_ini(&bytes), "[Bookmarks]\nSeñal=x");
}

#[test]
fn falls_back_to_windows_1252_when_the_bytes_are_not_valid_utf8() {
    // "Señal=€" in cp1252
    assert_eq!(decode_ini(&[0x53, 0x65, 0xf1, 0x61, 0x6c, 0x3d, 0x80]), "Señal=€");
}

// describe("parseSessionsIni")

#[test]
fn imports_ssh_and_telnet_sessions_from_every_bookmarks_section_with_their_subrep_folder() {
    // it("imports SSH and Telnet sessions from every Bookmarks section with their SubRep folder")
    assert_eq!(
        parse_sessions_ini(SAMPLE).sessions,
        [
            ssh("web", "Imported", "web.example.com", 2222, "deploy"),
            ssh("bare", "Imported", "bare.example.com", 22, ""),
            telnet("telnet box", "Imported", "tel.example.com", 23),
            ssh("db1", "Imported/Prod/DB", "10.0.0.5", 22, "admin"),
        ]
    );
}

#[test]
fn defaults_the_telnet_port_to_23_and_skips_telnet_lines_without_a_host() {
    let parsed = parse_sessions_ini("[Bookmarks]\nsw=#98#1%sw.lan%%\nbad=#98#1%  %23%\n");
    assert_eq!(parsed.sessions, [telnet("sw", "Imported", "sw.lan", 23)]);
    assert_eq!(counts(&parsed.skipped), map(&[("invalid", 1)]));
}

#[test]
fn counts_skipped_lines_per_type_and_reason_telnet_is_imported_not_skipped() {
    // it("counts skipped lines per type and reason (Telnet is imported, not skipped)")
    assert_eq!(
        counts(&parse_sessions_ini(SAMPLE).skipped),
        map(&[
            ("type 91", 1),
            ("RDP", 1),
            ("unrecognized", 1),
            ("invalid", 1),
            ("VNC", 1),
            ("Serial", 1),
            ("WSL", 1),
            ("FTP", 1),
        ])
    );
}

#[test]
fn never_reads_other_sections_such_as_passwords() {
    // it("never reads other sections such as [Passwords]")
    let text = format!("{:?}", parse_sessions_ini(SAMPLE));
    assert!(!text.contains("SECRET_PASSWORD_HASH"));
    assert!(!text.contains("misc.example.com"));
}

#[test]
fn accepts_lf_line_endings_and_rejects_out_of_range_ports() {
    let lf = parse_sessions_ini("[Bookmarks_3]\nSubRep=A\\\\B\\\na=#109#0%h%70000%u%\nb=#109#0%h2%0%u%\n");
    assert_eq!(
        lf.sessions,
        [
            ssh("a", "Imported/A/B", "h", 22, "u"),
            ssh("b", "Imported/A/B", "h2", 22, "u")
        ]
    );
}

#[test]
fn returns_nothing_for_a_file_without_bookmarks() {
    assert_eq!(parse_sessions_ini("[Misc]\nfoo=bar\n"), ParsedImport::default());
}

// describe("planImport")

fn existing() -> SshSession {
    SshSession {
        id: "old".into(),
        name: "web".into(),
        folder: "Imported".into(),
        host: "web.example.com".into(),
        port: 2222,
        username: "deploy".into(),
        auth: SavedAuth::Agent,
        jump_ids: vec![],
    }
}

fn library() -> SessionLibrary {
    SessionLibrary::default().reduce(&LibraryAction::SaveSession(existing().into()))
}

fn counter() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("id{n}")
    }
}

#[test]
fn adds_new_sessions_with_password_auth_and_skips_duplicates_of_saved_sessions() {
    let plan = plan_import(&library(), &parse_sessions_ini(SAMPLE), counter());
    assert_eq!(plan.duplicates, 1);
    assert_eq!(
        plan.to_add,
        [
            LibrarySession::Ssh(SshSession {
                id: "id1".into(),
                name: "bare".into(),
                folder: "Imported".into(),
                host: "bare.example.com".into(),
                port: 22,
                username: String::new(),
                auth: SavedAuth::Password,
                jump_ids: vec![],
            }),
            LibrarySession::Telnet(TelnetSession {
                id: "id2".into(),
                name: "telnet box".into(),
                folder: "Imported".into(),
                host: "tel.example.com".into(),
                port: 23,
            }),
            LibrarySession::Ssh(SshSession {
                id: "id3".into(),
                name: "db1".into(),
                folder: "Imported/Prod/DB".into(),
                host: "10.0.0.5".into(),
                port: 22,
                username: "admin".into(),
                auth: SavedAuth::Password,
                jump_ids: vec![],
            }),
        ]
    );
    assert_eq!(plan.skipped, parse_sessions_ini(SAMPLE).skipped);
}

#[test]
fn de_duplicates_repeated_lines_inside_the_same_file() {
    let text = "[Bookmarks]\nSubRep=\na=#109#0%h%22%u%\n[Bookmarks_1]\nSubRep=\na=#109#0%h%22%u%\n";
    let plan = plan_import(&SessionLibrary::default(), &parse_sessions_ini(text), || "x".into());
    assert_eq!(plan.to_add.len(), 1);
    assert_eq!(plan.duplicates, 1);
}

#[test]
fn does_not_compare_imported_hosts_against_serial_sessions() {
    let serial = SerialSession {
        id: "c".into(),
        name: "web".into(),
        folder: "Imported".into(),
        settings: SerialSettings {
            path: "/dev/ttyUSB0".into(),
            baud: 9600,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow: SerialFlow::None,
        },
    };
    let with_serial = library().reduce(&LibraryAction::SaveSession(serial.into()));
    let text = "[Bookmarks]\nweb=#109#0%web.example.com%2222%deploy%\n";
    assert_eq!(
        plan_import(&with_serial, &parse_sessions_ini(text), || "x".into()).duplicates,
        1
    );
}

#[test]
fn skips_telnet_duplicates_of_saved_telnet_sessions_only() {
    let sw = TelnetSession {
        id: "t".into(),
        name: "sw".into(),
        folder: "Imported".into(),
        host: "sw.lan".into(),
        port: 23,
    };
    let with_telnet = SessionLibrary::default().reduce(&LibraryAction::SaveSession(sw.into()));
    let text = "[Bookmarks]\nsw=#98#1%sw.lan%23%\nsw2=#109#0%sw.lan%23%%\n";
    let plan = plan_import(&with_telnet, &parse_sessions_ini(text), || "x".into());
    assert_eq!(plan.duplicates, 1);
    assert_eq!(
        plan.to_add.iter().map(LibrarySession::kind).collect::<Vec<_>>(),
        [SessionKind::Ssh]
    );
}

#[test]
fn treats_a_different_port_or_user_as_a_different_session() {
    let text = "[Bookmarks]\nweb=#109#0%web.example.com%22%deploy%\n";
    assert_eq!(
        plan_import(&library(), &parse_sessions_ini(text), || "x".into())
            .to_add
            .len(),
        1
    );
}

// skippedLabel (iniImport.ts; shown under "Not imported:" in the preview, SES-2)

#[test]
fn labels_every_skipped_reason_like_the_preview() {
    let cases = [
        ("invalid", "Sessions without a host"),
        ("unrecognized", "Unrecognized lines"),
        ("Serial", "Serial sessions"),
        ("include", "Include lines (not followed)"),
        ("match", "Match blocks (not applied)"),
        ("proxyCommand", "Hosts with a ProxyCommand (imported without it)"),
        (
            "jump",
            "Hosts whose ProxyJump is not a host of the file (imported without it)",
        ),
        ("type 91", "type 91 sessions"),
        ("RDP", "RDP sessions"),
    ];
    for (key, label) in cases {
        assert_eq!(skipped_label(key), label);
    }
}

// Reading through the backend with the 5 MiB cap (SES-2; `import_read_file` in v0.2.15).

#[test]
fn reads_an_ini_file_through_the_bounded_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.ini");
    std::fs::write(&path, b"[Bookmarks]\r\nSe\xf1al=#109#0%h%22%u%\r\n").unwrap();
    let parsed = read_ini_import(&path).unwrap();
    assert_eq!(parsed.sessions, [ssh("Señal", "Imported", "h", 22, "u")]);

    let big = dir.path().join("big.ini");
    std::fs::write(&big, vec![b' '; (core_storage::IMPORT_MAX_BYTES + 1) as usize]).unwrap();
    let error = read_ini_import(&big).unwrap_err().to_string();
    assert!(error.contains("file is too large"), "{error}");
}
