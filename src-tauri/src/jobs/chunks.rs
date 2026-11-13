//! Where a long recording is cut for decoding: about every 20 seconds, at the
//! quietest moment near the end, so a cut rarely lands inside a word.

use super::RATE;

const TARGET: usize = RATE as usize * 20;
/// How far back from the target a quieter cut is looked for.
const SEARCH: usize = RATE as usize * 4;
/// Granularity of the search: 50 ms.
const BLOCK: usize = RATE as usize / 20;
/// A tail shorter than this joins the chunk before it; decoders want at least a second.
const MIN_TAIL: usize = RATE as usize * 2;

/// End offsets of each chunk, the last being `samples.len()`.
pub fn ends(samples: &[f32]) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut start = 0;
    while samples.len() - start > TARGET + MIN_TAIL {
        let target = start + TARGET;
        let end = quietest_block(&samples[target - SEARCH..target]) + target - SEARCH;
        ends.push(end);
        start = end;
    }
    ends.push(samples.len());
    ends
}

/// Offset of the start of the quietest block in `window`.
fn quietest_block(window: &[f32]) -> usize {
    window
        .as_chunks::<BLOCK>()
        .0
        .iter()
        .enumerate()
        .map(|(i, block)| (i * BLOCK, block.iter().map(|s| s * s).sum::<f32>()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(window.len(), |(offset, _)| offset)
}

#[cfg(test)]
#[path = "../../tests/unit/jobs/chunks.rs"]
mod tests;
