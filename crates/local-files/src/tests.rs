//! Ported from PlexusTerm v0.2.15 `src-tauri/src/localfs.rs` (tests module) and the open/edit
//! tests of `src-tauri/src/sftp/tests.rs`, plus the FIX-1 and FIX-2 tests.

use super::*;
use std::path::Path;

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn lists_folders_first_then_by_case_insensitive_name() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("b.txt"), "hello").unwrap();
    fs::write(dir.path().join("A.txt"), "").unwrap();
    fs::create_dir(dir.path().join("zeta")).unwrap();
    fs::create_dir(dir.path().join("Alpha")).unwrap();
    let entries = list(dir.path()).unwrap();
    assert_eq!(names(&entries), ["Alpha", "zeta", "A.txt", "b.txt"]);
    let b = &entries[3];
    assert_eq!(b.kind, EntryKind::File);
    assert_eq!(b.size, 5);
    assert_eq!(b.path, dir.path().join("b.txt").to_string_lossy());
    assert!(b.modified.is_some());
    assert_eq!(entries[0].kind, EntryKind::Dir);
}

#[test]
fn listing_a_missing_folder_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope");
    let err = list(&missing).unwrap_err();
    assert!(
        err.starts_with(&format!("cannot list {}: ", missing.display())),
        "{err}"
    );
}

#[test]
fn stat_reports_a_folder_and_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f");
    fs::write(&file, "abc").unwrap();
    assert_eq!(stat(dir.path()).unwrap().kind, EntryKind::Dir);
    let entry = stat(&file).unwrap();
    assert_eq!((entry.name.as_str(), entry.kind, entry.size), ("f", EntryKind::File, 3));
    assert!(stat(&dir.path().join("x")).unwrap_err().starts_with("cannot read "));
}

#[cfg(unix)]
#[test]
fn a_symlink_is_reported_and_removed_without_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "").unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_eq!(stat(&link).unwrap().kind, EntryKind::Symlink);
    remove(&link, true).unwrap();
    assert!(!link.exists());
    assert!(target.join("keep").exists());
}

#[cfg(unix)]
#[test]
fn a_trailing_slash_never_deletes_through_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "").unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    remove(Path::new(&format!("{}/", link.display())), true).unwrap();
    assert!(fs::symlink_metadata(&link).is_err(), "the link is removed");
    assert!(target.join("keep").exists(), "its target is kept");
}

#[test]
fn creates_renames_and_removes() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("new");
    mkdir(&folder).unwrap();
    assert!(mkdir(&folder).unwrap_err().starts_with("cannot create "));
    let file = folder.join("a.txt");
    create_file(&file).unwrap();
    assert!(create_file(&file).unwrap_err().starts_with("cannot create "));
    let other = folder.join("b.txt");
    create_file(&other).unwrap();
    let err = rename(&file, &other).unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert!(file.exists());
    rename(&file, &folder.join("c.txt")).unwrap();
    assert!(folder.join("c.txt").exists() && !file.exists());
    remove(&other, false).unwrap();
    assert!(!other.exists());
    remove(&folder, true).unwrap();
    assert!(!folder.exists());
    assert!(remove(&folder, true).unwrap_err().starts_with("cannot delete "));
}

#[test]
fn reads_text_files_up_to_the_limit_and_refuses_binary_ones() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("notes.txt");
    fs::write(&small, "line 1\nline 2").unwrap();
    let read = read_text(&small).unwrap();
    assert_eq!(read.content, "line 1\nline 2");
    assert!(!read.truncated);
    assert_eq!(read.path, small.to_string_lossy());

    // A two-byte character cut by the limit is dropped, not refused.
    let big = dir.path().join("big.txt");
    let mut text = "a".repeat(MAX_ATTACH_BYTES - 1);
    text.push('ñ');
    text.push_str("tail");
    fs::write(&big, &text).unwrap();
    let read = read_text(&big).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_ATTACH_BYTES - 1));

    let binary = dir.path().join("a.bin");
    fs::write(&binary, [0x7f, b'E', b'L', b'F', 0, 1]).unwrap();
    let err = read_text(&binary).unwrap_err();
    assert_eq!(
        err,
        format!("cannot attach {}: it is not a text file", binary.display())
    );
    fs::write(&binary, [0xff, 0xfe, b'a']).unwrap();
    assert!(read_text(&binary).unwrap_err().contains("not a text file"));
    assert!(
        read_text(&dir.path().join("x"))
            .unwrap_err()
            .starts_with("cannot read ")
    );
}

