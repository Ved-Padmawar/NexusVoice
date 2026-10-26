//! Recording and delivery: capture while the user speaks, then decode, clean
//! up, save and paste the transcript.

mod finalize;
pub mod recorder;
mod workers;

pub use recorder::{Mode, RecordError, Recorder};
