//! The recording lifecycle: arm the microphone, run the live decoders, and on
//! stop hand the captured audio to finalize.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::audio::{chime::Chime, error::friendly_capture_error, MicStream, WaveformMeter};
use crate::bridge::events::{
    RecordingStateChanged, TranscriptionComplete, TranscriptionError, TranscriptionWarning,
};
use crate::models::cache::{EngineCache, EngineLease};
use crate::models::decode::StreamingSession;
use crate::platform::focus::FocusTarget;
use crate::settings::Settings;
use crate::sync::lock_recovering;

use super::finalize::{self, Recording};
use super::workers::{self, Capture, Generation};

/// The post-roll alone is 0.2 s, so this only drops a tap.
const MIN_DURATION_SECS: f64 = 0.3;
/// A warm stream delivers on the next callback; a cold open takes longer.
const READY_TIMEOUT: Duration = Duration::from_secs(3);
const STREAM_DONE_TIMEOUT: Duration = Duration::from_secs(10);
/// Finalize takes the buffer, leaving it with no capacity. Reserved at start so
/// the audio callback isn't reallocating under the buffer lock.
const RESERVE_SECS: usize = 30;
/// Silence left where a hands-free dictation was paused; long enough for the
/// model to hear a sentence end.
const PAUSE_GAP_MS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Records while the hotkey is held.
    PushToTalk,
    /// Hands-free: toggled on, paused and resumed, then committed.
    Dictation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording(Mode),
    /// Dictation only.
    Paused,
    Finalizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RecordingStatus {
    Idle,
    Recording,
    Dictation,
    DictationPaused,
    Processing,
}

fn status_of(phase: Phase) -> RecordingStatus {
    match phase {
        Phase::Idle => RecordingStatus::Idle,
        Phase::Recording(Mode::PushToTalk) => RecordingStatus::Recording,
        Phase::Recording(Mode::Dictation) => RecordingStatus::Dictation,
        Phase::Paused => RecordingStatus::DictationPaused,
        Phase::Finalizing => RecordingStatus::Processing,
    }
}

/// What a recording captures and decodes with. Each recording keeps the ones
/// it started with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RecordingInput {
    Model,
    Microphone,
    Language,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct RecordingSnapshot {
    pub revision: u32,
    pub status: RecordingStatus,
    /// Saved changes the recording in progress does not use.
    pub deferred: Vec<RecordingInput>,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Press,
    Release,
    ToggleDictation,
    Pause,
    Resume,
    Commit,
    Cancel,
}

struct Request {
    action: Action,
    at: Instant,
    reply: Option<tokio::sync::oneshot::Sender<Result<(), RecordError>>>,
}

impl Request {
    fn stale(&self, finished_at: Option<Instant>) -> bool {
        finished_at.is_some_and(|finished| self.at <= finished)
    }
}

#[derive(Debug)]
pub enum RecordError {
    AlreadyRunning,
    NotRunning,
    /// Push-to-talk stop while dictation owns the recording.
    DictationRunning,
    NotDictating,
    NotRecording,
    NotPaused,
    Finalizing,
    Capture(String),
    Unavailable(String),
    /// The recorder's own task panicked.
    Failed,
    /// The app is about to restart.
    Restarting,
}

pub struct Recorder {
    phase: Mutex<Phase>,
    /// Set and read under `phase`, so no recording slips in once a restart holds.
    restarting: AtomicBool,
    /// Set and read under `phase`: a device saved during a recording, which
    /// keeps its stream.
    mic_change_pending: AtomicBool,
    revision: AtomicU32,
    inputs: OnceLock<tokio::sync::mpsc::UnboundedSender<Request>>,
    /// Shared with the capture callback: drops samples while set.
    paused: Arc<AtomicBool>,
    /// Silence inserted at resumes: in the buffer, but not speaking time.
    pause_gap_samples: AtomicUsize,
    capture: Capture,
    chime: Chime,
    focus: Mutex<Option<FocusTarget>>,
    lease: Mutex<Option<EngineLease>>,
}

