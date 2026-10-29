//! The start sound: a short, quiet two-note chime, synthesized so no asset
//! ships. A missing output device only loses the sound.
//!
//! A Bluetooth sink drops the start of a stream it was just woken for, so the
//! output stays open between recordings and the chime is mixed into it. Only a
//! freshly opened Bluetooth output waits for the sink to wake.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use super::capture::device_name;
use crate::platform::audio_output::default_output_is_bluetooth;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, I24, U24};

/// (frequency Hz, start s, length s).
const NOTES: [(f32, f32, f32); 2] = [(659.25, 0.0, 0.07), (880.0, 0.055, 0.09)];
const DURATION: f32 = 0.145;
const ATTACK: f32 = 0.004;
/// Soft, so the live mic picks up little of it.
const AMPLITUDE: f32 = 0.18;

const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_secs(5);
/// How long a Bluetooth sink takes to wake. Also used when the platform cannot
/// tell, so a Bluetooth headset never loses the chime.
const BLUETOOTH_WAKE: Duration = Duration::from_millis(1500);

pub(crate) fn sample_at(t: f32) -> f32 {
    NOTES
        .iter()
        .map(|&(freq, start, len)| {
            let local = t - start;
            if !(0.0..len).contains(&local) {
                return 0.0;
            }
            let attack = (local / ATTACK).min(1.0);
            let decay = (-local * 5.0 / len).exp();
            (std::f32::consts::TAU * freq * local).sin() * attack * decay
        })
        .sum::<f32>()
        * AMPLITUDE
}

#[derive(Default)]
struct Playhead {
    frame: Option<u32>,
}

impl Playhead {
    #[allow(clippy::cast_precision_loss)] // A few thousand frames.
    fn next(&mut self, trigger: bool, rate: f32) -> f32 {
        if trigger {
            self.frame = Some(0);
        }
        let Some(frame) = self.frame else {
            return 0.0;
        };
        let t = frame as f32 / rate;
        if t >= DURATION {
            self.frame = None;
            return 0.0;
        }
        self.frame = Some(frame + 1);
        sample_at(t)
    }
}

enum Command {
    Warm,
    Play,
}

pub struct Chime {
    commands: mpsc::Sender<Command>,
    /// Cleared by [`Self::cancel`], so a chime still waiting on the output never sounds late.
    wanted: Arc<AtomicBool>,
    /// Mirrors the keep-ready microphone: the output is never released as idle.
    keep_open: Arc<AtomicBool>,
}

impl Chime {
    pub fn new() -> Self {
        let (commands, rx) = mpsc::channel();
        let wanted = Arc::new(AtomicBool::new(false));
        let keep_open = Arc::new(AtomicBool::new(false));
        let (thread_wanted, thread_keep_open) = (Arc::clone(&wanted), Arc::clone(&keep_open));
        let spawned = std::thread::Builder::new()
            .name("start-chime".into())
            .spawn(move || supervise(&rx, &thread_wanted, &thread_keep_open));
        if let Err(e) = spawned {
            log::warn!("start sound thread failed: {e}");
        }
        Self {
            commands,
            wanted,
            keep_open,
        }
    }

    pub fn set_keep_open(&self, keep: bool) {
        self.keep_open.store(keep, Ordering::SeqCst);
    }

    pub fn warm(&self) {
        let _ = self.commands.send(Command::Warm);
    }

    pub fn play(&self) {
        self.wanted.store(true, Ordering::SeqCst);
        let _ = self.commands.send(Command::Play);
    }

