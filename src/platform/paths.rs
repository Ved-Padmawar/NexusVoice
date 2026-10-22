//! Where the app keeps its files: the same dirs as the Tauri build.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Debug builds get their own data dir, a "Dev" badge and instance lock.
pub const IS_DEV: bool = cfg!(debug_assertions);

pub const IDENTIFIER: &str = if IS_DEV {
    "com.nexusvoice.app.dev"
} else {
    "com.nexusvoice.app"
};

pub const PRODUCT_NAME: &str = if IS_DEV {
    "NexusVoice Dev"
} else {
    "NexusVoice"
};

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

/// The release models dir even in dev (`<id>.dev` minus `.dev`), so dev reuses downloads.
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