#[test]
fn reads_images_for_the_markdown_preview_and_refuses_other_files() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("shot.PNG");
    fs::write(&png, [0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
    assert_eq!(read_image(&png).unwrap(), [0x89, b'P', b'N', b'G', 1, 2, 3]);

    let key = dir.path().join("id_ed25519");
    fs::write(&key, "secret").unwrap();
    assert_eq!(
        read_image(&key).unwrap_err(),
        format!("cannot show {}: it is not an image", key.display())
    );
    let folder = dir.path().join("pics.png");
    fs::create_dir(&folder).unwrap();
    assert_eq!(
        read_image(&folder).unwrap_err(),
        format!("cannot show {}: it is not an image", folder.display())
    );
    let big = dir.path().join("big.jpg");
    fs::write(&big, vec![0u8; MAX_IMAGE_BYTES + 1]).unwrap();
    assert_eq!(
        read_image(&big).unwrap_err(),
        format!("cannot show {}: it is larger than 20 MiB", big.display())
    );
    assert!(read_image(&dir.path().join("missing.gif")).is_err());
    // Network shares and devices are never opened: Windows would sign in to the share's host.
    for path in [
        r"\\attacker.example\s\x.png",
        r"\\?\C:\x.png",
        "//attacker.example/s/x.png",
    ] {
        assert_eq!(read_image(Path::new(path)).unwrap_err(), not_an_image(path), "{path}");
    }
}

#[cfg(unix)]
#[test]
fn an_image_name_on_something_that_is_not_a_file_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe.png");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(read_image(&fifo).unwrap_err(), not_an_image(&fifo.to_string_lossy()));
}

#[test]
fn views_text_files_up_to_the_view_limit_and_refuses_folders_and_binary_ones() {
    let dir = tempfile::tempdir().unwrap();
    // Longer than an attachment, within the viewer's limit: read whole.
    let long = dir.path().join("long.rs");
    let text = "x".repeat(MAX_ATTACH_BYTES * 2);
    fs::write(&long, &text).unwrap();
    let read = view_text(&long, None).unwrap();
    assert_eq!(read.content, text);
    assert!(!read.truncated);

    let huge = dir.path().join("huge.log");
    fs::write(&huge, "y".repeat(MAX_VIEW_BYTES + 10)).unwrap();
    let read = view_text(&huge, None).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content.len(), MAX_VIEW_BYTES);

    let binary = dir.path().join("a.bin");
    fs::write(&binary, [0x7f, b'E', b'L', b'F', 0, 1]).unwrap();
    assert_eq!(
        view_text(&binary, None).unwrap_err(),
        format!("cannot view {}: it is not a text file", binary.display())
    );
    assert_eq!(
        view_text(dir.path(), None).unwrap_err(),
        format!("cannot view {}: it is a folder", dir.path().display())
    );
    assert!(
        view_text(&dir.path().join("x"), None)
            .unwrap_err()
            .starts_with("cannot read ")
    );
}

#[test]
fn a_folder_is_attached_as_its_listing() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("README.md"), "").unwrap();
    let read = read_text(dir.path()).unwrap();
    assert_eq!(read.content, "src/\nREADME.md");
    assert!(!read.truncated);
}

#[test]
fn write_text_saves_over_the_text_that_was_opened() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "old").unwrap();
    write_text(&file, "old", "new text", None, None).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "new text");
}

#[test]
fn write_text_refuses_a_file_that_changed_since_it_was_opened() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "changed by someone else").unwrap();
    let err = write_text(&file, "old", "mine", None, None).unwrap_err();
    assert!(err.contains("changed on disk"), "{err}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "changed by someone else");
}

