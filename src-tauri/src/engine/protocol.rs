//! Wire protocol between the app and its transcribe.cpp worker process.
//!
//! Strict request/response: every [`Request`] gets exactly one [`Response`].
//! Each message is one frame: `[u32 json_len][json][u32 pcm_len][pcm bytes]`
//! (lengths little-endian, PCM as little-endian f32). JSON, not a binary
//! format: transcribe-cpp's serde impls round-trip NaN confidences through
//! `null`, which only a self-describing format handles.

use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use transcribe_cpp::{
    Backend, Capabilities, RunOptions, StreamOptions, StreamText, StreamUpdate, Transcript,
};

/// Refuse frames larger than this: a garbage length must not turn into a
/// multi-gigabyte allocation. One hour of 16 kHz f32 PCM is ~230 MB.
const MAX_SECTION_BYTES: usize = 1 << 30;
const PCM_BYTES_PER_MINUTE: usize = 16_000 * 4 * 60;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// First request to every worker; answered once backend init is done,
    /// with an error if no compute device registered.
    Hello,
    Load {
        path: PathBuf,
        backend: Backend,
    },
    /// Followed by the PCM to transcribe.
    Run {
        options: RunOptions,
    },
    StreamBegin {
        run: RunOptions,
        stream: StreamOptions,
    },
    /// Followed by the PCM frame to feed.
    Feed,
    Finalize,
    StreamReset,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Hello,
    Loaded(LoadedInfo),
    Transcript {
        transcript: Transcript,
        /// The decode stopped early (length budget or a repetition loop), so
        /// the transcript may be missing its end.
        truncated: bool,
    },
    Fed {
        update: StreamUpdate,
        /// Present only when the committed or tentative text changed.
        text: Option<StreamText>,
    },
    Finalized {
        text: StreamText,
        truncated: bool,
    },
    Ok,
    Error(String),
    /// A run, feed or finalize failed in the compute backend (e.g. a lost
    /// GPU device). The worker answered, but its backend may be unusable.
    BackendError(String),
}

/// What the parent needs to know about a loaded model, read once at load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedInfo {
    pub arch: String,
    /// Kind of the device the model bound to: "cuda", "vulkan" or "cpu".
    pub backend: String,
    /// Label of that device, for logs.
    pub device: String,
    /// Whether that device is a GPU (a crash there may be the GPU's fault).
    pub on_gpu: bool,
    pub capabilities: Capabilities,
}

/// Encode one frame. Refuses a section the reader would refuse, so an
/// oversized request fails cleanly here instead of killing the worker (which
/// would read as a crash and be blamed on the GPU).
pub fn encode_message<T: Serialize>(message: &T, pcm: Option<&[f32]>) -> io::Result<Vec<u8>> {
    let json = serde_json::to_vec(message).map_err(io::Error::other)?;
    check_section_len(json.len())?;
    let pcm = pcm.unwrap_or(&[]);
    if check_section_len(pcm.len() * 4).is_err() {
        return Err(io::Error::other(format!(
            "audio too long for one transcription: {} minutes (limit {} minutes)",
            pcm.len() * 4 / PCM_BYTES_PER_MINUTE,
            MAX_SECTION_BYTES / PCM_BYTES_PER_MINUTE,
        )));
    }
    let mut frame = Vec::with_capacity(8 + json.len() + pcm.len() * 4);
    frame.extend_from_slice(&section_len(json.len()).to_le_bytes());
    frame.extend_from_slice(&json);
    frame.extend_from_slice(&section_len(pcm.len() * 4).to_le_bytes());
    for sample in pcm {
        frame.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(frame)
}

pub fn write_message<T: Serialize>(
    w: &mut impl Write,
    message: &T,
    pcm: Option<&[f32]>,
) -> io::Result<()> {
    w.write_all(&encode_message(message, pcm)?)?;
    w.flush()
}

/// Read one frame. `Ok(None)` only on a clean EOF before the first byte of a
/// frame; a frame cut off part-way is an error.
pub fn read_message<T: for<'de> Deserialize<'de>>(
    r: &mut impl Read,
) -> io::Result<Option<(T, Vec<f32>)>> {
    let mut len = [0u8; 4];
    let mut filled = 0;
    while filled < len.len() {
        match r.read(&mut len[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let json = read_section_body(r, len)?;
    let message = serde_json::from_slice(&json).map_err(io::Error::other)?;
    let pcm_bytes = read_section(r)?;
    if pcm_bytes.len() % 4 != 0 {
        return Err(io::Error::other(
            "PCM section is not a whole number of f32s",
        ));
    }
    let pcm = pcm_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    Ok(Some((message, pcm)))
}

fn read_section(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    read_section_body(r, len)
}

fn read_section_body(r: &mut impl Read, len: [u8; 4]) -> io::Result<Vec<u8>> {
    let len = u32::from_le_bytes(len) as usize;
    check_section_len(len)?;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Also keeps every section length within the frame's `u32` length prefix.
fn check_section_len(len: usize) -> io::Result<()> {
    if len > MAX_SECTION_BYTES {
        return Err(io::Error::other(format!("frame section too large: {len}")));
    }
    Ok(())
}

/// A section length already checked against [`MAX_SECTION_BYTES`].
#[allow(clippy::cast_possible_truncation)] // MAX_SECTION_BYTES fits u32
fn section_len(len: usize) -> u32 {
    len as u32
}

#[cfg(test)]
#[path = "../../tests/unit/engine/protocol.rs"]
mod tests;
