//! The local folders DEV mode was last used in, and the workspace files last opened, offered by
//! the Explorer to open again. Port of v0.2.15 `src/features/devmode/recentFolders.ts` without
//! its storage (the settings store keeps the lists).

/// How many folders are kept unless Settings say otherwise, and the most they may say.
pub const DEFAULT_RECENT_FOLDERS: usize = 10;
pub const MAX_RECENT_FOLDERS: usize = 50;

/// Puts `folder` first, without its older entry, keeping the last `limit`; false when nothing
/// changes (it was already first within the limit).
pub fn add_recent_folder(list: &mut Vec<String>, folder: &str, limit: usize) -> bool {
    if list.first().is_some_and(|first| first == folder) && list.len() <= limit {
        return false;
    }
    list.retain(|f| f != folder);
    list.insert(0, folder.to_owned());
    list.truncate(limit);
    true
}

/// A saved list read back: no blanks, no duplicates, at most `MAX_RECENT_FOLDERS` whatever the
/// limit now.
pub fn clean_recent_folders<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut folders: Vec<String> = Vec::new();
    for item in items {
        let item = item.as_ref();
        if !item.is_empty() && !folders.iter().any(|f| f == item) {
            folders.push(item.to_owned());
        }
    }
    folders.truncate(MAX_RECENT_FOLDERS);
    folders
}
