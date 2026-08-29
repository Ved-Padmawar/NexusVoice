use super::{WaveformMeter, BANDS, FFT_SIZE};
use std::f32::consts::TAU;

const SR: u32 = 48_000;

#[allow(clippy::cast_precision_loss)]
fn levels_for_tone(meter: &WaveformMeter, freq: f32, amp: f32) -> [f32; BANDS] {
    let sr = SR as f32;
    let samples: Vec<f32> = (0..FFT_SIZE * 4)
        .map(|i| amp * (TAU * freq * i as f32 / sr).sin())
        .collect();
    for block in samples.chunks(256) {
        meter.push(block);
    }
    meter.levels()
}

#[test]
fn silence_reads_flat() {
    let meter = WaveformMeter::new(SR);
    meter.push(&vec![0.0; FFT_SIZE]);
    assert!(meter.levels().iter().all(|&l| l == 0.0));
}

#[test]
fn low_amplitude_stays_near_floor() {
    let meter = WaveformMeter::new(SR);
    let levels = levels_for_tone(&meter, 1000.0, 0.0002);
    assert!(levels.iter().all(|&l| l < 0.05), "got {levels:?}");
}

#[test]
fn room_tone_gates_to_true_zero() {
    // Band tilt lifts room tone back over DB_FLOOR: 8 bars look flat, but a
    // consumer taking the frame's max sees a standing level.
    let meter = WaveformMeter::new(SR);
    let noise: Vec<f32> = (0..FFT_SIZE * 4)
        .map(|i| {
            let x = f32::from(u8::try_from(i % 251).unwrap()) / 251.0 - 0.5;
            x * 0.00008
        })
        .collect();
    for block in noise.chunks(256) {
        meter.push(block);
    }
    let levels = meter.levels();
    assert!(
        levels.iter().all(|&l| l <= f32::EPSILON),
        "room tone left a standing level: {levels:?}"
    );
}

#[test]
fn speech_level_keeps_full_range() {
    let meter = WaveformMeter::new(SR);
    let levels = levels_for_tone(&meter, 1000.0, 0.5);
    assert!(levels[4] > 0.9, "loud tone lost headroom: {levels:?}");
}

#[test]
fn tone_peaks_in_its_own_band() {
    let meter = WaveformMeter::new(SR);
    // 1000 Hz falls in band 4 (850–1300 Hz).
    let levels = levels_for_tone(&meter, 1000.0, 0.1);
    let loudest = levels
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    assert_eq!(loudest, 4, "got {levels:?}");
}

#[test]
fn separated_tones_drive_separate_bands() {
    let meter = WaveformMeter::new(SR);
    // A low tone (200 Hz, band 1) should not pin a high band (3000 Hz, band 7).
    let low = levels_for_tone(&meter, 200.0, 0.1);
    assert!(low[1] > low[7], "low tone leaked to high band: {low:?}");
}
