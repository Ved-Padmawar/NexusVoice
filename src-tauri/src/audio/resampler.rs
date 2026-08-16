use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

// Input is processed in fixed-size chunks.
// chunk_size must be > sinc_len / 2; 1024 is a safe default.
const CHUNK_SIZE: usize = 1024;

/// Cached resamplers per (from, to) rate pair. Building one computes the full
/// sinc filter bank — far more expensive than the resampling itself.
/// `process_all` resets filter history, so instances are safe to reuse.
type ResamplerCache = Mutex<HashMap<(u32, u32), Async<f32>>>;
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
            // ≤0.85 recommended for downsampling to avoid aliasing near Nyquist
            f_cutoff: Some(0.85),
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        Async::<f32>::new_sinc(
            ratio,
            2.0,
            &params,
            chunk_size,
            1, // mono
            FixedAsync::Input,
        )
        .expect("resampler init failed — invalid parameters")
    });

    // `process_all` handles chunking, padding, and the startup-delay trim.
    let input = vec![samples.to_vec()];
    let Ok(adapter) = SequentialSliceOfVecs::new(&input, 1, samples.len()) else {
        log::error!(
            "resampler input adapter rejected a {}-frame buffer",
            samples.len()
        );
        return samples.to_vec();
    };

    let mut out = match resampler.process_all(&adapter, samples.len(), None) {
        Ok(buffer) => buffer.take_data(),
        Err(e) => {
            log::error!("resampler error: {e}");
            return samples.to_vec();
        }
    };

    // Mono, so interleaved output is already the single channel's samples.
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
