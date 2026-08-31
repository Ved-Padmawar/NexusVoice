//! Injecting a finished transcript into whatever window has focus.
//!
//! The two platforms take deliberately different routes. Windows and macOS put
//! the text on the clipboard and send the paste chord, which is instant no
//! matter how long the transcript is. Linux types the text directly, because
//! Wayland does not let an application synthesize a keystroke into another
//! application's window at all.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "linux")]
pub mod session;

#[cfg(not(target_os = "linux"))]
mod clipboard;

#[cfg(not(target_os = "linux"))]
pub use clipboard::type_text;
#[cfg(target_os = "linux")]
pub use linux::type_text;
