//! Reading and writing settings.json through core-storage (PRD DATA-1a, DATA-2a).

use std::path::{Path, PathBuf};

use core_storage::StorageError;

use crate::settings::Settings;

/// `<config dir>/settings.json`.
pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join(core_storage::SETTINGS_FILE)
}

/// Reads the settings file: the defaults when it does not exist; an error only when it cannot be
/// read (the caller keeps the settings it has, as v0.2.15 does).
pub fn load(path: &Path) -> Result<Settings, StorageError> {
    Ok(Settings::parse(core_storage::load(path)?.as_deref()))
}

/// Atomically replaces the settings file.
pub fn save(path: &Path, settings: &Settings) -> Result<(), StorageError> {
    core_storage::save(path, &settings.to_json())
}
