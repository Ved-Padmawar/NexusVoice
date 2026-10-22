//! Operating-system services outside the app's windows.

#[cfg(not(target_os = "windows"))] // Windows pastes through its own clipboard code
pub mod clipboard;
pub mod hotkeys;
pub mod logging;
pub mod paths;
pub mod single_instance;
pub mod tray;
pub mod updater;
