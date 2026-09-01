use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Condvar,
};

use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock, SetOnce};

use std::collections::HashMap;

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

/// Which global hotkey a save/load refers to; also its key in `hotkeys.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyKind {
    PushToTalk,
    Dictation,
    DictationCommit,
}

impl HotkeyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PushToTalk => "pushToTalk",
            Self::Dictation => "dictation",
            Self::DictationCommit => "dictationCommit",
        }
    }
}

/// How many models may transfer at once. Extras wait in `Queued` rather than
/// being refused, so the user can line several up and walk away.
pub const MAX_CONCURRENT_DOWNLOADS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    Running,
    Error,
}

impl DownloadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Error => "error",
        }
    }
}

pub struct DownloadEntry {
    pub status: DownloadStatus,
    pub progress: u8,
    pub error: Option<String>,
    /// Awaited alongside the network read, so a cancel interrupts immediately
    /// instead of waiting for the current chunk to finish.
    pub cancel: tokio_util::sync::CancellationToken,
}

/// Every download in flight or waiting, keyed by model id. Success and cancel
/// remove the entry — the file on disk is the truth for what is installed.
pub struct Downloads {
    entries: std::sync::Mutex<HashMap<String, DownloadEntry>>,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Downloads {
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(HashMap::new()),
            permits: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, DownloadEntry>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn permits(&self) -> Arc<tokio::sync::Semaphore> {
        Arc::clone(&self.permits)
    }

    pub fn is_pending(&self, id: &str) -> bool {
        self.lock()
            .get(id)
            .is_some_and(|e| e.status != DownloadStatus::Error)
    }

    /// Registers a queued download, returning its cancel token. A model already
    /// queued or running keeps its existing entry and returns `None`.
    pub fn enqueue(&self, id: &str) -> Option<tokio_util::sync::CancellationToken> {
        let mut entries = self.lock();
        if entries
            .get(id)
            .is_some_and(|e| e.status != DownloadStatus::Error)
        {
            return None;
        }
        let cancel = tokio_util::sync::CancellationToken::new();
        entries.insert(
            id.to_string(),
            DownloadEntry {
                status: DownloadStatus::Queued,
                progress: 0,
                error: None,
                cancel: cancel.clone(),
            },
        );
        Some(cancel)
    }

    pub fn set_running(&self, id: &str) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = DownloadStatus::Running;
        }
    }

    pub fn set_progress(&self, id: &str, pct: u8) {
        if let Some(e) = self.lock().get_mut(id) {
            e.progress = pct;
        }
    }

    pub fn set_error(&self, id: &str, msg: String) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = DownloadStatus::Error;
            e.error = Some(msg);
        }
    }

    pub fn remove(&self, id: &str) {
        self.lock().remove(id);
    }

    /// Trips the cancel token, if this model has an entry to cancel.
    pub fn cancel(&self, id: &str) -> bool {
        match self.lock().get(id) {
            Some(e) if e.status != DownloadStatus::Error => {
                e.cancel.cancel();
                true
            }
            _ => false,
        }
    }

    /// Snapshot for the frontend: `(id, status, progress, error)` per entry.
    pub fn snapshot(&self) -> Vec<(String, &'static str, u8, Option<String>)> {
        self.lock()
            .iter()
            .map(|(id, e)| (id.clone(), e.status.as_str(), e.progress, e.error.clone()))
            .collect()
    }
}

/// Database init failed. Commands surface this instead of waiting forever.
pub struct DbUnavailable(pub String);

