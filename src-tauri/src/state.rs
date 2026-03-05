use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::auth::AuthService;
use crate::models::ModelSize;

/// Holds the authenticated session for the current app run.
/// Set after login or successful silent re-auth on startup.
#[derive(Debug, Default)]
pub struct AuthSession {
    pub user_id: Option<i64>,
    pub access_token: Option<String>,
}

pub struct AppState {
    pub pool: SqlitePool,
    pub auth: AuthService,
    /// Path used to persist the refresh token across restarts.
    pub token_store_path: PathBuf,
    /// Current authenticated session (populated after login or silent re-auth).
    pub auth_session: Mutex<AuthSession>,
    /// Shared so the transcription task can check when to stop.
    pub transcription_running: Arc<AtomicBool>,
    pub model_override: Mutex<Option<ModelSize>>,
    pub current_hotkey: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(pool: SqlitePool, auth: AuthService, token_store_path: PathBuf) -> Self {
        Self {
            pool,
            auth,
            token_store_path,
            auth_session: Mutex::new(AuthSession::default()),
            transcription_running: Arc::new(AtomicBool::new(false)),
            model_override: Mutex::new(None),
            current_hotkey: Mutex::new(None),
        }
    }

    /// Returns the current authenticated user_id, or None if not authenticated.
    pub async fn current_user_id(&self) -> Option<i64> {
        self.auth_session.lock().await.user_id
    }

    /// Populate the auth session after a successful login or token refresh.
    pub async fn set_auth_session(&self, user_id: i64, access_token: String) {
        let mut session = self.auth_session.lock().await;
        session.user_id = Some(user_id);
        session.access_token = Some(access_token);
    }

    /// Clear the auth session on logout.
    pub async fn clear_auth_session(&self) {
        let mut session = self.auth_session.lock().await;
        session.user_id = None;
        session.access_token = None;
    }

    /// Persist the raw refresh token to disk.
    pub fn save_refresh_token(&self, raw: &str) -> std::io::Result<()> {
        std::fs::write(&self.token_store_path, raw)
    }

    /// Read the persisted refresh token from disk, if any.
    pub fn load_refresh_token(&self) -> Option<String> {
        std::fs::read_to_string(&self.token_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Delete the persisted refresh token.
    pub fn delete_refresh_token(&self) {
        let _ = std::fs::remove_file(&self.token_store_path);
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
