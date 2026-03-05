use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::auth::AuthService;
use crate::models::ModelSize;

pub struct AppState {
    pub pool: SqlitePool,
    pub auth: AuthService,
    /// Shared so the transcription task can check when to stop.
    pub transcription_running: Arc<AtomicBool>,
    pub model_override: Mutex<Option<ModelSize>>,
    pub current_hotkey: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(pool: SqlitePool, auth: AuthService) -> Self {
        Self {
            pool,
            auth,
            transcription_running: Arc::new(AtomicBool::new(false)),
            model_override: Mutex::new(None),
            current_hotkey: Mutex::new(None),
        }
    }

    #[allow(dead_code)]
    pub fn set_transcription_running(&self, running: bool) {
        self.transcription_running.store(running, Ordering::SeqCst);
    }

    pub fn try_start_transcription(&self) -> bool {
        self.transcription_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn try_stop_transcription(&self) -> bool {
        self.transcription_running
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    #[allow(dead_code)]
    pub fn transcription_running(&self) -> bool {
        self.transcription_running.load(Ordering::SeqCst)
    }
}
