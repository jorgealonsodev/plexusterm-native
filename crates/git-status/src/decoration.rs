//! Git status of the entries in the file lists, decorated like VS Code's Explorer
//! (microsoft/vscode, extensions/git/src/repository.ts): a letter, a colour, a tooltip, deleted
//! files struck through, ignored ones faded, and folders coloured like their most important
//! descendant (PRD TREE-1, TREE-3). Ported from v0.2.15 `src/features/files/gitStatus.ts`.

use std::collections::HashMap;

use crate::{FileStatus, StatusEntry};

/// VS Code's `gitDecoration.*ResourceForeground` colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitColor {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Ignored,
    StageModified,
    StageDeleted,
    Conflicting,
}

/// The tooltip of a status (v0.2.15 `sftp.git.<label>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitLabel {
    Modified,
    StagedModified,
    Added,
    Deleted,
    StagedDeleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Ignored,
    Conflict,
}

/// How an entry is decorated; a folder coloured by its contents has no letter (a dot is shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoration {
    pub status: FileStatus,
    pub letter: Option<char>,
    pub color: GitColor,
    pub label: GitLabel,
    pub strike: bool,
    pub faded: bool,
}

/// The letter, colour and label of a status, and VS Code's priority when one file has several:
/// conflict 4, ignored 3, modified 2, others 1.
fn info(status: FileStatus) -> (char, GitColor, GitLabel, u8) {
    use FileStatus::*;
    match status {
        IndexModified => ('M', GitColor::StageModified, GitLabel::StagedModified, 2),
        Modified => ('M', GitColor::Modified, GitLabel::Modified, 2),
        IndexAdded | IntentToAdd => ('A', GitColor::Added, GitLabel::Added, 1),
        IndexDeleted => ('D', GitColor::StageDeleted, GitLabel::StagedDeleted, 1),
        Deleted => ('D', GitColor::Deleted, GitLabel::Deleted, 1),
        IndexRenamed | IntentToRename => ('R', GitColor::Renamed, GitLabel::Renamed, 1),
        IndexCopied => ('C', GitColor::Renamed, GitLabel::Copied, 2),
        IndexTypeChanged | TypeChanged => ('T', GitColor::Modified, GitLabel::TypeChanged, 2),
        Untracked => ('U', GitColor::Untracked, GitLabel::Untracked, 1),
        Ignored => ('I', GitColor::Ignored, GitLabel::Ignored, 3),
        AddedByUs | AddedByThem | DeletedByUs | DeletedByThem | BothAdded | BothDeleted | BothModified => {
            ('!', GitColor::Conflicting, GitLabel::Conflict, 4)
        }
    }
}

fn priority(status: FileStatus) -> u8 {
    info(status).3
}

fn is_deleted(status: FileStatus) -> bool {
    matches!(status, FileStatus::Deleted | FileStatus::IndexDeleted)
}

/// Like VS Code, deleted and ignored entries do not colour the folders above them.
fn propagates(status: FileStatus) -> bool {
    !is_deleted(status) && status != FileStatus::Ignored
}

/// The more important of two statuses; on a tie the later one (the working tree) wins.
fn pick(current: Option<FileStatus>, next: FileStatus) -> FileStatus {
    match current {
        Some(current) if priority(next) < priority(current) => current,
        _ => next,
    }
}

/// The statuses of one list, indexed so each row is decorated without scanning them all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusIndex {
    /// Statuses of exact paths, in the backend's order (index column first).
    own: HashMap<String, Vec<FileStatus>>,
    /// Folder blocks (`dir/`, or `""` for the whole list) and their statuses.
    blocks: Vec<(String, FileStatus)>,
    /// The most important propagating status below each folder path.
    folders: HashMap<String, FileStatus>,
}

