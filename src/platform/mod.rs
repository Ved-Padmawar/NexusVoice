//! Operating-system services the app needs outside its own windows: where its
//! files live, logging, the clipboard, a single running instance, the tray,
//! global hotkeys and self-update.

pub mod clipboard;
pub mod hotkeys;
pub mod logging;
pub mod paths;
pub mod single_instance;
pub mod tray;
pub mod updater;
