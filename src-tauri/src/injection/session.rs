//! Linux session and desktop-environment detection.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    Wayland,
    X11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub server: Server,
    pub desktop: Desktop,
}

impl Session {
    #[must_use]
    pub fn is_wayland(self) -> bool {
        self.server == Server::Wayland
    }

    #[must_use]
    pub fn describe(self) -> String {
        let server = match self.server {
            Server::Wayland => "Wayland",
            Server::X11 => "X11",
        };
        let desktop = match self.desktop {
            Desktop::Gnome => "GNOME",
            Desktop::Kde => "KDE",
            Desktop::Other => "other",
        };
        format!("{server} ({desktop})")
    }
}

/// Detected once — a session cannot change without restarting the app.
pub fn detect() -> Session {
    static SESSION: OnceLock<Session> = OnceLock::new();
    *SESSION.get_or_init(|| Session {
        server: detect_server(),
        desktop: detect_desktop(),
    })
}

fn detect_server() -> Server {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"))
    {
        Server::Wayland
    } else {
        Server::X11
    }
}

fn detect_desktop() -> Desktop {
    let current = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_uppercase();

    if current.contains("KDE") || std::env::var_os("KDE_SESSION_VERSION").is_some() {
        Desktop::Kde
    } else if current.contains("GNOME") {
        Desktop::Gnome
    } else {
        Desktop::Other
    }
}
