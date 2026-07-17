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
