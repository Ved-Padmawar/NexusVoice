//! Microphone capture, held open across recordings.
//!
//! Opening a capture device is slow — WASAPI takes a beat, Bluetooth headsets
//! far longer — so opening one per recording clipped the first words. The device
//! is opened once and kept open; a recording only *arms* it, which is a single
//! atomic store, so capture begins on the very next callback.
//!
//! `cpal::Stream` is `!Send`, so it lives on a dedicated thread driven by a
//! command channel.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Condvar, Mutex,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{ErrorKind, SampleFormat};

use crate::audio::WaveformMeter;
use crate::state::lock_recovering;

/// How long a warm stream is kept with nothing recording. Long enough to cover
/// back-to-back dictation, short enough that the OS mic indicator doesn't stay
/// lit after the user has moved on.
const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Post-roll after the hotkey is released: cpal has samples in flight, and
/// trailing speech lands a beat late.
const POST_ROLL: std::time::Duration = std::time::Duration::from_millis(200);

/// Supervisor tick — bounds how long the idle check waits.
const TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// Conversion helper: normalize any cpal sample type to f32 in [-1.0, 1.0].
pub trait ToF32 {
    fn to_f32(self) -> f32;
}

impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        f32::from(self) / f32::from(Self::MAX)
    }
}

impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (f32::from(self) / f32::from(Self::MAX)).mul_add(2.0, -1.0)
    }
}

/// A selectable input device: its name and whether it's the OS default.
#[derive(Debug, Clone)]
pub struct InputDevice {
    pub name: String,
    pub is_default: bool,
}

/// True if the device can actually capture — the reliable signal, unlike
/// name matching which is locale-dependent and drops real USB/headset mics.
#[allow(deprecated)]
fn is_capture_capable(device: &cpal::Device) -> bool {
    device.default_input_config().is_ok()
        || device
            .supported_input_configs()
            .is_ok_and(|mut cfgs| cfgs.next().is_some())
}

/// cpal 0.18 device name (full WASAPI `FriendlyName` on Windows).
fn device_name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

/// Enumerate usable input devices, validated by capability rather than name.
/// The OS default is kept and marked; duplicate names (ALSA aliases) collapse.
pub fn list_input_devices() -> Vec<InputDevice> {
    use cpal::traits::HostTrait;

    let host = cpal::default_host();
    let default_name = host.default_input_device().as_ref().and_then(device_name);

    let mut names = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            let Some(name) = device_name(&device) else {
                continue;
            };
            let is_default = default_name.as_deref() == Some(name.as_str());
            // Keep the default unconditionally so the user's mic is never missing.
            if is_default || is_capture_capable(&device) {
                names.push(name);
            }
        }
    }

    collapse_devices(names, default_name.as_deref())
}

/// Build the deduped device list from raw names: collapse duplicate names (ALSA
/// `hw:`/`plughw:` aliases), mark the OS default, and fail open to the default
/// alone when nothing enumerated so the picker is never empty.
fn collapse_devices(names: Vec<String>, default_name: Option<&str>) -> Vec<InputDevice> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in names {
        let is_default = default_name == Some(name.as_str());
        if seen.insert(name.clone()) {
            out.push(InputDevice { name, is_default });
        }
    }
    if out.is_empty() {
        if let Some(name) = default_name {
            out.push(InputDevice {
                name: name.to_string(),
                is_default: true,
            });
        }
    }
    out
}

/// Resolve the device to capture from: the preferred one if still present,
/// otherwise the OS default so recording never breaks when a mic is unplugged.
fn resolve_input_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> Result<cpal::Device, String> {
    use cpal::traits::HostTrait;
    if let Some(want) = preferred {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if device_name(&device).as_deref() == Some(want) {
                    return Ok(device);
                }
            }
        }
        log::warn!("preferred input device '{want}' unavailable — using default");
    }
    host.default_input_device()
        .ok_or_else(|| "no input device available".to_string())
}

enum Command {
    Open(Option<String>),
    Close,
    Shutdown,
}

/// State the audio callback touches, shared with the supervisor thread.
struct Shared {
    /// Samples are appended only while this is set.
    armed: AtomicBool,
    /// Set by `pause_dictation` — the callback drops samples but stays armed.
    paused: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<f32>>>,
    waveform: Arc<WaveformMeter>,
    rate: Arc<Mutex<u32>>,
    /// Signalled on the first sample after arming.
    ready: Arc<(Mutex<bool>, Condvar)>,
}

/// The persistent capture stream. One lives in `AppState` for the app's life.
pub struct MicStream {
    shared: Arc<Shared>,
    tx: Mutex<Option<mpsc::Sender<Command>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl MicStream {
    pub fn new(
        buffer: Arc<Mutex<Vec<f32>>>,
        rate: Arc<Mutex<u32>>,
        waveform: Arc<WaveformMeter>,
        paused: Arc<AtomicBool>,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                armed: AtomicBool::new(false),
                paused,
                buffer,
                waveform,
                rate,
                ready: Arc::new((Mutex::new(false), Condvar::new())),
            }),
            tx: Mutex::new(None),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Open the device, starting the supervisor thread on first use. A no-op
    /// when the same device is already open, so it is safe to call per recording.
    pub fn warm_up(&self, preferred: Option<String>) {
        let mut slot = lock_recovering(&self.tx);
        if slot.is_none() {
            let (tx, rx) = mpsc::channel();
            let shared = Arc::clone(&self.shared);
            let last_error = Arc::clone(&self.last_error);
            std::thread::spawn(move || supervise(&rx, &shared, &last_error));
            *slot = Some(tx);
        }
        if let Some(tx) = slot.as_ref() {
            let _ = tx.send(Command::Open(preferred));
        }
    }

