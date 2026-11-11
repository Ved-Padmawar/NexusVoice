//! Background work for the recording in progress: the waveform feed and the
//! decoders that run while the user speaks.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::audio::{waveform::BANDS, MicStream, WaveformMeter};
use crate::bridge::events::{
    PillWaveform, TranscriptionPartial, TranscriptionPartialEnd, TranscriptionWarning,
};
use crate::models::cache::EngineCache;
use crate::models::decode::{Route, StreamSession, Streamed, StreamingSession};
use crate::models::preprocess::To16k;
use crate::sync::lock_recovering;

/// Poll cadence of the stream workers. Decode frequency is governed by the
/// pipeline's minimum-new-audio gate, not this; polling is just the check.
const STREAM_POLL: Duration = Duration::from_millis(200);

/// Waveform frame interval, ~30 Hz.
const WAVEFORM_TICK: Duration = Duration::from_millis(33);

/// Counts recordings. A worker that outlives its recording holds a stale
/// ticket: it stops, and anything it still produces is discarded.
#[derive(Clone, Default)]
pub struct Generation(Arc<AtomicU64>);

impl Generation {
    /// Start a recording; every earlier ticket becomes stale.
    pub fn begin(&self) -> Ticket {
        let id = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Ticket {
            id,
            latest: Arc::clone(&self.0),
        }
    }

    /// Make every ticket stale, for a worker the recording stopped waiting on.
    pub fn retire(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct Ticket {
    id: u64,
    latest: Arc<AtomicU64>,
}

impl Ticket {
    pub fn is_current(&self) -> bool {
        self.latest.load(Ordering::SeqCst) == self.id
    }
}

/// State the capture callback and the workers share with the recorder.
#[derive(Clone)]
pub struct Capture {
    pub generation: Generation,
    /// Set while the user is recording; the workers run until it drops.
    pub capturing: Arc<AtomicBool>,
    pub audio: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: Arc<Mutex<u32>>,
    pub waveform: Arc<WaveformMeter>,
    pub mic: Arc<MicStream>,
    /// Growing-window decode state; created at start, taken by finalize.
    pub session: Arc<Mutex<Option<StreamingSession>>>,
    /// The saved language when the recording started.
    pub language: Arc<Mutex<Option<String>>>,
    /// Transcript a streaming-native model produced. `None` means finalize
    /// decodes the buffer itself.
    pub streamed: Arc<Mutex<Option<Streamed>>>,
    /// Signalled when the streaming-native worker is done, so finalize does
    /// not read `streamed` before its tail is in.
    pub stream_done: Arc<(Mutex<bool>, Condvar)>,
}

pub fn spawn_all(app: &AppHandle, capture: &Capture, ticket: &Ticket) {
    *lock_recovering(&capture.stream_done.0) = false;
    spawn_waveform(app, capture.clone(), ticket.clone());
    spawn_local_agreement(app, capture.clone(), ticket.clone());
    spawn_native_stream(app, capture.clone(), ticket.clone());
}

/// Push one live-transcript frame to the pill, skipping empty ones and any
/// from a recording that has been replaced.
fn emit_partial(app: &AppHandle, ticket: &Ticket, (committed, tentative): (String, String)) {
    if !ticket.is_current() || (committed.is_empty() && tentative.is_empty()) {
        return;
    }
    let _ = TranscriptionPartial {
        committed,
        tentative,
    }
    .emit_to(app, "pill");
}

/// Signals on drop, so every early return unblocks finalize. A stale worker
/// stays silent; the flag belongs to the newer recording.
struct StreamDone {
    flag: Arc<(Mutex<bool>, Condvar)>,
    ticket: Ticket,
}

impl Drop for StreamDone {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.flag;
        let mut done = lock_recovering(lock);
        if self.ticket.is_current() {
            *done = true;
            cvar.notify_all();
        }
    }
}

/// Feed captured audio into a streaming model's own session, stashing the
/// finished transcript for finalize to collect. The engine may still be
/// loading, so this worker and the growing-window one both start, and each
/// exits if the other owns the recording.
fn spawn_native_stream(app: &AppHandle, capture: Capture, ticket: Ticket) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _done = StreamDone {
            flag: Arc::clone(&capture.stream_done),
            ticket: ticket.clone(),
        };
        let cache = app.state::<EngineCache>();

        let engine = loop {
            if !capture.capturing.load(Ordering::SeqCst) || !ticket.is_current() {
                return;
            }
            if let Some(engine) = cache.loaded() {
                break engine;
            }
            std::thread::sleep(STREAM_POLL);
        };

        let Ok(mut guard) = engine.lock() else {
            return;
        };
        if Route::for_engine(&guard) != Route::Streaming {
            return;
        }
        let options = guard.stream_run_options(lock_recovering(&capture.language).as_deref());
        let Ok(mut stream) = StreamSession::begin(guard.session_mut(), &options) else {
            return;
        };
        log::info!("decoding via {}", Route::Streaming.as_str());

