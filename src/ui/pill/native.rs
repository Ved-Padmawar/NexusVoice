//! Places the pill's window through the window system (X11, Win32), which
//! GPUI cannot do. Wayland allows no positioning, so there it only resizes.

use gpui_kit::{point, px, App, Bounds, Pixels, Point, Size, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Debug, Clone, Copy)]
pub struct Pointer {
    /// Screen position in device pixels.
    pub x: f32,
    pub y: f32,
    pub pressed: bool,
}

/// The cursor over the pill's body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grab {
    None,
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy)]
pub struct Drag {
    from: Pointer,
    origin: Point<Pixels>,
}

#[cfg(any(target_os = "linux", windows))]
#[allow(clippy::cast_possible_truncation)] // screen coordinates are nowhere near i32's range
fn device_frame(frame: Bounds<Pixels>, scale: f32) -> (i32, i32, i32, i32) {
    let device = |length: Pixels| (f32::from(length) * scale).round() as i32;
    (
        device(frame.origin.x),
        device(frame.origin.y),
        device(frame.size.width),
        device(frame.size.height),
    )
}

enum Backend {
    #[cfg(target_os = "linux")]
    X11(Box<x11::Connection>),
    #[cfg(windows)]
    Win32(win32::Handle),
    /// Resize only; the compositor moves the window.
    Fixed,
}

pub struct NativeWindow {
    backend: Backend,
    /// The frame last asked for: GPUI does not track a window moved behind its back.
    frame: Bounds<Pixels>,
    scale: f32,
}

impl NativeWindow {
    pub fn new(window: &Window, cx: &App) -> Self {
        let backend = match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
            #[cfg(target_os = "linux")]
            Ok(RawWindowHandle::Xcb(handle)) => x11::Connection::open(handle.window.get())
                .map_or(Backend::Fixed, |conn| Backend::X11(Box::new(conn))),
            #[cfg(windows)]
            Ok(RawWindowHandle::Win32(handle)) => {
                Backend::Win32(win32::Handle::new(handle, cx.foreground_executor().clone()))
            }
            _ => Backend::Fixed,
        };
        let native = Self {
            backend,
            frame: window.bounds(),
            scale: window.scale_factor(),
        };
        native.keep_on_top();
        native
    }

    /// Only Windows needs this; elsewhere GPUI draws the grab cursors itself.
    pub fn set_grab(&self, grab: Grab) {
        #[cfg(windows)]
        if let Backend::Win32(handle) = &self.backend {
            handle.set_grab(grab);
        }
        let _ = grab;
    }

    pub fn is_movable(&self) -> bool {
        !matches!(self.backend, Backend::Fixed)
    }

    fn place(&mut self, window: &mut Window, frame: Bounds<Pixels>) {
        self.scale = window.scale_factor();
        let resized = frame.size != self.frame.size;
        self.frame = frame;
        match &self.backend {
            #[cfg(target_os = "linux")]
            Backend::X11(connection) => connection.place(frame, self.scale),
            #[cfg(windows)]
            Backend::Win32(handle) => handle.place(frame, self.scale),
            Backend::Fixed => {
                if resized {
                    window.resize(frame.size);
                }
            }
        }
    }

    /// Resize around the bottom-centre, so a card grows upward from the capsule.
    pub fn resize(&mut self, window: &mut Window, new_size: Size<Pixels>) {
        let current = self.frame;
        let anchor = point(
            current.origin.x + current.size.width / 2.0,
            current.origin.y + current.size.height,
        );
        let origin = point(anchor.x - new_size.width / 2.0, anchor.y - new_size.height);
        self.place(window, Bounds::new(origin, new_size));
    }

    pub fn begin_drag(&self) -> Option<Drag> {
        Some(Drag {
            from: self.pointer()?,
            origin: self.frame.origin,
        })
    }

    /// Follow the pointer; `false` once the button is released.
    pub fn drag(&mut self, drag: Drag, window: &mut Window) -> bool {
        let Some(pointer) = self.pointer().filter(|pointer| pointer.pressed) else {
            return false;
        };
        let travel = point(
            px((pointer.x - drag.from.x) / self.scale),
            px((pointer.y - drag.from.y) / self.scale),
        );
        let size = self.frame.size;
        self.place(window, Bounds::new(drag.origin + travel, size));
        true
    }

    fn pointer(&self) -> Option<Pointer> {
        match &self.backend {
            #[cfg(target_os = "linux")]
            Backend::X11(connection) => connection.pointer(),
            #[cfg(windows)]
            Backend::Win32(_) => win32::Handle::pointer(),
            Backend::Fixed => None,
        }
    }

    /// Re-raise: Windows demotes topmost windows under fullscreen apps, and an
    /// unmanaged X11 pop-up is only restacked by raising it.
    pub fn keep_on_top(&self) {
        match &self.backend {
            #[cfg(target_os = "linux")]
            Backend::X11(connection) => connection.raise(),
            #[cfg(windows)]
            Backend::Win32(handle) => handle.raise(),
            Backend::Fixed => {}
        }
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use gpui_kit::{Bounds, Pixels};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _, KeyButMask, StackMode};
    use x11rb::rust_connection::RustConnection;

    use super::Pointer;

    /// Its own connection, so requests on it don't disturb GPUI's.
    pub struct Connection {
        conn: RustConnection,
        window: u32,
        root: u32,
    }

    impl Connection {
        pub fn open(window: u32) -> Option<Self> {
            let (conn, screen) = RustConnection::connect(None).ok()?;
            let root = conn.setup().roots.get(screen)?.root;
            Some(Self { conn, window, root })
        }

        pub fn place(&self, frame: Bounds<Pixels>, scale: f32) {
            let (x, y, width, height) = super::device_frame(frame, scale);
            let aux = ConfigureWindowAux::new()
                .x(x)
                .y(y)
                .width(width.max(1).unsigned_abs())
                .height(height.max(1).unsigned_abs());
            self.send(&aux);
        }

        pub fn raise(&self) {
            self.send(&ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        }

        fn send(&self, aux: &ConfigureWindowAux) {
            if let Err(e) = self.conn.configure_window(self.window, aux) {
                log::debug!("pill: configure_window failed: {e}");
            }
            let _ = self.conn.flush();
        }

        pub fn pointer(&self) -> Option<Pointer> {
            let reply = self.conn.query_pointer(self.root).ok()?.reply().ok()?;
            Some(Pointer {
                x: f32::from(reply.root_x),
                y: f32::from(reply.root_y),
                pressed: reply.mask.contains(KeyButMask::BUTTON1),
            })
        }
    }
}

