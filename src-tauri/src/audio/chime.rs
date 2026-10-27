//! The start sound: a short, quiet two-note chime, synthesized so no asset
//! ships. A missing output device only loses the sound.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, I24, U24};

/// (frequency Hz, start s, length s).
const NOTES: [(f32, f32, f32); 2] = [(659.25, 0.0, 0.07), (880.0, 0.055, 0.09)];
const DURATION: f32 = 0.145;
const ATTACK: f32 = 0.004;
/// Soft, so the live mic picks up little of it.
const AMPLITUDE: f32 = 0.18;

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

/// Play on a background thread; returns at once.
pub fn play_start() {
    let spawned = std::thread::Builder::new()
        .name("start-chime".into())
        .spawn(|| {
            if let Err(e) = play() {
                log::warn!("start sound failed: {e}");
            }
        });
    if let Err(e) = spawned {
        log::warn!("start sound thread failed: {e}");
    }
}

fn play() -> Result<(), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("failed to get output config: {e}"))?;
    let format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();
    let stream = match format {
        SampleFormat::I8 => build::<i8>(&device, stream_config)?,
        SampleFormat::I16 => build::<i16>(&device, stream_config)?,
        SampleFormat::I24 => build::<I24>(&device, stream_config)?,
        SampleFormat::I32 => build::<i32>(&device, stream_config)?,
        SampleFormat::I64 => build::<i64>(&device, stream_config)?,
        SampleFormat::U8 => build::<u8>(&device, stream_config)?,
        SampleFormat::U16 => build::<u16>(&device, stream_config)?,
        SampleFormat::U24 => build::<U24>(&device, stream_config)?,
        SampleFormat::U32 => build::<u32>(&device, stream_config)?,
        SampleFormat::U64 => build::<u64>(&device, stream_config)?,
        SampleFormat::F32 => build::<f32>(&device, stream_config)?,
        SampleFormat::F64 => build::<f64>(&device, stream_config)?,
        // DSD is not PCM.
        fmt => return Err(format!("unsupported sample format: {fmt:?}")),
    };
    stream
        .play()
        .map_err(|e| format!("failed to start stream: {e}"))?;
    // Dropping the stream stops it.
    std::thread::sleep(std::time::Duration::from_secs_f32(DURATION + 0.08));
    Ok(())
}

fn build<T>(device: &cpal::Device, config: cpal::StreamConfig) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    #[allow(clippy::cast_precision_loss)] // Sample rates are far below 2^24.
    let rate = config.sample_rate as f32;
    let mut frame = 0_u32;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for out in data.chunks_mut(channels) {
                    #[allow(clippy::cast_precision_loss)] // A few thousand frames.
                    let value = T::from_sample(sample_at(frame as f32 / rate));
                    out.fill(value);
                    frame = frame.saturating_add(1);
                }
            },
            |err| log::warn!("start sound stream error: {err}"),
            None,
        )
        .map_err(|e| format!("failed to build output stream: {e}"))
}

#[cfg(test)]
#[path = "../../tests/unit/audio/chime.rs"]
mod tests;
