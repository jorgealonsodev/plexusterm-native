//! Port of v0.2.15 `src/features/devmode/recentFolders.test.ts`. Saving to local storage is not
//! this crate's (the storage crate keeps settings); the rules for a saved list read back are.

use workspace_model::{DEFAULT_RECENT_FOLDERS, MAX_RECENT_FOLDERS, add_recent_folder, clean_recent_folders};

fn list(items: &[&str]) -> Vec<String> {
    items.iter().map(|i| (*i).to_owned()).collect()
}

// "puts the latest folder first, once, and keeps the last ten by default"
#[test]
fn puts_the_latest_folder_first_once_and_keeps_the_last_ten_by_default() {
    let mut folders = list(&["/a", "/b"]);
    assert!(add_recent_folder(&mut folders, "/c", DEFAULT_RECENT_FOLDERS));
    assert_eq!(folders, list(&["/c", "/a", "/b"]));
    let mut folders = list(&["/a", "/b", "/c"]);
    add_recent_folder(&mut folders, "/b", DEFAULT_RECENT_FOLDERS);
    assert_eq!(folders, list(&["/b", "/a", "/c"]));
    let mut many: Vec<String> = (0..DEFAULT_RECENT_FOLDERS).map(|i| format!("/p{i}")).collect();
    add_recent_folder(&mut many, "/new", DEFAULT_RECENT_FOLDERS);
    assert_eq!(many.len(), 10);
    assert_eq!(many[0], "/new");
    assert!(!many.contains(&"/p9".to_owned()));
}

// "keeps as many folders as asked"
#[test]
fn keeps_as_many_folders_as_asked() {
    let mut folders = list(&["/a", "/b", "/c"]);
    add_recent_folder(&mut folders, "/d", 2);
    assert_eq!(folders, list(&["/d", "/a"]));
    let mut folders = list(&["/a", "/b", "/c"]);
    assert!(add_recent_folder(&mut folders, "/a", 2));
    assert_eq!(folders, list(&["/a", "/b"]));
}

// "returns the same list when the folder is already first"
#[test]
fn returns_the_same_list_when_the_folder_is_already_first() {
    let mut folders = list(&["/a", "/b"]);
    assert!(!add_recent_folder(&mut folders, "/a", DEFAULT_RECENT_FOLDERS));
    assert_eq!(folders, list(&["/a", "/b"]));
}

// "survives a restart in local storage and ignores a damaged value": the storage half is the
// storage crate's; the list read back drops blanks and duplicates and keeps at most 50.
#[test]
fn a_saved_list_read_back_drops_blanks_and_duplicates_and_keeps_at_most_fifty() {
    assert_eq!(clean_recent_folders(["/ok", "", "/ok", "/two"]), list(&["/ok", "/two"]));
    let many: Vec<String> = (0..60).map(|i| format!("/p{i}")).collect();
    assert_eq!(clean_recent_folders(&many).len(), MAX_RECENT_FOLDERS);
}

// "keeps the recent workspace files apart from the folders": two storage keys; dropped, as
// storage is not this crate's (the same functions serve both lists).
