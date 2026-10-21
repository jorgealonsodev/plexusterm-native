//! SFTP file operations against proto-ssh's in-process test server, ported from PlexusTerm
//! v0.2.15 `src-tauri/src/sftp/tests.rs`. Errors are checked as values and messages instead of
//! their Tauri IPC JSON (`{ kind: "sftp", message }`).

mod common;

use std::sync::Arc;

use proto_sftp::*;
use proto_ssh::{SshError, SshManager};

#[tokio::test]
async fn home_is_the_canonical_working_directory() {
    let root = tempfile::tempdir().unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    assert_eq!(home(&sftp).await.unwrap(), "/");
}

#[tokio::test]
async fn the_sftp_session_is_opened_once_and_reused() {
    let root = tempfile::tempdir().unwrap();
    let conn = common::connect(root.path()).await;
    let first = conn.mgr.sftp(conn.id).await.unwrap();
    let second = conn.mgr.sftp(conn.id).await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}

// Creates a symlink, which needs privileges on Windows.
#[cfg(unix)]
#[tokio::test]
async fn list_puts_directories_first_then_names_case_insensitively() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    std::fs::write(r.join("b.txt"), b"12345").unwrap();
    std::fs::write(r.join("A.txt"), b"").unwrap();
    std::fs::create_dir(r.join("zeta")).unwrap();
    std::fs::create_dir(r.join("Alpha")).unwrap();
    std::os::unix::fs::symlink("b.txt", r.join("link")).unwrap();
    let (sftp, _conn) = common::session(r).await;

    let entries = list(&sftp, "/").await.unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "zeta", "A.txt", "b.txt", "link"]);
    let kinds: Vec<EntryKind> = entries.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            EntryKind::Dir,
            EntryKind::Dir,
            EntryKind::File,
            EntryKind::File,
            EntryKind::Symlink
        ]
    );
    let b = &entries[3];
    assert_eq!((b.path.as_str(), b.size), ("/b.txt", 5));
    assert!(b.modified.is_some_and(|m| m > 0));
    assert_eq!(b.permissions.map(|p| p & 0o600), Some(0o600));

    // Entries of a nested folder carry their full path.
    std::fs::write(r.join("zeta/inner"), b"x").unwrap();
    let nested = list(&sftp, "/zeta").await.unwrap();
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].path, "/zeta/inner");
}

#[tokio::test]
async fn upload_then_download_round_trips_a_large_binary_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("in")).unwrap();
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("blob.bin");
    let bytes: Vec<u8> = (0..(1024 * 1024 + 4321)).map(|i| (i * 31 % 251) as u8).collect();
    std::fs::write(&source, &bytes).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;

    let remote = upload(&sftp, &source, "/in").await.unwrap();
    assert_eq!(remote, "/in/blob.bin");
    assert_eq!(std::fs::read(root.path().join("in/blob.bin")).unwrap(), bytes);

    let target = local.path().join("copy.bin");
    download(&sftp, &remote, &target).await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), bytes);
}

#[tokio::test]
async fn upload_into_the_root_does_not_double_the_slash() {
    let root = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("a.txt");
    std::fs::write(&source, b"hi").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    assert_eq!(upload(&sftp, &source, "/").await.unwrap(), "/a.txt");
}

#[tokio::test]
async fn a_failed_download_keeps_the_existing_local_file_and_leaves_no_temp_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let local_dir = tempfile::tempdir().unwrap();
    let target = local_dir.path().join("keep.txt");
    std::fs::write(&target, b"keep me").unwrap();

    // Opening a directory as a file succeeds but reading it fails mid-transfer.
    assert!(download(&sftp, "/folder", &target).await.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"keep me");
    let names: Vec<_> = std::fs::read_dir(local_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec!["keep.txt"]);
}

#[tokio::test]
async fn create_file_makes_an_empty_file_and_never_replaces_one() {
    let root = tempfile::tempdir().unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    create_file(&sftp, "/new.txt").await.unwrap();
    assert_eq!(std::fs::read(root.path().join("new.txt")).unwrap(), b"");
    std::fs::write(root.path().join("keep.txt"), b"data").unwrap();
    let err = create_file(&sftp, "/keep.txt").await.unwrap_err();
    assert!(err.to_string().contains("cannot create file /keep.txt"), "{err}");
    assert_eq!(std::fs::read(root.path().join("keep.txt")).unwrap(), b"data");
}

