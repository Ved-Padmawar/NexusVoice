use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Condvar,
};

use sqlx::SqlitePool;
use tokio::sync::{Mutex, OnceCell, RwLock};

use std::collections::HashMap;

use crate::auth::AuthService;
use crate::database::models::dictionary::DictionaryEntry;
use crate::inference::WhisperEngine;
use crate::pipeline::StreamingPipeline;

/// Dictionary cache keyed by term for O(1) lookup and deduplication.
pub type DictCache = Arc<RwLock<HashMap<String, DictionaryEntry>>>;

pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;
pub type NativeSampleRate = Arc<std::sync::Mutex<u32>>;

#[derive(Debug, Default)]
pub struct AuthSession {
    pub user_id: Option<i64>,
    pub access_token: Option<String>,
}

/// Model download state tracked in `AppState` so the frontend can poll via command.
/// 0 = idle/unknown, 1 = downloading, 2 = complete, 3 = error, 4 = cancelled
pub struct ModelDownloadState {
    pub status: AtomicU8,
    pub progress: std::sync::Mutex<u8>,
    pub error: std::sync::Mutex<Option<String>>,
    /// Set to true by `cancel_model_download`; checked each chunk in `download_file`.
    pub cancelled: AtomicBool,
}

impl ModelDownloadState {
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(0),
            progress: std::sync::Mutex::new(0),
            error: std::sync::Mutex::new(None),
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn set_downloading(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
        self.status.store(1, Ordering::SeqCst);
        *self.progress.lock().expect("progress lock poisoned") = 0;
        *self.error.lock().expect("error lock poisoned") = None;
    }

    pub fn set_progress(&self, pct: u8) {
        *self.progress.lock().expect("progress lock poisoned") = pct;
    }

    pub fn set_complete(&self) {
        self.status.store(2, Ordering::SeqCst);
        *self.progress.lock().expect("progress lock poisoned") = 100;
    }

    pub fn set_error(&self, msg: String) {
        self.status.store(3, Ordering::SeqCst);
        *self.error.lock().expect("error lock poisoned") = Some(msg);
    }

