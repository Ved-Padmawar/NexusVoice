//! Where the app keeps its files. Matches the directories the Tauri build used,
//! so an existing install keeps its models, history and settings.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Debug builds are the dev app: their own data dir, a "Dev" badge, and a
/// separate single-instance lock so they run beside an installed release.
pub const IS_DEV: bool = cfg!(debug_assertions);

pub const IDENTIFIER: &str = if IS_DEV {
    "com.nexusvoice.app.dev"
} else {
    "com.nexusvoice.app"
};

pub const PRODUCT_NAME: &str = if IS_DEV { "NexusVoice Dev" } else { "NexusVoice" };

fn base(dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(std::env::temp_dir).join(IDENTIFIER)
}

/// Settings, database, hotkeys. `%APPDATA%` on Windows, `$XDG_DATA_HOME` on Linux.
pub fn app_data_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| base(dirs::data_dir()))
}

/// `%LOCALAPPDATA%\<id>\logs` on Windows, `$XDG_DATA_HOME/<id>/logs` on Linux.
pub fn log_dir() -> PathBuf {
    base(dirs::data_local_dir()).join("logs")
}

/// Models live in the release data dir even in dev, so a dev build reuses
/// downloads. Dev's dir is the release one plus a `.dev` suffix.
pub fn shared_models_dir() -> PathBuf {
    models_dir_for(app_data_dir())
}

fn models_dir_for(app_data_dir: &Path) -> PathBuf {
    let shared_root = app_data_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".dev"))
        .map_or_else(
            || app_data_dir.to_path_buf(),
            |release| app_data_dir.with_file_name(release),
        );
    shared_root.join("models")
}

#[cfg(test)]
#[path = "../../tests/unit/platform/paths.rs"]
mod tests;
