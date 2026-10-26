//! The recording lifecycle: arm the microphone, run the live decoders, and on
//! stop hand the captured audio to finalize.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::audio::{error::friendly_capture_error, MicStream, WaveformMeter};
use crate::events::{
    RecordingCancelled, TranscriptionComplete, TranscriptionError, TranscriptionWarning,
};
use crate::focus::FocusTarget;
use crate::inference::cache::{EngineCache, EngineLease};
use crate::settings::Settings;
use crate::sync::lock_recovering;
use crate::transcribe::StreamingSession;

use super::finalize::{self, Recording};
use super::workers::{self, Capture};

/// The post-roll alone is 0.2 s, so this only drops a tap.
const MIN_DURATION_SECS: f64 = 0.3;
/// A warm stream delivers on the next callback; a cold open takes longer.
const READY_TIMEOUT: Duration = Duration::from_secs(3);
const STREAM_DONE_TIMEOUT: Duration = Duration::from_secs(10);
/// Finalize takes the buffer, leaving it with no capacity. Reserved at start so
/// the audio callback isn't reallocating under the buffer lock.
const RESERVE_SECS: usize = 30;

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
}

pub struct Recorder {
    phase: Mutex<Phase>,
    /// Shared with the capture callback: drops samples while set.
    paused: Arc<AtomicBool>,
    capture: Capture,
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
            paused,
            capture: Capture {
                capturing: Arc::new(AtomicBool::new(false)),
                audio,
                sample_rate,
                waveform,
                mic,
                session: Arc::new(Mutex::new(None)),
                streamed: Arc::new(Mutex::new(None)),
                stream_done: Arc::new((Mutex::new(false), Condvar::new())),
            },
            focus: Mutex::new(None),
            lease: Mutex::new(None),
        }
    }

    /// Start recording. Reports success only once the mic delivers audio; a
    /// failed open, a dead stream or a silent device aborts the recording.
    pub async fn start(&self, app: &AppHandle, mode: Mode) -> Result<(), RecordError> {
        {
            let mut phase = lock_recovering(&self.phase);
            if *phase != Phase::Idle {
                return Err(RecordError::AlreadyRunning);
            }
            *phase = Phase::Recording(mode);
        }
        self.prepare(app);

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
    fn prepare(&self, app: &AppHandle) {
        self.paused.store(false, Ordering::SeqCst);
        {
            let rate = *lock_recovering(&self.capture.sample_rate) as usize;
            let mut audio = lock_recovering(&self.capture.audio);
            audio.clear();
            audio.reserve(rate * RESERVE_SECS);
        }
        *lock_recovering(&self.capture.session) = Some(StreamingSession::new());
        *lock_recovering(&self.capture.streamed) = None;
        // The hotkey press is the moment the focused app means anything.
        *lock_recovering(&self.focus) = crate::focus::foreground_app();
        *lock_recovering(&self.lease) = Some(app.state::<EngineCache>().lease());

        self.capture.capturing.store(true, Ordering::SeqCst);
        // Arm first: it clears the previous error, which the open below may set.
        self.capture.mic.arm();
        self.capture
            .mic
            .warm_up(app.state::<Settings>().input_device());

        load_engine(app);
        workers::spawn_all(app, &self.capture);
    }

    pub fn pause(&self) -> Result<(), RecordError> {
        self.transition(
            Phase::Recording(Mode::Dictation),
            Phase::Paused,
            RecordError::NotRecording,
        )?;
        self.paused.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), RecordError> {
        self.transition(
            Phase::Paused,
            Phase::Recording(Mode::Dictation),
            RecordError::NotPaused,
        )?;
        self.paused.store(false, Ordering::SeqCst);
        Ok(())
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
                *phase = to;
                Ok(())
            }
            Phase::Recording(Mode::Dictation) | Phase::Paused => Err(wrong_phase),
            _ => Err(RecordError::NotDictating),
        }
    }

    /// Finish a push-to-talk recording.
    pub async fn stop(&self, app: &AppHandle) -> Result<(), RecordError> {
        {
            let mut phase = lock_recovering(&self.phase);
            match *phase {
                Phase::Recording(Mode::PushToTalk) => *phase = Phase::Finalizing,
                Phase::Recording(Mode::Dictation) | Phase::Paused => {
                    return Err(RecordError::DictationRunning)
                }
                Phase::Idle | Phase::Finalizing => return Err(RecordError::NotRunning),
            }
        }
        self.finish(app).await;
        Ok(())
    }

    /// Finish a dictation, paused or not.
    pub async fn commit(&self, app: &AppHandle) -> Result<(), RecordError> {
        {
            let mut phase = lock_recovering(&self.phase);
            match *phase {
                Phase::Recording(Mode::Dictation) | Phase::Paused => *phase = Phase::Finalizing,
                Phase::Finalizing => return Err(RecordError::Finalizing),
                Phase::Idle | Phase::Recording(Mode::PushToTalk) => {
                    return Err(RecordError::NotDictating)
                }
            }
        }
        self.paused.store(false, Ordering::SeqCst);
        self.finish(app).await;
        Ok(())
    }

    /// Discard the recording without transcribing it.
    pub fn cancel(&self, app: &AppHandle) {
        {
            let mut phase = lock_recovering(&self.phase);
            if !matches!(*phase, Phase::Recording(_) | Phase::Paused) {
                return;
            }
            *phase = Phase::Finalizing;
        }
        self.capture.capturing.store(false, Ordering::SeqCst);
        self.capture.mic.disarm();
        lock_recovering(&self.capture.session).take();
        self.reset();
        let _ = RecordingCancelled.emit(app);
    }

    /// Release the device so the next recording opens the newly chosen one.
    /// A recording in progress keeps its stream.
    pub fn close_mic_if_idle(&self) {
        if *lock_recovering(&self.phase) == Phase::Idle {
            self.capture.mic.close();
        }
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
        *lock_recovering(&self.phase) = Phase::Idle;
    }

    /// Stop capturing and hand the audio to finalize. Every outcome reaches
    /// the user as an event, so this never fails the caller.
    async fn finish(&self, app: &AppHandle) {
        // Before capture stops: a long recording would otherwise look idle and
        // the watcher could unload the engine this stop is about to use.
        app.state::<EngineCache>().touch();
        self.capture.capturing.store(false, Ordering::SeqCst);

        let mic = Arc::clone(&self.capture.mic);
        tauri::async_runtime::spawn_blocking(move || mic.disarm_after_post_roll())
            .await
            .ok();
        self.wait_for_stream_done().await;

        // Session first, audio buffer second: the lock order the workers use.
        // Taking the session waits out any in-flight stream decode.
        let session = lock_recovering(&self.capture.session).take();
        let streamed = lock_recovering(&self.capture.streamed).take();
        let samples = std::mem::take(&mut *lock_recovering(&self.capture.audio));
        let sample_rate = *lock_recovering(&self.capture.sample_rate);

        #[allow(clippy::cast_precision_loss)]
        let duration_seconds = (sample_rate > 0 && !samples.is_empty())
            .then(|| samples.len() as f64 / f64::from(sample_rate));

        // Set when the mic died mid-recording; what it captured first still counts.
        let mic_error = self.capture.mic.take_error();
        if duration_seconds.is_none_or(|secs| secs < MIN_DURATION_SECS) {
            self.reset();
            let _ = match mic_error {
                Some(e) => TranscriptionError(friendly_capture_error(&e)).emit(app),
                None => TranscriptionComplete { paste_error: None }.emit(app),
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
                let _ = TranscriptionError(format!("model not ready: {e}")).emit(app);
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

        finalize::spawn(
            app.clone(),
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
        );
        self.reset();
    }

    /// The streaming-native worker writes its transcript only after capture
    /// stops, so reading it straight away would lose the tail.
    async fn wait_for_stream_done(&self) {
        let done = Arc::clone(&self.capture.stream_done);
        tauri::async_runtime::spawn_blocking(move || {
            let (lock, cvar) = &*done;
            let (_guard, wait) = cvar
                .wait_timeout_while(lock_recovering(lock), STREAM_DONE_TIMEOUT, |done| !*done)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if wait.timed_out() {
                log::warn!("stream-done signal missed; finalizing without the streamed tail");
            }
        })
        .await
        .ok();
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