#[test]
fn write_text_refuses_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gone.txt");
    let err = write_text(&file, "old", "mine", None, None).unwrap_err();
    assert!(err.starts_with("cannot save"), "{err}");
    assert!(!file.exists());
}

#[cfg(unix)]
#[test]
fn write_text_keeps_a_symlink_and_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("real.sh");
    fs::write(&target, "echo 1").unwrap();
    chmod(&target, 0o755).unwrap();
    let link = dir.path().join("link.sh");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    write_text(&link, "echo 1", "echo 2", None, None).unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&target).unwrap(), "echo 2");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode, 0o755);
}

/// Writes `bytes` to a new file in `dir` and views it with `encoding`.
#[allow(clippy::unwrap_used, reason = "a test helper: a failure here is a failed test")]
fn view_bytes(dir: &Path, bytes: &[u8], encoding: Option<&str>) -> Result<TextFile, String> {
    let file = dir.join("f.txt");
    fs::write(&file, bytes).unwrap();
    view_text(&file, encoding)
}

#[test]
fn viewing_plain_utf8_reports_utf8_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let read = view_bytes(dir.path(), "año\n".as_bytes(), None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("año\n", "utf-8"));
    let attached = read_text(&dir.path().join("f.txt")).unwrap();
    assert_eq!(attached.encoding, "utf-8");
    assert_eq!(read_text(dir.path()).unwrap().encoding, "utf-8");
}

#[test]
fn viewing_detects_a_bom_and_leaves_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let read = view_bytes(dir.path(), b"\xEF\xBB\xBFhi \xC3\xA9", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hi é", "utf-8-bom"));
    let read = view_bytes(dir.path(), b"\xFF\xFEh\0i\0\xE9\0", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hié", "utf-16le"));
    let read = view_bytes(dir.path(), b"\xFE\xFF\0h\0i\0\xE9", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hié", "utf-16be"));
}

#[test]
fn viewing_with_an_encoding_decodes_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let read = view_bytes(dir.path(), b"caf\xE9 \x80", Some("windows-1252")).unwrap();
    assert_eq!(
        (read.content.as_str(), read.encoding.as_str()),
        ("café €", "windows-1252")
    );
    let read = view_bytes(dir.path(), b"\x93\xFA\x96\x7B", Some("shift_jis")).unwrap();
    assert_eq!(read.content, "日本");
    // UTF-16 text holds NUL bytes; without a BOM it is decoded as asked.
    let read = view_bytes(dir.path(), b"a\0b\0", Some("utf-16le")).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("ab", "utf-16le"));
    let read = view_bytes(dir.path(), b"\xEF\xBB\xBFx", Some("utf-8-bom")).unwrap();
    assert_eq!(read.content, "x");
}

#[test]
fn viewing_refuses_unknown_encodings_and_malformed_text() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    let label = file.display();
    assert_eq!(
        view_bytes(dir.path(), b"x", Some("klingon")).unwrap_err(),
        format!("cannot view {label}: unknown encoding klingon")
    );
    // A Shift_JIS lead byte with nothing after it, and an odd UTF-16 length.
    assert_eq!(
        view_bytes(dir.path(), b"a\x93", Some("shift_jis")).unwrap_err(),
        format!("cannot view {label}: it is not a text file in shift_jis")
    );
    assert_eq!(
        view_bytes(dir.path(), b"a\0b", Some("utf-16le")).unwrap_err(),
        format!("cannot view {label}: it is not a text file in utf-16le")
    );
    assert_eq!(
        view_bytes(dir.path(), b"a\0b", Some("windows-1252")).unwrap_err(),
        format!("cannot view {label}: it is not a text file")
    );
    assert_eq!(
        view_bytes(dir.path(), b"\xC3(", Some("utf-8")).unwrap_err(),
        format!("cannot view {label}: it is not a text file")
    );
}

