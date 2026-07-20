use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::SampleFormat;

use crate::audio::WaveformMeter;

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
    use cpal::traits::DeviceTrait;
    device.default_input_config().is_ok()
        || device
            .supported_input_configs()
            .is_ok_and(|mut cfgs| cfgs.next().is_some())
}

/// cpal 0.18 device name (full WASAPI `FriendlyName` on Windows).
fn device_name(device: &cpal::Device) -> Option<String> {
    use cpal::traits::DeviceTrait;
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

/// Stream mono f32 samples from `preferred_device` (or the OS default) into
/// `buffer` until `running` clears. Signals `done` before returning so the caller
/// can wait without a fixed sleep.
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)] // Arcs moved into the capture thread
pub fn capture_microphone(
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<f32>>>,
    native_rate: Arc<Mutex<u32>>,
    waveform: Arc<WaveformMeter>,
    done: Arc<(Mutex<bool>, Condvar)>,
    ready: Arc<(Mutex<bool>, Condvar)>,
    preferred_device: Option<String>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = resolve_input_device(&host, preferred_device.as_deref())?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("failed to get input config: {e}"))?;

    let channels = config.channels() as usize;
    let sample_rate = config.sample_rate();
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    *native_rate.lock().expect("native_rate lock poisoned") = sample_rate;
    waveform.reset(sample_rate);

    // The callback gates on `capturing`, not `running`: when the user releases
    // the hotkey `running` flips false immediately, but we keep accepting samples
    // through a short post-roll so trailing speech isn't clipped before teardown.
    let capturing = Arc::new(AtomicBool::new(true));

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            &stream_config,
            channels,
            Arc::clone(&buffer),
            Arc::clone(&running),
            Arc::clone(&paused),
            Arc::clone(&waveform),
            Arc::clone(&done),
            Arc::clone(&ready),
            Arc::clone(&capturing),
        )?,
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            &stream_config,
            channels,
            Arc::clone(&buffer),
            Arc::clone(&running),
            Arc::clone(&paused),
            Arc::clone(&waveform),
            Arc::clone(&done),
            Arc::clone(&ready),
            Arc::clone(&capturing),
        )?,
        SampleFormat::U16 => build_stream::<u16>(
            &device,
            &stream_config,
            channels,
            Arc::clone(&buffer),
            Arc::clone(&running),
            Arc::clone(&paused),
            Arc::clone(&waveform),
            Arc::clone(&done),
            Arc::clone(&ready),
            Arc::clone(&capturing),
        )?,
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    };

    stream
        .play()
        .map_err(|e| format!("failed to start stream: {e}"))?;

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Post-roll: keep the callback accepting samples briefly after release so
    // trailing speech (and cpal's in-flight buffer) lands before teardown.
    std::thread::sleep(std::time::Duration::from_millis(200));
    capturing.store(false, Ordering::SeqCst);

    drop(stream); // flush final cpal callback before signalling

    // Notify stop_transcription that the stream is fully stopped and buffer is ready.
    let (lock, cvar) = &*done;
    *lock.lock().expect("capture_done lock poisoned") = true;
    cvar.notify_one();

    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)] // Arcs moved into the stream closures
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: Arc<Mutex<Vec<f32>>>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    waveform: Arc<WaveformMeter>,
    done: Arc<(Mutex<bool>, Condvar)>,
    ready: Arc<(Mutex<bool>, Condvar)>,
    capturing: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + ToF32,
{
    let running_err = Arc::clone(&running);
    let stream = device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                if !capturing.load(Ordering::SeqCst) || paused.load(Ordering::SeqCst) {
                    return;
                }
                // First real sample — unblock start_transcription's ready wait.
                let (rlock, rcvar) = &*ready;
                if let Ok(mut r) = rlock.lock() {
                    if !*r {
                        *r = true;
                        rcvar.notify_all();
                    }
                }
                // Downmix straight into the shared buffer (a persistent Vec, so
                // this only amortizes growth — no per-callback allocation), then
                // hand the newly appended slice to the waveform meter.
                if let Ok(mut buf) = buffer.lock() {
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
                    waveform.push(&buf[start..]);
                }
            },
            move |err| {
                // Device disconnected or stream error — stop the recording loop and
                // signal the condvar so stop_transcription unblocks immediately.
                // Without this, the app hangs with the mic held open until restarted.
                log::error!("cpal stream error: {err}");
                running_err.store(false, Ordering::SeqCst);
                let (lock, cvar) = &*done;
                if let Ok(mut guard) = lock.lock() {
                    *guard = true;
                    cvar.notify_one();
                }
            },
            None,
        )
        .map_err(|e| format!("failed to build input stream: {e}"))?;

    Ok(stream)
}

#[cfg(test)]
#[path = "../../tests/unit/audio/capture.rs"]
mod tests;
