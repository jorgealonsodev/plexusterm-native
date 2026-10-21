//! The text helpers shared by the SFTP browser and the file viewer, checked on bytes instead of
//! files. Adapted from PlexusTerm v0.2.15 `src-tauri/src/localfs.rs` tests, which reach the same
//! helpers through local files.

use proto_sftp::text::{
    MAX_ATTACH_BYTES, MAX_IMAGE_BYTES, MAX_VIEW_BYTES, TextFile, check_unchanged, decode_text, encode_text,
    image_too_large, is_image_path, listing_text, not_an_image, text_for, text_from,
};
use proto_sftp::{Entry, EntryKind};

fn view(bytes: &[u8], encoding: Option<&str>) -> Result<TextFile, String> {
    decode_text("view", "/f.txt", bytes.to_vec(), false, encoding)
}

fn entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        name: name.into(),
        path: format!("/{name}"),
        kind,
        size: 0,
        modified: None,
        permissions: None,
        uid: None,
        gid: None,
        user: None,
        group: None,
    }
}

#[test]
fn reads_text_up_to_the_limit_and_refuses_binary_bytes() {
    let read = text_from("/notes.txt", b"line 1\nline 2".to_vec(), false).unwrap();
    assert_eq!(
        read,
        TextFile {
            path: "/notes.txt".into(),
            content: "line 1\nline 2".into(),
            truncated: false,
            encoding: "utf-8".into(),
        }
    );

    // A two-byte character cut by the limit is dropped, not refused.
    let mut text = "a".repeat(MAX_ATTACH_BYTES - 1);
    text.push('ñ');
    let cut = text.as_bytes()[..MAX_ATTACH_BYTES].to_vec();
    let read = text_from("/big.txt", cut, true).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_ATTACH_BYTES - 1));

    assert_eq!(
        text_from("/a.bin", vec![0x7f, b'E', b'L', b'F', 0, 1], false).unwrap_err(),
        "cannot attach /a.bin: it is not a text file"
    );
    assert_eq!(
        text_from("/a.bin", vec![0xff, 0xfe, b'a'], false).unwrap_err(),
        "cannot attach /a.bin: it is not a text file"
    );
    assert_eq!(
        text_for("view", "/a.bin", vec![0], false).unwrap_err(),
        "cannot view /a.bin: it is not a text file"
    );
}

#[test]
fn viewing_plain_utf8_reports_utf8_as_before() {
    let read = view("año\n".as_bytes(), None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("año\n", "utf-8"));
}

#[test]
fn viewing_detects_a_bom_and_leaves_it_out() {
    let read = view(b"\xEF\xBB\xBFhi \xC3\xA9", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hi é", "utf-8-bom"));
    let read = view(b"\xFF\xFEh\0i\0\xE9\0", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hié", "utf-16le"));
    let read = view(b"\xFE\xFF\0h\0i\0\xE9", None).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("hié", "utf-16be"));
}

#[test]
fn viewing_with_an_encoding_decodes_with_it() {
    let read = view(b"caf\xE9 \x80", Some("windows-1252")).unwrap();
    assert_eq!(
        (read.content.as_str(), read.encoding.as_str()),
        ("café €", "windows-1252")
    );
    let read = view(b"\x93\xFA\x96\x7B", Some("shift_jis")).unwrap();
    assert_eq!(read.content, "日本");
    // UTF-16 text holds NUL bytes; without a BOM it is decoded as asked.
    let read = view(b"a\0b\0", Some("utf-16le")).unwrap();
    assert_eq!((read.content.as_str(), read.encoding.as_str()), ("ab", "utf-16le"));
    let read = view(b"\xEF\xBB\xBFx", Some("utf-8-bom")).unwrap();
    assert_eq!(read.content, "x");
}

#[test]
fn viewing_refuses_unknown_encodings_and_malformed_text() {
    assert_eq!(
        view(b"x", Some("klingon")).unwrap_err(),
        "cannot view /f.txt: unknown encoding klingon"
    );
    // A Shift_JIS lead byte with nothing after it, and an odd UTF-16 length.
    assert_eq!(
        view(b"a\x93", Some("shift_jis")).unwrap_err(),
        "cannot view /f.txt: it is not a text file in shift_jis"
    );
    assert_eq!(
        view(b"a\0b", Some("utf-16le")).unwrap_err(),
        "cannot view /f.txt: it is not a text file in utf-16le"
    );
    assert_eq!(
        view(b"a\0b", Some("windows-1252")).unwrap_err(),
        "cannot view /f.txt: it is not a text file"
    );
    assert_eq!(
        view(b"\xC3(", Some("utf-8")).unwrap_err(),
        "cannot view /f.txt: it is not a text file"
    );
}

#[test]
fn a_character_cut_by_the_view_limit_is_dropped_in_any_encoding() {
    // UTF-16: a surrogate pair cut after its first unit.
    let text = format!("{}😀", "a".repeat(MAX_VIEW_BYTES / 2 - 2));
    let mut bytes = b"\xFF\xFE".to_vec();
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes.truncate(MAX_VIEW_BYTES);
    let read = decode_text("view", "/f.txt", bytes, true, None).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_VIEW_BYTES / 2 - 2));
    // Shift_JIS: a two-byte character cut after its first byte.
    let mut bytes = b"a".repeat(MAX_VIEW_BYTES - 1);
    bytes.push(0x93);
    let read = decode_text("view", "/f.txt", bytes, true, Some("shift_jis")).unwrap();
    assert!(read.truncated);
    assert_eq!(read.content, "a".repeat(MAX_VIEW_BYTES - 1));
}

