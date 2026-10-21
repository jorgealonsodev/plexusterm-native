//! A project's own PlexusTerm settings, kept with the project like VS Code's `.vscode/`:
//! `<project>/.plexusterm/settings.json` (PRD DATA-1b), ported from PlexusTerm v0.2.15
//! `src-tauri/src/project.rs`. DEV mode keeps the project's colours there (its tabs' and the
//! toolbar's, like VS Code's Peacock); the folder ignores itself in git.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use core_storage as storage;

/// Which of the project's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKind {
    /// The project's tabs.
    Tab,
    /// The window's toolbar while one of the project's tabs is active.
    Toolbar,
}

impl ColorKind {
    fn key(self) -> &'static str {
        match self {
            ColorKind::Tab => "tabColor",
            ColorKind::Toolbar => "toolbarColor",
        }
    }
}

/// The colours a project saved (`#rrggbb`), each `None` when unset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectColors {
    pub tab_color: Option<String>,
    pub toolbar_color: Option<String>,
}

/// The settings file of a project folder.
fn settings_path(project: &Path) -> PathBuf {
    project.join(".plexusterm").join("settings.json")
}

/// `#rrggbb`, lower-cased; `None` for anything else.
fn valid_color(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| color.to_ascii_lowercase())
}

/// The project's settings object: empty when there is no file yet, an error when the file
/// is not a JSON object (it is never overwritten then).
fn read_settings(project: &Path) -> Result<Map<String, Value>, String> {
    let path = settings_path(project);
    let text = storage::load(&path).map_err(|err| err.to_string())?;
    let Some(text) = text else {
        return Ok(Map::new());
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(format!("cannot read {}: it is not a JSON object", path.display())),
    }
}

/// The colours saved in the project; a hand-edited value that is not a colour is ignored.
pub fn colors_of(project: &Path) -> ProjectColors {
    let settings = read_settings(project).unwrap_or_default();
    let color = |kind: ColorKind| settings.get(kind.key())?.as_str().and_then(valid_color);
    ProjectColors {
        tab_color: color(ColorKind::Tab),
        toolbar_color: color(ColorKind::Toolbar),
    }
}

/// Saves one of the project's colours (`None` removes it), keeping the file's other settings.
pub fn set_color(project: &Path, kind: ColorKind, color: Option<&str>) -> Result<(), String> {
    if !project.is_dir() {
        return Err(format!("cannot save the colour: {} is not a folder", project.display()));
    }
    let color = match color {
        Some(c) => Some(valid_color(c).ok_or_else(|| format!("invalid colour: {c}"))?),
        None => None,
    };
    if color.is_none() && !settings_path(project).exists() {
        return Ok(());
    }
    // One read-change-write at a time, so two colours saved together are both kept.
    static SAVING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _saving = SAVING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = read_settings(project)?;
    match color {
        Some(c) => settings.insert(kind.key().into(), Value::String(c)),
        None => settings.remove(kind.key()),
    };
    let text = serde_json::to_string_pretty(&Value::Object(settings)).map_err(|err| err.to_string())?;
    let path = settings_path(project);
    storage::save(&path, &format!("{text}\n")).map_err(|err| err.to_string())?;
    readable_by_all(&path);
    ignore_in_git(path.parent().unwrap_or(project));
    Ok(())
}