#[test]
fn a_character_cut_by_the_view_limit_is_dropped_in_any_encoding() {
    let dir = tempfile::tempdir().unwrap();
    // UTF-16: a surrogate pair cut after its first unit.
    let text = format!("{}😀", "a".repeat(MAX_VIEW_BYTES / 2 - 2));
    let mut bytes = b"\xFF\xFE".to_vec();
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let read = view_bytes(dir.path(), &bytes, None).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_VIEW_BYTES / 2 - 2));
    // Shift_JIS: a two-byte character cut after its first byte.
    let mut bytes = b"a".repeat(MAX_VIEW_BYTES - 1);
    bytes.extend(b"\x93\xFAtail");
    let read = view_bytes(dir.path(), &bytes, Some("shift_jis")).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_VIEW_BYTES - 1));
}

#[test]
fn writing_keeps_the_encoding_and_its_bom() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    let cases: [(&str, &[u8], &[u8]); 5] = [
        ("utf-8", b"old", b"n\xC3\xA9w"),
        ("utf-8-bom", b"\xEF\xBB\xBFold", b"\xEF\xBB\xBFn\xC3\xA9w"),
        ("utf-16le", b"\xFF\xFEo\0l\0d\0", b"\xFF\xFEn\0\xE9\0w\0"),
        ("utf-16be", b"\xFE\xFF\0o\0l\0d", b"\xFE\xFF\0n\0\xE9\0w"),
        ("windows-1252", b"old", b"n\xE9w"),
    ];
    for (encoding, before, after) in cases {
        fs::write(&file, before).unwrap();
        assert_eq!(view_text(&file, Some(encoding)).unwrap().content, "old");
        write_text(&file, "old", "néw", Some(encoding), None).unwrap();
        assert_eq!(fs::read(&file).unwrap(), after, "{encoding}");
        assert_eq!(view_text(&file, Some(encoding)).unwrap().content, "néw");
    }
}

#[test]
fn writing_with_another_encoding_compares_with_the_one_it_was_opened_with() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    fs::write(&file, "café").unwrap();
    // Opened as UTF-8, saved as Windows-1252.
    write_text(&file, "café", "café!", Some("windows-1252"), Some("utf-8")).unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"caf\xE9!");
}

#[test]
fn writing_refuses_characters_the_encoding_cannot_hold() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    fs::write(&file, b"old").unwrap();
    let err = write_text(&file, "old", "日本", Some("windows-1252"), None).unwrap_err();
    assert_eq!(
        err,
        format!(
            "cannot save {}: some characters cannot be written in windows-1252",
            file.display()
        )
    );
    assert_eq!(fs::read(&file).unwrap(), b"old");
    let err = write_text(&file, "old", "x", Some("klingon"), None).unwrap_err();
    assert_eq!(err, format!("cannot save {}: unknown encoding klingon", file.display()));
    assert_eq!(fs::read(&file).unwrap(), b"old");
}

#[test]
fn writing_with_an_encoding_still_refuses_a_file_that_changed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    fs::write(&file, b"caf\xE9 changed").unwrap();
    let err = write_text(&file, "café", "mine", Some("windows-1252"), None).unwrap_err();
    assert!(err.contains("changed on disk"), "{err}");
    assert_eq!(fs::read(&file).unwrap(), b"caf\xE9 changed");
    // Bytes that do not decode in the encoding count as changed too.
    fs::write(&file, b"a\0b").unwrap();
    let err = write_text(&file, "ab", "mine", Some("utf-16le"), None).unwrap_err();
    assert!(err.contains("changed on disk"), "{err}");
}

#[test]
fn move_into_moves_a_file_and_a_folder_into_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dir.path().join("f.txt"), "x").unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub").join("inner"), "y").unwrap();

    let moved = move_into(&dir.path().join("f.txt"), &dest).unwrap();
    assert_eq!(moved, dest.join("f.txt"));
    assert_eq!(fs::read_to_string(dest.join("f.txt")).unwrap(), "x");
    assert!(!dir.path().join("f.txt").exists());

    move_into(&dir.path().join("sub"), &dest).unwrap();
    assert_eq!(fs::read_to_string(dest.join("sub").join("inner")).unwrap(), "y");
    assert!(!dir.path().join("sub").exists());
}

