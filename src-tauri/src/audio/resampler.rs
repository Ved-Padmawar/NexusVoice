use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// Resample `samples` from `from_rate` Hz to `to_rate` Hz using a high-quality
/// sinc interpolation filter. Returns the resampled mono f32 buffer.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    // SincFixedIn processes input in fixed-size chunks.
    // chunk_size must be > sinc_len / 2; 1024 is a safe default.
    let chunk_size = 1024usize;

    let mut resampler = SincFixedIn::<f32>::new(
        ratio, 2.0, params, chunk_size, 1, // mono
    )
    .expect("resampler init failed — invalid parameters");

    // Pad to a multiple of chunk_size so every chunk is full.
    let needed = chunk_size - (samples.len() % chunk_size);
    let needed = if needed == chunk_size { 0 } else { needed };
    let mut padded = samples.to_vec();
    padded.extend(std::iter::repeat_n(0.0f32, needed));

    let mut out = Vec::with_capacity((padded.len() as f64 * ratio) as usize + 16);

    for chunk in padded.chunks(chunk_size) {
        let waves_in = vec![chunk.to_vec()];
        match resampler.process(&waves_in, None) {
            Ok(waves_out) => out.extend_from_slice(&waves_out[0]),
            Err(e) => {
                eprintln!("resampler chunk error: {e}");
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
    let expected_len = (samples.len() as f64 * ratio).round() as usize;
    out.truncate(expected_len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_returns_input() {
        let input: Vec<f32> = (0..1000).map(|i| (i as f32) / 1000.0).collect();
        let output = resample(&input, 16_000, 16_000);
        assert_eq!(output, input);
    }

    #[test]
    fn upsample_output_length() {
        let input = vec![0.0f32; 16_000]; // 1 second at 16 kHz
        let output = resample(&input, 16_000, 48_000);
        // Should be approximately 48_000 samples (within 1%)
        let expected = 48_000usize;
        let diff = (output.len() as i64 - expected as i64).unsigned_abs() as usize;
        assert!(diff < 500, "got {}, expected ~{}", output.len(), expected);
    }

    #[test]
    fn downsample_output_length() {
        let input = vec![0.0f32; 48_000]; // 1 second at 48 kHz
        let output = resample(&input, 48_000, 16_000);
        let expected = 16_000usize;
        let diff = (output.len() as i64 - expected as i64).unsigned_abs() as usize;
        assert!(diff < 200, "got {}, expected ~{}", output.len(), expected);
    }

    #[test]
    fn empty_input_returns_empty() {
        let output = resample(&[], 44_100, 16_000);
        assert!(output.is_empty());
    }
}
