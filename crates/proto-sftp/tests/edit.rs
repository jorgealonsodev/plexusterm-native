//! Temporary copies, edit-and-upload and external programs (SFTP-2, SFTP-3, SEC-e). The first
//! tests are ported from PlexusTerm v0.2.15 `src-tauri/src/sftp/tests.rs` (its "S2" part); the
//! `open` tests drive the actions v0.2.15 ran as Tauri commands, with a callback in place of the
//! `sftp-edit-saved` event.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use proto_sftp::edit::{self, TempFiles};
use proto_sftp::open::{self, EditSaved, OpenWith};
use proto_sftp::{SftpError, download_copy, upload_file};

#[tokio::test]
async fn edited_copy_is_uploaded_back_to_the_same_remote_path() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("etc")).unwrap();
    std::fs::write(root.path().join("etc/app.conf"), b"old").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let temp = TempFiles::default();

    let local = download_copy(&sftp, &temp, 7, "edit", "/etc/app.conf").await.unwrap();
    assert_eq!(local.file_name().unwrap(), "app.conf");
    assert_eq!(std::fs::read(&local).unwrap(), b"old");

    std::fs::write(&local, b"new contents").unwrap();
    upload_file(&sftp, &local, "/etc/app.conf").await.unwrap();
    assert_eq!(
        std::fs::read(root.path().join("etc/app.conf")).unwrap(),
        b"new contents"
    );
}

#[cfg(unix)]
#[test]
fn copies_live_in_a_private_folder_one_subfolder_per_remote_file() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempFiles::default();
    let a = temp.local_path(1, "edit", "/x/a.txt").unwrap();
    let same = temp.local_path(1, "edit", "/x/a.txt").unwrap();
    let other = temp.local_path(1, "edit", "/y/a.txt").unwrap();
    let compare = temp.local_path(1, "compare", "/x/a.txt").unwrap();
    assert_eq!(a, same);
    assert_ne!(a, other);
    assert_ne!(a, compare);
    assert_eq!(other.file_name().unwrap(), "a.txt");
    let dir = temp.session_dir(1).unwrap();
    assert!(a.starts_with(&dir) && other.starts_with(&dir));
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_eq!(temp.existing(1, "edit", "/x/a.txt"), Some(a));
    assert_eq!(temp.existing(1, "edit", "/z"), None);
}

#[test]
fn removing_a_session_deletes_its_copies() {
    let temp = TempFiles::default();
    let a = temp.local_path(1, "edit", "/a").unwrap();
    std::fs::write(&a, b"x").unwrap();
    let b = temp.local_path(2, "edit", "/b").unwrap();
    let dir = temp.session_dir(1).unwrap();
    temp.remove(1);
    assert!(!dir.exists());
    assert_eq!(temp.existing(1, "edit", "/a"), None);
    assert!(b.parent().unwrap().exists(), "other sessions keep their copies");
}

#[tokio::test]
async fn the_watcher_reports_each_save_and_stops_with_the_session() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let temp = TempFiles::default();
    let local = temp.local_path(1, "edit", "/a").unwrap();
    std::fs::write(&local, b"v1").unwrap();
    let saves = Arc::new(AtomicUsize::new(0));
    let counter = saves.clone();
    temp.watch(1, "edit", "/a", Duration::from_millis(20), move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(saves.load(Ordering::SeqCst), 0, "no save yet");

    std::fs::write(&local, b"version 2").unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(saves.load(Ordering::SeqCst), 1);

    temp.remove(1);
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(saves.load(Ordering::SeqCst), 1);
}

