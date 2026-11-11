//! Microphone capture, held open across recordings.
//!
//! Opening a capture device is slow — WASAPI takes a beat, Bluetooth headsets
//! far longer — so opening one per recording clipped the first words. The device
//! is opened once and kept open; a recording only *arms* it, which is a single
//! atomic store, so capture begins on the very next callback.
//! The supervisor grants the arm only once the requested device is open, so a
//! stale "Default" stream can't feed the new recording.
//!
//! `cpal::Stream` is `!Send`, so it lives on a dedicated thread driven by a
//! command channel.

use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc, Arc, Condvar, Mutex,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{ErrorKind, SampleFormat};

use crate::audio::WaveformMeter;
use crate::sync::lock_recovering;

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

/// A selectable input device. Identical mics share a `name`, never an `id`.
#[derive(Debug, Clone)]
pub struct InputDevice {
    pub id: String,
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
pub(super) fn device_name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

/// Stable endpoint id as `host:device`, e.g. `wasapi:{0.0.1…}`.
pub(super) fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

fn describe(device: &cpal::Device) -> Option<(String, String)> {
    Some((device_id(device)?, device_name(device)?))
}

/// Enumerate usable input devices, validated by capability rather than name.
/// The OS default is kept and marked.
pub fn list_input_devices() -> Vec<InputDevice> {
    use cpal::traits::HostTrait;

    let host = cpal::default_host();
    let default = host.default_input_device().as_ref().and_then(describe);
    let default_id = default.as_ref().map(|(id, _)| id.as_str());

    let mut listed = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            let Some(entry) = describe(&device) else {
                continue;
            };
            // Keep the default unconditionally so the user's mic is never missing.
            if default_id == Some(entry.0.as_str()) || is_capture_capable(&device) {
                listed.push(entry);
            }
        }
    }

    // ALSA lists one mic under several PCM ids (`hw:`, `plughw:`); elsewhere a
    // shared name means separate mics.
    collapse_devices(listed, default, cfg!(target_os = "linux"))
}

/// Build the device list from `(id, name)` pairs: drop repeats (by name when
/// `merge_same_name`, else by id), mark the OS default, and fail open to the
/// default alone when nothing enumerated so the picker is never empty.
fn collapse_devices(
    listed: Vec<(String, String)>,
    default: Option<(String, String)>,
    merge_same_name: bool,
) -> Vec<InputDevice> {
    let default_id = default.as_ref().map(|(id, _)| id.clone());
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (id, name) in listed {
        let is_default = default_id.as_deref() == Some(id.as_str());
        let key = if merge_same_name {
            name.clone()
        } else {
            id.clone()
        };
        if seen.insert(key) {
            out.push(InputDevice {
                id,
                name,
                is_default,
            });
        } else if is_default {
            // The default may be a merged alias.
            if let Some(kept) = out.iter_mut().find(|d| d.name == name) {
                kept.is_default = true;
            }
        }
    }
    if out.is_empty() {
        if let Some((id, name)) = default {
            out.push(InputDevice {
                id,
                name,
                is_default: true,
            });
        }
    }
    out
}

/// Id of the OS default input device right now.
fn default_input_id() -> Option<String> {
    use cpal::traits::HostTrait;
    cpal::default_host()
        .default_input_device()
        .as_ref()
        .and_then(device_id)
}

/// Resolve the device to capture from: the preferred one if still present,
/// otherwise the OS default so recording never breaks when a mic is unplugged.
fn resolve_input_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> Result<cpal::Device, String> {
    use cpal::traits::HostTrait;
    if let Some(want) = preferred {
        if let Ok(mut devices) = host.input_devices() {
            if let Some(device) = devices.find(|d| device_id(d).as_deref() == Some(want)) {
                return Ok(device);
            }
        }
        log::warn!("preferred input device '{want}' unavailable — using default");
    }
    host.default_input_device()
        .ok_or_else(|| "no input device available".to_string())
}

/// The id of the mic an older version saved by name; `None` when `saved` is
/// already an id or no connected mic has that name.
pub fn id_for_saved_name(saved: &str) -> Option<String> {
    use cpal::traits::HostTrait;
    if saved.parse::<cpal::DeviceId>().is_ok() {
        return None;
    }
    cpal::default_host()
        .input_devices()
        .ok()?
        .find(|d| device_name(d).as_deref() == Some(saved))
        .as_ref()
        .and_then(device_id)
}

