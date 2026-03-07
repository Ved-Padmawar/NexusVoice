use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::auth::AuthService;
use crate::inference::{ExecutionProvider, WhisperEngine};

/// Shared audio sample buffer filled by the cpal capture thread.
pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;

/// Native sample rate captured from the cpal device, set when recording starts.
pub type NativeSampleRate = Arc<std::sync::Mutex<u32>>;

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
    /// Path used to persist the registered hotkey across restarts.
    pub hotkey_store_path: PathBuf,
    /// Current authenticated session (populated after login or silent re-auth).
    pub auth_session: Mutex<AuthSession>,
    /// Shared so the transcription task can check when to stop.
    pub transcription_running: Arc<AtomicBool>,
    pub current_hotkey: Mutex<Option<String>>,
    /// Audio samples collected during an active recording session.
    pub audio_buffer: AudioBuffer,
    /// Native sample rate of the captured audio (set when recording starts).
    pub native_sample_rate: NativeSampleRate,
    /// Path to the directory where whisper model files are stored.
    pub models_dir: PathBuf,
    /// Cached Whisper engine — loaded once, reused across recordings.
    pub whisper: Mutex<Option<Arc<WhisperEngine>>>,
    /// Execution provider detected at startup.
    pub exec_provider: ExecutionProvider,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        auth: AuthService,
        token_store_path: PathBuf,
        hotkey_store_path: PathBuf,
        models_dir: PathBuf,
        exec_provider: ExecutionProvider,
    ) -> Self {
        Self {
            pool,
            auth,
            token_store_path,
            hotkey_store_path,
            auth_session: Mutex::new(AuthSession::default()),
            transcription_running: Arc::new(AtomicBool::new(false)),
            current_hotkey: Mutex::new(None),
            audio_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_sample_rate: Arc::new(std::sync::Mutex::new(44100)),
            models_dir,
            whisper: Mutex::new(None),
            exec_provider,
        }
    }

    /// Get or load the cached WhisperEngine. Returns Err if model file is missing.
    pub async fn get_or_load_whisper(&self) -> Result<Arc<WhisperEngine>, String> {
        let mut guard = self.whisper.lock().await;
        if let Some(engine) = guard.as_ref() {
            return Ok(Arc::clone(engine));
        }
        let model_path = self.models_dir.join("ggml-large-v3-turbo.bin");
        if !model_path.exists() {
            return Err("model not downloaded yet".to_string());
        }
        let engine = WhisperEngine::new(&model_path, self.exec_provider)?;
        let arc = Arc::new(engine);
        *guard = Some(Arc::clone(&arc));
        Ok(arc)
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

    /// Persist the hotkey string to disk.
    pub fn save_hotkey(&self, hotkey: &str) -> std::io::Result<()> {
        std::fs::write(&self.hotkey_store_path, hotkey)
    }

    /// Read the persisted hotkey from disk, if any.
    pub fn load_hotkey(&self) -> Option<String> {
        std::fs::read_to_string(&self.hotkey_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Delete the persisted hotkey.
    pub fn delete_hotkey(&self) {
        let _ = std::fs::remove_file(&self.hotkey_store_path);
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
