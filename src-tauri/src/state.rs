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
use crate::llm::FormatConfig;

/// Dictionary cache keyed by term for O(1) lookup and deduplication.
pub type DictCache = Arc<RwLock<HashMap<String, DictionaryEntry>>>;

pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;
pub type NativeSampleRate = Arc<std::sync::Mutex<u32>>;

/// A loaded engine, shared across the poller and finalize task.
pub type SharedEngine = Arc<std::sync::Mutex<crate::parakeet::ParakeetEngine>>;
/// The cached engine slot — `None` until first load, evicted on model switch.
pub type EngineCache = Arc<Mutex<Option<SharedEngine>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingMode {
    PushToTalk = 0,
    Dictation = 1,
}

impl RecordingMode {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Dictation,
            _ => Self::PushToTalk,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionPhase {
    Idle = 0,
    Recording = 1,
    Paused = 2,
    Finalizing = 3,
}

impl SessionPhase {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Recording,
            2 => Self::Paused,
            3 => Self::Finalizing,
            _ => Self::Idle,
        }
    }
}

#[derive(Debug, Default)]
pub struct AuthSession {
    pub user_id: Option<i64>,
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
    pub const fn new() -> Self {
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
    pub hotkey_store_path: PathBuf,
    pub dictation_hotkey_store_path: PathBuf,
    pub dictation_commit_hotkey_store_path: PathBuf,
    pub model_override_path: PathBuf,
    pub format_config_path: PathBuf,
    pub auth_session: Mutex<AuthSession>,
    pub transcription_running: Arc<AtomicBool>,
    pub recording_mode: Arc<AtomicU8>,
    pub session_phase: Arc<AtomicU8>,
    pub capture_paused: Arc<AtomicBool>,
    pub current_hotkey: Mutex<Option<String>>,
    pub current_dictation_hotkey: Mutex<Option<String>>,
    pub current_dictation_commit_hotkey: Mutex<Option<String>>,
    pub audio_buffer: AudioBuffer,
    pub native_sample_rate: NativeSampleRate,
    /// Spectrum meter for the pill waveform, fed by the capture thread.
    pub waveform: Arc<crate::audio::WaveformMeter>,
    pub models_dir: PathBuf,
    pub resource_dir: PathBuf,
    /// Cached parakeet.cpp context — loaded once and reused across recordings.
    pub engine: EngineCache,
    pub model_download: Arc<ModelDownloadState>,
    /// In-memory dictionary cache — loaded at startup, mutated on add/delete.
    pub dict_cache: DictCache,
    /// Signalled by the capture thread when it has fully stopped and dropped the stream.
    /// `stop_transcription` waits on this instead of sleeping a fixed duration.
    pub capture_done: Arc<(std::sync::Mutex<bool>, Condvar)>,
    /// Signalled by the capture callback on its first sample, so
    /// `start_transcription` doesn't report "started" until the mic is producing
    /// audio (cpal warms up after `play()`, clipping leading speech otherwise).
    pub capture_ready: Arc<(std::sync::Mutex<bool>, Condvar)>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_data_dir: PathBuf,
        hotkey_store_path: PathBuf,
        dictation_hotkey_store_path: PathBuf,
        dictation_commit_hotkey_store_path: PathBuf,
        model_override_path: PathBuf,
        format_config_path: PathBuf,
        models_dir: PathBuf,
        resource_dir: PathBuf,
    ) -> Self {
        Self {
            db: OnceCell::new(),
            auth_cell: OnceCell::new(),
            app_data_dir,
            hotkey_store_path,
            dictation_hotkey_store_path,
            dictation_commit_hotkey_store_path,
            model_override_path,
            format_config_path,
            auth_session: Mutex::new(AuthSession::default()),
            transcription_running: Arc::new(AtomicBool::new(false)),
            recording_mode: Arc::new(AtomicU8::new(RecordingMode::PushToTalk as u8)),
            session_phase: Arc::new(AtomicU8::new(SessionPhase::Idle as u8)),
            capture_paused: Arc::new(AtomicBool::new(false)),
            current_hotkey: Mutex::new(None),
            current_dictation_hotkey: Mutex::new(None),
            current_dictation_commit_hotkey: Mutex::new(None),
            audio_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_sample_rate: Arc::new(std::sync::Mutex::new(44100)),
            waveform: Arc::new(crate::audio::WaveformMeter::new(44100)),
            models_dir,
            resource_dir,
            engine: Arc::new(Mutex::new(None)),
            model_download: Arc::new(ModelDownloadState::new()),
            dict_cache: Arc::new(RwLock::new(HashMap::new())),
            capture_done: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
            capture_ready: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
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
        self.db
            .get_or_init(|| async {
                // This branch should never execute — the pool is always set by the init task.
                // But if somehow it does, open the DB as a fallback.
                let db_path = self.app_data_dir.join("nexusvoice.db");
                crate::database::connection::open_database(&db_path)
                    .await
                    .expect("fallback database init failed")
            })
            .await
    }

    /// Get the auth service, waiting if it's still initializing.
    pub async fn auth(&self) -> &AuthService {
        self.auth_cell
            .get_or_init(|| async {
                let pool = self.db().await.clone();
                AuthService::new(pool)
            })
            .await
    }

    /// Get or load the cached engine for the active model.
    /// Returns Err if the model file is missing (not yet downloaded).
    pub async fn get_or_load_engine(&self) -> Result<SharedEngine, String> {
        let mut guard = self.engine.lock().await;
        if let Some(engine) = guard.as_ref() {
            return Ok(Arc::clone(engine));
        }
        let model =
            crate::inference::provider::selected_model(self.load_model_override().as_deref());
        let model_path = self.models_dir.join(model.filename());
        if !model_path.is_file() {
            return Err(format!("{} is not downloaded", model.display_name()));
        }
        let arc: SharedEngine = Arc::new(std::sync::Mutex::new(
            crate::parakeet::ParakeetEngine::load(&model_path, &self.resource_dir)?,
        ));
        *guard = Some(Arc::clone(&arc));
        drop(guard);
        Ok(arc)
    }

    pub async fn current_user_id(&self) -> Option<i64> {
        self.auth_session.lock().await.user_id
    }

    /// Mirror the signed-in user id in memory (the persisted source of truth is
    /// the `app_session` table, written by `AuthService`).
    pub async fn set_auth_session(&self, user_id: i64) {
        self.auth_session.lock().await.user_id = Some(user_id);
    }

    pub async fn clear_auth_session(&self) {
        self.auth_session.lock().await.user_id = None;
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

    pub fn save_dictation_hotkey(&self, hotkey: &str) -> std::io::Result<()> {
        std::fs::write(&self.dictation_hotkey_store_path, hotkey)
    }

    pub fn load_dictation_hotkey(&self) -> Option<String> {
        std::fs::read_to_string(&self.dictation_hotkey_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_dictation_hotkey(&self) {
        let _ = std::fs::remove_file(&self.dictation_hotkey_store_path);
    }

    pub fn save_dictation_commit_hotkey(&self, hotkey: &str) -> std::io::Result<()> {
        std::fs::write(&self.dictation_commit_hotkey_store_path, hotkey)
    }

    pub fn load_dictation_commit_hotkey(&self) -> Option<String> {
        std::fs::read_to_string(&self.dictation_commit_hotkey_store_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_dictation_commit_hotkey(&self) {
        let _ = std::fs::remove_file(&self.dictation_commit_hotkey_store_path);
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

    /// Load the formatter config from disk. Returns the default (disabled)
    /// config if the file is missing or unparseable.
    pub fn load_format_config(&self) -> FormatConfig {
        std::fs::read_to_string(&self.format_config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist the formatter config as JSON.
    pub fn save_format_config(&self, cfg: &FormatConfig) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(cfg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.format_config_path, json)
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

    pub fn recording_mode(&self) -> RecordingMode {
        RecordingMode::from_u8(self.recording_mode.load(Ordering::SeqCst))
    }

    pub fn set_recording_mode(&self, mode: RecordingMode) {
        self.recording_mode.store(mode as u8, Ordering::SeqCst);
    }

    pub fn session_phase(&self) -> SessionPhase {
        SessionPhase::from_u8(self.session_phase.load(Ordering::SeqCst))
    }

    pub fn set_session_phase(&self, phase: SessionPhase) {
        self.session_phase.store(phase as u8, Ordering::SeqCst);
    }

    pub fn transition_session_phase(&self, from: SessionPhase, to: SessionPhase) -> bool {
        self.session_phase
            .compare_exchange(from as u8, to as u8, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn reset_recording_session(&self) {
        self.capture_paused.store(false, Ordering::SeqCst);
        self.set_session_phase(SessionPhase::Idle);
        self.set_recording_mode(RecordingMode::PushToTalk);
    }
}