#[test]
fn desktop_exec_puts_the_file_in_place_of_its_field_code() {
    let file = Path::new("/tmp/x/a b.txt");
    let args = |exec: &str| -> Vec<String> {
        edit::desktop_exec_args(exec, file)
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect()
    };
    assert_eq!(args("gedit %U"), ["gedit", "/tmp/x/a b.txt"]);
    assert_eq!(
        args("flatpak run --branch=stable org.gnome.TextEditor %F --new-window"),
        [
            "flatpak",
            "run",
            "--branch=stable",
            "org.gnome.TextEditor",
            "/tmp/x/a b.txt",
            "--new-window"
        ]
    );
    assert_eq!(
        args("\"/opt/My Editor/edit\" -n"),
        ["/opt/My Editor/edit", "-n", "/tmp/x/a b.txt"]
    );
    assert_eq!(args("kate %i %c -b %u"), ["kate", "-b", "/tmp/x/a b.txt"]);
}

#[test]
fn the_chosen_text_editor_gets_the_file_as_its_own_argument() {
    let file = Path::new("/tmp/x/a b; rm -rf ~.txt");
    let args = |command: &str| -> Option<Vec<String>> {
        edit::chosen_editor_command(command, file)
            .map(|args| args.into_iter().map(|a| a.into_string().unwrap()).collect())
    };
    // Empty: the system default editor.
    assert_eq!(args(""), None);
    assert_eq!(args("   "), None);
    assert_eq!(args("kate"), Some(vec!["kate".into(), file.display().to_string()]));
    assert_eq!(
        args("  code --wait "),
        Some(vec!["code".into(), "--wait".into(), file.display().to_string()])
    );
    assert_eq!(
        args("\"/opt/My Editor/edit\" -n %f --end"),
        Some(vec![
            "/opt/My Editor/edit".into(),
            "-n".into(),
            file.display().to_string(),
            "--end".into()
        ])
    );
    // Shell syntax is only text: no shell ever runs the command.
    assert_eq!(
        args("vim | true"),
        Some(vec![
            "vim".into(),
            "|".into(),
            "true".into(),
            file.display().to_string()
        ])
    );
}

#[test]
fn an_existing_program_path_with_spaces_is_one_program() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("My Editor");
    std::fs::write(&program, b"").unwrap();
    let file = Path::new("/tmp/a.txt");
    let args = edit::chosen_editor_command(program.to_str().unwrap(), file).unwrap();
    assert_eq!(args, [program.as_os_str(), file.as_os_str()]);
}

#[test]
fn desktop_exec_is_read_from_the_desktop_entry_group() {
    let contents = "[Desktop Entry]\nName=Ed\nExec=ed --x %f\n\n[Desktop Action new]\nExec=ed --new\n";
    assert_eq!(edit::desktop_exec(contents).as_deref(), Some("ed --x %f"));
    let only_action = "[Desktop Action new]\nExec=ed --new\n";
    assert_eq!(edit::desktop_exec(only_action), None);
}

#[cfg(unix)]
#[test]
fn the_first_installed_diff_tool_wins_in_order() {
    let bin1 = tempfile::tempdir().unwrap();
    let bin2 = tempfile::tempdir().unwrap();
    std::fs::write(bin1.path().join("code"), b"").unwrap();
    std::fs::write(bin2.path().join("kompare"), b"").unwrap();
    let path = std::env::join_paths([bin1.path(), bin2.path()]).unwrap();

    let (program, args) = edit::find_diff_tool(Some(path.clone())).unwrap();
    assert_eq!(program, bin2.path().join("kompare"));
    assert!(args.is_empty());

    std::fs::remove_file(bin2.path().join("kompare")).unwrap();
    let (program, args) = edit::find_diff_tool(Some(path.clone())).unwrap();
    assert_eq!(program, bin1.path().join("code"));
    assert_eq!(args, ["--diff"]);

    std::fs::remove_file(bin1.path().join("code")).unwrap();
    assert!(edit::find_diff_tool(Some(path)).is_none());
    assert!(edit::find_diff_tool(None).is_none());
}

