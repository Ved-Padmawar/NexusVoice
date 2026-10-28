//! Streaming decode for models that own their session state.

use transcribe_cpp::{RunOptions, Stream, StreamOptions};

pub struct Streamed {
    pub text: String,
    pub truncated: bool,
}

/// A live streaming decode. Borrows the session for its lifetime, so the owning
/// engine cannot run a single-shot decode concurrently.
pub struct StreamSession<'a> {
    stream: Stream<'a>,
    /// A chunk failed to feed, so the streamed text has a hole in it.
    dropped_chunk: bool,
}

impl<'a> StreamSession<'a> {
    /// Begin streaming on `session`. Default options let each family apply the
    /// commit strategy it was tuned for.
    pub fn begin(
        session: &'a mut transcribe_cpp::Session,
        run: &RunOptions,
    ) -> Result<Self, String> {
        let stream = session
            .stream(run, &StreamOptions::default())
            .map_err(|e| format!("failed to begin stream: {e}"))?;
        Ok(Self {
            stream,
            dropped_chunk: false,
        })
    }

    /// Feed one chunk of 16 kHz mono f32 audio, returning `true` when the text
    /// changed. A failed chunk doesn't abort the session; [`Self::finalize`]
    /// then hands the whole recording back for a single-shot decode.
    pub fn feed(&mut self, pcm: &[f32]) -> bool {
        match self.stream.feed(pcm) {
            Ok(update) => update.committed_changed || update.tentative_changed,
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

        let text = self.stream.text();
        if !text.committed.is_empty() {
            return (text.committed, text.tentative);
        }

        let words: Vec<&str> = text.full.split_whitespace().collect();
        if words.len() <= IN_FLUX_WORDS {
            return (String::new(), text.full);
        }
        let split = words.len() - IN_FLUX_WORDS;
        (
            words[..split].join(" "),
            format!(" {}", words[split..].join(" ")),
        )
    }

    /// Flush the stream and return the complete transcript. `None` means the
    /// caller should fall back to a single-shot decode of the buffered audio.
    pub fn finalize(mut self) -> Option<String> {
        if self.dropped_chunk {
            log::warn!("stream dropped a chunk — decoding the full recording instead");
            return None;
        }
        match self.stream.finalize() {
            Ok(_) => Some(self.stream.text().full),
            Err(e) => {
                log::error!("stream finalize failed: {e}");
                None
            }
        }
    }
}
