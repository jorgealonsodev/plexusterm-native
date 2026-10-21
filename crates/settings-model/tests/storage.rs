//! Ported from PlexusTerm v0.2.15 `src/features/settings/useSettings.test.ts`: the hook's load and
//! save through core-storage (PRD SET-1, DATA-1a, DATA-2a).

use settings_model::settings::Settings;
use settings_model::storage::{load, save, settings_path};

#[allow(clippy::unwrap_used, reason = "test helper")]
fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// it("loads the saved settings")
#[test]
fn loads_the_saved_settings() {
    let dir = temp_dir();
    let path = settings_path(dir.path());
    assert_eq!(path, dir.path().join("settings.json"));
    std::fs::write(&path, r#"{"fontSize":20}"#).unwrap();
    assert_eq!(load(&path).unwrap().font_size, 20);
    // No file yet: the defaults.
    assert_eq!(load(&dir.path().join("missing.json")).unwrap(), Settings::default());
}

// it("keeps the defaults when loading fails")
#[test]
fn keeps_the_defaults_when_loading_fails() {
    let dir = temp_dir();
    // A folder cannot be read as a file.
    let result = load(dir.path());
    assert!(result.is_err());
    assert_eq!(result.unwrap_or_default(), Settings::default());
}

// it("saves every change, clamped, and keeps it over a late load")
// The "late load" half is React state ordering and is dropped; the save and the clamp are ported.
#[test]
fn saves_every_change_clamped() {
    let dir = temp_dir();
    let path = settings_path(dir.path());
    let mut settings = Settings::default();
    settings.set_font_size(50.0);
    settings.copy_on_select = false;
    assert_eq!(
        settings,
        Settings {
            font_size: 32,
            copy_on_select: false,
            ..Settings::default()
        }
    );
    save(&path, &settings).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("\"fontSize\": 32"));
    assert_eq!(load(&path).unwrap(), settings);
}

// Native (§4.1 deviation 2): a save keeps the keys a newer PlexusTerm wrote.
#[test]
fn a_save_keeps_unknown_keys_of_the_file() {
    let dir = temp_dir();
    let path = settings_path(dir.path());
    std::fs::write(&path, r#"{"version":2,"futureKey":{"x":[1,2]},"fontSize":12}"#).unwrap();
    let mut settings = load(&path).unwrap();
    settings.set_font_size(13.0);
    save(&path, &settings).unwrap();
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(value.get("futureKey"), Some(&serde_json::json!({"x":[1,2]})));
    assert_eq!(value.get("fontSize"), Some(&serde_json::json!(13)));
}
