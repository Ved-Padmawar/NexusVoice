use crate::inference::{InferenceEngine, InferenceError};

#[derive(Debug, Clone)]
pub struct AudioInput {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct AudioPipelineConfig {
    pub target_sample_rate: u32,
    pub chunk_size_samples: usize,
}

impl AudioPipelineConfig {
    pub fn new(target_sample_rate: u32, chunk_ms: u32) -> Self {
        let chunk_size_samples =
            ((target_sample_rate as f64) * (chunk_ms as f64) / 1000.0).round() as usize;
        Self {
            target_sample_rate,
            chunk_size_samples: chunk_size_samples.max(1),
        }
    }
}

pub struct AudioPipeline {
    engine: Box<dyn InferenceEngine>,
    config: AudioPipelineConfig,
    buffer: Vec<f32>,
}

impl AudioPipeline {
    pub fn new(engine: Box<dyn InferenceEngine>, config: AudioPipelineConfig) -> Self {
        Self {
            engine,
            config,
            buffer: Vec::new(),
        }
    }

    pub fn process_input(&mut self, input: AudioInput) -> Result<Vec<Vec<f32>>, InferenceError> {
        let mono = to_mono(&input.samples, input.channels);
        let resampled = resample_linear(&mono, input.sample_rate, self.config.target_sample_rate);

        self.buffer.extend_from_slice(&resampled);

        let mut outputs = Vec::new();
        while self.buffer.len() >= self.config.chunk_size_samples {
            let chunk: Vec<f32> = self
                .buffer
                .drain(..self.config.chunk_size_samples)
                .collect();
            let result = self.engine.run(&chunk)?;
            outputs.push(result);
        }

        Ok(outputs)
    }

    #[allow(dead_code)]
    pub fn buffered_samples(&self) -> usize {
        self.buffer.len()
    }
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    let channels = channels as usize;
    let frame_count = samples.len() / channels;
    let mut mono = Vec::with_capacity(frame_count);

    for frame in 0..frame_count {
        let start = frame * channels;
        let mut sum = 0.0;
        for ch in 0..channels {
            sum += samples[start + ch];
        }
        mono.push(sum / channels as f32);
    }

    mono
}

fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f64 / source_rate as f64;
    let out_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    let max_index = samples.len() - 1;

    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = (i as f64) / ratio;
        let left = src_pos.floor() as usize;
        let right = (left + 1).min(max_index);
        let frac = (src_pos - left as f64) as f32;

        let value = samples[left] * (1.0 - frac) + samples[right] * frac;
        out.push(value);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::{ExecutionProvider, MockInferenceEngine};

    #[test]
    fn stereo_to_mono_average() {
        let input = AudioInput {
            sample_rate: 16000,
            channels: 2,
            samples: vec![1.0, -1.0, 0.5, 0.5],
        };

        let mono = to_mono(&input.samples, input.channels);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn resample_doubles_from_8k_to_16k() {
        let samples = vec![0.0, 1.0, 2.0, 3.0];
        let resampled = resample_linear(&samples, 8000, 16000);
        assert_eq!(resampled.len(), 8);
        assert!((resampled[0] - 0.0).abs() < 1e-6);
        assert!((resampled[7] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn chunking_calls_engine() {
        let engine = MockInferenceEngine::new(ExecutionProvider::Cpu, |input| Ok(input.to_vec()));
        let config = AudioPipelineConfig {
            target_sample_rate: 16000,
            chunk_size_samples: 4,
        };

        let mut pipeline = AudioPipeline::new(Box::new(engine), config);

        let input = AudioInput {
            sample_rate: 16000,
            channels: 1,
            samples: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        };

        let outputs = pipeline.process_input(input).expect("process");
        assert_eq!(outputs.len(), 2);
        assert_eq!(pipeline.buffered_samples(), 2);
        assert_eq!(outputs[0], vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(outputs[1], vec![5.0, 6.0, 7.0, 8.0]);
    }
}
