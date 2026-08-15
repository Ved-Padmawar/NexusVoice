use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

// SincFixedIn processes input in fixed-size chunks.
// chunk_size must be > sinc_len / 2; 1024 is a safe default.
const CHUNK_SIZE: usize = 1024;

/// Cached resamplers per (from, to) rate pair. Building a `SincFixedIn`
/// computes the full sinc filter bank — far more expensive than the
/// resampling itself for typical chunk sizes.
/// Instances are stateful (filter history), so each use is `reset()` first.
type ResamplerCache = Mutex<HashMap<(u32, u32), SincFixedIn<f32>>>;
static RESAMPLERS: OnceLock<ResamplerCache> = OnceLock::new();

/// Resample `samples` from `from_rate` Hz to `to_rate` Hz using a high-quality
/// sinc interpolation filter. Returns the resampled mono f32 buffer.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = f64::from(to_rate) / f64::from(from_rate);
    let chunk_size = CHUNK_SIZE;

    let mut cache = RESAMPLERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let resampler = cache.entry((from_rate, to_rate)).or_insert_with(|| {
        let params = SincInterpolationParameters {
            sinc_len: 64,
            f_cutoff: 0.85, // ≤0.85 recommended for downsampling to avoid aliasing near Nyquist
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        SincFixedIn::<f32>::new(
            ratio, 2.0, params, chunk_size, 1, // mono
        )
        .expect("resampler init failed — invalid parameters")
    });
    // Clear filter history left over from the previous (unrelated) call.
    resampler.reset();

    // Pad to a multiple of chunk_size so every chunk is full.
    let needed = chunk_size - (samples.len() % chunk_size);
    let needed = if needed == chunk_size { 0 } else { needed };
    let mut padded = samples.to_vec();
    padded.extend(std::iter::repeat_n(0.0f32, needed));

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let capacity = (padded.len() as f64 * ratio) as usize + 16;
    let mut out = Vec::with_capacity(capacity);

    for chunk in padded.chunks(chunk_size) {
        let waves_in = vec![chunk.to_vec()];
        match resampler.process(&waves_in, None) {
            Ok(waves_out) => out.extend_from_slice(&waves_out[0]),
            Err(e) => {
                log::error!("resampler chunk error: {e}");
                break;
            }
        }
    }

    // Flush any remaining samples in the resampler's internal buffer.
    if let Ok(waves_out) = resampler.process_partial::<Vec<f32>>(None, None) {
        if let Some(ch) = waves_out.first() {
            out.extend_from_slice(ch);
        }
    }

    // Trim to the expected output length to remove zero-padding artifacts.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let expected_len = (samples.len() as f64 * ratio).round() as usize;
    out.truncate(expected_len);
    out
}

#[cfg(test)]
#[path = "../../tests/unit/audio/resampler.rs"]
mod tests;