    /// Begin capturing into the shared buffer. Wait on
    /// [`MicStream::ready_signal`] for the first sample.
    pub fn arm(&self) {
        // Drop a stale error so it can't abort this recording.
        *lock_recovering(&self.last_error) = None;
        *lock_recovering(&self.shared.ready.0) = false;
        self.shared.armed.store(true, Ordering::SeqCst);
    }

    /// Stop capturing after the post-roll, so trailing speech still lands.
    /// Blocking — call it off the async runtime.
    pub fn disarm_after_post_roll(&self) {
        std::thread::sleep(POST_ROLL);
        self.shared.armed.store(false, Ordering::SeqCst);
    }

    pub fn ready_signal(&self) -> Arc<(Mutex<bool>, Condvar)> {
        Arc::clone(&self.shared.ready)
    }

    /// Error from the last open attempt, if it failed.
    pub fn take_error(&self) -> Option<String> {
        lock_recovering(&self.last_error).take()
    }

    /// Release the device — the selected input changed, or the app is exiting.
    pub fn close(&self) {
        self.shared.armed.store(false, Ordering::SeqCst);
        if let Some(tx) = lock_recovering(&self.tx).as_ref() {
            let _ = tx.send(Command::Close);
        }
    }

    pub fn shutdown(&self) {
        self.shared.armed.store(false, Ordering::SeqCst);
        if let Some(tx) = lock_recovering(&self.tx).take() {
            let _ = tx.send(Command::Shutdown);
        }
    }
}

/// Owns the `!Send` `cpal::Stream` and services commands, releasing an idle
/// device after [`IDLE_TIMEOUT`].
fn supervise(
    rx: &mpsc::Receiver<Command>,
    shared: &Arc<Shared>,
    last_error: &Arc<Mutex<Option<String>>>,
) {
    let mut stream = None;
    let mut open_device = None;
    let mut idle_since = std::time::Instant::now();

    loop {
        match rx.recv_timeout(TICK) {
            Ok(Command::Open(preferred)) => {
                // Reopening the same device would throw away a warm stream.
                if stream.is_some() && open_device == preferred {
                    idle_since = std::time::Instant::now();
                    continue;
                }
                drop(stream.take());
                match open_stream(shared, preferred.as_deref()) {
                    Ok(s) => {
                        stream = Some(s);
                        open_device = preferred;
                        *lock_recovering(last_error) = None;
                        idle_since = std::time::Instant::now();
                    }
                    Err(e) => {
                        log::error!("microphone open failed: {e}");
                        *lock_recovering(last_error) = Some(e);
                        open_device = None;
                        release_ready(shared);
                    }
                }
            }
            Ok(Command::Close) => {
                drop(stream.take());
                open_device = None;
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if shared.armed.load(Ordering::SeqCst) {
                    idle_since = std::time::Instant::now();
                } else if stream.is_some()
                    && idle_since.elapsed() >= IDLE_TIMEOUT
                    // A recording that armed just now must not lose its device.
                    && !shared.armed.load(Ordering::SeqCst)
                {
                    log::debug!("releasing idle microphone stream");
                    drop(stream.take());
                    open_device = None;
                }
            }
        }
    }
}

/// Unblock a start that is waiting on the first sample which will never come.
fn release_ready(shared: &Arc<Shared>) {
    let (lock, cvar) = &*shared.ready;
    *lock_recovering(lock) = true;
    cvar.notify_all();
}

fn open_stream(shared: &Arc<Shared>, preferred: Option<&str>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = resolve_input_device(&host, preferred)?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get input config: {e}"))?;

    let channels = config.channels() as usize;
    let sample_rate = config.sample_rate();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    *lock_recovering(&shared.rate) = sample_rate;
    shared.waveform.reset(sample_rate);

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, channels, shared)?,
        SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, channels, shared)?,
        SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, channels, shared)?,
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to start stream: {e}"))?;

    Ok(stream)
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    shared: &Arc<Shared>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + ToF32,
{
    let cb = Arc::clone(shared);
    let on_err = Arc::clone(shared);
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                if !cb.armed.load(Ordering::SeqCst) || cb.paused.load(Ordering::SeqCst) {
                    return;
                }
                // First sample of this recording — unblock the start wait.
                let (rlock, rcvar) = &*cb.ready;
                if let Ok(mut r) = rlock.lock() {
                    if !*r {
                        *r = true;
                        rcvar.notify_all();
                    }
                }
                // Downmix straight into the shared buffer (a persistent Vec, so
                // this only amortizes growth — no per-callback allocation), then
                // hand the newly appended slice to the waveform meter.
                if let Ok(mut buf) = cb.buffer.lock() {
                    let start = buf.len();
                    if channels == 1 {
                        buf.extend(data.iter().map(|s| s.to_f32()));
                    } else {
                        buf.extend(data.chunks(channels).map(|frame| {
                            #[allow(clippy::cast_precision_loss)]
                            let n = channels as f32; // channels ≤ 8 in practice, no real precision loss
                            frame.iter().map(|s| s.to_f32()).sum::<f32>() / n
                        }));
                    }
                    cb.waveform.push(&buf[start..]);
                }
            },
            move |err| {
                // Xrun (a dropped buffer, common on WASAPI at stream start) and
                // DeviceChanged leave the stream live — tearing down here would
                // lose the whole dictation.
                if matches!(err.kind(), ErrorKind::Xrun | ErrorKind::DeviceChanged) {
                    log::warn!("cpal stream glitch (continuing): {err}");
                    return;
                }
                // Fatal: release any start waiting on a sample that won't come.
                log::error!("cpal stream error: {err}");
                release_ready(&on_err);
            },
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))
}

#[cfg(test)]
#[path = "../../tests/unit/audio/capture.rs"]
mod tests;
