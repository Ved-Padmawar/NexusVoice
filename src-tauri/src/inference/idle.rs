//! Unloading the model after a stretch without dictation, to give its RAM and
//! VRAM back. The next recording loads it again, so its first words wait on
//! the load.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// How often the watcher looks at the engine; an unload lands up to this late.
const CHECK_EVERY: Duration = Duration::from_secs(30);

/// When an idle model is unloaded. Persisted as the `model_unload` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ModelUnload {
    #[default]
    Never,
    After2Minutes,
    After5Minutes,
    After10Minutes,
    After15Minutes,
    After1Hour,
}

impl ModelUnload {
    pub const fn timeout(self) -> Option<Duration> {
        match self {
            Self::Never => None,
            Self::After2Minutes => Some(Duration::from_mins(2)),
            Self::After5Minutes => Some(Duration::from_mins(5)),
            Self::After10Minutes => Some(Duration::from_mins(10)),
            Self::After15Minutes => Some(Duration::from_mins(15)),
            Self::After1Hour => Some(Duration::from_hours(1)),
        }
    }

    /// Whether a model idle for `idle` should be unloaded now. Never mid-recording.
    pub fn should_unload(self, idle: Duration, recording: bool) -> bool {
        !recording && self.timeout().is_some_and(|limit| idle >= limit)
    }
}

/// Check periodically and unload the engine once it has idled past the
/// configured limit.
pub fn spawn_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(CHECK_EVERY);
        loop {
            ticks.tick().await;
            let state = app.state::<AppState>();
            let policy = state.load_model_unload();
            let recording = state
                .transcription_running
                .load(std::sync::atomic::Ordering::SeqCst);
            if policy.should_unload(state.engine_idle_for(), recording)
                && state.evict_engine().await
            {
                log::info!("model unloaded after idling ({policy:?})");
            }
        }
    });
}

#[cfg(test)]
#[path = "../../tests/unit/inference/idle.rs"]
mod tests;
