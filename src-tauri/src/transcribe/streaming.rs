//! Streaming decode for models that own their session state.

use transcribe_cpp::{RunOptions, Stream, StreamOptions};

/// A live streaming decode. Borrows the session for its lifetime, so the owning
/// engine cannot run a single-shot decode concurrently.
pub struct StreamSession<'a> {
    stream: Stream<'a>,
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
        Ok(Self { stream })
    }

    /// Feed one chunk of 16 kHz mono f32 audio, returning `true` when the text
    /// changed. A failed chunk is skipped rather than aborting the session —
    /// dropping one chunk is recoverable, losing the dictation is not.
    pub fn feed(&mut self, pcm: &[f32]) -> bool {
        match self.stream.feed(pcm) {
            Ok(update) => update.committed_changed || update.tentative_changed,
            Err(e) => {
                log::warn!("stream feed failed: {e}");
                false
            }
        }
    }

    /// Flush the stream and return the complete transcript. `None` means the
    /// caller should fall back to a single-shot decode of the buffered audio.
    pub fn finalize(mut self) -> Option<String> {
        match self.stream.finalize() {
            Ok(_) => Some(self.stream.text().full),
            Err(e) => {
                log::error!("stream finalize failed: {e}");
                None
            }
        }
    }
}