#[test]
fn writing_keeps_the_encoding_and_its_bom() {
    let cases: [(&str, &[u8]); 5] = [
        ("utf-8", b"n\xC3\xA9w"),
        ("utf-8-bom", b"\xEF\xBB\xBFn\xC3\xA9w"),
        ("utf-16le", b"\xFF\xFEn\0\xE9\0w\0"),
        ("utf-16be", b"\xFE\xFF\0n\0\xE9\0w"),
        ("windows-1252", b"n\xE9w"),
    ];
    for (encoding, bytes) in cases {
        assert_eq!(
            encode_text("/f.txt", "néw", Some(encoding)).unwrap(),
            bytes,
            "{encoding}"
        );
        assert_eq!(view(bytes, Some(encoding)).unwrap().content, "néw", "{encoding}");
    }
    assert_eq!(encode_text("/f.txt", "néw", None).unwrap(), "néw".as_bytes());
}

#[test]
fn writing_refuses_characters_the_encoding_cannot_hold() {
    assert_eq!(
        encode_text("/f.txt", "日本", Some("windows-1252")).unwrap_err(),
        "cannot save /f.txt: some characters cannot be written in windows-1252"
    );
    assert_eq!(
        encode_text("/f.txt", "x", Some("klingon")).unwrap_err(),
        "cannot save /f.txt: unknown encoding klingon"
    );
}

#[test]
fn a_file_is_unchanged_only_while_it_still_decodes_to_the_opened_text() {
    assert_eq!(check_unchanged("/f.txt", b"old".to_vec(), false, "old", None), Ok(()));
    assert_eq!(
        check_unchanged("/f.txt", b"caf\xE9".to_vec(), false, "café", Some("windows-1252")),
        Ok(())
    );
    let changed = Err("cannot save /f.txt: it changed on disk since it was opened".to_string());
    assert_eq!(
        check_unchanged("/f.txt", b"theirs".to_vec(), false, "old", None),
        changed
    );
    // A file longer than the viewer reads is never written over.
    assert_eq!(check_unchanged("/f.txt", b"old".to_vec(), true, "old", None), changed);
    // Bytes that do not decode in the encoding count as changed too.
    assert_eq!(
        check_unchanged("/f.txt", b"a\0b".to_vec(), false, "ab", Some("utf-16le")),
        changed
    );
    assert_eq!(
        check_unchanged("/f.txt", b"old".to_vec(), false, "old", Some("klingon")),
        Err("cannot save /f.txt: unknown encoding klingon".to_string())
    );
}

#[test]
fn a_folder_is_attached_as_its_listing() {
    let entries = [entry("src", EntryKind::Dir), entry("README.md", EntryKind::File)];
    let read = listing_text("/repo", &entries);
    assert_eq!(read.content, "src/\nREADME.md");
    assert_eq!(
        (read.path.as_str(), read.truncated, read.encoding.as_str()),
        ("/repo", false, "utf-8")
    );

    let many: Vec<Entry> = (0..501).map(|i| entry(&format!("f{i}"), EntryKind::File)).collect();
    let read = listing_text("/big", &many);
    assert!(read.truncated);
    assert_eq!(read.content.lines().count(), 500);
}

#[test]
fn only_image_names_are_images() {
    assert!(is_image_path("/x/shot.PNG"));
    assert!(is_image_path("/x/logo.svg"));
    assert!(!is_image_path("/x/id_ed25519"));
    assert!(!is_image_path("/x/notes.txt"));
    assert_eq!(not_an_image("/notes.txt"), "cannot show /notes.txt: it is not an image");
    assert_eq!(
        image_too_large("/big.jpg"),
        "cannot show /big.jpg: it is larger than 20 MiB"
    );
    assert_eq!(MAX_IMAGE_BYTES, 20 * 1024 * 1024);
}
