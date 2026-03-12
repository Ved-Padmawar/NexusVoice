use nnnoiseless::DenoiseState;

/// Apply RNNoise frame-by-frame. Expects samples at 48 kHz.
/// Input and output are f32 in [-1.0, 1.0].
pub fn denoise(samples: &[f32]) -> Vec<f32> {
    const FRAME: usize = DenoiseState::FRAME_SIZE;
    let mut state = DenoiseState::new();
    let mut out = Vec::with_capacity(samples.len());

    // nnnoiseless expects f32 in the range of i16 (-32768..32768)
    let scaled: Vec<f32> = samples.iter().map(|&s| s * 32768.0).collect();

    let mut frame_in = [0.0f32; FRAME];
    let mut frame_out = [0.0f32; FRAME];

    for chunk in scaled.chunks(FRAME) {
        let len = chunk.len();
        frame_in[..len].copy_from_slice(chunk);
        if len < FRAME {
            frame_in[len..].fill(0.0);
        }
        state.process_frame(&mut frame_out, &frame_in);
        out.extend(frame_out[..len].iter().map(|&s| s / 32768.0));
    }
    out
}
