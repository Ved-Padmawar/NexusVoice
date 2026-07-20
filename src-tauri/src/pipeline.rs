//! Transcription pipeline: the whole recording is decoded once on release.
//!
//! Two mid-recording strategies were measured against this and both lost.
//! Sliding windows reconciled by a `LocalAgreement` stabilizer scored ~4x the
//! word error rate of a single decode (35.95% vs 9.37%, `LibriSpeech` dev-clean,
//! `base.en`). Decoding complete VAD-delimited segments fixed that but still lost
//! (11.49% vs 9.05%) at ~29% more decode time: a segment boundary denies whisper
//! the context it uses to choose words.

use std::sync::{Arc, Mutex};

use crate::inference::WhisperEngine;

/// VAD frame size at 16 kHz (Silero V5 constraint).
const VAD_CHUNK_16K: usize = 512;
/// Prob ≥ this starts a speech region (Silero default).
const VAD_SPEECH_ENTER: f32 = 0.5;
/// Frames kept before the first speech frame (Silero `speech_pad_ms` ≈ 30ms).
const VAD_PAD_FRAMES: usize = 1;

/// Trim silence, then decode the recording in a single pass.
pub fn transcribe_recording(
    buffer: &[f32],
    native_rate: u32,
    engine: &Arc<Mutex<WhisperEngine>>,
    beam_size: i32,
) -> String {
    if native_rate == 0 || buffer.is_empty() {
        return String::new();
    }

    let at_16k = crate::preprocess::to_16k_denoised(buffer, native_rate);
    let speech = trim_silence(&at_16k);
    let leveled = crate::preprocess::normalize_level(speech);
    if leveled.is_empty() {
        return String::new();
    }

    let Ok(guard) = engine.lock() else {
        log::error!("WhisperEngine mutex poisoned during transcription");
        return String::new();
    };
    match guard.transcribe(&leveled, "", beam_size) {
        Ok(t) => t.trim().to_string(),
        Err(e) => {
            log::warn!("transcription failed: {e}");
            String::new()
        }
    }
}

/// Trim the silent lead-in from a 16 kHz buffer.
///
/// The tail is never trimmed: Silero is a streaming model whose confidence drifts
/// over a long buffer, so using it to mark the *end* of speech silently truncated
/// real dictation. Interior pauses are left alone — closing them would splice
/// apart words the speaker separated.
fn trim_silence(samples: &[f32]) -> &[f32] {
    use voice_activity_detector::{IteratorExt, VoiceActivityDetector};

    let Ok(mut vad) = VoiceActivityDetector::builder()
        .sample_rate(16_000)
        .chunk_size(VAD_CHUNK_16K)
        .build()
    else {
        return samples;
    };

    let predictions: Vec<f32> = samples
        .iter()
        .copied()
        .predict(&mut vad)
        .map(|(_, prob)| prob)
        .collect();

    let Some(first) = predictions.iter().position(|&p| p >= VAD_SPEECH_ENTER) else {
        return samples; // no confident speech — hand whisper the audio anyway
    };

    let start = first.saturating_sub(VAD_PAD_FRAMES) * VAD_CHUNK_16K;
    if start >= samples.len() {
        return samples;
    }
    &samples[start..]
}
