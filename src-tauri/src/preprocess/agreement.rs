//! LocalAgreement-2 stabilizer.
//!
//! Confirms words that two consecutive hypotheses agree on over the same audio
//! interval. Everything after the agreed prefix stays tentative and is re-decoded
//! next chunk. Replaces text-only stitching, which cannot tell a boundary duplicate
//! from a phrase the user genuinely said twice — the same words at different audio
//! times are two real occurrences.

use crate::inference::{Hypothesis, Word};

/// Words at the newest audio edge lack right context and are the least stable, so
/// they are held back from confirmation until the next decode extends past them.
const EDGE_GUARD_MS: i64 = 200;

/// Two words match if they are the same lexical token at the same moment. The
/// window tolerates whisper's timestamp jitter between decodes of the same audio.
const TIME_TOLERANCE_MS: i64 = 400;

#[derive(Debug, Default)]
pub struct Stabilizer {
    /// Immutable — already agreed by two hypotheses.
    confirmed: Vec<Word>,
    /// Latest hypothesis past the commit watermark; replaceable.
    pending: Vec<Word>,
}

impl Stabilizer {
    pub const fn new() -> Self {
        Self {
            confirmed: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// End of the last confirmed word — the commit watermark. Audio and prompt text
    /// before this point are settled.
    pub fn commit_watermark_ms(&self) -> i64 {
        self.confirmed.last().map_or(0, |w| w.end_ms)
    }

    /// Confirmed words whose audio ended before `cutoff_ms`.
    ///
    /// The prompt must not describe audio still inside the decode window: feeding
    /// the decoder text it is about to hear again invites it to repeat that text
    /// rather than transcribe. Callers pass the window start as the cutoff.
    pub fn prompt_before(&self, cutoff_ms: i64, max_words: usize) -> String {
        let eligible: Vec<&Word> = self
            .confirmed
            .iter()
            .filter(|w| w.end_ms <= cutoff_ms)
            .collect();
        let start = eligible.len().saturating_sub(max_words);
        eligible[start..]
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Fold in a decode of the audio starting at `window_start_ms`.
    ///
    /// A sliding window only re-hears the overlap, so it says nothing about earlier
    /// speech: those pending words are promoted, not compared.
    pub fn observe(&mut self, hypothesis: &Hypothesis, window_start_ms: i64) {
        let watermark = self.commit_watermark_ms();
        let fresh: Vec<Word> = hypothesis
            .words
            .iter()
            .filter(|w| w.start_ms >= watermark)
            .cloned()
            .collect();

        if fresh.is_empty() {
            // An empty or fully-stale decode is not evidence of silence: keep the
            // pending buffer so the next decode can still confirm against it.
            return;
        }

        // What this decode couldn't re-hear is settled; what it could is contested.
        let (settled, contested): (Vec<Word>, Vec<Word>) = self
            .pending
            .drain(..)
            .partition(|w| w.end_ms <= window_start_ms);
        self.confirmed.extend(settled);

        // Words at the newest audio edge lack right context — hold them back.
        let edge = fresh.last().map_or(0, |w| w.end_ms) - EDGE_GUARD_MS;
        let stable_zone: Vec<&Word> = fresh.iter().filter(|w| w.end_ms <= edge).collect();

        let agreed = common_prefix(&contested, &stable_zone);
        self.confirmed.extend(agreed.into_iter().cloned());

        let new_watermark = self.commit_watermark_ms();
        self.pending = fresh
            .into_iter()
            .filter(|w| w.start_ms >= new_watermark)
            .collect();
    }

    /// Release: commit everything tentative and return the full transcript.
    ///
    /// Nothing further will arrive to confirm the pending words, so waiting for
    /// agreement would silently drop the end of the user's dictation.
    pub fn finish(mut self) -> String {
        self.confirmed.append(&mut self.pending);
        Hypothesis {
            words: self.confirmed,
        }
        .text()
    }
}

/// Longest run where both hypotheses report the same word at the same time.
fn common_prefix<'a>(prev: &[Word], next: &[&'a Word]) -> Vec<&'a Word> {
    let mut agreed = Vec::new();
    for (a, b) in prev.iter().zip(next.iter()) {
        if a.normalized() != b.normalized() || (a.start_ms - b.start_ms).abs() > TIME_TOLERANCE_MS {
            break;
        }
        agreed.push(*b);
    }
    agreed
}

#[cfg(test)]
#[path = "../../tests/unit/preprocess/agreement.rs"]
mod tests;
