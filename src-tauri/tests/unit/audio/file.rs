use super::*;

/// A stereo 16-bit WAV of `seconds` at `rate`, the same tone in both channels.
fn wav(rate: u32, seconds: u32) -> Vec<u8> {
    let frames = rate * seconds;
    let data_len = frames * 4;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + data_len).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend([1, 0, 2, 0]); // PCM, stereo
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * 4).to_le_bytes());
    bytes.extend([4, 0, 16, 0]);
    bytes.extend(b"data");
    bytes.extend(data_len.to_le_bytes());
    for i in 0..frames {
        let sample =
            ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 12_000.0) as i16;
        bytes.extend(sample.to_le_bytes());
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

#[test]
fn a_wav_at_another_rate_becomes_16k_mono_of_the_same_length() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    std::fs::write(&path, wav(44_100, 2)).unwrap();

    let samples = decode_16k(&path, Duration::from_secs(60)).unwrap();

    assert!(
        samples.len().abs_diff(32_000) < 1_000,
        "{} samples",
        samples.len()
    );
    assert!(samples.iter().any(|s| s.abs() > 0.1));
    assert_eq!(duration_seconds(&path).unwrap(), Some(2.0));
}

#[test]
fn a_file_past_the_limit_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    std::fs::write(&path, wav(16_000, 3)).unwrap();

    assert!(decode_16k(&path, Duration::from_secs(2)).is_err());
    assert!(decode_16k(&path, Duration::from_secs(3)).is_ok());
}
