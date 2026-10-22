//! The app's own icons: Phosphor, under `assets/icons/app/`. The kit's built-in
//! icon paths are served Phosphor files too, so every icon is from one set.

use gpui_kit::component::IconNamed;
use gpui_kit::SharedString;

macro_rules! app_icons {
    ($($name:ident => $file:literal,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AppIcon {
            $($name,)*
        }

        impl IconNamed for AppIcon {
            fn path(self) -> SharedString {
                match self {
                    $(Self::$name => concat!("icons/app/", $file, ".svg"),)*
                }
                .into()
            }
        }
    };
}

app_icons! {
    Activity => "activity",
    AlertCircle => "alert-circle",
    AlertTriangle => "alert-triangle",
    ArrowUp => "arrow-up",
    ArrowUpCircle => "arrow-up-circle",
    AudioLines => "audio-lines",
    BookOpen => "book-open",
    Captions => "captions",
    Check => "check",
    CheckCircle => "check-circle-2",
    ChevronDown => "chevron-down",
    Copy => "copy",
    Cpu => "cpu",
    Database => "database",
    Download => "download",
    FolderOpen => "folder-open",
    Globe => "globe",
    HardDrive => "hard-drive",
    Hash => "hash",
    Info => "info",
    Keyboard => "keyboard",
    Languages => "languages",
    LayoutDashboard => "layout-dashboard",
    Loader => "loader-2",
    MemoryStick => "memory-stick",
    Mic => "mic",
    Monitor => "monitor",
    Moon => "moon",
    Palette => "palette",
    Pencil => "pencil",
    Plug => "plug",
    Plus => "plus",
    Radio => "radio",
    RefreshCw => "refresh-cw",
    RotateCcw => "rotate-ccw",
    Save => "save",
    Search => "search",
    Settings => "settings",
    Settings2 => "settings-2",
    Shield => "shield",
    SlidersHorizontal => "sliders-horizontal",
    Sparkles => "sparkles",
    Sun => "sun",
    Timer => "timer",
    Trash => "trash-2",
    X => "x",
    Zap => "zap",
}