/// A remote name is only data on this machine: it can never place the copy outside the
/// session's private folder, whatever separators or drive letters it contains.
#[test]
fn copy_names_never_leave_the_private_folder() {
    let temp = TempFiles::default();
    for (remote, expected) in [
        ("/srv/..\\..\\evil.bat", ".._.._evil.bat"),
        ("/srv/C:\\Users\\x\\Startup\\a.bat", "C__Users_x_Startup_a.bat"),
        ("/srv/a<b>c|d?e*f\"g", "a_b_c_d_e_f_g"),
        ("/srv/..", "file"),
        ("/srv/.", "file"),
        ("/srv/x\u{1}y", "x_y"),
    ] {
        let local = temp.local_path(7, "edit", remote).unwrap();
        let dir = temp.session_dir(7).unwrap();
        assert_eq!(local.parent().unwrap().parent().unwrap(), dir, "{remote}");
        assert_eq!(local.file_name().unwrap(), expected, "{remote}");
    }
}

#[test]
fn a_compare_target_must_be_an_absolute_local_path() {
    let absolute = std::env::temp_dir().join("notes.txt");
    assert_eq!(open::compare_target(absolute.to_str().unwrap()).unwrap(), absolute);
    for bad in ["notes.txt", "-o", "--help", "../etc/passwd", ""] {
        let err = open::compare_target(bad).unwrap_err();
        assert!(matches!(err, SftpError::Sftp { .. }), "{bad}: {err:?}");
        assert_eq!(
            err.to_string(),
            format!("cannot compare with \"{bad}\": choose a local file")
        );
    }
}

// --- The browser's open/edit/compare actions (v0.2.15 `sftp_edit`, `sftp_edit_upload`,
// `sftp_compare`, `sftp_discard_copies`). ---

/// A program in `dir` named `name` that writes its arguments, one per line, to `<program>.args`.
#[cfg(unix)]
#[allow(clippy::unwrap_used, reason = "test helper")]
fn recorder(dir: &Path, name: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join(name);
    std::fs::write(
        &program,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args.tmp\" && mv \"$0.args.tmp\" \"$0.args\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

/// The arguments `recorder` wrote, once it ran.
#[cfg(unix)]
async fn recorded(program: &Path) -> Vec<String> {
    let args = std::path::PathBuf::from(format!("{}.args", program.display()));
    assert!(
        common::eventually(|| args.exists()).await,
        "{} never ran",
        program.display()
    );
    std::fs::read_to_string(&args)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn editing_opens_a_watched_copy_reports_each_save_and_uploads_it_back() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("etc")).unwrap();
    std::fs::write(root.path().join("etc/app.conf"), b"old").unwrap();
    let conn = common::connect(root.path()).await;
    let temp = TempFiles::default();
    let bin = tempfile::tempdir().unwrap();
    let editor = recorder(bin.path(), "my editor");
    let saved: Arc<Mutex<Vec<EditSaved>>> = Arc::default();
    let sink = saved.clone();

    let with = OpenWith::TextEditor {
        command: Some(editor.to_string_lossy().into_owned()),
    };
    let local = open::edit(&conn.mgr, &temp, conn.id, "/etc/app.conf", &with, move |event| {
        sink.lock().unwrap().push(event);
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read(&local).unwrap(), b"old");
    assert!(local.starts_with(temp.session_dir(conn.id).unwrap()));
    assert_eq!(recorded(&editor).await, [local.to_string_lossy()]);

    // Wait for the watcher's first look before saving, then save once.
    tokio::time::sleep(edit::WATCH_INTERVAL + Duration::from_millis(200)).await;
    assert!(saved.lock().unwrap().is_empty(), "no save yet");
    std::fs::write(&local, b"new contents").unwrap();
    assert!(common::eventually(|| !saved.lock().unwrap().is_empty()).await);
    assert_eq!(
        saved.lock().unwrap().clone(),
        [EditSaved {
            id: conn.id,
            remote: "/etc/app.conf".into(),
            local: local.clone(),
        }]
    );

    open::edit_upload(&conn.mgr, &temp, conn.id, "/etc/app.conf")
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.path().join("etc/app.conf")).unwrap(),
        b"new contents"
    );

    // Closing the session discards its copies: nothing is left to upload.
    let dir = temp.session_dir(conn.id).unwrap();
    open::discard_copies(&temp, conn.id);
    assert!(!dir.exists());
    let err = open::edit_upload(&conn.mgr, &temp, conn.id, "/etc/app.conf")
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot upload to /etc/app.conf: it is not open for editing"
    );
}

