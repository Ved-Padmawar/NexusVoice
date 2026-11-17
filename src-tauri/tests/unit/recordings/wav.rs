use super::*;

#[test]
fn a_recording_comes_back_at_its_length_and_level() {
    let tone: Vec<f32> = (0..SAMPLE_RATE as usize)
        .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin() * 0.5)
        .collect();

    let decoded = decode(&encode(&tone).unwrap()).unwrap();

    assert_eq!(decoded.len(), tone.len());
    assert!(tone.iter().zip(&decoded).all(|(a, b)| (a - b).abs() < 1e-3));
}
