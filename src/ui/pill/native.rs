//! What GPUI cannot do for the pill: place its window.
//!
//! GPUI can resize a window but not move one, and the pill must sit where the
//! user dragged it, grow upward from there, and never take focus from the app
//! being dictated into. So this talks to the window system directly, by the
//! window's raw handle: X11 via `x11rb`, Windows via Win32. Wayland offers
//! clients no positioning at all; there the pill only resizes and the
//! compositor drags it.

use gpui_kit::{point, px, size, Bounds, Pixels, Point, Size, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Where the pointer is, and whether the primary button is down.
#[derive(Debug, Clone, Copy)]
pub struct Pointer {
    /// Screen position in device pixels.
    pub x: f32,
    pub y: f32,
    pub pressed: bool,
}

/// A drag in progress: the window follows the pointer's travel since it began.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    from: Pointer,
    origin: Point<Pixels>,
}

/// A frame as whole device pixels: x, y, width, height.
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
    /// The frame last asked for. The window system reports it back late, and
    /// GPUI does not track a window moved behind its back, so this is the
    /// only source of the window's position.
    frame: Bounds<Pixels>,
    scale: f32,
}

impl NativeWindow {
    /// Take the window as GPUI opened it, keeping it out of the focus order.
    pub fn new(window: &Window) -> Self {
        let backend = match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
            #[cfg(target_os = "linux")]
            Ok(RawWindowHandle::Xcb(handle)) => x11::Connection::open(handle.window.get())
                .map_or(Backend::Fixed, |conn| Backend::X11(Box::new(conn))),
            #[cfg(windows)]
            Ok(RawWindowHandle::Win32(handle)) => Backend::Win32(win32::Handle::new(handle)),
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

    /// Resize to `new_size`, holding the bottom-centre still so a card grows
    /// upward from the capsule it came from.
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
            Backend::Win32(handle) => handle.pointer(),
            Backend::Fixed => None,
        }
    }

    /// Put the window back above its peers. Windows demotes topmost windows
    /// when a fullscreen app appears; an X11 pop-up is unmanaged, so nothing
    /// restacks it but a window raised after it.
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

    /// A connection of the pill's own: the window id is global, and requests
    /// on it do not disturb GPUI's.
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

    use gpui_kit::{Bounds, Pixels};
    use raw_window_handle::Win32WindowHandle;
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };

    use super::Pointer;

    pub struct Handle(HWND);

    impl Handle {
        /// NOACTIVATE keeps clicks on the pill from stealing focus from the
        /// app being dictated into; TOOLWINDOW keeps it out of the taskbar and
        /// Alt+Tab.
        pub fn new(handle: Win32WindowHandle) -> Self {
            let hwnd = HWND(handle.hwnd.get() as *mut c_void);
            // SAFETY: `hwnd` is the live window GPUI just created.
            unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let wanted = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | wanted);
            }
            Self(hwnd)
        }

        pub fn place(&self, frame: Bounds<Pixels>, scale: f32) {
            let (x, y, width, height) = super::device_frame(frame, scale);
            // SAFETY: `self.0` is the window this handle was made for.
            let placed = unsafe {
                SetWindowPos(
                    self.0,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE,
                )
            };
            if let Err(e) = placed {
                log::debug!("pill: SetWindowPos failed: {e}");
            }
        }

        pub fn raise(&self) {
            // SAFETY: as above. Toggling topmost is what re-promotes a window
            // that already has the flag.
            let raised = unsafe {
                SetWindowPos(
                    self.0,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
            };
            if let Err(e) = raised {
                log::debug!("pill: could not re-raise the window: {e}");
            }
        }

        pub fn pointer(&self) -> Option<Pointer> {
            let mut at = POINT::default();
            // SAFETY: `at` is a valid out-pointer for the call.
            unsafe { GetCursorPos(&mut at) }.ok()?;
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
}