enum Command {
    Open(Option<String>),
    Close,
    Shutdown,
}

const DISARMED: u8 = 0;
const ARM_REQUESTED: u8 = 1;
const ARMED: u8 = 2;

/// State the audio callback touches, shared with the supervisor thread.
struct Shared {
    /// Samples are appended only while [`ARMED`].
    armed: AtomicU8,
    /// Set by `pause_dictation` — the callback drops samples but stays armed.
    paused: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<f32>>>,
    waveform: Arc<WaveformMeter>,
    rate: Arc<Mutex<u32>>,
    /// Signalled on the first sample after arming.
    ready: Arc<(Mutex<bool>, Condvar)>,
    /// Why the device failed to open or died; cleared when taken.
    last_error: Mutex<Option<String>>,
    /// The open stream hit a fatal error and delivers nothing more.
    dead: AtomicBool,
    /// Skips the idle release.
    keep_open: AtomicBool,
}

impl Shared {
    fn captures(&self) -> bool {
        self.armed.load(Ordering::SeqCst) == ARMED && !self.paused.load(Ordering::SeqCst)
    }
}

/// The persistent capture stream. The recorder holds one for the app's life.
pub struct MicStream {
    shared: Arc<Shared>,
    tx: Mutex<Option<mpsc::Sender<Command>>>,
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
                armed: AtomicU8::new(DISARMED),
                paused,
                buffer,
                waveform,
                rate,
                ready: Arc::new((Mutex::new(false), Condvar::new())),
                last_error: Mutex::new(None),
                dead: AtomicBool::new(false),
                keep_open: AtomicBool::new(false),
            }),
            tx: Mutex::new(None),
        }
    }

    /// Hold the device open past [`IDLE_TIMEOUT`].
    pub fn set_keep_open(&self, keep: bool) {
        self.shared.keep_open.store(keep, Ordering::SeqCst);
    }

    /// Open the device, starting the supervisor thread on first use. A no-op
    /// when the same device is already open, so it is safe to call per recording.
    pub fn warm_up(&self, preferred: Option<String>) {
        let mut slot = lock_recovering(&self.tx);
        if slot.is_none() {
            let (tx, rx) = mpsc::channel();
            let shared = Arc::clone(&self.shared);
            std::thread::spawn(move || supervise(&rx, &shared));
            *slot = Some(tx);
        }
        if let Some(tx) = slot.as_ref() {
            let _ = tx.send(Command::Open(preferred));
        }
    }

    /// Request capture; the next [`MicStream::warm_up`] grants it. Wait on
    /// [`MicStream::ready_signal`] for the first sample.
    pub fn arm(&self) {
        // Drop a stale error so it can't abort this recording.
        *lock_recovering(&self.shared.last_error) = None;
        *lock_recovering(&self.shared.ready.0) = false;
        self.shared.armed.store(ARM_REQUESTED, Ordering::SeqCst);
    }

    /// Counts a pending request: the recording owns the device.
    pub fn is_armed(&self) -> bool {
        self.shared.armed.load(Ordering::SeqCst) != DISARMED
    }

    /// The stream died and is delivering nothing; cleared by the next open.
    pub fn is_dead(&self) -> bool {
        self.shared.dead.load(Ordering::SeqCst)
    }

    /// Stop capturing immediately, with no post-roll.
    pub fn disarm(&self) {
        self.shared.armed.store(DISARMED, Ordering::SeqCst);
    }

    /// Stop capturing after the post-roll, so trailing speech still lands.
    /// Blocking — call it off the async runtime.
    pub fn disarm_after_post_roll(&self) {
        std::thread::sleep(POST_ROLL);
        self.disarm();
    }

    pub fn ready_signal(&self) -> Arc<(Mutex<bool>, Condvar)> {
        Arc::clone(&self.shared.ready)
    }

    /// Error from the last open attempt or the live stream, if one failed.
    pub fn take_error(&self) -> Option<String> {
        lock_recovering(&self.shared.last_error).take()
    }

    /// Release the device — the selected input changed, or the app is exiting.
    pub fn close(&self) {
        self.disarm();
        if let Some(tx) = lock_recovering(&self.tx).as_ref() {
            let _ = tx.send(Command::Close);
        }
    }

    pub fn shutdown(&self) {
        self.disarm();
        if let Some(tx) = lock_recovering(&self.tx).take() {
            let _ = tx.send(Command::Shutdown);
        }
    }
}

