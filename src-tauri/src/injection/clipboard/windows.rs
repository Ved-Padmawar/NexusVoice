//! Windows paste that gives the user's clipboard back.
//!
//! The previous clipboard is snapshotted, then the transcript is published as
//! a delayed-render promise (`SetClipboardData(CF_UNICODETEXT, NULL)`) owned
//! by a hidden message-only window. Windows sends that window
//! `WM_RENDERFORMAT` the moment a consumer reads the text — proof the paste
//! landed — and the snapshot is restored once reads go quiet. If the user
//! copies something in the meantime, their copy wins and nothing is restored.
//!
//! The promise carries opt-out markers that keep the transcript out of Win+V
//! history and the cloud clipboard, so history never reads it ahead of the
//! target.
//!
//! Clipboard ownership and delayed rendering are tied to the owning thread's
//! message pump, so each paste runs on its own thread, and the chord is sent
//! from that thread's timer.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard, Once};
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GlobalFree, SetLastError, ERROR_SUCCESS, HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT,
    WPARAM,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardFormatNameW, GetClipboardOwner, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::{
    CF_BITMAP, CF_DSPBITMAP, CF_DSPENHMETAFILE, CF_DSPMETAFILEPICT, CF_DSPTEXT, CF_ENHMETAFILE,
    CF_METAFILEPICT, CF_OWNERDISPLAY, CF_PALETTE, CF_UNICODETEXT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostQuitMessage, RegisterClassW, SetTimer, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_DESTROYCLIPBOARD, WM_RENDERALLFORMATS, WM_RENDERFORMAT, WM_TIMER, WNDCLASSW,
};

use super::settle::{Step, Timeline};

const CLASS_NAME: PCWSTR = w!("NexusVoicePaste");
const TIMER_ID: usize = 1;
const TICK_MS: u32 = 25;
/// Formats larger than this are not snapshotted.
const MAX_FORMAT_BYTES: usize = 64 * 1024 * 1024;
/// Markers that keep the transcript out of clipboard history and sync.
const PRIVACY_FORMATS: [(PCWSTR, u32); 3] = [
    (w!("ExcludeClipboardContentFromMonitorProcessing"), 1),
    (w!("CanIncludeInClipboardHistory"), 0),
    (w!("CanUploadToCloudClipboard"), 0),
];
/// Another app can hold the clipboard open for a moment; retry this long.
const OPEN_ATTEMPTS: u32 = 5;
const OPEN_RETRY: Duration = Duration::from_millis(10);

/// Held for a paste's whole life. A newer paste waits for the previous one to
/// settle, so it snapshots the user's clipboard rather than a transcript, and
/// never restores that clipboard before the previous target has read.
static TURN: Mutex<()> = Mutex::new(());

thread_local! {
    /// The paste this thread runs. Window procedures borrow it briefly and
    /// never across a clipboard call, which can re-enter them.
    static PASTE: RefCell<Option<Paste>> = const { RefCell::new(None) };
}

struct Paste {
    /// NUL-terminated UTF-16.
    text: Vec<u16>,
    snapshot: Vec<SavedFormat>,
    /// Clipboard sequence number right after publishing.
    sequence: u32,
    timeline: Timeline,
    settled: bool,
    /// The clipboard still holds the unrendered promise: no consumer has read
    /// it, and settling has not replaced it.
    promised: bool,
}

struct SavedFormat {
    format: u32,
    data: Vec<u8>,
}

/// Publish `text` and paste it; returns once the clipboard holds the promise.
///
/// # Errors
/// Returns an error when the clipboard cannot be snapshotted or published —
/// the clipboard is then left as it was.
pub async fn paste(text: &str) -> Result<(), String> {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let (ready, published) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("paste".into())
        .spawn(move || run(text, ready))
        .map_err(|e| format!("failed to start paste thread: {e}"))?;
    published
        .await
        .map_err(|_| "paste thread exited before publishing".to_string())?
}