    pub fn set_cancelled(&self) {
        self.status.store(4, Ordering::SeqCst);
        *self.progress.lock().expect("progress lock poisoned") = 0;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub struct AppState {
    /// Database pool — initialized asynchronously after setup returns.
    /// All commands that need the DB call `db()` which awaits readiness.
    db: OnceCell<SqlitePool>,
    /// Auth service — initialized after the DB is ready.
    auth_cell: OnceCell<AuthService>,
    pub app_data_dir: PathBuf,
    pub token_store_path: PathBuf,
    pub hotkey_store_path: PathBuf,
    pub model_override_path: PathBuf,
    pub beam_size_path: PathBuf,
    pub auth_session: Mutex<AuthSession>,
    pub transcription_running: Arc<AtomicBool>,
    pub current_hotkey: Mutex<Option<String>>,
    pub audio_buffer: AudioBuffer,
    pub native_sample_rate: NativeSampleRate,
    pub models_dir: PathBuf,
    /// Cached whisper engine — loaded once, reused across recordings.
    /// Wrapped in Arc so it can be captured by the spawn closure in `stop_transcription`.
    pub engine: Arc<Mutex<Option<Arc<std::sync::Mutex<WhisperEngine>>>>>,
    pub model_download: Arc<ModelDownloadState>,
    /// In-memory dictionary cache — loaded at startup, mutated on add/delete.
    pub dict_cache: DictCache,
    /// Signalled by the capture thread when it has fully stopped and dropped the stream.
    /// `stop_transcription` waits on this instead of sleeping a fixed duration.
    pub capture_done: Arc<(std::sync::Mutex<bool>, Condvar)>,
    /// Active streaming pipeline — Some while recording, None otherwise.
    pub pipeline: Arc<Mutex<Option<StreamingPipeline>>>,
}

impl AppState {
    pub fn new(
        app_data_dir: PathBuf,
        token_store_path: PathBuf,
        hotkey_store_path: PathBuf,
        model_override_path: PathBuf,
        beam_size_path: PathBuf,
        models_dir: PathBuf,
    ) -> Self {
        Self {
            db: OnceCell::new(),
            auth_cell: OnceCell::new(),
            app_data_dir,
            token_store_path,
            hotkey_store_path,
            model_override_path,
            beam_size_path,
            auth_session: Mutex::new(AuthSession::default()),
            transcription_running: Arc::new(AtomicBool::new(false)),
            current_hotkey: Mutex::new(None),
            audio_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_sample_rate: Arc::new(std::sync::Mutex::new(44100)),
            models_dir,
            engine: Arc::new(Mutex::new(None)),
            model_download: Arc::new(ModelDownloadState::new()),
            dict_cache: Arc::new(RwLock::new(HashMap::new())),
            capture_done: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
            pipeline: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the database pool once it's ready (called from the background init task).
    pub fn set_pool(&self, pool: SqlitePool) {
        let _ = self.db.set(pool);
    }

    /// Set the auth service once the pool is ready.
    pub fn set_auth(&self, auth: AuthService) {
        let _ = self.auth_cell.set(auth);
    }

    /// Get the database pool, waiting if it's still initializing.
    pub async fn db(&self) -> &SqlitePool {
        self.db.get_or_init(|| async {
            // This branch should never execute — the pool is always set by the init task.
            // But if somehow it does, open the DB as a fallback.
            let db_path = self.app_data_dir.join("nexusvoice.db");
            crate::database::connection::open_database(&db_path)
                .await
                .expect("fallback database init failed")
        }).await
    }

    /// Get the auth service, waiting if it's still initializing.
    pub async fn auth(&self) -> &AuthService {
        self.auth_cell.get_or_init(|| async {
            let pool = self.db().await.clone();
            let jwt_secret_path = self.app_data_dir.join("jwt_secret");
            let jwt_secret = crate::auth::load_or_create_jwt_secret(&jwt_secret_path)
                .expect("fallback jwt secret init failed");
            AuthService::new(pool, jwt_secret)
        }).await
    }

    /// Get or load the cached `WhisperEngine`.
    /// Returns Err if the model file is missing (not yet downloaded).
    pub async fn get_or_load_engine(
        &self,
    ) -> Result<Arc<std::sync::Mutex<WhisperEngine>>, String> {
        let mut guard = self.engine.lock().await;
        if let Some(engine) = guard.as_ref() {
            return Ok(Arc::clone(engine));
        }
        let override_size = self.load_model_override();
        let backend = crate::inference::provider::detect_backend();
        let model_size = crate::inference::provider::select_model_size(
            backend,
            override_size.as_deref(),
        );
        let model_path = self.models_dir.join(model_size.filename());
        if !model_path.exists() {
            return Err("model not downloaded yet".to_string());
        }
        let engine = WhisperEngine::new(&self.models_dir, override_size.as_deref())?;
        let arc = Arc::new(std::sync::Mutex::new(engine));
        *guard = Some(Arc::clone(&arc));
        Ok(arc)
    }

    pub async fn current_user_id(&self) -> Option<i64> {
        self.auth_session.lock().await.user_id
    }

    pub async fn set_auth_session(&self, user_id: i64, access_token: String) {
        let mut session = self.auth_session.lock().await;
        session.user_id = Some(user_id);
        session.access_token = Some(access_token);
    }

    pub async fn clear_auth_session(&self) {
        let mut session = self.auth_session.lock().await;
        session.user_id = None;
        session.access_token = None;
    }

    pub fn save_refresh_token(&self, raw: &str) -> std::io::Result<()> {
        std::fs::write(&self.token_store_path, raw)
    }

    pub fn load_refresh_token(&self) -> Option<String> {
        std::fs::read_to_string(&self.token_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_refresh_token(&self) {
        let _ = std::fs::remove_file(&self.token_store_path);
    }

    pub fn save_hotkey(&self, hotkey: &str) -> std::io::Result<()> {
        std::fs::write(&self.hotkey_store_path, hotkey)
    }

    pub fn load_hotkey(&self) -> Option<String> {
        std::fs::read_to_string(&self.hotkey_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_hotkey(&self) {
        let _ = std::fs::remove_file(&self.hotkey_store_path);
    }

    pub fn save_model_override(&self, variant: &str) -> std::io::Result<()> {
        std::fs::write(&self.model_override_path, variant)
    }

    pub fn load_model_override(&self) -> Option<String> {
        std::fs::read_to_string(&self.model_override_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_model_override(&self) {
        let _ = std::fs::remove_file(&self.model_override_path);
    }

    /// Beam size preset: 2 = Fast, 5 = Balanced (default), 8 = Accurate
    pub fn save_beam_size(&self, beam_size: i32) -> std::io::Result<()> {
        std::fs::write(&self.beam_size_path, beam_size.to_string())
    }

    pub fn load_beam_size(&self) -> i32 {
        std::fs::read_to_string(&self.beam_size_path)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .filter(|&v| v == 2 || v == 5 || v == 8)
            .unwrap_or(5) // default: Balanced
    }

    #[allow(dead_code)]
    pub fn delete_beam_size(&self) {
        let _ = std::fs::remove_file(&self.beam_size_path);
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
}
