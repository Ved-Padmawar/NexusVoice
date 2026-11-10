//! Foreground window → executable, via Win32.
//!
//! `PROCESS_QUERY_LIMITED_INFORMATION` is the minimum right
//! `QueryFullProcessImageNameW` accepts and, unlike `PROCESS_QUERY_INFORMATION`,
//! is granted for elevated and protected processes — so an admin terminal is
//! identified rather than silently failing.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE, MAX_PATH};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

#[allow(clippy::cast_possible_truncation)] // a four-byte struct
const TOKEN_ELEVATION_SIZE: u32 = size_of::<TOKEN_ELEVATION>() as u32;

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

fn foreground_pid() -> Option<u32> {
    // SAFETY: each call is checked before its result is used.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0_u32;
        // Returns the thread id — 0 only for an invalid window. The pid we want
        // is written out only when the call succeeds.
        (GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) != 0 && pid != 0).then_some(pid)
    }
}

fn foreground_executable_path() -> Option<String> {
    let pid = foreground_pid()?;
    // SAFETY: the process handle is owned by `OwnedHandle` for the rest of the
    // function, and each call is checked before its result is used.
    unsafe {
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

/// Whether the foreground window belongs to an elevated process while this one
/// is not. Windows (UIPI) then silently drops the input we synthesize into it.
///
/// A token this process may not even query belongs to a more privileged one,
/// so a refused query counts as elevated.
pub fn foreground_outranks_us() -> bool {
    let Some(pid) = foreground_pid() else {
        return false;
    };
    // SAFETY: the pseudo-handle needs no closing.
    if is_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false) {
        return false;
    }
    // SAFETY: the handle is owned by `OwnedHandle` until the end of the function.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
    };
    let process = OwnedHandle(process);
    is_elevated(process.0)
        .unwrap_or_else(|e| e.code() == windows::core::HRESULT::from_win32(ERROR_ACCESS_DENIED.0))
}

fn is_elevated(process: HANDLE) -> windows::core::Result<bool> {
    let mut token = HANDLE::default();
    // SAFETY: on success the token is closed by `OwnedHandle`; the elevation
    // struct is written only up to its size.
    unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &raw mut token)?;
        let token = OwnedHandle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0_u32;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&raw mut elevation).cast()),
            TOKEN_ELEVATION_SIZE,
            &raw mut returned,
        )?;
        Ok(elevation.TokenIsElevated != 0)
    }
}