fn run(text: Vec<u16>, ready: tokio::sync::oneshot::Sender<Result<(), String>>) {
    let _turn = lock(&TURN);
    let hwnd = match create_window() {
        Ok(hwnd) => hwnd,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    match begin(hwnd, text) {
        Ok(paste) => {
            PASTE.with(|cell| *cell.borrow_mut() = Some(paste));
            // SAFETY: `hwnd` is this thread's live window.
            unsafe { SetTimer(Some(hwnd), TIMER_ID, TICK_MS, None) };
            let _ = ready.send(Ok(()));
            pump_messages();
            // SAFETY: as above; the timer and window die with this paste.
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
            }
        }
        Err(e) => {
            let _ = ready.send(Err(e));
        }
    }
    // SAFETY: as above. Destroying the window may send WM_RENDERALLFORMATS,
    // which still reads `PASTE`, so it is cleared only afterwards.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    PASTE.with(|cell| cell.borrow_mut().take());
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Snapshot the clipboard, then publish the transcript in its place.
fn begin(hwnd: HWND, text: Vec<u16>) -> Result<Paste, String> {
    let snapshot = snapshot(hwnd)?;
    let sequence = match publish(hwnd) {
        Ok(sequence) => sequence,
        Err(e) => {
            // Publishing may have emptied the clipboard before failing.
            restore(hwnd, &snapshot);
            return Err(e);
        }
    };
    Ok(Paste {
        text,
        snapshot,
        sequence,
        timeline: Timeline::new(Instant::now()),
        settled: false,
        promised: true,
    })
}

fn create_window() -> Result<HWND, String> {
    static REGISTER: Once = Once::new();
    // SAFETY: a null module name yields this executable's handle.
    let instance: HINSTANCE = unsafe { GetModuleHandleW(PCWSTR::null()) }
        .map_err(|e| format!("GetModuleHandleW failed: {e}"))?
        .into();
    REGISTER.call_once(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        // SAFETY: `class` outlives the call and names a static string.
        unsafe { RegisterClassW(&raw const class) };
    });
    // SAFETY: a message-only window of the class registered above.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            CLASS_NAME,
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW failed: {e}"))
}

fn pump_messages() {
    let mut msg = MSG::default();
    // SAFETY: a standard message loop on this thread's queue; it ends when
    // the window procedure posts WM_QUIT.
    unsafe {
        while GetMessageW(&raw mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&raw const msg);
        }
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT if wparam.0 == text_format() as usize => {
            // The system opened the clipboard for us; render into it.
            if let Some(text) = with_paste(|paste| {
                paste.timeline.record_read(Instant::now());
                paste.promised = false;
                paste.text.clone()
            }) {
                set_text(&text);
            }
        }
        WM_RENDERALLFORMATS => {
            // Sent as the window goes away while it owns the clipboard. Only a
            // promise nothing read and settling could not replace still needs
            // the text; anything else would overwrite the restored clipboard.
            if let Some(text) =
                with_paste(|paste| paste.promised.then(|| paste.text.clone())).flatten()
            {
                if let Some(_open) = ClipboardGuard::open(hwnd) {
                    // SAFETY: the clipboard is open on this thread.
                    if unsafe { GetClipboardOwner() }.is_ok_and(|owner| owner == hwnd) {
                        set_text(&text);
                    }
                }
            }
        }
        WM_DESTROYCLIPBOARD => {
            with_paste(|paste| paste.timeline.lose_ownership());
        }
        WM_TIMER => tick(hwnd),
        // SAFETY: default handling for everything else.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT(0)
}

/// Run `f` on this thread's paste. `None` when there is none, or when it is
/// already borrowed further up the stack (a clipboard call re-entered).
fn with_paste<R>(f: impl FnOnce(&mut Paste) -> R) -> Option<R> {
    PASTE.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(f))
}

fn tick(hwnd: HWND) {
    let now = Instant::now();
    let step = with_paste(|paste| {
        if paste.settled {
            Step::Wait
        } else {
            paste.timeline.next_step(now)
        }
    });
    match step {
        Some(Step::SendChord) => {
            // Marked before sending: the target may read while keys go out.
            let sent_at = Instant::now();
            let ok = super::send_paste_chord();
            with_paste(|paste| paste.timeline.chord_sent(sent_at, ok));
            if !ok {
                log::warn!("paste chord could not be sent");
            }
        }
        Some(Step::Settle) => settle(hwnd),
        Some(Step::Wait) | None => {}
    }
}