#[test]
fn move_into_never_replaces_an_entry_with_the_same_name() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("f.txt"), "kept").unwrap();
    fs::write(dir.path().join("f.txt"), "moving").unwrap();
    let err = move_into(&dir.path().join("f.txt"), &dest).unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(fs::read_to_string(dest.join("f.txt")).unwrap(), "kept");
    assert_eq!(fs::read_to_string(dir.path().join("f.txt")).unwrap(), "moving");
}

#[test]
fn move_into_refuses_a_folder_into_itself_or_its_subfolders() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("a");
    fs::create_dir_all(folder.join("b")).unwrap();
    for target in [folder.clone(), folder.join("b")] {
        let err = move_into(&folder, &target).unwrap_err();
        assert!(err.contains("into itself"), "{err}");
    }
    // A sibling whose name starts the same is not inside it.
    fs::create_dir(dir.path().join("ab")).unwrap();
    move_into(&folder, &dir.path().join("ab")).unwrap();
    assert!(dir.path().join("ab").join("a").join("b").is_dir());
}

#[test]
fn move_into_its_own_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("f.txt"), "x").unwrap();
    let err = move_into(&dir.path().join("f.txt"), dir.path()).unwrap_err();
    assert!(err.contains("already in"), "{err}");
}

#[cfg(unix)]
#[test]
fn chmod_sets_the_mode_and_rejects_invalid_ones() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f");
    fs::write(&file, "").unwrap();
    chmod(&file, 0o640).unwrap();
    assert_eq!(stat(&file).unwrap().permissions, Some(0o640));
    let err = chmod(&file, 0o17777).unwrap_err();
    assert!(err.contains("invalid mode"), "{err}");
}

// Ported from v0.2.15 `src-tauri/src/sftp/tests.rs`: the helpers of `launch` now live here.

#[test]
fn desktop_exec_puts_the_file_in_place_of_its_field_code() {
    let file = Path::new("/tmp/x/a b.txt");
    let args = |exec: &str| -> Vec<String> {
        desktop_exec_args(exec, file)
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
        chosen_editor_command(command, file).map(|args| args.into_iter().map(|a| a.into_string().unwrap()).collect())
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
    let args = chosen_editor_command(program.to_str().unwrap(), file).unwrap();
    assert_eq!(args, [program.as_os_str(), file.as_os_str()]);
}

#[test]
fn desktop_exec_is_read_from_the_desktop_entry_group() {
    let contents = "[Desktop Entry]\nName=Ed\nExec=ed --x %f\n\n[Desktop Action new]\nExec=ed --new\n";
    assert_eq!(desktop_exec(contents).as_deref(), Some("ed --x %f"));
    let only_action = "[Desktop Action new]\nExec=ed --new\n";
    assert_eq!(desktop_exec(only_action), None);
}

/// The chosen editor runs as its own program with the file as one argument: here a script that
/// records its arguments.
#[cfg(unix)]
#[test]
fn launch_runs_the_chosen_text_editor_without_a_shell() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("args.txt");
    let editor = dir.path().join("editor");
    fs::write(
        &editor,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display()),
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).unwrap();
    let file = dir.path().join("a b; touch pwned.txt");
    let with = OpenWith::TextEditor {
        command: Some(format!("{} -n %f", editor.display())),
    };
    launch(&with, &file).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !out.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(fs::read_to_string(&out).unwrap(), format!("-n\n{}\n", file.display()));
    assert!(!dir.path().join("pwned.txt").exists());
}

// FIX-1: the save writes a new file and renames it over the old one.