/// Owns the `!Send` `cpal::Stream` and services commands, releasing an idle
/// device after [`IDLE_TIMEOUT`].
fn supervise(rx: &mpsc::Receiver<Command>, shared: &Arc<Shared>) {
    let mut stream = None;
    let mut open_device = None;
    // The device the stream actually captures from, which for "Default" is
    // whatever the OS default was when it opened.
    let mut opened_id: Option<String> = None;
    let mut idle_since = std::time::Instant::now();

    loop {
        match rx.recv_timeout(TICK) {
            Ok(Command::Open(preferred)) => {
                // Reopening the same device would throw away a warm stream,
                // unless "Default" now names a different OS device.
                if stream.is_some()
                    && open_device == preferred
                    && !shared.dead.load(Ordering::SeqCst)
                    && (preferred.is_some() || opened_id == default_input_id())
                {
                    idle_since = std::time::Instant::now();
                    grant_arm(shared);
                    continue;
                }
                // Dropped before the flag clears, so only the new stream can set it.
                drop(stream.take());
                shared.dead.store(false, Ordering::SeqCst);
                match open_stream(shared, preferred.as_deref()) {
                    Ok((s, id)) => {
                        stream = Some(s);
                        opened_id = id;
                        open_device = preferred;
                        *lock_recovering(&shared.last_error) = None;
                        idle_since = std::time::Instant::now();
                        grant_arm(shared);
                    }
                    Err(e) => {
                        log::error!("microphone open failed: {e}");
                        *lock_recovering(&shared.last_error) = Some(e);
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
                if stream.is_some() && shared.dead.load(Ordering::SeqCst) {
                    log::warn!("releasing dead microphone stream");
                    drop(stream.take());
                    open_device = None;
                } else if shared.armed.load(Ordering::SeqCst) != DISARMED
                    || shared.keep_open.load(Ordering::SeqCst)
                {
                    idle_since = std::time::Instant::now();
                } else if stream.is_some()
                    && idle_since.elapsed() >= IDLE_TIMEOUT
                    // A recording that armed just now must not lose its device.
                    && shared.armed.load(Ordering::SeqCst) == DISARMED
                {
                    log::debug!("releasing idle microphone stream");
                    drop(stream.take());
                    open_device = None;
                }
            }
        }
    }
}

/// A disarm since the request wins.
fn grant_arm(shared: &Shared) {
    let _ = shared
        .armed
        .compare_exchange(ARM_REQUESTED, ARMED, Ordering::SeqCst, Ordering::SeqCst);
}

/// Unblock a start that is waiting on the first sample which will never come.
fn release_ready(shared: &Arc<Shared>) {
    let (lock, cvar) = &*shared.ready;
    *lock_recovering(lock) = true;
    cvar.notify_all();
}

/// Open the preferred device, returning the stream and the id of the device it captures from.
fn open_stream(
    shared: &Arc<Shared>,
    preferred: Option<&str>,
) -> Result<(cpal::Stream, Option<String>), String> {
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

    Ok((stream, device_id(&device)))
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
                let mut buf = lock_recovering(&cb.buffer);
                // Checked under the buffer lock: a reader that sees `armed`
                // off and then takes the lock has every sample this recording got.
                if !cb.captures() {
                    return;
                }
                // Downmix straight into the shared buffer, then hand the newly
                // appended slice to the waveform meter.
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
                drop(buf);
                // First sample of this recording — unblock the start wait.
                let (rlock, rcvar) = &*cb.ready;
                if let Ok(mut r) = rlock.lock() {
                    if !*r {
                        *r = true;
                        rcvar.notify_all();
                    }
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
                // Fatal: fail any start waiting on a sample that won't come.
                log::error!("cpal stream error: {err}");
                *lock_recovering(&on_err.last_error) = Some(err.to_string());
                on_err.dead.store(true, Ordering::SeqCst);
                release_ready(&on_err);
            },
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // asserts compare exact-representable values
#[path = "../../tests/unit/audio/capture.rs"]
mod tests;