impl Recorder {
    pub fn new() -> Self {
        let paused = Arc::new(AtomicBool::new(false));
        let audio = Arc::new(Mutex::new(Vec::new()));
        let sample_rate = Arc::new(Mutex::new(44_100));
        let waveform = Arc::new(WaveformMeter::new(44_100));
        let mic = Arc::new(MicStream::new(
            Arc::clone(&audio),
            Arc::clone(&sample_rate),
            Arc::clone(&waveform),
            Arc::clone(&paused),
        ));
        Self {
            phase: Mutex::new(Phase::Idle),
            restarting: AtomicBool::new(false),
            mic_change_pending: AtomicBool::new(false),
            revision: AtomicU32::new(0),
            inputs: OnceLock::new(),
            paused,
            pause_gap_samples: AtomicUsize::new(0),
            capture: Capture {
                generation: Generation::default(),
                capturing: Arc::new(AtomicBool::new(false)),
                audio,
                sample_rate,
                waveform,
                mic,
                session: Arc::new(Mutex::new(None)),
                language: Arc::new(Mutex::new(None)),
                streamed: Arc::new(Mutex::new(None)),
                stream_done: Arc::new((Mutex::new(true), Condvar::new())),
            },
            chime: Chime::new(),
            focus: Mutex::new(None),
            lease: Mutex::new(None),
        }
    }

    pub fn status(&self) -> RecordingStatus {
        status_of(*lock_recovering(&self.phase))
    }

    /// `engine` owns the model and language a recording keeps.
    pub fn snapshot(&self, engine: &EngineCache) -> RecordingSnapshot {
        let (revision, status, mic) = {
            let phase = lock_recovering(&self.phase);
            (
                self.revision.load(Ordering::SeqCst),
                status_of(*phase),
                self.mic_change_pending.load(Ordering::SeqCst),
            )
        };
        let kept = engine.deferred();
        let deferred = [
            (kept.model, RecordingInput::Model),
            (mic, RecordingInput::Microphone),
            (kept.language, RecordingInput::Language),
        ]
        .into_iter()
        .filter_map(|(deferred, input)| deferred.then_some(input))
        .collect();
        RecordingSnapshot {
            revision,
            status,
            deferred,
        }
    }

    fn publish(&self, app: &AppHandle) {
        let snapshot = self.snapshot(&app.state::<EngineCache>());
        let _ = RecordingStateChanged(snapshot).emit(app);
    }

    /// Announce a change to what the recording in progress keeps.
    fn publish_change(&self, app: &AppHandle) {
        self.revision.fetch_add(1, Ordering::SeqCst);
        self.publish(app);
    }

