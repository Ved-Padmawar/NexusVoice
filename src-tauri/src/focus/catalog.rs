//! Mapping executables to a display name and a destination category.
//!
//! A hand-curated table by design. An uncatalogued app still works: it reports
//! its executable stem and [`AppCategory::Unknown`], and the formatter falls
//! back to default behaviour. Adding an app is one line.

use serde::Serialize;

/// What kind of destination the text is going into.
///
/// The formatter keys off the category, not the app, because what changes is
/// the shape of the writing — and that is shared across every app of a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
// Only `Unknown` is constructed off Windows, where nothing detects the app.
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
pub enum AppCategory {
    /// Short, informal, light punctuation.
    Chat,
    /// Greeting/sign-off structure, full sentences.
    Email,
    /// Identifiers and paths must survive verbatim.
    Code,
    /// Long-form prose; headings and lists welcome.
    Notes,
    /// The real destination is a web app we cannot see.
    Browser,
    /// Commands, not prose.
    Terminal,
    /// Uncatalogued — no app-specific shaping.
    Unknown,
}

impl AppCategory {
    /// A noun phrase naming the destination, for the prompt. `None` for
    /// [`Self::Unknown`], which carries no usable signal.
    pub(crate) const fn describe(self) -> Option<&'static str> {
        match self {
            Self::Chat => Some("a chat message"),
            Self::Email => Some("an email"),
            Self::Code => Some("a code editor"),
            Self::Notes => Some("a notes or document editor"),
            Self::Browser => Some("a web browser"),
            Self::Terminal => Some("a terminal"),
            Self::Unknown => None,
        }
    }
}

/// Executable stem (lowercased, no `.exe`), display name, category. A flat
/// tuple rather than a struct so rustfmt keeps one app per line.
#[cfg(any(target_os = "windows", test))]
const CATALOG: &[(&str, &str, AppCategory)] = &[
    // Chat
    ("slack", "Slack", AppCategory::Chat),
    ("discord", "Discord", AppCategory::Chat),
    ("teams", "Microsoft Teams", AppCategory::Chat),
    ("ms-teams", "Microsoft Teams", AppCategory::Chat),
    ("whatsapp", "WhatsApp", AppCategory::Chat),
    ("telegram", "Telegram", AppCategory::Chat),
    // Email
    ("outlook", "Outlook", AppCategory::Email),
    ("thunderbird", "Thunderbird", AppCategory::Email),
    // Code
    ("code", "VS Code", AppCategory::Code),
    ("code - insiders", "VS Code Insiders", AppCategory::Code),
    ("cursor", "Cursor", AppCategory::Code),
    ("windsurf", "Windsurf", AppCategory::Code),
    ("zed", "Zed", AppCategory::Code),
    ("devenv", "Visual Studio", AppCategory::Code),
    ("idea64", "IntelliJ IDEA", AppCategory::Code),
    ("pycharm64", "PyCharm", AppCategory::Code),
    ("webstorm64", "WebStorm", AppCategory::Code),
    ("rustrover64", "RustRover", AppCategory::Code),
    ("sublime_text", "Sublime Text", AppCategory::Code),
    ("notepad++", "Notepad++", AppCategory::Code),
    // Notes and documents
    ("notion", "Notion", AppCategory::Notes),
    ("obsidian", "Obsidian", AppCategory::Notes),
    ("winword", "Word", AppCategory::Notes),
    ("onenote", "OneNote", AppCategory::Notes),
    ("notepad", "Notepad", AppCategory::Notes),
    ("typora", "Typora", AppCategory::Notes),
    // Browsers
    ("chrome", "Chrome", AppCategory::Browser),
    ("msedge", "Edge", AppCategory::Browser),
    ("firefox", "Firefox", AppCategory::Browser),
    ("brave", "Brave", AppCategory::Browser),
    ("arc", "Arc", AppCategory::Browser),
    ("opera", "Opera", AppCategory::Browser),
    // Terminals
    ("windowsterminal", "Windows Terminal", AppCategory::Terminal),
    ("powershell", "PowerShell", AppCategory::Terminal),
    ("pwsh", "PowerShell", AppCategory::Terminal),
    ("cmd", "Command Prompt", AppCategory::Terminal),
    ("alacritty", "Alacritty", AppCategory::Terminal),
    ("wezterm-gui", "WezTerm", AppCategory::Terminal),
];

/// Case-insensitive: Win32 reports whatever casing is on disk.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn lookup(stem: &str) -> Option<(&'static str, AppCategory)> {
    let needle = stem.to_ascii_lowercase();
    CATALOG
        .iter()
        .find(|(key, _, _)| *key == needle)
        .map(|(_, name, category)| (*name, *category))
}
