use std::sync::OnceLock;

use nnnoiseless::{DenoiseState, RnnModel};

/// The `RNNoise` weights, built once and shared by every `DenoiseState`.
///
/// `DenoiseState::new()` clones the whole model per call, which dominates the
/// cost of denoising a short recording. The state itself stays per-call: it
/// carries noise history, so reusing it would leak one take's profile into the next.
static MODEL: OnceLock<RnnModel> = OnceLock::new();

/// Apply `RNNoise` frame-by-frame. Expects samples at 48 kHz.
/// Input and output are f32 in [-1.0, 1.0].
pub fn denoise(samples: &[f32]) -> Vec<f32> {
    const FRAME: usize = DenoiseState::FRAME_SIZE;
    let mut state = DenoiseState::with_model(MODEL.get_or_init(RnnModel::default));
    let mut out = Vec::with_capacity(samples.len());

    let mut frame_in = [0.0f32; FRAME];
    let mut frame_out = [0.0f32; FRAME];

    // nnnoiseless expects f32 in i16 range (-32768..32768). Scaling per frame
    // avoids a full-length intermediate copy.
    for chunk in samples.chunks(FRAME) {
        let len = chunk.len();
        for (dst, &src) in frame_in.iter_mut().zip(chunk) {
            *dst = src * 32768.0;
        }
        if len < FRAME {
            frame_in[len..].fill(0.0);
        }
        state.process_frame(&mut frame_out, &frame_in);
        out.extend(frame_out[..len].iter().map(|&s| s / 32768.0));
    }
    out
}