pub struct AppState {
    /// Set once by the startup task; `db()` waits on it.
    db: SetOnce<Result<SqlitePool, String>>,
    pub app_data_dir: PathBuf,
    pub hotkeys_path: PathBuf,
    pub model_override_path: PathBuf,
    pub language_path: PathBuf,
    pub format_config_path: PathBuf,
    pub input_device_path: PathBuf,
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
    /// App that had focus when this recording started. Consumed by finalize.
    pub focus_target: Arc<std::sync::Mutex<Option<crate::focus::FocusTarget>>>,
    pub downloads: Arc<Downloads>,
    /// In-memory dictionary cache — loaded at startup, mutated on add/delete.
    pub dict_cache: DictCache,
    /// Signalled by the capture thread when it has fully stopped and dropped the stream.
    /// `stop_transcription` waits on this instead of sleeping a fixed duration.
    pub capture_done: Arc<(std::sync::Mutex<bool>, Condvar)>,
    /// Signalled when the streaming-native worker finalizes. It owns its
    /// session locally, so there is no lock for finalize to block on.
    pub stream_done: Arc<(std::sync::Mutex<bool>, Condvar)>,
    /// Signalled by the capture callback on its first sample, so
    /// `start_transcription` doesn't report "started" until the mic is producing
    /// audio (cpal warms up after `play()`, clipping leading speech otherwise).
    pub capture_ready: Arc<(std::sync::Mutex<bool>, Condvar)>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_data_dir: PathBuf,
        hotkeys_path: PathBuf,
        model_override_path: PathBuf,
        language_path: PathBuf,
        format_config_path: PathBuf,
        input_device_path: PathBuf,
        models_dir: PathBuf,
    ) -> Self {
        Self {
            db: SetOnce::new(),
            app_data_dir,
            hotkeys_path,
            model_override_path,
            language_path,
            format_config_path,
            input_device_path,
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
            focus_target: Arc::new(std::sync::Mutex::new(None)),
            downloads: Arc::new(Downloads::new()),
            dict_cache: Arc::new(RwLock::new(HashMap::new())),
            capture_done: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
            stream_done: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
            capture_ready: Arc::new((std::sync::Mutex::new(false), Condvar::new())),
        }
    }

    /// Called once, by the startup task.
    pub fn set_database(&self, result: Result<SqlitePool, String>) {
        let _ = self.db.set(result);
    }

    pub async fn db(&self) -> Result<&SqlitePool, DbUnavailable> {
        self.db
            .wait()
            .await
            .as_ref()
            .map_err(|e| DbUnavailable(e.clone()))
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
        self.reset_language_if_unsupported(&arc, saved.as_deref());
        *self.engine.lock().await = Some(Arc::clone(&arc));
        Ok(arc)
    }

    /// Otherwise every decode silently falls back to English and the picker
    /// still shows a language the model cannot speak.
    pub(crate) fn reset_language_if_unsupported(
        &self,
        engine: &std::sync::Mutex<TranscriptionEngine>,
        saved: Option<&str>,
    ) -> bool {
        use crate::inference::language::{primary_of, AUTO, DEFAULT};

        let Some(code) = saved.filter(|c| *c != AUTO && *c != DEFAULT) else {
            return false;
        };
        let Ok(guard) = engine.lock() else {
            return false;
        };
        let languages = guard.languages();
        if languages.is_empty() || languages.iter().any(|l| l == code || primary_of(l) == code) {
            return false;
        }
        drop(guard);

        if let Err(e) = self.save_language(Some(DEFAULT)) {
            log::warn!("could not reset unsupported language {code}: {e}");
            return false;
        }
        true
    }

    fn read_hotkeys(&self) -> HashMap<String, String> {
        std::fs::read_to_string(&self.hotkeys_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_hotkeys(&self, map: &HashMap<String, String>) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(map)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.hotkeys_path, json)
    }

    pub fn save_hotkey(&self, kind: HotkeyKind, hotkey: &str) -> std::io::Result<()> {
        let mut map = self.read_hotkeys();
        map.insert(kind.as_str().to_string(), hotkey.to_string());
        self.write_hotkeys(&map)
    }

    pub fn load_hotkey(&self, kind: HotkeyKind) -> Option<String> {
        self.read_hotkeys()
            .get(kind.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn delete_hotkey(&self, kind: HotkeyKind) {
        let mut map = self.read_hotkeys();
        map.remove(kind.as_str());
        let _ = self.write_hotkeys(&map);
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

    /// Arm a fresh streaming session for the recording about to start, and note
    /// which app has focus — the hotkey press is the moment that means anything.
    pub fn begin_stream_session(&self) {
        *lock_recovering(&self.stream_session) = Some(crate::transcribe::StreamingSession::new());
        *lock_recovering(&self.streamed_text) = None;
        *lock_recovering(&self.focus_target) = crate::focus::foreground_app();
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

    /// Take the app captured at recording start, for finalize.
    pub fn take_focus_target(&self) -> Option<crate::focus::FocusTarget> {
        lock_recovering(&self.focus_target).take()
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
        // An aborted recording returns before finalize takes this.
        *lock_recovering(&self.focus_target) = None;
    }
}

#[cfg(test)]
#[path = "../tests/unit/downloads.rs"]
mod downloads_tests;