/// A project file, not a private one: readable like the project's other files (the atomic save
/// writes it owner-only).
fn readable_by_all(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Makes git ignore the `.plexusterm` folder from inside it (`*`), as `.pytest_cache` does, so the
/// project's own `.gitignore` is never edited; a `.gitignore` already there is left alone.
fn ignore_in_git(folder: &Path) {
    let gitignore = folder.join(".gitignore");
    if !gitignore.exists() {
        // Best effort: the colour is saved either way.
        let _ = std::fs::write(gitignore, "*\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[allow(clippy::unwrap_used, reason = "a test helper: a failure here is a failed test")]
    fn settings(dir: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(dir.join(".plexusterm/settings.json")).unwrap()).unwrap()
    }

    #[test]
    fn saves_and_reads_both_colours_in_one_file_in_the_project_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(colors_of(dir.path()), ProjectColors::default());
        set_color(dir.path(), ColorKind::Tab, Some("#C0392B")).unwrap();
        set_color(dir.path(), ColorKind::Toolbar, Some("#2e86de")).unwrap();
        assert_eq!(
            settings(dir.path()),
            serde_json::json!({ "tabColor": "#c0392b", "toolbarColor": "#2e86de" })
        );
        assert_eq!(
            colors_of(dir.path()),
            ProjectColors {
                tab_color: Some("#c0392b".into()),
                toolbar_color: Some("#2e86de".into()),
            }
        );
        // Each one is removed alone.
        set_color(dir.path(), ColorKind::Tab, None).unwrap();
        assert_eq!(settings(dir.path()), serde_json::json!({ "toolbarColor": "#2e86de" }));
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_like_the_project_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        set_color(dir.path(), ColorKind::Tab, Some("#c0392b")).unwrap();
        let mode = fs::metadata(dir.path().join(".plexusterm/settings.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o644);
    }

    #[test]
    fn the_folder_ignores_itself_in_git_without_touching_the_project_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        set_color(dir.path(), ColorKind::Tab, Some("#c0392b")).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(".plexusterm/.gitignore")).unwrap(),
            "*\n"
        );
        assert_eq!(fs::read_to_string(dir.path().join(".gitignore")).unwrap(), "target/\n");
        // One the user changed is kept.
        fs::write(dir.path().join(".plexusterm/.gitignore"), "# shared\n").unwrap();
        set_color(dir.path(), ColorKind::Tab, Some("#16a085")).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(".plexusterm/.gitignore")).unwrap(),
            "# shared\n"
        );
    }

    #[test]
    fn colours_saved_at_the_same_time_are_both_kept() {
        for _ in 0..20 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().to_path_buf();
            let other = path.clone();
            let tab = std::thread::spawn(move || set_color(&other, ColorKind::Tab, Some("#111111")).unwrap());
            set_color(&path, ColorKind::Toolbar, Some("#222222")).unwrap();
            tab.join().unwrap();
            let colors = colors_of(&path);
            assert_eq!(colors.tab_color.as_deref(), Some("#111111"));
            assert_eq!(colors.toolbar_color.as_deref(), Some("#222222"));
        }
    }

    #[test]
    fn keeps_the_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".plexusterm")).unwrap();
        fs::write(dir.path().join(".plexusterm/settings.json"), r#"{ "other": [1, 2] }"#).unwrap();
        set_color(dir.path(), ColorKind::Toolbar, Some("#16a085")).unwrap();
        assert_eq!(
            settings(dir.path()),
            serde_json::json!({ "other": [1, 2], "toolbarColor": "#16a085" })
        );
        set_color(dir.path(), ColorKind::Toolbar, None).unwrap();
        assert_eq!(settings(dir.path()), serde_json::json!({ "other": [1, 2] }));
    }

    #[test]
    fn clearing_a_colour_never_creates_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        set_color(dir.path(), ColorKind::Tab, None).unwrap();
        assert!(!dir.path().join(".plexusterm").exists());
    }

    #[test]
    fn refuses_invalid_colours_a_missing_project_and_a_settings_file_it_cannot_read() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            set_color(dir.path(), ColorKind::Tab, Some("red"))
                .unwrap_err()
                .contains("invalid colour")
        );
        assert!(set_color(&dir.path().join("missing"), ColorKind::Tab, Some("#000000")).is_err());
        fs::create_dir(dir.path().join(".plexusterm")).unwrap();
        fs::write(dir.path().join(".plexusterm/settings.json"), "not json").unwrap();
        // Never overwritten: the file may be the user's.
        assert!(set_color(dir.path(), ColorKind::Tab, Some("#000000")).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join(".plexusterm/settings.json")).unwrap(),
            "not json"
        );
        // A hand-edited colour that is not one is ignored.
        fs::write(
            dir.path().join(".plexusterm/settings.json"),
            r#"{ "tabColor": "url(x)" }"#,
        )
        .unwrap();
        assert_eq!(colors_of(dir.path()), ProjectColors::default());
    }
}
