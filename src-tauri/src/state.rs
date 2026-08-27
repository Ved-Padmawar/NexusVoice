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
use crate::inference::TranscriptionEngine;
use crate::llm::FormatConfig;

/// Dictionary cache keyed by term for O(1) lookup and deduplication.
pub type DictCache = Arc<RwLock<HashMap<String, DictionaryEntry>>>;

pub type AudioBuffer = Arc<std::sync::Mutex<Vec<f32>>>;
pub type NativeSampleRate = Arc<std::sync::Mutex<u32>>;

/// Lock a recording-path mutex, recovering the guard if a previous holder
/// panicked. Re-panicking here would take down the stop path and strand the
/// pill on screen.
pub fn lock_recovering<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    /// Cancels the in-flight download. Awaited alongside the network read, so a
    /// cancel interrupts immediately instead of waiting for the next chunk.
    /// Replaced with a fresh token on each new download.
    cancel: std::sync::Mutex<tokio_util::sync::CancellationToken>,
}

impl ModelDownloadState {
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(0),
            progress: std::sync::Mutex::new(0),
            error: std::sync::Mutex::new(None),
            cancel: std::sync::Mutex::new(tokio_util::sync::CancellationToken::new()),
        }
    }

    pub fn set_downloading(&self) {
        // A token stays cancelled once tripped, so each run gets a fresh one.
        *self.cancel.lock().expect("cancel lock poisoned") =
            tokio_util::sync::CancellationToken::new();
        self.status.store(1, Ordering::SeqCst);
        *self.progress.lock().expect("progress lock poisoned") = 0;
        *self.error.lock().expect("error lock poisoned") = None;
    }

    /// Token for the current download, to await inside the transfer loop.
    pub fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.cancel.lock().expect("cancel lock poisoned").clone()
    }

    pub fn cancel(&self) {
        self.cancel.lock().expect("cancel lock poisoned").cancel();
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
    pub language_path: PathBuf,
    pub format_config_path: PathBuf,
    pub input_device_path: PathBuf,
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
    /// Cached transcription engine — loaded once, reused across recordings.
    /// Wrapped in Arc so it can be captured by the spawn closure in `stop_transcription`.
    pub engine: Arc<Mutex<Option<Arc<std::sync::Mutex<TranscriptionEngine>>>>>,
    /// Serializes engine construction so two callers racing an empty cache
    /// build only one engine.
    engine_load: Arc<Mutex<()>>,
    /// Streaming transcription state for the recording in progress. Created on
    /// start, advanced by the stream worker, consumed (taken) by finalize.
    pub stream_session: Arc<std::sync::Mutex<Option<crate::transcribe::StreamingSession>>>,
    /// Transcript produced by a streaming-native model, published when its
    /// worker finalizes. `None` means finalize decodes the buffer itself.
    pub streamed_text: Arc<std::sync::Mutex<Option<String>>>,
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
        language_path: PathBuf,
        format_config_path: PathBuf,
        input_device_path: PathBuf,
        models_dir: PathBuf,
    ) -> Self {
        Self {
            db: OnceCell::new(),
            auth_cell: OnceCell::new(),
            app_data_dir,
            hotkey_store_path,
            dictation_hotkey_store_path,
            dictation_commit_hotkey_store_path,
            model_override_path,
            language_path,
            format_config_path,
            input_device_path,
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
            engine: Arc::new(Mutex::new(None)),
            engine_load: Arc::new(Mutex::new(())),
            stream_session: Arc::new(std::sync::Mutex::new(None)),
            streamed_text: Arc::new(std::sync::Mutex::new(None)),
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

    /// Get or load the cached `TranscriptionEngine`.
    /// Returns Err if the model file is missing (not yet downloaded).
    ///
    /// Load + GPU init + warmup take seconds, so they run on a blocking thread
    /// with no mutex held — blocking the async runtime here stalls every command.
    pub async fn get_or_load_engine(
        &self,
    ) -> Result<Arc<std::sync::Mutex<TranscriptionEngine>>, String> {
        if let Some(engine) = self.engine.lock().await.as_ref() {
            return Ok(Arc::clone(engine));
        }

        let _building = self.engine_load.lock().await;
        // Another caller may have finished building while we waited.
        if let Some(engine) = self.engine.lock().await.as_ref() {
            return Ok(Arc::clone(engine));
        }

        let mut override_id = self.load_model_override();
        let backend = crate::inference::provider::detect_backend();
        let entry = crate::inference::provider::select_model(backend, override_id.as_deref());

        // Selected model may still be downloading. Load whatever is present so
        // recording keeps working, but never rewrite the override — that would
        // silently cancel the user's choice mid-download.
        if !self.models_dir.join(&entry.filename).exists() {
            let Some(id) = self.first_downloaded_model() else {
                return Err("model not downloaded yet".to_string());
            };
            log::warn!("selected model not on disk; loading {id} for now");
            override_id = Some(id);
        }

        let models_dir = self.models_dir.clone();
        let saved = self.load_language();
        let language = crate::inference::language::resolve(saved.as_deref()).map(str::to_string);
        let engine = tauri::async_runtime::spawn_blocking(move || {
            TranscriptionEngine::new(&models_dir, override_id.as_deref(), language.as_deref())
        })
        .await
        .map_err(|e| format!("engine load task failed: {e}"))??;

        let arc = Arc::new(std::sync::Mutex::new(engine));
        *self.engine.lock().await = Some(Arc::clone(&arc));
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

    /// Most capable model present on disk, for recovering from a stale override.
    /// The catalog is tier-ascending, so the last match is the best one.
    fn first_downloaded_model(&self) -> Option<String> {
        crate::inference::catalog::all()
            .iter()
            .rfind(|entry| self.models_dir.join(&entry.filename).exists())
            .map(|entry| entry.id.clone())
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

    /// Persist the dictation language. `None` clears the setting.
    pub fn save_language(&self, code: Option<&str>) -> std::io::Result<()> {
        if let Some(code) = code {
            std::fs::write(&self.language_path, code)
        } else {
            let _ = std::fs::remove_file(&self.language_path);
            Ok(())
        }
    }

    /// The persisted value verbatim — an ISO code, `auto`, or `None` when never
    /// chosen. Pass through `language::resolve` for the engine's hint.
    pub fn load_language(&self) -> Option<String> {
        std::fs::read_to_string(&self.language_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Preferred input device by name. `None` means use the OS default.
    pub fn save_input_device(&self, name: &str) -> std::io::Result<()> {
        std::fs::write(&self.input_device_path, name)
    }

    pub fn load_input_device(&self) -> Option<String> {
        std::fs::read_to_string(&self.input_device_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_input_device(&self) {
        let _ = std::fs::remove_file(&self.input_device_path);
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

    /// Arm a fresh streaming session for the recording about to start.
    pub fn begin_stream_session(&self) {
        *lock_recovering(&self.stream_session) = Some(crate::transcribe::StreamingSession::new());
        *lock_recovering(&self.streamed_text) = None;
    }

    /// Take the streaming session for finalize (or to discard it on cancel).
    /// Blocks until any in-flight stream decode releases the session.
    pub fn take_stream_session(&self) -> Option<crate::transcribe::StreamingSession> {
        lock_recovering(&self.stream_session).take()
    }

    /// Take the transcript a streaming-native model produced, if one ran.
    pub fn take_streamed_text(&self) -> Option<String> {
        lock_recovering(&self.streamed_text).take()
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
