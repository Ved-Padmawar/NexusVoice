//! Audio preparation — runs on every decode, so a regression here degrades
//! every transcription rather than failing loudly.

use super::{normalize_level, to_16k};

#[test]
fn non_finite_samples_are_zeroed() {
    // A glitching device emits NaN/Inf. Left in, they poison the FFT and the
    // whole decode comes back empty or as garbage.
    let input = [0.5, f32::NAN, -0.5, f32::INFINITY, f32::NEG_INFINITY, 0.25];
    let out = to_16k(&input, 16_000);

    assert!(out.iter().all(|s| s.is_finite()), "{out:?}");
    assert_eq!(out, vec![0.5, 0.0, -0.5, 0.0, 0.0, 0.25]);
}

#[test]
fn out_of_range_samples_are_clamped_not_wrapped() {
    let out = to_16k(&[2.5, -2.5, 1.0, -1.0], 16_000);
    assert_eq!(out, vec![1.0, -1.0, 1.0, -1.0]);
}

#[test]
fn audio_already_at_16k_is_not_resampled() {
    // Resampling 16k→16k would cost time and shift the timeline for nothing.
    let input: Vec<f32> = (0..1000u16).map(|i| f32::from(i) / 1000.0 - 0.5).collect();
    let out = to_16k(&input, 16_000);
    assert_eq!(out.len(), input.len(), "length must be untouched");
    assert_eq!(out, input);
}

#[test]
fn a_native_rate_is_resampled_to_16k() {
    let input = vec![0.0f32; 48_000]; // 1 s at 48 kHz
    let out = to_16k(&input, 48_000);
    assert!(
        out.len().abs_diff(16_000) < 200,
        "expected ~16000 samples, got {}",
        out.len()
    );
}

#[test]
fn an_empty_buffer_stays_empty() {
    assert!(to_16k(&[], 48_000).is_empty());
    assert!(normalize_level(&[]).is_empty());
}

#[test]
fn normalize_lifts_a_quiet_recording_to_minus_three_dbfs() {
    // Whisper's mel extraction is level-sensitive; a quiet mic transcribes
    // poorly without this gain.
    let quiet: Vec<f32> = vec![0.01, -0.005, 0.0075, -0.01];
    let out = normalize_level(&quiet);

    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!((peak - 0.707).abs() < 1e-3, "peak was {peak}");
}

#[test]
fn normalize_attenuates_a_clipping_recording() {
    let loud = vec![1.0f32, -1.0, 0.5];
    let out = normalize_level(&loud);
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!((peak - 0.707).abs() < 1e-3, "peak was {peak}");
}

#[test]
fn normalize_preserves_relative_levels_and_signs() {
    // Gain is a single scalar: the waveform's shape must survive, or the model
    // hears different speech.
    let input = vec![0.2f32, -0.1, 0.4, 0.0];
    let out = normalize_level(&input);

    assert_eq!(out.len(), input.len());
    assert!(out[1] < 0.0, "sign must be preserved");
    assert_eq!(out[3], 0.0, "silence stays silent");
    // out[2] is twice out[0] in the input, so it must stay twice as large.
    assert!((out[2] / out[0] - 2.0).abs() < 1e-4, "{out:?}");
}

#[test]
fn normalize_leaves_silence_untouched() {
    // Dividing by a ~zero peak would explode the gain into noise.
    let silence = vec![0.0f32; 100];
    assert_eq!(normalize_level(&silence), silence);

    let near_silence = vec![1e-9f32; 100];
    assert_eq!(
        normalize_level(&near_silence),
        near_silence,
        "a below-threshold peak must not be amplified"
    );
}

#[test]
fn normalize_keeps_the_timeline_intact() {
    // Word timestamps index into this buffer, so a dropped or inserted sample
    // would desync every timestamp after it.
    let input: Vec<f32> = (0..16_000)
        .map(|i| f32::from(u8::try_from(i % 100).expect("under 100")) / 100.0 - 0.5)
        .collect();
    assert_eq!(normalize_level(&input).len(), input.len());
}
