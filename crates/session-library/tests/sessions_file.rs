//! `sessions.json` read and written like v0.2.15 (DATA-1a, SES-1a, SES-1i).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use session_library::file::library_path;
use session_library::*;

/// Written by v0.2.15's `serializeLibrary` (`JSON.stringify(_, null, 2)`) for every session kind.
const V0_2_15_FILE: &str = include_str!("fixtures/sessions-v0.2.15.json");

#[test]
fn round_trips_a_v0_2_15_file_byte_for_byte() {
    let lib = parse_library(Some(V0_2_15_FILE)).unwrap();
    assert_eq!(lib.sessions.len(), 10);
    assert_eq!(serialize_library(&lib), V0_2_15_FILE);
}

#[test]
fn reads_every_kind_of_the_v0_2_15_file() {
    let lib = parse_library(Some(V0_2_15_FILE)).unwrap();
    let kinds: Vec<&str> = lib.sessions.iter().map(|s| s.kind().as_str()).collect();
    assert_eq!(
        kinds,
        [
            "ssh", "ssh", "ssh", "serial", "telnet", "mosh", "mosh", "local", "local", "ssh"
        ]
    );
    assert_eq!(
        lib.sessions[1].as_ssh().map(|s| s.jump_ids.clone()),
        Some(vec!["0b6c1f0e-1".to_string()])
    );
}

#[test]
fn accepts_integers_written_as_floats_like_json_parse() {
    // `JSON.parse` reads 22.0 as the integer 22.
    let file = r#"{"version":1.0,"folders":[],"sessions":[{"id":"s","name":"s","folder":"","host":"h","port":22.0,"username":"u","auth":{"method":"agent"}}]}"#;
    let lib = parse_library(Some(file)).unwrap();
    assert_eq!(lib.sessions[0].as_ssh().map(|s| s.port), Some(22));
}

#[test]
fn rejects_null_where_a_field_must_be_absent_or_valid() {
    // `undefined` and `null` differ in v0.2.15: a `null` jumpIds or kind is invalid.
    for entry in [
        r#"{"kind":null,"id":"s","name":"s","folder":"","host":"h","port":22,"username":"u","auth":{"method":"agent"}}"#,
        r#"{"id":"s","name":"s","folder":"","host":"h","port":22,"username":"u","auth":{"method":"agent"},"jumpIds":null}"#,
        r#"{"kind":"local","id":"l","name":"l","folder":"","program":null}"#,
    ] {
        let file = format!(r#"{{"version":1,"folders":[],"sessions":[{entry}]}}"#);
        assert_eq!(
            parse_library(Some(&file)),
            Err(LibraryError::InvalidSessions),
            "{entry}"
        );
    }
    assert_eq!(parse_library(Some("null")), Err(LibraryError::UnsupportedVersion));
    assert_eq!(
        parse_library(Some(r#"{"version":1,"folders":[],"sessions":{}}"#)),
        Err(LibraryError::InvalidSessions)
    );
}

#[test]
fn normalizes_session_folders_but_keeps_the_folder_list_as_written() {
    let file = r#"{"version":1,"folders":[" A "],"sessions":[{"kind":"telnet","id":"t","name":"t","folder":" A / B ","host":"h","port":23}]}"#;
    let lib = parse_library(Some(file)).unwrap();
    assert_eq!(lib.folders, [" A "]);
    assert_eq!(lib.sessions[0].folder(), "A/B");
}

#[test]
fn loads_and_saves_sessions_json_in_the_config_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = library_path(dir.path());
    assert!(path.ends_with("sessions.json"));
    // A missing file is an empty library.
    assert_eq!(load_library(&path), Ok(SessionLibrary::default()));
    let lib = parse_library(Some(V0_2_15_FILE)).unwrap();
    save_library(&path, &lib).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), V0_2_15_FILE);
    assert_eq!(load_library(&path), Ok(lib));
}

#[test]
fn an_unreadable_file_is_reported_and_left_untouched() {
    // SES-1a: the panel shows the error and disables editing, so the file is never overwritten.
    let dir = tempfile::tempdir().unwrap();
    let path = library_path(dir.path());
    std::fs::write(&path, r#"{"version":3}"#).unwrap();
    let error = load_library(&path).unwrap_err();
    assert_eq!(error.to_string(), "unsupported sessions file version");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), r#"{"version":3}"#);
    let folder = dir.path().join("dir-not-file");
    std::fs::create_dir(&folder).unwrap();
    assert!(matches!(load_library(&folder), Err(LibraryError::Storage(_))));
}