    pub fn cancel(&self) {
        self.wanted.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct Control {
    play: AtomicBool,
    broken: AtomicBool,
}

struct Output {
    _stream: cpal::Stream,
    device: String,
    /// When the sink is ready for sound.
    awake_at: Instant,
    control: Arc<Control>,
}

impl Output {
    fn is_current(&self) -> bool {
        !self.control.broken.load(Ordering::SeqCst)
            && cpal::default_host()
                .default_output_device()
                .as_ref()
                .and_then(device_name)
                .as_deref()
                == Some(self.device.as_str())
    }

    fn trigger(&self, wanted: &AtomicBool) {
        std::thread::sleep(self.awake_at.saturating_duration_since(Instant::now()));
        if wanted.swap(false, Ordering::SeqCst) {
            self.control.play.store(true, Ordering::SeqCst);
        }
    }
}

fn supervise(rx: &mpsc::Receiver<Command>, wanted: &AtomicBool, keep_open: &AtomicBool) {
    let mut output: Option<Output> = None;
    let mut last_used = Instant::now();
    loop {
        match rx.recv_timeout(TICK) {
            Ok(command) => {
                last_used = Instant::now();
                if !output.as_ref().is_some_and(Output::is_current) {
                    output = open()
                        .inspect_err(|e| log::warn!("start sound failed: {e}"))
                        .ok();
                }
                if let (Command::Play, Some(output)) = (command, &output) {
                    output.trigger(wanted);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if output.is_some()
                    && !keep_open.load(Ordering::SeqCst)
                    && last_used.elapsed() >= IDLE_TIMEOUT
                {
                    log::debug!("releasing idle start sound output");
                    output = None;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// cpal's default-device handle fails to activate under this thread's COM mode, so
/// the same endpoint is taken from the enumerated list, as the microphone is.
fn default_output() -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let default = host.default_output_device().ok_or("no output device")?;
    let Some(name) = device_name(&default) else {
        return Ok(default);
    };
    let listed = host
        .output_devices()
        .ok()
        .and_then(|mut devices| devices.find(|d| device_name(d).as_deref() == Some(name.as_str())));
    Ok(listed.unwrap_or(default))
}

fn open() -> Result<Output, String> {
    let device = default_output()?;
    let name = device_name(&device).unwrap_or_default();
    let config = device
        .default_output_config()
        .map_err(|e| format!("failed to get output config: {e}"))?;
    let format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();
    let control = Arc::new(Control::default());
    let stream = match format {
        SampleFormat::I8 => build::<i8>(&device, stream_config, &control)?,
        SampleFormat::I16 => build::<i16>(&device, stream_config, &control)?,
        SampleFormat::I24 => build::<I24>(&device, stream_config, &control)?,
        SampleFormat::I32 => build::<i32>(&device, stream_config, &control)?,
        SampleFormat::I64 => build::<i64>(&device, stream_config, &control)?,
        SampleFormat::U8 => build::<u8>(&device, stream_config, &control)?,
        SampleFormat::U16 => build::<u16>(&device, stream_config, &control)?,
        SampleFormat::U24 => build::<U24>(&device, stream_config, &control)?,
        SampleFormat::U32 => build::<u32>(&device, stream_config, &control)?,
        SampleFormat::U64 => build::<u64>(&device, stream_config, &control)?,
        SampleFormat::F32 => build::<f32>(&device, stream_config, &control)?,
        SampleFormat::F64 => build::<f64>(&device, stream_config, &control)?,
        // DSD is not PCM.
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    };
    stream
        .play()
        .map_err(|e| format!("failed to start stream: {e}"))?;
    let opened = Instant::now();
    let bluetooth = default_output_is_bluetooth();
    log::info!("start sound output open on {name} (bluetooth: {bluetooth:?})");
    let wake = if bluetooth == Some(false) {
        Duration::ZERO
    } else {
        BLUETOOTH_WAKE
    };
    Ok(Output {
        _stream: stream,
        device: name,
        awake_at: opened + wake,
        control,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    control: &Arc<Control>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    #[allow(clippy::cast_precision_loss)] // Sample rates are far below 2^24.
    let rate = config.sample_rate as f32;
    let mut playhead = Playhead::default();
    let callback_control = Arc::clone(control);
    let error_control = Arc::clone(control);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut trigger = callback_control.play.swap(false, Ordering::SeqCst);
                for out in data.chunks_mut(channels) {
                    out.fill(T::from_sample(playhead.next(trigger, rate)));
                    trigger = false;
                }
            },
            move |err| {
                log::warn!("start sound stream error: {err}");
                error_control.broken.store(true, Ordering::SeqCst);
            },
            None,
        )
        .map_err(|e| format!("failed to build output stream: {e}"))
}

#[cfg(test)]
#[path = "../../tests/unit/audio/chime.rs"]
mod tests;