#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::OnceLock;

    use gpui_kit::{Bounds, ForegroundExecutor, Pixels};
    use raw_window_handle::Win32WindowHandle;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        DWMWINDOWATTRIBUTE,
    };
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, GetCursor, GetCursorPos, GetWindowLongPtrW, LoadCursorW, SetCursor,
        SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HCURSOR, HWND_TOPMOST, ICONINFO,
        IDC_ARROW, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SWP_SHOWWINDOW, WM_SETCURSOR, WM_USER, WS_CAPTION, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    };

    use super::{Grab, Pointer};

    /// GPUI's own "cursor style changed" message.
    const WM_GPUI_CURSOR_STYLE_CHANGED: u32 = WM_USER + 1;

    pub struct Handle {
        hwnd: HWND,
        executor: ForegroundExecutor,
    }

    /// Set a DWM attribute; a failure (pre-Windows 11) leaves the default.
    fn set_dwm<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) {
        // SAFETY: `value` is a live `T` and the size passed is its own.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute,
                std::ptr::from_ref(value).cast(),
                u32::try_from(size_of::<T>()).unwrap_or(0),
            )
        };
    }

    impl Handle {
        /// NOACTIVATE: clicks don't steal focus. TOOLWINDOW: no taskbar or Alt+Tab.
        /// Frame stripped, as Windows 11 draws a border and shadow round GPUI pop-ups.
        pub fn new(handle: Win32WindowHandle, executor: ForegroundExecutor) -> Self {
            let hwnd = HWND(handle.hwnd.get() as *mut c_void);
            // SAFETY: `hwnd` is the live window GPUI just created.
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                // Sign-extended, as Windows widens a LONG style to LONG_PTR.
                let wanted = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0)
                    .cast_signed() as isize;
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | wanted);
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                let frame = (WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0).cast_signed() as isize;
                SetWindowLongPtrW(
                    hwnd,
                    GWL_STYLE,
                    (style & !frame) | WS_POPUP.0.cast_signed() as isize,
                );
                let _ = SetWindowSubclass(hwnd, Some(subclass), 1, 0);
            }
            set_dwm(hwnd, DWMWA_NCRENDERING_POLICY, &DWMNCRP_DISABLED);
            set_dwm(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_DONOTROUND);
            set_dwm(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE);
            let this = Self { hwnd, executor };
            this.set_pos(
                None,
                (0, 0, 0, 0),
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            this
        }

        /// `SetWindowPos` after the current update: it sends window messages
        /// synchronously, which GPUI cannot take while the app is borrowed.
        fn set_pos(
            &self,
            after: Option<HWND>,
            (x, y, width, height): (i32, i32, i32, i32),
            flags: SET_WINDOW_POS_FLAGS,
        ) {
            let hwnd = self.hwnd;
            self.executor
                .spawn(async move {
                    // SAFETY: a stale handle only makes the call fail.
                    if let Err(e) = unsafe { SetWindowPos(hwnd, after, x, y, width, height, flags) }
                    {
                        log::debug!("pill: SetWindowPos failed: {e}");
                    }
                })
                .detach();
        }

        pub fn place(&self, frame: Bounds<Pixels>, scale: f32) {
            let frame = super::device_frame(frame, scale);
            self.set_pos(Some(HWND_TOPMOST), frame, SWP_NOACTIVATE);
        }

        /// Re-setting topmost re-promotes the window.
        pub fn raise(&self) {
            self.set_pos(
                Some(HWND_TOPMOST),
                (0, 0, 0, 0),
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }

        pub fn set_grab(&self, grab: Grab) {
            GRAB.store(grab as u8, Ordering::Relaxed);
            apply_grab(self.hwnd);
        }

        pub fn pointer() -> Option<Pointer> {
            let mut at = POINT::default();
            // SAFETY: `at` is a valid out-pointer for the call.
            unsafe { GetCursorPos(&raw mut at) }.ok()?;
            // SAFETY: a plain query of the key state.
            let pressed = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } < 0;
            #[allow(clippy::cast_precision_loss)]
            Some(Pointer {
                x: at.x as f32,
                y: at.y as f32,
                pressed,
            })
        }
    }

    /// Windows has no open or closed hand, so GPUI shows an arrow for them;
    /// the pill swaps in its own right after GPUI sets the cursor.
    static GRAB: AtomicU8 = AtomicU8::new(Grab::None as u8);

    unsafe extern "system" fn subclass(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _: usize,
        _: usize,
    ) -> LRESULT {
        // SAFETY: forwards the message as received.
        let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        if msg == WM_SETCURSOR || msg == WM_GPUI_CURSOR_STYLE_CHANGED {
            apply_grab(hwnd);
        }
        result
    }

    /// Show the grab cursor in place of GPUI's arrow, or put the arrow back.
    /// Any other cursor, like a button's pointer, stays.
    fn apply_grab(hwnd: HWND) {
        let [open, closed] = *hand_cursors(hwnd);
        // SAFETY: plain cursor queries and sets on the UI thread.
        unsafe {
            let Ok(arrow) = LoadCursorW(None, IDC_ARROW) else {
                return;
            };
            let arrow = arrow.0 as isize;
            let current = GetCursor().0 as isize;
            if ![arrow, open, closed].contains(&current) {
                return;
            }
            let wanted = match GRAB.load(Ordering::Relaxed) {
                1 => open,
                2 => closed,
                _ => arrow,
            };
            if wanted != 0 && wanted != current {
                SetCursor(Some(HCURSOR(wanted as *mut c_void)));
            }
        }
    }

    /// The open and closed hand, sized for the window's DPI on first use.
    fn hand_cursors(hwnd: HWND) -> &'static [isize; 2] {
        static CURSORS: OnceLock<[isize; 2]> = OnceLock::new();
        CURSORS.get_or_init(|| {
            // SAFETY: a plain query.
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let (open, closed): (&[u8], &[u8]) = match dpi {
                0..=119 => (
                    include_bytes!("../../../assets/cursors/hand-32.png"),
                    include_bytes!("../../../assets/cursors/hand-grabbing-32.png"),
                ),
                120..=167 => (
                    include_bytes!("../../../assets/cursors/hand-48.png"),
                    include_bytes!("../../../assets/cursors/hand-grabbing-48.png"),
                ),
                _ => (
                    include_bytes!("../../../assets/cursors/hand-64.png"),
                    include_bytes!("../../../assets/cursors/hand-grabbing-64.png"),
                ),
            };
            [cursor_from_png(open), cursor_from_png(closed)]
        })
    }

    /// A cursor from an RGBA PNG, its hotspot in the middle; 0 on failure.
    fn cursor_from_png(png: &[u8]) -> isize {
        let Ok(image) = image::load_from_memory(png) else {
            return 0;
        };
        let image = image.to_rgba8();
        let (Ok(width), Ok(height)) = (i32::try_from(image.width()), i32::try_from(image.height()))
        else {
            return 0;
        };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).unwrap_or(0),
                biWidth: width,
                // Negative: rows run top to bottom, as in the PNG.
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        // SAFETY: `info` describes a 32bpp DIB of `image`'s size, so `bits`
        // holds exactly its pixels. The cursor copies both bitmaps, which are
        // then deleted.
        unsafe {
            let Ok(color) = CreateDIBSection(
                None,
                &raw const info,
                DIB_RGB_COLORS,
                &raw mut bits,
                None,
                0,
            ) else {
                return 0;
            };
            let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), image.as_raw().len());
            for (dst, src) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(image.pixels()) {
                let [r, g, b, a] = src.0;
                dst.copy_from_slice(&[b, g, r, a]);
            }
            let mask = CreateBitmap(width, height, 1, 1, None);
            let icon = ICONINFO {
                fIcon: false.into(),
                xHotspot: image.width() / 2,
                yHotspot: image.height() / 2,
                hbmMask: mask,
                hbmColor: color,
            };
            let cursor = CreateIconIndirect(&raw const icon);
            let _ = DeleteObject(color.into());
            let _ = DeleteObject(mask.into());
            cursor.map_or(0, |c| c.0 as isize)
        }
    }
}