        // Built on the first audio: the mic sets its rate when it opens.
        let mut converter: Option<To16k> = None;

        // Capture outlives `capturing` by the post-roll. Flags are read before
        // the buffer, so the pass that sees both off has everything.
        let mut fed = 0usize;
        loop {
            let capturing = capture.capturing.load(Ordering::SeqCst) || capture.mic.is_armed();
            let chunk = {
                let buf = lock_recovering(&capture.audio);
                // Under the lock `prepare` clears with, so `fed` stays in range.
                if !ticket.is_current() {
                    break;
                }
                let chunk = buf[fed..].to_vec();
                fed = buf.len();
                chunk
            };
            if converter.is_none() && !chunk.is_empty() {
                match To16k::new(*lock_recovering(&capture.sample_rate)) {
                    Ok(built) => converter = Some(built),
                    Err(e) => stream.drop_chunk(&e),
                }
            }
            if let Some(converter) = converter.as_mut() {
                let prepared = if capturing {
                    converter.push(&chunk)
                } else {
                    converter.push(&chunk).and_then(|mut head| {
                        head.extend(converter.finish()?);
                        Ok(head)
                    })
                };
                match prepared {
                    Ok(prepared) => {
                        if !prepared.is_empty() && stream.feed(&prepared) {
                            emit_partial(&app, &ticket, stream.partial());
                        }
                    }
                    Err(e) => stream.drop_chunk(&e),
                }
            }
            if !capturing {
                break;
            }
            std::thread::sleep(STREAM_POLL);
        }

        if ticket.is_current() {
            let _ = TranscriptionPartialEnd.emit(&app);
        }
        let text = stream.finalize();
        let truncated = guard.was_truncated();
        let mut streamed = lock_recovering(&capture.streamed);
        if ticket.is_current() {
            *streamed = text.map(|text| Streamed { text, truncated });
        }
    });
}

/// Feed the growing buffer to the `StreamingSession` so finalize only has the
/// tail left to decode. Without a loaded engine, finalize decodes everything
/// in one pass.
fn spawn_local_agreement(app: &AppHandle, capture: Capture, ticket: Ticket) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cache = app.state::<EngineCache>();
        let language = lock_recovering(&capture.language).clone();
        let mut route_logged = false;
        while capture.capturing.load(Ordering::SeqCst) && ticket.is_current() {
            std::thread::sleep(STREAM_POLL);
            let Some(engine) = cache.loaded() else {
                continue;
            };
            if engine
                .lock()
                .is_ok_and(|e| Route::for_engine(&e) == Route::Streaming)
            {
                break;
            }

            // Lock order everywhere is session, then audio buffer. Finalize
            // takes the session first too, so it waits out an in-flight decode
            // and this worker exits on the emptied slot.
            let mut slot = lock_recovering(&capture.session);
            // Checked under the lock `prepare` takes to replace the session.
            if !ticket.is_current() {
                break;
            }
            let Some(session) = slot.as_mut() else {
                break;
            };
            let rate = *lock_recovering(&capture.sample_rate);
            let Some(new_audio) = ({
                let buf = lock_recovering(&capture.audio);
                session
                    .would_decode(buf.len(), rate)
                    .then(|| buf[session.fed().min(buf.len())..].to_vec())
            }) else {
                continue;
            };

            if !route_logged {
                log::info!("decoding via {}", Route::LocalAgreement.as_str());
                route_logged = true;
            }

            let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.poll(&new_audio, rate, &engine, language.as_deref());
            }));
            if polled.is_err() {
                // Drop the session too, so finalize falls back to a clean
                // single-pass decode on a fresh engine.
                log::error!("engine panicked during streaming; discarding it");
                *slot = None;
                cache.discard();
                break;
            }
            emit_partial(&app, &ticket, session.partial());
        }
        if ticket.is_current() {
            let _ = TranscriptionPartialEnd.emit(&app);
        }
    });
}

/// Emit waveform levels while recording. The same tick warns once if the mic
/// dies, so the user stops talking into it.
fn spawn_waveform(app: &AppHandle, capture: Capture, ticket: Ticket) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(WAVEFORM_TICK);
        let mut warned = false;
        while capture.capturing.load(Ordering::SeqCst) && ticket.is_current() {
            tick.tick().await;
            let _ = PillWaveform(capture.waveform.levels()).emit_to(&app, "pill");
            if !warned && capture.mic.is_dead() {
                warned = true;
                let _ = TranscriptionWarning(
                    "Your microphone stopped. Stop recording to keep what was captured."
                        .to_string(),
                )
                .emit(&app);
            }
        }
        if ticket.is_current() {
            let _ = PillWaveform([0.0; BANDS]).emit_to(&app, "pill");
        }
    });
}

#[cfg(test)]
#[path = "../../tests/unit/session/workers.rs"]
mod tests;
