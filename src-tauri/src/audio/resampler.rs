use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

// Input is processed in fixed-size chunks.
// chunk_size must be > sinc_len / 2; 1024 is a safe default.
const CHUNK_SIZE: usize = 1024;

/// Sinc resampler whose filter state carries across [`Self::push`] calls.
pub struct StreamResampler {
    resampler: Async<f32>,
    ratio: f64,
    pending: Vec<f32>,
    delay_left: usize,
    frames_in: usize,
    frames_out: usize,
}

impl StreamResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Result<Self, String> {
        let ratio = f64::from(to_rate) / f64::from(from_rate);
        let params = SincInterpolationParameters {
            sinc_len: 64,
            // ≤0.85 recommended for downsampling to avoid aliasing near Nyquist
            f_cutoff: Some(0.85),
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        let resampler = Async::<f32>::new_sinc(
            ratio,
            2.0,
            &params,
            CHUNK_SIZE,
            1, // mono
            FixedAsync::Input,
        )
        .map_err(|e| format!("resampler {from_rate}→{to_rate} Hz rejected: {e}"))?;
        Ok(Self {
            delay_left: resampler.output_delay(),
            resampler,
            ratio,
            pending: Vec::new(),
            frames_in: 0,
            frames_out: 0,
        })
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        self.frames_in += samples.len();
        self.pending.extend_from_slice(samples);

        let mut out = Vec::new();
        let chunk = self.resampler.input_frames_next();
        let pending = std::mem::take(&mut self.pending);
        let mut chunks = pending.chunks_exact(chunk);
        for input in &mut chunks {
            self.step(input.to_vec(), &mut out)?;
        }
        self.pending = chunks.remainder().to_vec();
        Ok(out)
    }

    /// Flushes the filter; the resampler is spent afterwards.
    pub fn finish(&mut self) -> Result<Vec<f32>, String> {
        let chunk = self.resampler.input_frames_next();
        let mut out = Vec::new();
        if !self.pending.is_empty() {
            let mut input = std::mem::take(&mut self.pending);
            input.resize(chunk, 0.0);
            self.step(input, &mut out)?;
        }

        let expected = self.expected_frames();
        while self.frames_out < expected {
            self.step(vec![0.0; chunk], &mut out)?;
        }
        out.truncate(out.len().saturating_sub(self.frames_out - expected));
        Ok(out)
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn expected_frames(&self) -> usize {
        (self.frames_in as f64 * self.ratio).round() as usize
    }

    fn step(&mut self, chunk: Vec<f32>, out: &mut Vec<f32>) -> Result<(), String> {
        let frames = chunk.len();
        let input = [chunk];
        let adapter = SequentialSliceOfVecs::new(&input, 1, frames)
            .map_err(|e| format!("resampler input adapter rejected {frames} frames: {e}"))?;

        let capacity = self.resampler.output_frames_max();
        let mut buffer = [vec![0.0f32; capacity]];
        let mut output = SequentialSliceOfVecs::new_mut(&mut buffer, 1, capacity)
            .map_err(|e| format!("resampler output adapter rejected {capacity} frames: {e}"))?;
        let (_, produced) = self
            .resampler
            .process_into_buffer(&adapter, &mut output, None)
            .map_err(|e| format!("resampler error: {e}"))?;

        let skip = self.delay_left.min(produced);
        self.delay_left -= skip;
        self.frames_out += produced - skip;
        out.extend_from_slice(&buffer[0][skip..produced]);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/audio/resampler.rs"]
mod tests;
