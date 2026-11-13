//! Audio files to the 16 kHz mono samples the speech models take: WAV, MP3,
//! M4A (AAC), FLAC and OGG through symphonia, Ogg Opus through
//! libopus.

use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia_adapter_libopus::OpusDecoder;

use crate::models::preprocess::To16k;

const UNREADABLE: &str = "That file isn't audio NexusVoice can read.";

/// How long the file plays, when it says so without decoding it.
pub fn duration_seconds(path: &Path) -> Result<Option<f64>, String> {
    let format = open(path)?;
    let Some(track) = format.default_track() else {
        return Err(UNREADABLE.to_string());
    };
    let params = &track.codec_params;
    #[allow(clippy::cast_precision_loss)] // frame counts of real files fit f64 exactly
    Ok(params
        .n_frames
        .zip(params.sample_rate)
        .map(|(frames, rate)| frames as f64 / f64::from(rate)))
}

/// Decode the whole file, refusing one longer than `limit`.
pub fn decode_16k(path: &Path, limit: Duration) -> Result<Vec<f32>, String> {
    let max_samples = usize::try_from(limit.as_secs() * 16_000).unwrap_or(usize::MAX);
    let mut format = open(path)?;
    let (mut track_id, mut decoder) = default_decoder(&*format, path)?;
    // With the rate it converts from: each stream of a chained file has its own.
    let mut converter: Option<(u32, To16k)> = None;
    let mut samples_16k = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            // The next stream of a chained file (Ogg): it brings its own track.
            Err(SymphoniaError::ResetRequired) => {
                (track_id, decoder) = default_decoder(&*format, path)?;
                continue;
            }
            Err(e) => return Err(read_failure(path, &e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // One damaged frame costs a blip, not the file.
            Err(SymphoniaError::DecodeError(e)) => {
                log::debug!("skipping a damaged frame in {}: {e}", path.display());
                continue;
            }
            Err(e) => return Err(read_failure(path, &e)),
        };
        let spec = *buffer.spec();
        let channels = spec.channels.count().max(1);
        let mut interleaved = SampleBuffer::<f32>::new(buffer.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(buffer);
        #[allow(clippy::cast_precision_loss)] // channel counts are tiny
        let mono: Vec<f32> = interleaved
            .samples()
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        if let Some((_, mut previous)) = converter.take_if(|(rate, _)| *rate != spec.rate) {
            samples_16k.extend(previous.finish()?);
        }
        let (_, converter) = match &mut converter {
            Some(converter) => converter,
            slot => slot.insert((spec.rate, To16k::new(spec.rate)?)),
        };
        samples_16k.extend(converter.push(&mono)?);
        if samples_16k.len() > max_samples {
            return Err(format!(
                "Files longer than {} minutes can't be transcribed.",
                limit.as_secs() / 60
            ));
        }
    }
    if let Some((_, mut converter)) = converter {
        samples_16k.extend(converter.finish()?);
    }
    if samples_16k.is_empty() {
        return Err(UNREADABLE.to_string());
    }
    Ok(samples_16k)
}

/// The id of `format`'s default track, and a decoder for it.
fn default_decoder(
    format: &dyn FormatReader,
    path: &Path,
) -> Result<(u32, Box<dyn Decoder>), String> {
    let track = format.default_track().ok_or(UNREADABLE)?;
    let decoder = codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| {
            log::warn!("no decoder for {}: {e}", path.display());
            UNREADABLE.to_string()
        })?;
    Ok((track.id, decoder))
}

/// symphonia's codecs, plus Opus, which only the libopus adapter decodes.
fn codecs() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(|| {
        let mut registry = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        registry.register_all::<OpusDecoder>();
        registry
    })
}

fn open(path: &Path) -> Result<Box<dyn FormatReader>, String> {
    let file = File::open(path).map_err(|e| {
        log::warn!("could not open {}: {e}", path.display());
        "Couldn't open that file.".to_string()
    })?;
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default()),
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            log::warn!("could not probe {}: {e}", path.display());
            UNREADABLE.to_string()
        })?;
    Ok(probed.format)
}

fn read_failure(path: &Path, e: &SymphoniaError) -> String {
    log::warn!("could not read {}: {e}", path.display());
    UNREADABLE.to_string()
}

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp
)]
#[path = "../../tests/unit/audio/file.rs"]
mod tests;