#[tokio::test]
async fn mkdir_then_remove_an_empty_directory() {
    let root = tempfile::tempdir().unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    mkdir(&sftp, "/new").await.unwrap();
    assert!(root.path().join("new").is_dir());
    remove(&sftp, "/new", true).await.unwrap();
    assert!(!root.path().join("new").exists());
}

#[tokio::test]
async fn removing_a_non_empty_directory_fails_and_keeps_it() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("full")).unwrap();
    std::fs::write(root.path().join("full/f"), b"x").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let err = remove(&sftp, "/full", true).await.unwrap_err();
    assert!(matches!(err, SftpError::Sftp { .. }), "got {err:?}");
    assert!(root.path().join("full/f").exists());
}

#[tokio::test]
async fn remove_deletes_a_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("f"), b"x").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    remove(&sftp, "/f", false).await.unwrap();
    assert!(!root.path().join("f").exists());
}

#[tokio::test]
async fn rename_moves_an_entry() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("old"), b"x").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    rename(&sftp, "/old", "/new").await.unwrap();
    assert!(!root.path().join("old").exists());
    assert_eq!(std::fs::read(root.path().join("new")).unwrap(), b"x");
}

#[tokio::test]
async fn read_image_reads_remote_images_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("logo.svg"), b"<svg/>").unwrap();
    std::fs::write(root.path().join("notes.txt"), b"text").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    assert_eq!(read_image(&sftp, "/logo.svg").await.unwrap(), b"<svg/>");
    let err = read_image(&sftp, "/notes.txt").await.unwrap_err();
    assert_eq!(err.to_string(), "cannot show /notes.txt: it is not an image");
}

#[tokio::test]
async fn write_text_saves_over_the_text_that_was_opened() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), b"a much longer old text").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    write_text(&sftp, "/a.txt", "a much longer old text", "short", None, None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(root.path().join("a.txt")).unwrap(), b"short");
}

#[tokio::test]
async fn write_text_refuses_a_file_that_changed_since_it_was_opened() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), b"theirs").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let err = write_text(&sftp, "/a.txt", "old", "mine", None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("changed on disk"), "{err}");
    assert_eq!(std::fs::read(root.path().join("a.txt")).unwrap(), b"theirs");
}

#[tokio::test]
async fn move_into_moves_an_entry_into_a_folder() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("dest")).unwrap();
    std::fs::write(root.path().join("f"), b"x").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let moved = move_into(&sftp, "/f", "/dest").await.unwrap();
    assert_eq!(moved, "/dest/f");
    assert_eq!(std::fs::read(root.path().join("dest/f")).unwrap(), b"x");
    assert!(!root.path().join("f").exists());
}

#[tokio::test]
async fn move_into_never_replaces_an_entry_with_the_same_name() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("dest")).unwrap();
    std::fs::write(root.path().join("dest/f"), b"kept").unwrap();
    std::fs::write(root.path().join("f"), b"moving").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let err = move_into(&sftp, "/f", "/dest").await.unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    assert_eq!(std::fs::read(root.path().join("dest/f")).unwrap(), b"kept");
    assert_eq!(std::fs::read(root.path().join("f")).unwrap(), b"moving");
}

#[tokio::test]
async fn move_into_refuses_a_folder_into_itself_or_its_subfolders() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("a/b")).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    for target in ["/a", "/a/b", "/a/b/"] {
        let err = move_into(&sftp, "/a", target).await.unwrap_err();
        assert!(err.to_string().contains("into itself"), "{target}: {err}");
    }
    assert!(root.path().join("a/b").is_dir());
}

#[tokio::test]
async fn listing_a_missing_path_is_an_sftp_error() {
    let root = tempfile::tempdir().unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let err = list(&sftp, "/missing").await.unwrap_err();
    let SftpError::Sftp { message } = err else {
        panic!("expected an SFTP error, got {err:?}");
    };
    assert!(message.contains("/missing"), "{message}");
}

#[tokio::test]
async fn sftp_on_an_unknown_session_is_an_unknown_session_error() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = SshManager::new(dir.path().join("known_hosts"));
    let Err(err) = mgr.sftp(99).await.map_err(SftpError::from) else {
        panic!("expected an error");
    };
    assert!(
        matches!(err, SftpError::Ssh(SshError::UnknownSession { id: 99 })),
        "got {err:?}"
    );
    assert_eq!(err.to_string(), "unknown terminal session 99");
}

