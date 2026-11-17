//! The format saved recordings are kept in: 16 kHz mono 16-bit WAV. Lossless
//! at the rate the speech models take, so re-transcribing hears what dictation
//! did, for about 1.9 MB a minute.

use std::io::Cursor;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

pub const SAMPLE_RATE: u32 = 16_000;

const SPEC: WavSpec = WavSpec {
    channels: 1,
    sample_rate: SAMPLE_RATE,
    bits_per_sample: 16,
    sample_format: SampleFormat::Int,
};

pub fn encode(samples: &[f32]) -> Result<Vec<u8>, String> {
    let fail = |e: hound::Error| format!("couldn't encode audio: {e}");
    let mut bytes = Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    let mut writer = WavWriter::new(&mut bytes, SPEC).map_err(fail)?;
    for sample in samples {
        #[allow(clippy::cast_possible_truncation)] // clamped to the i16 range
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        writer.write_sample(pcm).map_err(fail)?;
    }
    writer.finalize().map_err(fail)?;
    Ok(bytes.into_inner())
}

/// Samples of a file written by [`encode`].
pub fn decode(bytes: &[u8]) -> Result<Vec<f32>, String> {
    let fail = |e: hound::Error| format!("couldn't decode audio: {e}");
    WavReader::new(Cursor::new(bytes))
        .map_err(fail)?
        .samples::<i16>()
        .map(|s| s.map(|s| f32::from(s) / f32::from(i16::MAX)).map_err(fail))
        .collect()
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss, clippy::float_cmp)]
#[path = "../../tests/unit/recordings/wav.rs"]
mod tests;