/// A reader that opened the file before the save still reads the old text: the save never
/// truncated and rewrote the file it had open (v0.2.15 did, so a crash could leave it empty).
#[cfg(unix)]
#[test]
fn write_text_replaces_the_file_instead_of_truncating_it_in_place() {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "old").unwrap();
    let before = fs::metadata(&file).unwrap().ino();
    let mut reader = fs::File::open(&file).unwrap();
    write_text(&file, "old", "new text", None, None).unwrap();
    let mut seen = String::new();
    reader.read_to_string(&mut seen).unwrap();
    assert_eq!(seen, "old");
    assert_eq!(fs::read_to_string(&file).unwrap(), "new text");
    assert_ne!(fs::metadata(&file).unwrap().ino(), before);
    // No temporary file is left next to it.
    assert_eq!(names(&list(dir.path()).unwrap()), ["a.txt"]);
}

/// A file with other hard links is still one file after the save: every name shows the new text.
#[cfg(unix)]
#[test]
fn write_text_keeps_the_hard_links_of_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "old").unwrap();
    let other = dir.path().join("b.txt");
    fs::hard_link(&file, &other).unwrap();
    write_text(&file, "old", "new", None, None).unwrap();
    assert_eq!(fs::read_to_string(&other).unwrap(), "new");
}

/// A symlink through a symlinked folder still saves its final target.
#[cfg(unix)]
#[test]
fn write_text_saves_the_final_target_of_a_chain_of_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    fs::write(real.join("f.txt"), "old").unwrap();
    std::os::unix::fs::symlink(&real, dir.path().join("folder")).unwrap();
    let link = dir.path().join("link.txt");
    std::os::unix::fs::symlink(dir.path().join("folder/f.txt"), &link).unwrap();
    write_text(&link, "old", "new", None, None).unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(real.join("f.txt")).unwrap(), "new");
    assert_eq!(names(&list(&real).unwrap()), ["f.txt"]);
}

// FIX-2: renames and moves never replace an entry, even one that appears after a check.

#[test]
fn the_no_replace_rename_never_replaces_a_file_or_an_empty_folder() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("from.txt");
    let to = dir.path().join("to.txt");
    fs::write(&from, "moving").unwrap();
    fs::write(&to, "kept").unwrap();
    let err = noreplace::rename_noreplace(&from, &to).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists, "{err}");
    assert_eq!(fs::read_to_string(&to).unwrap(), "kept");
    assert_eq!(fs::read_to_string(&from).unwrap(), "moving");

    // A plain rename would replace an empty folder with another folder.
    let from_dir = dir.path().join("a");
    let to_dir = dir.path().join("b");
    fs::create_dir_all(from_dir.join("inner")).unwrap();
    fs::create_dir(&to_dir).unwrap();
    let err = noreplace::rename_noreplace(&from_dir, &to_dir).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists, "{err}");
    assert!(from_dir.join("inner").is_dir());
    assert!(to_dir.is_dir() && !to_dir.join("inner").exists());

    // Nothing in the way: renamed.
    noreplace::rename_noreplace(&from, &dir.path().join("free.txt")).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("free.txt")).unwrap(), "moving");
}

#[cfg(unix)]
#[test]
fn the_no_replace_rename_moves_a_symlink_itself() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir(&dest).unwrap();
    let moved = move_into(&link, &dest).unwrap();
    assert!(fs::symlink_metadata(&moved).unwrap().file_type().is_symlink());
    assert!(target.is_dir() && fs::symlink_metadata(&link).is_err());
}

#[test]
fn rename_and_move_refuse_an_existing_empty_folder_with_the_same_name() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("a/inner")).unwrap();
    fs::create_dir(dir.path().join("b")).unwrap();
    let err = rename(&dir.path().join("a"), &dir.path().join("b")).unwrap_err();
    assert_eq!(
        err,
        format!(
            "cannot rename {}: {} already exists",
            dir.path().join("a").display(),
            dir.path().join("b").display()
        )
    );
    let dest = dir.path().join("dest");
    fs::create_dir_all(dest.join("a")).unwrap();
    let err = move_into(&dir.path().join("a"), &dest).unwrap_err();
    assert_eq!(
        err,
        format!(
            "cannot move {}: {} already exists",
            dir.path().join("a").display(),
            dest.join("a").display()
        )
    );
    assert!(dir.path().join("a/inner").is_dir());
}