/// Indexes the statuses [`crate::status_of`] (or [`crate::ssh_git_status`]) read for one list.
pub fn index_status(entries: &[StatusEntry]) -> StatusIndex {
    let mut index = StatusIndex::default();
    for StatusEntry { path, status } in entries {
        let status = *status;
        if path.is_empty() || path.ends_with('/') {
            index.blocks.push((path.clone(), status));
        } else {
            index.own.entry(path.clone()).or_default().push(status);
        }
        if !propagates(status) {
            continue;
        }
        // Every folder above the entry (a block also colours the folders above itself).
        let trimmed = path.strip_suffix('/').unwrap_or(path);
        for (end, _) in trimmed.match_indices('/') {
            if let Some(folder) = trimmed.get(..end) {
                let best = pick(index.folders.get(folder).copied(), status);
                index.folders.insert(folder.to_string(), best);
            }
        }
    }
    index
}

fn decoration(status: FileStatus, is_dir: bool) -> Decoration {
    let (letter, color, label, _) = info(status);
    Decoration {
        status,
        letter: (!is_dir).then_some(letter),
        color,
        label,
        strike: is_deleted(status),
        faded: status == FileStatus::Ignored,
    }
}

/// The decoration of the entry at `path` (relative to the listed folder), or `None` for none.
pub fn decoration_for(index: &StatusIndex, path: &str, is_dir: bool) -> Option<Decoration> {
    let mut best = None;
    for &status in index.own.get(path).into_iter().flatten() {
        best = Some(pick(best, status));
    }
    let inside = format!("{path}/");
    for (block, status) in &index.blocks {
        if block.is_empty() || inside.starts_with(block.as_str()) {
            best = Some(pick(best, *status));
        }
    }
    if let Some(best) = best {
        return Some(decoration(best, is_dir));
    }
    let below = if is_dir { index.folders.get(path) } else { None };
    below.map(|&status| decoration(status, true))
}

/// The decoration of a whole listed folder (a workspace root): a dot coloured as its folders are.
pub fn root_decoration(entries: &[StatusEntry]) -> Option<Decoration> {
    let mut best = None;
    for entry in entries.iter().filter(|entry| propagates(entry.status)) {
        best = Some(pick(best, entry.status));
    }
    best.map(|status| decoration(status, true))
}

#[cfg(test)]
mod tests {
    //! Ported from v0.2.15 `src/features/files/gitStatus.test.ts`.
    use super::*;
    use FileStatus::*;

    fn e(path: &str, status: FileStatus) -> StatusEntry {
        StatusEntry {
            path: path.into(),
            status,
        }
    }

    fn deco(entries: &[StatusEntry], path: &str, is_dir: bool) -> Option<Decoration> {
        decoration_for(&index_status(entries), path, is_dir)
    }

    /// The letter, colour and label of `path` as a file.
    fn shown(entries: &[StatusEntry], path: &str) -> Option<(Option<char>, GitColor, GitLabel)> {
        deco(entries, path, false).map(|d| (d.letter, d.color, d.label))
    }

    #[test]
    fn gives_files_vs_codes_letter_colour_and_label() {
        use GitColor as C;
        use GitLabel as L;
        let cases = [
            (Modified, 'M', C::Modified, L::Modified),
            (IndexModified, 'M', C::StageModified, L::StagedModified),
            (IndexAdded, 'A', C::Added, L::Added),
            (IntentToAdd, 'A', C::Added, L::Added),
            (IndexDeleted, 'D', C::StageDeleted, L::StagedDeleted),
            (Deleted, 'D', C::Deleted, L::Deleted),
            (IndexRenamed, 'R', C::Renamed, L::Renamed),
            (IntentToRename, 'R', C::Renamed, L::Renamed),
            (IndexCopied, 'C', C::Renamed, L::Copied),
            (TypeChanged, 'T', C::Modified, L::TypeChanged),
            (Untracked, 'U', C::Untracked, L::Untracked),
            (Ignored, 'I', C::Ignored, L::Ignored),
        ];
        for (status, letter, color, label) in cases {
            assert_eq!(
                shown(&[e("a.ts", status)], "a.ts"),
                Some((Some(letter), color, label)),
                "{status:?}"
            );
        }
        assert!(deco(&[e("a.ts", IndexDeleted)], "a.ts", false).unwrap().strike);
        assert!(deco(&[e("a.ts", Deleted)], "a.ts", false).unwrap().strike);
        assert!(deco(&[e("a.ts", Ignored)], "a.ts", false).unwrap().faded);
        for status in [
            BothModified,
            BothAdded,
            BothDeleted,
            AddedByUs,
            AddedByThem,
            DeletedByUs,
            DeletedByThem,
        ] {
            assert_eq!(
                shown(&[e("a.ts", status)], "a.ts"),
                Some((Some('!'), C::Conflicting, L::Conflict)),
                "{status:?}"
            );
        }
        assert_eq!(deco(&[e("a.ts", Modified)], "b.ts", false), None);
    }

