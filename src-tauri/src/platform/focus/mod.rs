//! Identifying the application the user is dictating into.
//!
//! Captured when recording starts, which shapes the formatter's output. The
//! transcript card is labelled with the app focused again at paste time, since
//! that is where the text lands.
//!
//! The formatter consumes [`AppCategory`] — how text should be shaped depends
//! on the kind of destination, not on which editor it is. The transcript card
//! consumes [`FocusTarget::name`], which is what a person wants to read.

mod catalog;

#[cfg(target_os = "windows")]
mod windows;

pub use catalog::AppCategory;

/// An application that had focus: when dictation started, or at paste time.
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
        // Our own window can hold focus: show the product name, not the binary's.
        if stem.eq_ignore_ascii_case(env!("CARGO_PKG_NAME")) {
            return Self {
                name: "NexusVoice".to_string(),
                category: AppCategory::Unknown,
            };
        }
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

/// Whether the foreground window is an elevated process while this app is not,
/// which Windows shields from the input we synthesize.
#[cfg(target_os = "windows")]
#[must_use]
pub fn foreground_outranks_us() -> bool {
    windows::foreground_outranks_us()
}

#[cfg(test)]
#[path = "../../../tests/unit/platform/focus/mod.rs"]
mod tests;
