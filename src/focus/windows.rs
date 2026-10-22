//! Foreground window → executable, via Win32.
//!
//! `PROCESS_QUERY_LIMITED_INFORMATION` is the minimum right
//! `QueryFullProcessImageNameW` accepts and, unlike `PROCESS_QUERY_INFORMATION`,
//! is granted for elevated and protected processes — so an admin terminal is
//! identified rather than silently failing.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Closes its handle on drop, so every early return stays leak-free.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// `"Code"` for `C:\...\Code.exe`. `None` when no window has focus or the
/// query is refused.
pub fn foreground_executable_stem() -> Option<String> {
    let path = foreground_executable_path()?;
    std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
}

fn foreground_executable_path() -> Option<String> {
    // SAFETY: each call is checked before its result is used, and the process
    // handle is owned by `OwnedHandle` for the rest of the function.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut pid = 0_u32;
        // Returns the thread id — 0 only for an invalid window. The pid we want
        // is written out only when the call succeeds.
        if GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) == 0 || pid == 0 {
            return None;
        }

        let handle = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);

        let mut buffer = [0_u16; MAX_PATH as usize];
        let mut len = MAX_PATH;
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
        .ok()?;

        // `len` is updated to the character count written, excluding the NUL.
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}
