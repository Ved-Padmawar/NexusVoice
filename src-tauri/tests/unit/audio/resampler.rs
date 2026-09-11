use super::*;

#[test]
fn same_rate_returns_input() {
    let input: Vec<f32> = (0..1000_i16).map(|i| f32::from(i) / 1000.0).collect();
    let output = resample(&input, 16_000, 16_000);
    assert_eq!(output, input);
}

#[test]
fn upsample_output_length() {
    let input = vec![0.0f32; 16_000]; // 1 second at 16 kHz
    let output = resample(&input, 16_000, 48_000);
    // Should be approximately 48_000 samples (within 1%)
    let expected = 48_000usize;
    let diff = output.len().abs_diff(expected);
    assert!(diff < 500, "got {}, expected ~{}", output.len(), expected);
}

#[test]
fn downsample_output_length() {
    let input = vec![0.0f32; 48_000]; // 1 second at 48 kHz
    let output = resample(&input, 48_000, 16_000);
    let expected = 16_000usize;
    let diff = output.len().abs_diff(expected);
    assert!(diff < 200, "got {}, expected ~{}", output.len(), expected);
}

#[test]
fn empty_input_returns_empty() {
    let output = resample(&[], 44_100, 16_000);
    assert!(output.is_empty());
}

/// Peak amplitude of a buffer.
fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0, f32::max)
}

/// Zero crossings, which track frequency without needing an FFT.
fn crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count()
}

fn sine(freq: f32, rate: u32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f32 / rate as f32;
            (std::f32::consts::TAU * freq * t).sin() * 0.8
        })
        .collect()
}

/// The length tests above pass on an all-zero buffer, so they would not notice a
/// resampler that returned silence or noise. This checks the signal survives.
#[test]
fn downsampling_preserves_the_tone() {
    let input = sine(440.0, 48_000, 48_000); // 1 s of A4
    let out = resample(&input, 48_000, 16_000);

    assert!(
        (peak(&out) - 0.8).abs() < 0.1,
        "amplitude was not preserved: peak {}",
        peak(&out)
    );
    // 440 Hz over 1 s is ~880 zero crossings, at either rate.
    let crossings = crossings(&out);
    assert!(
        crossings.abs_diff(880) < 40,
        "frequency shifted: {crossings} crossings, expected ~880"
    );
}

#[test]
fn upsampling_preserves_the_tone() {
    let input = sine(440.0, 16_000, 16_000);
    let out = resample(&input, 16_000, 48_000);

    assert!((peak(&out) - 0.8).abs() < 0.1, "peak {}", peak(&out));
    let crossings = crossings(&out);
    assert!(
        crossings.abs_diff(880) < 40,
        "frequency shifted: {crossings} crossings, expected ~880"
    );
}

#[test]
fn output_stays_finite_and_in_range() {
    // A NaN here would poison the mel spectrogram and empty the transcript.
    let input = sine(3_000.0, 44_100, 44_100);
    let out = resample(&input, 44_100, 16_000);

    assert!(
        out.iter().all(|s| s.is_finite()),
        "resampler emitted NaN/Inf"
    );
    assert!(peak(&out) < 1.5, "resampler overshot badly: {}", peak(&out));
}

#[test]
fn a_cached_resampler_gives_the_same_answer_on_reuse() {
    // Instances are cached and reused across recordings, so stale filter state
    // from the previous call must not change the result.
    let input = sine(440.0, 48_000, 16_000);
    let first = resample(&input, 48_000, 16_000);
    let second = resample(&input, 48_000, 16_000);
    assert_eq!(first, second, "reuse changed the output");
}