#[cfg(unix)]
#[tokio::test]
async fn list_entries_carry_the_owner_ids() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("f"), b"x").unwrap();
    let meta = std::fs::metadata(root.path().join("f")).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let entries = list(&sftp, "/").await.unwrap();
    assert_eq!((entries[0].uid, entries[0].gid), (Some(meta.uid()), Some(meta.gid())));
}

#[cfg(unix)]
#[tokio::test]
async fn chmod_sets_the_permission_bits() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("script.sh");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;

    chmod(&sftp, "/script.sh", 0o750).await.unwrap();
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o750);
    let listed = list(&sftp, "/").await.unwrap();
    assert_eq!(listed[0].permissions.map(|p| p & 0o7777), Some(0o750));
}

#[cfg(unix)]
#[tokio::test]
async fn chmod_rejects_modes_above_0o7777_and_keeps_the_file() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("f");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;

    let err = chmod(&sftp, "/f", 0o10000).await.unwrap_err();
    assert!(matches!(err, SftpError::Sftp { .. }), "got {err:?}");
    assert_eq!(err.to_string(), "cannot change permissions of /f: invalid mode 0o10000");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o644);
}

#[tokio::test]
async fn chmod_of_a_missing_path_is_an_sftp_error() {
    let root = tempfile::tempdir().unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let err = chmod(&sftp, "/missing", 0o644).await.unwrap_err();
    let SftpError::Sftp { message } = err else {
        panic!("expected an SFTP error, got {err:?}");
    };
    assert!(message.contains("/missing"), "{message}");
}

// --- Native additions: the text readers of the AI attachments and the file viewer, which
// v0.2.15 covered only through local files (`localfs.rs`). ---

#[tokio::test]
async fn read_text_attaches_a_file_or_a_folder_listing() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("README.md"), b"# hi").unwrap();
    std::fs::write(root.path().join("a.bin"), [0x7f, b'E', b'L', b'F', 0]).unwrap();
    let (sftp, _conn) = common::session(root.path()).await;

    let read = read_text(&sftp, "/README.md").await.unwrap();
    assert_eq!((read.content.as_str(), read.truncated), ("# hi", false));
    assert_eq!(read_text(&sftp, "/").await.unwrap().content, "src/\na.bin\nREADME.md");
    let err = read_text(&sftp, "/a.bin").await.unwrap_err();
    assert_eq!(err.to_string(), "cannot attach /a.bin: it is not a text file");
    let err = read_text(&sftp, "/missing").await.unwrap_err();
    assert!(err.to_string().starts_with("cannot read /missing: "), "{err}");
}

#[tokio::test]
async fn view_text_decodes_up_to_the_view_limit_and_refuses_folders() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("dir")).unwrap();
    let long = "x".repeat(text::MAX_ATTACH_BYTES * 2);
    std::fs::write(root.path().join("long.rs"), &long).unwrap();
    std::fs::write(root.path().join("latin.txt"), b"caf\xE9").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;

    let read = view_text(&sftp, "/long.rs", None).await.unwrap();
    assert_eq!((read.content, read.truncated), (long, false));
    let read = view_text(&sftp, "/latin.txt", Some("windows-1252")).await.unwrap();
    assert_eq!(
        (read.content.as_str(), read.encoding.as_str()),
        ("café", "windows-1252")
    );
    let err = view_text(&sftp, "/dir", None).await.unwrap_err();
    assert_eq!(err.to_string(), "cannot view /dir: it is a folder");
}

#[tokio::test]
async fn write_text_keeps_the_encoding_it_was_opened_with() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), b"caf\xE9").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    write_text(&sftp, "/a.txt", "café", "café!", Some("windows-1252"), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(root.path().join("a.txt")).unwrap(), b"caf\xE9!");
    let err = write_text(&sftp, "/gone.txt", "old", "mine", None, None)
        .await
        .unwrap_err();
    assert!(err.to_string().starts_with("cannot save /gone.txt: "), "{err}");
    assert!(!root.path().join("gone.txt").exists());
}

#[test]
fn a_download_writes_next_to_its_target_before_renaming() {
    let target = std::path::Path::new("/home/u/report.pdf");
    assert_eq!(
        partial_path(target),
        std::path::Path::new("/home/u/report.pdf.plexusterm-partial")
    );
}