#[tokio::test]
async fn editing_on_an_unknown_session_is_an_unknown_session_error() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = proto_ssh::SshManager::new(dir.path().join("known_hosts"));
    let temp = TempFiles::default();
    let err = open::edit(&mgr, &temp, 99, "/a", &OpenWith::Default, |_| {})
        .await
        .unwrap_err();
    assert!(
        matches!(err, SftpError::Ssh(proto_ssh::SshError::UnknownSession { id: 99 })),
        "got {err:?}"
    );
    assert_eq!(temp.session_dir(99), None, "no copy was made");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn compare_opens_a_copy_and_the_local_file_in_the_first_diff_tool() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("notes.txt"), b"remote text").unwrap();
    let conn = common::connect(root.path()).await;
    let temp = TempFiles::default();
    let bin = tempfile::tempdir().unwrap();
    let tool = recorder(bin.path(), "kdiff3");
    let local = bin.path().join("mine.txt");
    std::fs::write(&local, b"local text").unwrap();

    let path = Some(bin.path().as_os_str().to_os_string());
    open::compare_using(&conn.mgr, &temp, conn.id, "/notes.txt", local.to_str().unwrap(), path)
        .await
        .unwrap();
    let args = recorded(&tool).await;
    let copy = temp.existing(conn.id, open::COMPARE, "/notes.txt").unwrap();
    assert_eq!(args, [copy.to_string_lossy(), local.to_string_lossy()]);
    assert_eq!(std::fs::read(&copy).unwrap(), b"remote text");
    assert_eq!(
        temp.existing(conn.id, open::EDIT, "/notes.txt"),
        None,
        "compare copies are not edit copies"
    );
}

#[tokio::test]
async fn compare_without_a_diff_tool_says_what_to_install() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = proto_ssh::SshManager::new(dir.path().join("known_hosts"));
    let temp = TempFiles::default();
    let local = std::env::temp_dir().join("notes.txt");
    let err = open::compare_using(&mgr, &temp, 1, "/notes.txt", local.to_str().unwrap(), None)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "No diff tool found: install Meld, KDiff3, Kompare or Visual Studio Code."
    );
    let err = open::compare_using(&mgr, &temp, 1, "/notes.txt", "-o", None)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "cannot compare with \"-o\": choose a local file");
}

/// A failed upload must leave the remote original whole: v0.2.15 parity bug fixed natively.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_upload_never_destroys_the_remote_original() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("etc")).unwrap();
    std::fs::write(root.path().join("etc/app.conf"), b"original").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    // A directory opens but cannot be read, so the copy fails midway.
    let unreadable = tempfile::tempdir().unwrap();
    assert!(upload_file(&sftp, unreadable.path(), "/etc/app.conf").await.is_err());
    assert_eq!(std::fs::read(root.path().join("etc/app.conf")).unwrap(), b"original");
    let names: Vec<_> = std::fs::read_dir(root.path().join("etc"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["app.conf"], "no partial upload is left behind");
}

#[tokio::test]
async fn an_upload_replaces_the_file_and_leaves_nothing_else_behind() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), b"old").unwrap();
    let (sftp, _conn) = common::session(root.path()).await;
    let local = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(local.path(), b"new").unwrap();
    upload_file(&sftp, local.path(), "/a.txt").await.unwrap();
    upload_file(&sftp, local.path(), "/b.txt").await.unwrap();
    assert_eq!(std::fs::read(root.path().join("a.txt")).unwrap(), b"new");
    assert_eq!(std::fs::read(root.path().join("b.txt")).unwrap(), b"new");
    let mut names: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["a.txt", "b.txt"]);
}
