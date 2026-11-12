//! Streaming decode for models that own their session state.

use transcribe_cpp::StreamText;

use crate::engine::{Finalized, StreamHandle};

pub struct Streamed {
    pub text: String,
    pub truncated: bool,
}

/// A live streaming decode. Holds the engine's worker for its lifetime, so
/// no single-shot decode runs concurrently.
pub struct StreamSession<'a> {
    stream: StreamHandle<'a>,
    /// The text as of the last feed that changed it.
    text: StreamText,
    /// A chunk failed to feed, so the streamed text has a hole in it.
    dropped_chunk: bool,
}

impl<'a> StreamSession<'a> {
    pub fn new(stream: StreamHandle<'a>) -> Self {
        Self {
            stream,
            text: StreamText::default(),
            dropped_chunk: false,
        }
    }

    /// Feed one chunk of 16 kHz mono f32 audio, returning `true` when the text
    /// changed. A failed chunk doesn't abort the session; [`Self::finalize`]
    /// then hands the whole recording back for a single-shot decode.
    pub fn feed(&mut self, pcm: &[f32]) -> bool {
        // The stream's text is already incomplete; finalize won't use it.
        if self.dropped_chunk {
            return false;
        }
        match self.stream.feed(pcm) {
            Ok(Some(text)) => {
                self.text = text;
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.drop_chunk(&format!("stream feed failed: {e}"));
                false
            }
        }
    }

    /// Record a chunk that never reached the stream, so [`Self::finalize`]
    /// falls back to decoding the whole recording.
    pub fn drop_chunk(&mut self, reason: &str) {
        log::warn!("{reason}");
        self.dropped_chunk = true;
    }

    /// Transcript so far, split into settled and still-revising text. Models
    /// that leave `committed` empty fall back to holding only the tail in flux.
    pub fn partial(&self) -> (String, String) {
        const IN_FLUX_WORDS: usize = 4;

        let text = &self.text;
        if !text.committed.is_empty() {
            return (text.committed.clone(), text.tentative.clone());
        }

        let words: Vec<&str> = text.full.split_whitespace().collect();
        if words.len() <= IN_FLUX_WORDS {
            return (String::new(), text.full.clone());
        }
        let split = words.len() - IN_FLUX_WORDS;
        (
            words[..split].join(" "),
            format!(" {}", words[split..].join(" ")),
        )
    }

    /// Flush the stream and return the complete transcript. `None` means the
    /// caller should fall back to a single-shot decode of the buffered audio.
    pub fn finalize(self) -> Option<Streamed> {
        if self.dropped_chunk {
            log::warn!("stream dropped a chunk — decoding the full recording instead");
            return None;
        }
        self.stream
            .finalize()
            .map(|Finalized { text, truncated }| Streamed {
                text: text.full,
                truncated,
            })
    }
}
