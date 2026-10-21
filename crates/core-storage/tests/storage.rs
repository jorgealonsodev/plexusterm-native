//! Ported from PlexusTerm v0.2.15 `src-tauri/src/storage.rs` and `history.rs` tests (PRD DATA-1, DATA-2).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "test code")]

use std::path::PathBuf;

use core_storage::{
    APP_IDENTIFIER, HISTORY_MAX_BYTES, StorageError, append_text, config_dir, config_dir_in, load, read_bounded, save,
    save_history, write_text,
};

#[test]
fn the_config_directory_is_taurus_app_config_dir() {
    assert_eq!(APP_IDENTIFIER, "dev.plexusterm.app");
    assert_eq!(
        config_dir_in(Some(PathBuf::from("/home/u/.config"))),
        Some(PathBuf::from("/home/u/.config/dev.plexusterm.app"))
    );
    assert_eq!(config_dir_in(None), None);
    // `dirs` finds no base without HOME/XDG_CONFIG_HOME (some sandboxes); the name is checked when it does.
    if let Some(dir) = config_dir() {
        assert!(dir.ends_with("dev.plexusterm.app"));
    }
}

#[test]
fn read_bounded_returns_the_bytes_of_small_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.ini");
    std::fs::write(&path, b"[Bookmarks]\r\n\xf1").unwrap();
    assert_eq!(read_bounded(&path, 1024).unwrap(), b"[Bookmarks]\r\n\xf1");
}

#[test]
fn read_bounded_refuses_files_over_the_limit_and_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.ini");
    std::fs::write(&path, vec![b'x'; 2048]).unwrap();
    let err = read_bounded(&path, 1024).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
    assert!(read_bounded(&dir.path().join("missing.ini"), 1024).is_err());
}

#[test]
fn write_text_creates_or_replaces_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");
    write_text(&path, "first\n", 1024).unwrap();
    write_text(&path, "second\n", 1024).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second\n");
}

#[test]
fn write_text_refuses_contents_over_the_limit_and_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");
    let err = write_text(&path, &"x".repeat(2048), 1024).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
    assert!(!path.exists());
}

#[test]
fn append_text_creates_the_file_then_appends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.log");
    append_text(&path, "one\n", 1024).unwrap();
    append_text(&path, "two\n", 1024).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    assert!(append_text(&path, &"x".repeat(2048), 1024).is_err());
    assert!(append_text(&dir.path().join("missing/dir/x.log"), "a", 1024).is_err());
}

#[test]
fn missing_file_loads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load(&dir.path().join("sessions.json")).unwrap(), None);
}

#[test]
fn save_then_load_round_trips_and_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/dir/sessions.json");
    save(&path, r#"{"version":1}"#).unwrap();
    assert_eq!(load(&path).unwrap().as_deref(), Some(r#"{"version":1}"#));
}

#[test]
fn save_overwrites_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.json");
    save(&path, "[1]").unwrap();
    save(&path, "[2]").unwrap();
    assert_eq!(load(&path).unwrap().as_deref(), Some("[2]"));
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec!["sessions.json"]);
}

#[test]
fn invalid_json_is_rejected_and_keeps_the_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.json");
    save(&path, "[1]").unwrap();
    assert!(matches!(save(&path, "{not json"), Err(StorageError::InvalidJson(_))));
    assert_eq!(load(&path).unwrap().as_deref(), Some("[1]"));
}

#[test]
fn concurrent_saves_never_corrupt_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::sync::Arc::new(dir.path().join("sessions.json"));
    let threads: Vec<_> = (0..16)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || save(&path, &format!("[{}]", "1,".repeat(i * 500) + "0")).unwrap())
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    let text = load(&path).unwrap().unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&text).is_ok(),
        "corrupt: {}",
        &text[..40.min(text.len())]
    );
}

#[cfg(unix)]
#[test]
fn saved_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.json");
    save(&path, "{}").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn replacing_an_existing_world_readable_file_makes_it_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    save(&path, r#"{"version":2}"#).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn history_round_trips_and_refuses_oversized_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    save_history(&path, r#"{"version":1,"hosts":{}}"#).unwrap();
    assert_eq!(load(&path).unwrap().as_deref(), Some(r#"{"version":1,"hosts":{}}"#));
    let big = format!("[\"{}\"]", "x".repeat(HISTORY_MAX_BYTES));
    let err = save_history(&path, &big).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
    assert_eq!(
        load(&path).unwrap().as_deref(),
        Some(r#"{"version":1,"hosts":{}}"#),
        "the previous history is kept"
    );
}

#[test]
fn save_removes_old_temp_files_left_by_a_crashed_writer_but_not_fresh_ones() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let leftover = |name: &str, modified: Option<std::time::SystemTime>| {
        let file = std::fs::File::create(dir.path().join(name)).unwrap();
        if let Some(time) = modified {
            file.set_modified(time).unwrap();
        }
    };
    // Another writer (pid 1 is never us) died an hour ago; another one is writing right now.
    leftover("settings.json.tmp-1-0", Some(old));
    leftover("settings.json.tmp-1-1", None);
    leftover("other.json.tmp-1-0", Some(old));
    save(&path, "{}").unwrap();
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["other.json.tmp-1-0", "settings.json", "settings.json.tmp-1-1"],
        "only this document's old leftovers are removed; a concurrent writer's temp file is kept"
    );
}

#[cfg(unix)]
#[test]
fn write_text_replaces_the_file_atomically_keeping_its_mode_and_symlink() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.txt");
    write_text(&path, "old\n", 1024).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let link = dir.path().join("link.txt");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let before = std::fs::metadata(&path).unwrap().ino();
    write_text(&link, "new\n", 1024).unwrap();
    let after = std::fs::metadata(&path).unwrap();
    assert_ne!(
        after.ino(),
        before,
        "a new file was renamed into place instead of truncating the old one"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    assert_eq!(after.permissions().mode() & 0o777, 0o640, "the mode is kept");
    assert!(
        std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
        "the link still points at the file"
    );
    let mut names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, vec!["link.txt", "output.txt"], "no temp file is left");
}