/// Restore the snapshot if the clipboard is still ours, then end the paste.
fn settle(hwnd: HWND) {
    let Some((snapshot, sequence, ownership_lost)) = with_paste(|paste| {
        if paste.settled {
            return None;
        }
        paste.settled = true;
        Some((
            std::mem::take(&mut paste.snapshot),
            paste.sequence,
            paste.timeline.ownership_lost(),
        ))
    })
    .flatten() else {
        return;
    };
    let outcome = if ownership_lost {
        Restore::UserCopied
    } else {
        restore_if_unchanged(hwnd, &snapshot, sequence)
    };
    match outcome {
        Restore::Restored => {}
        Restore::UserCopied => log::info!("clipboard changed after paste; leaving the user's copy"),
        Restore::Busy => log::warn!("could not reopen the clipboard to restore it"),
    }
    if outcome != Restore::Busy {
        with_paste(|paste| paste.promised = false);
    }
    // SAFETY: ends this thread's message loop.
    unsafe { PostQuitMessage(0) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    Restored,
    UserCopied,
    /// Another app held the clipboard; the promise is still on it.
    Busy,
}

/// Put `snapshot` back unless the clipboard changed since `sequence`. Checked
/// with the clipboard held, so a copy cannot land between check and write.
fn restore_if_unchanged(hwnd: HWND, snapshot: &[SavedFormat], sequence: u32) -> Restore {
    let Some(_open) = ClipboardGuard::open(hwnd) else {
        return Restore::Busy;
    };
    // SAFETY: a plain read of a global counter.
    if unsafe { GetClipboardSequenceNumber() } != sequence {
        return Restore::UserCopied;
    }
    write_snapshot(snapshot);
    Restore::Restored
}

/// Every format of the current clipboard that lives in global memory. Bitmaps
/// are covered by their `CF_DIB` form, which Windows converts back on demand.
fn snapshot(hwnd: HWND) -> Result<Vec<SavedFormat>, String> {
    let _open = ClipboardGuard::open(hwnd).ok_or("clipboard is busy")?;
    let mut formats = Vec::new();
    let mut format = 0;
    loop {
        // SAFETY: the clipboard is open on this thread.
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        if !is_global_memory(format) || is_ole_bookkeeping(format) {
            continue;
        }
        // SAFETY: as above; the handle stays valid while the clipboard is open.
        if let Ok(handle) = unsafe { GetClipboardData(format) } {
            if let Some(data) = read_global(HGLOBAL(handle.0)) {
                formats.push(SavedFormat { format, data });
            }
        }
    }
    Ok(formats)
}

/// Whether a format's handle is plain global memory, safe to copy as bytes.
fn is_global_memory(format: u32) -> bool {
    const PRIVATE: std::ops::RangeInclusive<u32> = 0x0200..=0x03FF;
    let gdi_or_display = [
        CF_BITMAP,
        CF_METAFILEPICT,
        CF_PALETTE,
        CF_ENHMETAFILE,
        CF_OWNERDISPLAY,
        CF_DSPTEXT,
        CF_DSPBITMAP,
        CF_DSPMETAFILEPICT,
        CF_DSPENHMETAFILE,
    ];
    !PRIVATE.contains(&format) && !gdi_or_display.iter().any(|f| u32::from(f.0) == format)
}

/// Formats OLE uses to find the live data object behind a copy. Restored after
/// that object is gone, they send OLE readers chasing it and hang them; OLE
/// rebuilds them from the plain formats on its own.
fn is_ole_bookkeeping(format: u32) -> bool {
    const OLE_FORMATS: [&str; 3] = [
        "DataObject",
        "Ole Private Data",
        "OleClipboardPersistOnFlush",
    ];
    // Only registered formats have names.
    if format < 0xC000 {
        return false;
    }
    let mut name = [0u16; 64];
    // SAFETY: writes at most `name.len()` units into the buffer.
    let len = unsafe { GetClipboardFormatNameW(format, &mut name) };
    let Ok(len) = usize::try_from(len) else {
        return false;
    };
    let name = String::from_utf16_lossy(&name[..len]);
    OLE_FORMATS.contains(&name.as_str())
}

/// Empty the clipboard and publish the privacy markers plus the promise.
/// Returns the clipboard sequence number afterwards.
fn publish(hwnd: HWND) -> Result<u32, String> {
    {
        let _open = ClipboardGuard::open(hwnd).ok_or("clipboard is busy")?;
        // SAFETY: the clipboard is open on this thread.
        unsafe { EmptyClipboard() }.map_err(|e| format!("EmptyClipboard failed: {e}"))?;
        for (name, value) in PRIVACY_FORMATS {
            // SAFETY: `name` is a static NUL-terminated string.
            let format = unsafe { RegisterClipboardFormatW(name) };
            if format != 0 {
                set_data(format, &value.to_le_bytes());
            }
        }
        // SAFETY: as above. A null handle is the promise; success returns
        // that null handle, which the crate reports as an error carrying
        // GetLastError — so clear it first and only a real code is a failure.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            if let Err(e) = SetClipboardData(text_format(), None) {
                if e.code().is_err() {
                    return Err(format!("SetClipboardData failed: {e}"));
                }
            }
        }
    }
    // SAFETY: a plain read of a global counter, taken after closing.
    Ok(unsafe { GetClipboardSequenceNumber() })
}