    /// Republish whenever the engine's deferred changes do.
    pub fn follow_engine(app: &AppHandle) {
        let mut changes = app.state::<EngineCache>().watch_deferred();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while changes.changed().await.is_ok() {
                app.state::<Self>().publish_change(&app);
            }
        });
    }

    fn change_phase(&self, phase: &mut Phase, next: Phase) {
        *phase = next;
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    // Enqueue synchronously in OS callback order. The receiver survives every
    // renderer reload, including a release while microphone startup awaits.
    fn sender(&self, app: &AppHandle) -> &tokio::sync::mpsc::UnboundedSender<Request> {
        self.inputs.get_or_init(|| {
            let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Request>();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut finished_at = None;
                while let Some(request) = receiver.recv().await {
                    let recorder = app.state::<Self>();
                    let before = recorder.status();
                    let result = if request.stale(finished_at) {
                        Err(RecordError::Finalizing)
                    } else {
                        recorder.apply_supervised(&app, request.action).await
                    };
                    if result.is_ok()
                        && before != RecordingStatus::Idle
                        && recorder.status() == RecordingStatus::Idle
                    {
                        // Inputs queued behind finalization must not start a
                        // delayed recording after the user already released.
                        finished_at = Some(Instant::now());
                    }
                    recorder.publish(&app);
                    if let Some(reply) = request.reply {
                        let _ = reply.send(result);
                    } else if let Err(RecordError::Unavailable(message)) = result {
                        let _ = TranscriptionWarning(message).emit(&app);
                    }
                }
            });
            sender
        })
    }

    pub fn dispatch(&self, app: &AppHandle, action: Action) {
        if self.status() != RecordingStatus::Processing {
            let _ = self.sender(app).send(Request {
                action,
                at: Instant::now(),
                reply: None,
            });
        }
    }

    pub async fn request(&self, app: &AppHandle, action: Action) -> Result<(), RecordError> {
        if self.status() == RecordingStatus::Processing {
            return Err(RecordError::Finalizing);
        }
        let (reply, response) = tokio::sync::oneshot::channel();
        self.sender(app)
            .send(Request {
                action,
                at: Instant::now(),
                reply: Some(reply),
            })
            .map_err(|_| RecordError::NotRunning)?;
        response.await.map_err(|_| RecordError::NotRunning)?
    }

    /// Runs `apply` as its own task, so a panic cannot end the request loop.
    async fn apply_supervised(&self, app: &AppHandle, action: Action) -> Result<(), RecordError> {
        let task_app = app.clone();
        let outcome = tauri::async_runtime::spawn(async move {
            task_app.state::<Self>().apply(&task_app, action).await
        })
        .await;
        outcome.unwrap_or_else(|e| {
            log::error!("recorder task failed: {e}");
            self.recover(app);
            Err(RecordError::Failed)
        })
    }

    /// Back to idle after a task died holding the recording.
    fn recover(&self, app: &AppHandle) {
        self.capture.mic.disarm();
        self.reset();
        let _ = TranscriptionError("Recording failed unexpectedly. Please try again.".to_string())
            .emit(app);
    }

    async fn apply(&self, app: &AppHandle, action: Action) -> Result<(), RecordError> {
        match action {
            Action::Press => self.start(app, Mode::PushToTalk).await,
            Action::Release => self.stop(app).await,
            Action::ToggleDictation => {
                let phase = *lock_recovering(&self.phase);
                match phase {
                    Phase::Idle => self.start(app, Mode::Dictation).await,
                    Phase::Recording(Mode::Dictation) => self.pause(),
                    Phase::Paused => self.resume(),
                    _ => Err(RecordError::NotDictating),
                }
            }
            Action::Pause => self.pause(),
            Action::Resume => self.resume(),
            Action::Commit => self.commit(app).await,
            Action::Cancel => {
                self.cancel(&app.state::<EngineCache>()).await;
                Ok(())
            }
        }
    }

    /// Refuse new recordings until `release_restart`. False while a dictation is
    /// still being recorded or transcribed.
    pub fn hold_for_restart(&self) -> bool {
        let phase = lock_recovering(&self.phase);
        let idle = *phase == Phase::Idle;
        if idle {
            self.restarting.store(true, Ordering::SeqCst);
        }
        idle
    }

    pub fn release_restart(&self) {
        let _phase = lock_recovering(&self.phase);
        self.restarting.store(false, Ordering::SeqCst);
    }

    fn claim_start(&self, mode: Mode) -> Result<(), RecordError> {
        let mut phase = lock_recovering(&self.phase);
        if *phase != Phase::Idle {
            return Err(RecordError::AlreadyRunning);
        }
        if self.restarting.load(Ordering::SeqCst) {
            return Err(RecordError::Restarting);
        }
        self.change_phase(&mut phase, Phase::Recording(mode));
        Ok(())
    }

    /// Start recording. Reports success only once the mic delivers audio; a
    /// failed open, a dead stream or a silent device aborts the recording.
    async fn start(&self, app: &AppHandle, mode: Mode) -> Result<(), RecordError> {
        if self.status() != RecordingStatus::Idle {
            return Err(RecordError::AlreadyRunning);
        }
        let focus = crate::platform::focus::foreground_app();
        let handle = app.clone();
        let available = tauri::async_runtime::spawn_blocking(move || {
            if handle.state::<EngineCache>().loaded().is_some() {
                return true;
            }
            let library = handle.state::<crate::models::library::ModelLibrary>();
            let settings = handle.state::<Settings>();
            library.is_downloaded(crate::models::library::ModelLibrary::active(&settings))
                || library.best_on_disk(None).is_some()
        })
        .await
        .unwrap_or(false);
        if !available {
            return Err(RecordError::Unavailable(
                "No model installed - download one in Settings".to_string(),
            ));
        }
        self.claim_start(mode)?;
        self.publish(app);
        self.prepare(app, focus);

        let ready = self.capture.mic.ready_signal();
        let timed_out = tauri::async_runtime::spawn_blocking(move || {
            let (lock, cvar) = &*ready;
            let (_guard, wait) = cvar
                .wait_timeout_while(lock_recovering(lock), READY_TIMEOUT, |ready| !*ready)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            wait.timed_out()
        })
        .await
        .unwrap_or(false);

        let Some(e) = self
            .capture
            .mic
            .take_error()
            .or_else(|| timed_out.then(|| "microphone is not responding".to_string()))
        else {
            // The mic is delivering now, so the chime means "listening".
            if app.state::<Settings>().pill().settings.start_sound {
                self.chime.play();
            }
            return Ok(());
        };
        self.capture.mic.disarm();
        self.reset();
        let _ = TranscriptionError(friendly_capture_error(&e)).emit(app);
        Err(RecordError::Capture(e))
    }

    /// Clear the previous recording's state, arm the mic and start the workers.
    /// The mic is held open across recordings, so arming is an atomic store,
    /// not a device open.
    fn prepare(&self, app: &AppHandle, focus: Option<FocusTarget>) {
        // First, so leftover workers go stale before their buffers are replaced.
        let ticket = self.capture.generation.begin();
        self.paused.store(false, Ordering::SeqCst);
        self.pause_gap_samples.store(0, Ordering::SeqCst);
        {
            let rate = *lock_recovering(&self.capture.sample_rate) as usize;
            let mut audio = lock_recovering(&self.capture.audio);
            audio.clear();
            audio.reserve(rate * RESERVE_SECS);
        }
        *lock_recovering(&self.capture.session) = Some(StreamingSession::new());
        *lock_recovering(&self.capture.streamed) = None;
        // The hotkey press is the moment the focused app means anything.
        *lock_recovering(&self.focus) = focus;
        let lease = app.state::<EngineCache>().lease(&app.state::<Settings>());
        *lock_recovering(&self.capture.language) = lease.language().map(str::to_string);
        *lock_recovering(&self.lease) = Some(lease);

        self.capture.capturing.store(true, Ordering::SeqCst);
        // Arm first: it clears the previous error, which the open below may set.
        self.capture.mic.arm();
        if app.state::<Settings>().pill().settings.start_sound {
            self.chime.warm();
        }
        self.capture
            .mic
            .warm_up(app.state::<Settings>().input_device());

        load_engine(app);
        workers::spawn_all(app, &self.capture, &ticket);
    }

    fn pause(&self) -> Result<(), RecordError> {
        self.transition(
            Phase::Recording(Mode::Dictation),
            Phase::Paused,
            RecordError::NotRecording,
        )?;
        self.paused.store(true, Ordering::SeqCst);
        self.chime.cancel();
        Ok(())
    }

    fn resume(&self) -> Result<(), RecordError> {
        self.transition(
            Phase::Paused,
            Phase::Recording(Mode::Dictation),
            RecordError::NotPaused,
        )?;
        self.mark_pause_in_audio();
        self.paused.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Paused audio is dropped, so the speech either side of a pause would run
    /// together and the model would write them as one sentence. A stretch of
    /// silence keeps the pause audible to it.
    fn mark_pause_in_audio(&self) {
        let rate = *lock_recovering(&self.capture.sample_rate) as usize;
        let mut audio = lock_recovering(&self.capture.audio);
        if !audio.is_empty() {
            let gap = rate * PAUSE_GAP_MS / 1000;
            let length = audio.len() + gap;
            audio.resize(length, 0.0);
            self.pause_gap_samples.fetch_add(gap, Ordering::SeqCst);
        }
    }

    fn speaking_seconds(&self, recorded: usize, sample_rate: u32) -> Option<f64> {
        let spoken = recorded.saturating_sub(self.pause_gap_samples.load(Ordering::SeqCst));
        #[allow(clippy::cast_precision_loss)]
        (sample_rate > 0 && spoken > 0).then(|| spoken as f64 / f64::from(sample_rate))
    }

    fn transition(
        &self,
        from: Phase,
        to: Phase,
        wrong_phase: RecordError,
    ) -> Result<(), RecordError> {
        let mut phase = lock_recovering(&self.phase);
        match *phase {
            current if current == from => {
                self.change_phase(&mut phase, to);
                Ok(())
            }
            Phase::Recording(Mode::Dictation) | Phase::Paused => Err(wrong_phase),
            _ => Err(RecordError::NotDictating),
        }
    }

    fn claim_finish(&self, mode: Mode) -> Result<(), RecordError> {
        let mut phase = lock_recovering(&self.phase);
        match (mode, *phase) {
            (Mode::PushToTalk, Phase::Recording(Mode::PushToTalk))
            | (Mode::Dictation, Phase::Recording(Mode::Dictation) | Phase::Paused) => {}
            (_, Phase::Finalizing) => return Err(RecordError::Finalizing),
            (Mode::PushToTalk, Phase::Recording(Mode::Dictation) | Phase::Paused) => {
                return Err(RecordError::DictationRunning)
            }
            (Mode::PushToTalk, Phase::Idle) => return Err(RecordError::NotRunning),
            (Mode::Dictation, _) => return Err(RecordError::NotDictating),
        }
        self.change_phase(&mut phase, Phase::Finalizing);
        Ok(())
    }

    async fn stop(&self, app: &AppHandle) -> Result<(), RecordError> {
        self.claim_finish(Mode::PushToTalk)?;
        self.publish(app);
        self.finish(app).await;
        Ok(())
    }

    async fn commit(&self, app: &AppHandle) -> Result<(), RecordError> {
        self.claim_finish(Mode::Dictation)?;
        self.paused.store(false, Ordering::SeqCst);
        self.publish(app);
        self.finish(app).await;
        Ok(())
    }

    /// Discard the recording without transcribing it.
    async fn cancel(&self, engine: &EngineCache) {
        {
            let mut phase = lock_recovering(&self.phase);
            if !matches!(*phase, Phase::Recording(_) | Phase::Paused) {
                return;
            }
            self.change_phase(&mut phase, Phase::Finalizing);
        }
        self.capture.capturing.store(false, Ordering::SeqCst);
        self.chime.cancel();
        self.capture.mic.disarm();
        // The next recording clears the buffer this worker still reads.
        self.wait_for_stream_done(engine).await;
        lock_recovering(&self.capture.session).take();
        self.reset();
    }

    pub fn set_keep_mic_open(&self, keep: bool) {
        self.capture.mic.set_keep_open(keep);
        self.chime.set_keep_open(keep);
    }

    /// The saved device changed: the next recording opens it.
    pub fn change_mic(&self, app: &AppHandle) {
        if self.release_mic_or_defer() {
            self.publish_change(app);
        }
    }

    /// Idle, release the device now; otherwise the recording keeps its stream
    /// and the change waits. True when it newly started waiting. Holds the
    /// phase across the close, so a start can't slip in and lose its stream.
    fn release_mic_or_defer(&self) -> bool {
        let phase = lock_recovering(&self.phase);
        if *phase == Phase::Idle {
            self.capture.mic.close();
            return false;
        }
        !self.mic_change_pending.swap(true, Ordering::SeqCst)
    }

    /// The app is exiting: stop everything and release the device.
    pub fn shutdown(&self) {
        self.capture.capturing.store(false, Ordering::SeqCst);
        self.reset();
        self.capture.mic.shutdown();
    }

    fn reset(&self) {
        self.capture.capturing.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        lock_recovering(&self.focus).take();
        lock_recovering(&self.lease).take();
        let mut phase = lock_recovering(&self.phase);
        // The next recording opens the saved device anyway.
        self.mic_change_pending.store(false, Ordering::SeqCst);
        self.change_phase(&mut phase, Phase::Idle);
    }

    /// Stop capturing and hand the audio to finalize. Every outcome reaches
    /// the user as an event, so this never fails the caller.
    async fn finish(&self, app: &AppHandle) {
        // Before capture stops: a long recording would otherwise look idle and
        // the watcher could unload the engine this stop is about to use.
        app.state::<EngineCache>().touch();
        self.capture.capturing.store(false, Ordering::SeqCst);
        self.chime.cancel();

        let mic = Arc::clone(&self.capture.mic);
        tauri::async_runtime::spawn_blocking(move || mic.disarm_after_post_roll())
            .await
            .ok();
        self.wait_for_stream_done(&app.state::<EngineCache>()).await;

        // Session first, audio buffer second: the lock order the workers use.
        // Taking the session waits out any in-flight stream decode.
        let session = lock_recovering(&self.capture.session).take();
        let streamed = lock_recovering(&self.capture.streamed).take();
        let samples = std::mem::take(&mut *lock_recovering(&self.capture.audio));
        let sample_rate = *lock_recovering(&self.capture.sample_rate);

        let duration_seconds = self.speaking_seconds(samples.len(), sample_rate);

        // Set when the mic died mid-recording; what it captured first still counts.
        let mic_error = self.capture.mic.take_error();
        if duration_seconds.is_none_or(|secs| secs < MIN_DURATION_SECS) {
            self.reset();
            let _ = match mic_error {
                Some(e) => TranscriptionError(friendly_capture_error(&e)).emit(app),
                None => TranscriptionComplete {
                    paste_error: None,
                    saved: false,
                }
                .emit(app),
            };
            return;
        }
        if let Some(e) = mic_error {
            log::warn!("microphone failed mid-recording, finalizing what was captured: {e}");
            let _ = TranscriptionWarning(
                "Your microphone stopped mid-recording, so the transcript may be cut off."
                    .to_string(),
            )
            .emit(app);
        }

        let engine = match app
            .state::<EngineCache>()
            .get_or_load(&app.state::<Settings>())
            .await
        {
            Ok(engine) => engine,
            Err(e) => {
                log::error!("engine load failed: {e}");
                self.reset();
                let _ = TranscriptionError(e).emit(app);
                return;
            }
        };
        let (Some(lease), focus) = (
            lock_recovering(&self.lease).take(),
            lock_recovering(&self.focus).take(),
        ) else {
            self.reset();
            return;
        };

        finalize::run(
            app,
            Recording {
                engine,
                lease,
                session,
                streamed,
                samples,
                sample_rate,
                duration_seconds,
                focus,
            },
        )
        .await;
        self.reset();
    }

    /// The streaming-native worker writes its transcript only after capture
    /// stops, so reading it straight away would lose the tail. A worker that
    /// never finishes is stuck holding the engine, so it is discarded and
    /// finalize loads a fresh one.
    async fn wait_for_stream_done(&self, engine: &EngineCache) {
        let done = Arc::clone(&self.capture.stream_done);
        let timed_out = tauri::async_runtime::spawn_blocking(move || {
            let (lock, cvar) = &*done;
            let (_guard, wait) = cvar
                .wait_timeout_while(lock_recovering(lock), STREAM_DONE_TIMEOUT, |done| !*done)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            wait.timed_out()
        })
        .await
        .unwrap_or(true);
        if timed_out {
            log::error!("stream worker stalled; discarding the engine");
            // Before finish takes the buffer, so the worker stops instead of reading it.
            self.capture.generation.retire();
            engine.discard();
        }
    }
}

