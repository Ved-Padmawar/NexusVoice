//! Identifying the application the user is dictating into.
//!
//! Captured when recording starts, not at finalize: by then the user may have
//! switched away, and this app's own pill can hold focus.
//!
//! The formatter consumes [`AppCategory`] — how text should be shaped depends
//! on the kind of destination, not on which editor it is. The transcript card
//! consumes [`FocusTarget::name`], which is what a person wants to read.

mod catalog;

#[cfg(target_os = "windows")]
mod windows;

pub use catalog::AppCategory;

/// The application that had focus when dictation started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTarget {
    /// "VS Code" for a catalogued app, else the executable stem as-is.
    pub name: String,
    pub category: AppCategory,
}

impl FocusTarget {
    /// `stem` is an executable name without directory or `.exe`.
    #[cfg(any(target_os = "windows", test))]
    fn from_executable(stem: &str) -> Self {
        catalog::lookup(stem).map_or_else(
            || Self {
                name: stem.to_string(),
                category: AppCategory::Unknown,
            },
            |(name, category)| Self {
                name: name.to_string(),
                category,
            },
        )
    }
}

/// The foreground application, or `None` when it cannot be determined.
///
/// Best-effort by nature — the query can be refused on a protected process.
/// `None` means "format without app context", never an error to surface.
#[must_use]
pub fn foreground_app() -> Option<FocusTarget> {
    #[cfg(target_os = "windows")]
    {
        windows::foreground_executable_stem().map(|stem| FocusTarget::from_executable(&stem))
    }
    // Wayland deliberately hides the focused app and offers no portal for it;
    // macOS would need its own AppKit path.
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(test)]
#[path = "../../tests/unit/focus/mod.rs"]
mod tests;