/// Replace the clipboard with `snapshot`, if it can be opened.
fn restore(hwnd: HWND, snapshot: &[SavedFormat]) {
    if let Some(_open) = ClipboardGuard::open(hwnd) {
        write_snapshot(snapshot);
    }
}

/// Empty the clipboard and write `snapshot` into it. The clipboard must be open.
fn write_snapshot(snapshot: &[SavedFormat]) {
    // SAFETY: the clipboard is open on this thread.
    let _ = unsafe { EmptyClipboard() };
    for saved in snapshot {
        set_data(saved.format, &saved.data);
    }
}

/// Set the transcript as text. The clipboard must be open.
fn set_text(text: &[u16]) {
    let bytes: Vec<u8> = text.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    set_data(text_format(), &bytes);
}

/// Copy `data` into global memory and hand it to the open clipboard.
fn set_data(format: u32, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    // SAFETY: the block is `data.len()` bytes, written only while locked; on
    // success the clipboard owns it, otherwise it is freed here.
    unsafe {
        let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, data.len()) else {
            return;
        };
        let target = GlobalLock(memory).cast::<u8>();
        if target.is_null() {
            let _ = GlobalFree(Some(memory));
            return;
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), target, data.len());
        let _ = GlobalUnlock(memory);
        if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
            let _ = GlobalFree(Some(memory));
        }
    }
}

fn read_global(memory: HGLOBAL) -> Option<Vec<u8>> {
    // SAFETY: `memory` is a clipboard handle, read only while locked and only
    // up to its reported size.
    unsafe {
        let size = GlobalSize(memory);
        if size == 0 || size > MAX_FORMAT_BYTES {
            return None;
        }
        let source = GlobalLock(memory).cast::<u8>();
        if source.is_null() {
            return None;
        }
        let data = std::slice::from_raw_parts(source, size).to_vec();
        let _ = GlobalUnlock(memory);
        Some(data)
    }
}

fn text_format() -> u32 {
    u32::from(CF_UNICODETEXT.0)
}

/// The clipboard, open on this thread until dropped.
struct ClipboardGuard;

impl ClipboardGuard {
    /// Retries briefly: another app may hold the clipboard for a moment.
    fn open(hwnd: HWND) -> Option<Self> {
        for attempt in 0..OPEN_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(OPEN_RETRY);
            }
            // SAFETY: `hwnd` is this thread's live window.
            if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
                return Some(Self);
            }
        }
        None
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: only constructed after a successful open on this thread.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