    #[test]
    fn picks_the_highest_priority_when_the_index_and_the_working_tree_differ() {
        // modified (2) over added (1); conflict (4) over everything; equal priority: the working tree.
        let status = |entries: &[StatusEntry]| deco(entries, "a", false).map(|d| d.status);
        assert_eq!(status(&[e("a", IndexAdded), e("a", Modified)]), Some(Modified));
        assert_eq!(status(&[e("a", Modified), e("a", BothModified)]), Some(BothModified));
        assert_eq!(status(&[e("a", IndexModified), e("a", Modified)]), Some(Modified));
        assert_eq!(status(&[e("a", IndexAdded), e("a", Deleted)]), Some(Deleted));
    }

    #[test]
    fn colours_a_folder_like_its_most_important_descendant_without_a_letter_never_for_deleted_or_ignored_ones() {
        let entries = [
            e("src/a.ts", Untracked),
            e("src/deep/b.ts", Modified),
            e("src/gone.ts", Deleted),
            e("src/x.log", Ignored),
        ];
        let src = deco(&entries, "src", true).unwrap();
        assert_eq!(
            (src.status, src.letter, src.color),
            (Modified, None, GitColor::Modified)
        );
        let deep = deco(&entries, "src/deep", true).unwrap();
        assert_eq!((deep.status, deep.letter), (Modified, None));
        assert_eq!(
            deco(&[e("lib/gone.ts", Deleted), e("lib/old.ts", IndexDeleted)], "lib", true),
            None
        );
        assert_eq!(deco(&[e("lib/x.log", Ignored)], "lib", true), None);
        assert_eq!(
            deco(&[e("src/c.ts", BothModified), e("src/a.ts", Modified)], "src", true).map(|d| d.color),
            Some(GitColor::Conflicting)
        );
        assert_eq!(deco(&[e("srcx/a.ts", Modified)], "src", true), None);
    }

    #[test]
    fn treats_a_folder_entry_as_covering_everything_inside_it() {
        let entries = [e("node_modules/", Ignored), e("fresh/", Untracked)];
        let modules = deco(&entries, "node_modules", true).unwrap();
        assert_eq!((modules.status, modules.letter, modules.faded), (Ignored, None, true));
        let inside = deco(&entries, "node_modules/react/index.js", false).unwrap();
        assert_eq!((inside.status, inside.letter), (Ignored, Some('I')));
        let fresh = deco(&entries, "fresh", true).unwrap();
        assert_eq!((fresh.status, fresh.letter), (Untracked, None));
        let file = deco(&entries, "fresh/a.txt", false).unwrap();
        assert_eq!((file.status, file.letter), (Untracked, Some('U')));
        // The listed folder itself inside an ignored one.
        assert_eq!(
            deco(&[e("", Ignored)], "anything.js", false).map(|d| d.status),
            Some(Ignored)
        );
        // An ignored folder does not grey its parent.
        assert_eq!(deco(&entries, "", true), None);
        assert_eq!(deco(&[e("web/node_modules/", Ignored)], "web", true), None);
    }

    #[test]
    fn a_workspace_roots_decoration_is_a_dot_coloured_by_the_most_important_change_inside_as_a_folders() {
        let root = root_decoration(&[e("src/a.ts", Untracked), e("b.ts", Modified)]).unwrap();
        assert_eq!((root.letter, root.color), (None, GitColor::Modified));
    }

    #[test]
    fn a_workspace_roots_decoration_is_none_when_nothing_changed_or_only_ignored_files_are_listed() {
        assert_eq!(root_decoration(&[]), None);
        assert_eq!(root_decoration(&[e("node_modules/", Ignored)]), None);
    }
}