/// Startup warmup is fire-and-forget, so the engine can still be missing when
/// the hotkey lands. Spawned, not awaited, so the load races the recording
/// instead of delaying capture.
fn load_engine(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = app
            .state::<EngineCache>()
            .get_or_load(&app.state::<Settings>())
            .await
        {
            log::debug!("engine not available for streaming: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> EngineCache {
        EngineCache::new(std::env::temp_dir())
    }

    #[test]
    fn backend_owns_pause_resume_commit_and_finalizing_until_reset() {
        let recorder = Recorder::new();
        assert_eq!(recorder.status(), RecordingStatus::Idle);
        recorder.claim_start(Mode::Dictation).unwrap();
        assert!(matches!(
            recorder.claim_finish(Mode::PushToTalk),
            Err(RecordError::DictationRunning)
        ));
        recorder.pause().unwrap();
        assert!(recorder.paused.load(Ordering::SeqCst));
        let paused = recorder.snapshot(&engine());
        assert_eq!(paused.status, RecordingStatus::DictationPaused);
        lock_recovering(&recorder.capture.audio).push(0.5);
        recorder.resume().unwrap();
        assert!(!recorder.paused.load(Ordering::SeqCst));
        assert!(
            lock_recovering(&recorder.capture.audio).len() > 1,
            "resuming leaves silence where the pause was"
        );
        assert_eq!(
            recorder.speaking_seconds(lock_recovering(&recorder.capture.audio).len(), 1),
            Some(1.0),
            "the silence is not speaking time"
        );
        recorder.pause().unwrap();
        recorder.claim_finish(Mode::Dictation).unwrap();
        assert_eq!(recorder.status(), RecordingStatus::Processing);
        assert!(matches!(
            recorder.claim_start(Mode::PushToTalk),
            Err(RecordError::AlreadyRunning)
        ));
        assert!(matches!(
            recorder.claim_finish(Mode::Dictation),
            Err(RecordError::Finalizing)
        ));
        tauri::async_runtime::block_on(recorder.cancel(&engine()));
        assert_eq!(recorder.status(), RecordingStatus::Processing);
        recorder.reset();
        assert_eq!(recorder.status(), RecordingStatus::Idle);
        assert!(recorder.snapshot(&engine()).revision > paused.revision);
    }

    #[test]
    fn a_release_claims_push_to_talk_without_any_renderer_state() {
        let recorder = Recorder::new();
        recorder.claim_start(Mode::PushToTalk).unwrap();
        // No renderer reference or subscription participates in this transition.
        let before_reload = recorder.snapshot(&engine());
        assert!(recorder.pause().is_err());
        recorder.claim_finish(Mode::PushToTalk).unwrap();
        let reloaded = recorder.snapshot(&engine());
        assert_eq!(reloaded.status, RecordingStatus::Processing);
        assert!(reloaded.revision > before_reload.revision);
        recorder.reset();
        recorder.claim_start(Mode::PushToTalk).unwrap();
        tauri::async_runtime::block_on(recorder.cancel(&engine()));
        assert_eq!(recorder.status(), RecordingStatus::Idle);
    }

    #[test]
    fn a_restart_hold_refuses_new_recordings_until_released() {
        let recorder = Recorder::new();
        assert!(recorder.hold_for_restart());
        assert!(matches!(
            recorder.claim_start(Mode::PushToTalk),
            Err(RecordError::Restarting)
        ));
        recorder.release_restart();
        recorder.claim_start(Mode::PushToTalk).unwrap();
    }

    #[test]
    fn a_restart_cannot_hold_while_a_dictation_runs() {
        let recorder = Recorder::new();
        recorder.claim_start(Mode::Dictation).unwrap();
        assert!(!recorder.hold_for_restart());
        recorder.reset();
        recorder.claim_start(Mode::PushToTalk).unwrap();
    }

    #[test]
    fn a_device_change_releases_the_mic_only_while_idle() {
        let recorder = Recorder::new();
        let deferred = || recorder.snapshot(&engine()).deferred;
        assert!(!recorder.release_mic_or_defer());
        assert_eq!(deferred(), []);

        recorder.claim_start(Mode::PushToTalk).unwrap();
        recorder.capture.mic.arm();
        // The recording keeps its stream, and says so until it finishes.
        assert!(recorder.release_mic_or_defer());
        assert!(recorder.capture.mic.is_armed());
        assert_eq!(deferred(), [RecordingInput::Microphone]);
        // A second change while waiting is not news.
        assert!(!recorder.release_mic_or_defer());

        recorder.capture.mic.disarm();
        recorder.reset();
        assert_eq!(deferred(), []);
        assert!(!recorder.release_mic_or_defer());
    }

    #[test]
    fn a_cancel_with_no_stream_worker_returns_at_once() {
        let recorder = Recorder::new();
        recorder.claim_start(Mode::PushToTalk).unwrap();
        let started = Instant::now();
        tauri::async_runtime::block_on(recorder.cancel(&engine()));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(recorder.status(), RecordingStatus::Idle);
    }

    #[test]
    fn a_cancel_stays_busy_until_the_stream_worker_is_done() {
        let recorder = Recorder::new();
        recorder.claim_start(Mode::PushToTalk).unwrap();
        let done = Arc::clone(&recorder.capture.stream_done);
        *lock_recovering(&done.0) = false;
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            *lock_recovering(&done.0) = true;
            done.1.notify_all();
        });
        let started = Instant::now();
        tauri::async_runtime::block_on(recorder.cancel(&engine()));
        assert!(started.elapsed() >= Duration::from_millis(150));
        assert_eq!(recorder.status(), RecordingStatus::Idle);
        worker.join().unwrap();
    }

    #[test]
    fn inputs_queued_during_finalization_cannot_start_a_delayed_recording() {
        let at = Instant::now();
        let request = Request {
            action: Action::Press,
            at,
            reply: None,
        };
        assert!(!request.stale(None));
        assert!(request.stale(Some(at)));
        assert!(request.stale(Some(at + Duration::from_secs(1))));
        assert!(!request.stale(Some(at.checked_sub(Duration::from_secs(1)).unwrap())));
    }
}
