use super::*;

fn convert(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    convert_in_pieces(samples, from_rate, to_rate, samples.len().max(1))
}

fn convert_in_pieces(samples: &[f32], from_rate: u32, to_rate: u32, piece: usize) -> Vec<f32> {
    let mut resampler = StreamResampler::new(from_rate, to_rate).unwrap();
    let mut out = Vec::new();
    for chunk in samples.chunks(piece) {
        out.extend(resampler.push(chunk).unwrap());
    }
    out.extend(resampler.finish().unwrap());
    out
}

#[test]
fn upsample_output_length() {
    let input = vec![0.0f32; 16_000]; // 1 second at 16 kHz
    let output = convert(&input, 16_000, 48_000);
    // Should be approximately 48_000 samples (within 1%)
    let expected = 48_000usize;
    let diff = output.len().abs_diff(expected);
    assert!(diff < 500, "got {}, expected ~{}", output.len(), expected);
}

#[test]
fn downsample_output_length() {
    let input = vec![0.0f32; 48_000]; // 1 second at 48 kHz
    let output = convert(&input, 48_000, 16_000);
    let expected = 16_000usize;
    let diff = output.len().abs_diff(expected);
    assert!(diff < 200, "got {}, expected ~{}", output.len(), expected);
}

#[test]
fn empty_input_returns_empty() {
    let output = convert(&[], 44_100, 16_000);
    assert_eq!(output, Vec::<f32>::new());
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
    let out = convert(&input, 48_000, 16_000);

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
    let out = convert(&input, 16_000, 48_000);

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
    let out = convert(&input, 44_100, 16_000);

    assert!(
        out.iter().all(|s| s.is_finite()),
        "resampler emitted NaN/Inf"
    );
    assert!(peak(&out) < 1.5, "resampler overshot badly: {}", peak(&out));
}

/// 200 ms pushes, as the live stream feeds; lost filter history would glitch at each seam.
#[test]
fn pieces_give_the_same_samples_as_one_conversion() {
    let input = sine(440.0, 48_000, 48_000);
    let whole = convert(&input, 48_000, 16_000);
    let pieces = convert_in_pieces(&input, 48_000, 16_000, 9_600);

    assert_eq!(pieces.len(), whole.len());
    let worst = whole
        .iter()
        .zip(&pieces)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(
        worst < 1e-4,
        "seams differ from a continuous conversion by {worst}"
    );
}

#[test]
fn every_input_sample_is_accounted_for_whatever_the_piece_size() {
    for (from_rate, piece) in [(44_100, 4_410), (48_000, 7_777), (48_000, 1)] {
        let input = vec![0.1f32; from_rate as usize / 2];
        let out = convert_in_pieces(&input, from_rate, 16_000, piece);
        assert_eq!(out.len(), 8_000, "{from_rate} Hz in pieces of {piece}");
    }
}
