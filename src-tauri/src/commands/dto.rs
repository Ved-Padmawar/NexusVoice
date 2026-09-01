//! Serializable response types returned to the frontend, with `From` conversions
//! from the internal database models.

use serde::Serialize;

use crate::database::models::{dictionary::DictionaryEntry, transcript::Transcript};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResponse {
    pub id: i64,
    pub content: String,
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    /// App the transcript was dictated into; `None` when it wasn't determined.
    pub target_app: Option<String>,
    pub created_at: String,
}

impl From<Transcript> for TranscriptResponse {
    fn from(value: Transcript) -> Self {
        Self {
            id: value.id,
            content: value.content,
            word_count: value.word_count,
            duration_seconds: value.duration_seconds,
            target_app: value.target_app,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryResponse {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub hits: i64,
    pub created_at: String,
}

impl From<DictionaryEntry> for DictionaryResponse {
    fn from(value: DictionaryEntry) -> Self {
        Self {
            id: value.id,
            term: value.term,
            replacement: value.replacement,
            hits: value.hits,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InjectionTool {
    pub name: String,
    pub available: bool,
    pub preferred: bool,
    pub install_hint: String,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InjectionStatus {
    /// `true` where injection needs external tools the user must install.
    pub configurable: bool,
    /// Human-readable session, e.g. "Wayland (GNOME)". Empty off Linux.
    pub session: String,
    pub selected: Option<String>,
    /// Best first.
    pub tools: Vec<InjectionTool>,
}

impl InjectionStatus {
    /// Async to match the Linux arm, which probes the desktop portal.
    #[cfg(not(target_os = "linux"))]
    #[allow(clippy::unused_async, clippy::unused_async_trait_impl)]
    pub async fn detect() -> Self {
        Self {
            configurable: false,
            session: String::new(),
            selected: None,
            tools: Vec::new(),
        }
    }

    #[cfg(target_os = "linux")]
    pub async fn detect() -> Self {
        use crate::injection::{linux, session};

        let session = session::detect();
        let portal = linux::portal_available().await;
        let selected = linux::selected();

        // The portal leads the list because it is tried first and needs no
        // install; the tools below it are the fallback where it is unsupported.
        let mut tools = Vec::new();
        if session.is_wayland() {
            tools.push(InjectionTool {
                name: "desktop portal".to_string(),
                available: portal,
                preferred: portal,
                install_hint: "built into your desktop — approve the permission prompt".to_string(),
            });
        }
        tools.extend(linux::candidates(session).iter().map(|tool| InjectionTool {
            name: tool.binary().to_string(),
            available: tool.available(),
            preferred: !portal && selected == Some(*tool),
            install_hint: tool.install_hint().to_string(),
        }));

        let active = if portal {
            Some("desktop portal".to_string())
        } else {
            selected.map(|tool| tool.binary().to_string())
        };

        Self {
            configurable: true,
            session: session.describe(),
            selected: active,
            tools,
        }
    }
}
