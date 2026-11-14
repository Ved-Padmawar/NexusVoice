//! Background transcription of a History entry's audio: an imported file, or
//! a re-transcription. Jobs queue and run one at a time, in chunks, on an
//! engine of their own. Dictation goes first: a recording that starts cuts the
//! job's work, which is redone once the recording is done.

mod chunks;

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bridge::commands::dto::TranscriptResponse;
use crate::bridge::events::JobFinished;
use crate::data::dto::transcript::CreateTranscript;
use crate::data::repositories::transcript::TranscriptRepository;
use crate::data::rows::transcript::Transcript;
use crate::data::Database;
use crate::engine::{CancelMark, Canceller};
use crate::models::catalog::{self, ModelEntry};
use crate::models::decode::contains_speech;
use crate::models::library::ModelLibrary;
use crate::models::preprocess::normalize_level;
use crate::models::transcript::{join_words, strip_leading_dashes};
use crate::models::{Pass, TranscriptionEngine};
use crate::recordings::Recordings;
use crate::session::recorder::RecordingStatus;
use crate::session::Recorder;
use crate::settings::Settings;
use crate::sync::{announce, lock_recovering, Topic};
use crate::text::dictionary::Dictionary;
use crate::text::remove_fillers;

const RATE: u32 = 16_000;
/// How often a job waiting on a recording looks again.
const RECORDING_POLL: Duration = Duration::from_millis(250);
/// How much of an entry's text labels its re-transcription.
const LABEL_CHARS: usize = 80;
/// The longest file an import takes; its decoded samples are held in memory.
const MAX_IMPORT: Duration = Duration::from_hours(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum JobKind {
    Retranscribe,
    Import,
}

/// A queued or running job.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub transcript_id: i64,
    pub kind: JobKind,
    /// What the job is about: the file's name, or the start of the entry's text.
    pub label: String,
    pub model: String,
    /// Waiting for the jobs ahead of it.
    pub queued: bool,
    /// 0 to 1.
    pub progress: f64,
    /// Waiting for a recording to finish.
    pub paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum JobOutcome {
    Done,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobFinish {
    pub transcript_id: i64,
    pub kind: JobKind,
    pub outcome: JobOutcome,
    /// What a re-transcription replaced, which Undo restores.
    pub previous: Option<Previous>,
    /// The entry with its new text. An import that wasn't done is deleted.
    pub entry: Option<TranscriptResponse>,
    /// Why the job failed, for the user.
    pub message: Option<String>,
}

/// An entry's text and model before a re-transcription.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Previous {
    pub text: String,
    pub model: Option<String>,
}

struct Job {
    transcript_id: i64,
    kind: JobKind,
    label: String,
    model: &'static ModelEntry,
    language: Option<String>,
}

impl Job {
    fn snapshot(&self) -> JobSnapshot {
        JobSnapshot {
            transcript_id: self.transcript_id,
            kind: self.kind,
            label: self.label.clone(),
            model: self.model.display_name.clone(),
            queued: true,
            progress: 0.0,
            paused: false,
        }
    }
}

/// A job's cancel switch. Once the job commits, a cancel is refused, so a
/// cancelled job never saves.
#[derive(Default)]
struct Stop(AtomicU8);

impl Stop {
    const RUNNING: u8 = 0;
    const CANCELLED: u8 = 1;
    const COMMITTED: u8 = 2;

    /// False when the job already committed.
    fn cancel(&self) -> bool {
        self.swap(Self::CANCELLED)
    }

    fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == Self::CANCELLED
    }

    /// False when the job was cancelled first.
    fn commit(&self) -> bool {
        self.swap(Self::COMMITTED)
    }

    fn swap(&self, to: u8) -> bool {
        self.0
            .compare_exchange(Self::RUNNING, to, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

struct Running {
    snapshot: JobSnapshot,
    stop: Arc<Stop>,
    /// Reaches the job's engine from before its model loads.
    canceller: Canceller,
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    queue: VecDeque<Job>,
    /// A thread is taking jobs off the queue.
    working: bool,
}

/// Every change to the list `get_jobs` reads is announced under its lock.
#[derive(Default)]
pub struct Jobs {
    state: Mutex<State>,
}

/// A job found nothing to do, or could not finish.
struct Failure(String);

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl Jobs {
    /// The running job, then the queued ones in order.
    pub fn list(&self) -> Vec<JobSnapshot> {
        let state = lock_recovering(&self.state);
        state
            .running
            .iter()
            .map(|running| running.snapshot.clone())
            .chain(state.queue.iter().map(Job::snapshot))
            .collect()
    }

    fn has(state: &State, transcript_id: i64) -> bool {
        state
            .running
            .as_ref()
            .is_some_and(|running| running.snapshot.transcript_id == transcript_id)
            || state
                .queue
                .iter()
                .any(|job| job.transcript_id == transcript_id)
    }

    /// A recording started: cut the running job's model load or chunk, so it
    /// never competes with the recording. The job redoes it afterwards.
    pub fn yield_to_dictation(&self) {
        if let Some(running) = &lock_recovering(&self.state).running {
            running.canceller.cancel();
        }
    }

    /// Queue `job`, re-transcriptions ahead of imports: someone is waiting on
    /// those. False when the entry already has a job.
    fn enqueue(app: &AppHandle, job: Job) -> bool {
        let start = {
            let jobs = app.state::<Self>();
            let mut state = lock_recovering(&jobs.state);
            if Self::has(&state, job.transcript_id) {
                return false;
            }
            let at = match job.kind {
                JobKind::Retranscribe => state
                    .queue
                    .iter()
                    .take_while(|queued| queued.kind == JobKind::Retranscribe)
                    .count(),
                JobKind::Import => state.queue.len(),
            };
            state.queue.insert(at, job);
            announce(Topic::Jobs);
            !std::mem::replace(&mut state.working, true)
        };
        if start {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || work(&app));
        }
        true
    }

    /// Start the next job. `None` once the queue is empty.
    fn next(app: &AppHandle) -> Option<(Job, Arc<Stop>, Canceller)> {
        let stop = Arc::new(Stop::default());
        let canceller = Canceller::default();
        let jobs = app.state::<Self>();
        let mut state = lock_recovering(&jobs.state);
        let Some(job) = state.queue.pop_front() else {
            state.working = false;
            return None;
        };
        let snapshot = JobSnapshot {
            queued: false,
            ..job.snapshot()
        };
        state.running = Some(Running {
            snapshot,
            stop: Arc::clone(&stop),
            canceller: canceller.clone(),
        });
        announce(Topic::Jobs);
        Some((job, stop, canceller))
    }

    fn update(app: &AppHandle, change: impl FnOnce(&mut JobSnapshot)) {
        let jobs = app.state::<Self>();
        let mut state = lock_recovering(&jobs.state);
        if let Some(running) = state.running.as_mut() {
            change(&mut running.snapshot);
            announce(Topic::Jobs);
        }
    }
}

/// Decode `path`, add it to History as a pending entry with its audio, and
/// queue its transcription. Adds nothing when the file can't be read.
pub async fn import(
    app: &AppHandle,
    path: &Path,
    model: &'static ModelEntry,
    language: Option<String>,
) -> Result<(), String> {
    let samples = {
        let path = path.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || {
            crate::audio::file::decode_16k(&path, MAX_IMPORT)
        })
        .await
        .map_err(|e| {
            log::error!("audio file decode died: {e}");
            "Couldn't read that file.".to_string()
        })??
    };
    #[allow(clippy::cast_precision_loss)] // sample counts fit f64 exactly
    let seconds = samples.len() as f64 / f64::from(RATE);
    let label = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let repo = TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
    let entry = repo
        .create(CreateTranscript {
            content: String::new(),
            word_count: 0,
            duration_seconds: Some(seconds),
            target_app: None,
            source_name: Some(label.clone()),
            model: Some(model.id.clone()),
            pending: true,
            language: language.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = crate::recordings::store(app, entry.id, samples).await {
        log::warn!("could not keep the audio of entry {}: {e}", entry.id);
        let _ = repo.delete_by_id(entry.id).await;
        return Err("Couldn't save that file's audio.".to_string());
    }
    let entry = repo
        .get_by_id(entry.id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("The imported entry was deleted.")?;
    Jobs::enqueue(
        app,
        Job {
            transcript_id: entry.id,
            kind: JobKind::Import,
            label,
            model,
            language,
        },
    );
    Ok(())
}

/// Queue `entry`'s saved audio to be transcribed again with `model`. False
/// when it already has a job.
pub fn retranscribe(app: &AppHandle, entry: &Transcript, model: &'static ModelEntry) -> bool {
    Jobs::enqueue(
        app,
        Job {
            transcript_id: entry.id,
            kind: JobKind::Retranscribe,
            label: entry.content.chars().take(LABEL_CHARS).collect(),
            model,
            language: app.state::<Settings>().language(),
        },
    )
}

/// Queue again the imports a quit interrupted. One whose model is gone fails
/// like any job, which deletes its entry and tells the window.
pub async fn resume(app: &AppHandle) -> Result<(), String> {
    let repo = TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
    for import in repo.pending().await.map_err(|e| e.to_string())? {
        let model = import
            .model
            .as_deref()
            .and_then(catalog::find)
            .unwrap_or_else(|| ModelLibrary::active(&app.state::<Settings>()));
        Jobs::enqueue(
            app,
            Job {
                transcript_id: import.id,
                kind: JobKind::Import,
                label: import.source_name.unwrap_or_default(),
                model,
                language: import.language,
            },
        );
    }
    Ok(())
}

/// Stop entry `transcript_id`'s job at once, queued or running. Too late once
/// it is saving.
pub async fn cancel(app: &AppHandle, transcript_id: i64) {
    let dropped = {
        let jobs = app.state::<Jobs>();
        let mut state = lock_recovering(&jobs.state);
        match &state.running {
            // A running job reports its own end.
            Some(running) if running.snapshot.transcript_id == transcript_id => {
                if running.stop.cancel() {
                    running.canceller.cancel();
                }
                None
            }
            _ => {
                let at = state
                    .queue
                    .iter()
                    .position(|job| job.transcript_id == transcript_id);
                let dropped = at.and_then(|at| state.queue.remove(at));
                if dropped.is_some() {
                    announce(Topic::Jobs);
                }
                dropped
            }
        }
    };
    let Some(job) = dropped else {
        return;
    };
    // Like a job that ran: its import is gone before the window hears it ended.
    if job.kind == JobKind::Import {
        discard(app, job.transcript_id).await;
    }
    let jobs = app.state::<Jobs>();
    let _state = lock_recovering(&jobs.state);
    let _ = JobFinished(finish_of(&job, Ok(None))).emit(app);
}

/// Run queued jobs until there are none, on a blocking thread.
fn work(app: &AppHandle) {
    while let Some((job, stop, canceller)) = Jobs::next(app) {
        // A panic fails this job only; the queue still has to move on.
        let result =
            std::panic::catch_unwind(AssertUnwindSafe(|| run(app, &job, &stop, &canceller)))
                .unwrap_or_else(|_| {
                    log::error!("transcription job {} panicked", job.transcript_id);
                    Err(Failure(
                        "The transcription stopped unexpectedly.".to_string(),
                    ))
                });
        let finish = finish_of(&job, result);
        if job.kind == JobKind::Import && finish.outcome != JobOutcome::Done {
            tauri::async_runtime::block_on(discard(app, job.transcript_id));
        }
        let jobs = app.state::<Jobs>();
        let mut state = lock_recovering(&jobs.state);
        state.running = None;
        announce(Topic::Jobs);
        let _ = JobFinished(finish).emit(app);
    }
}

struct Done {
    previous: Option<Previous>,
    entry: TranscriptResponse,
}

fn finish_of(job: &Job, result: Result<Option<Done>, Failure>) -> JobFinish {
    let (outcome, done, message) = match result {
        Ok(Some(done)) => (JobOutcome::Done, Some(done), None),
        Ok(None) => (JobOutcome::Cancelled, None, None),
        Err(Failure(message)) => (JobOutcome::Failed, None, Some(message)),
    };
    let (previous, entry) = done.map_or((None, None), |d| (d.previous, Some(d.entry)));
    JobFinish {
        transcript_id: job.transcript_id,
        kind: job.kind,
        outcome,
        previous,
        entry,
        message,
    }
}

/// Delete an import that didn't finish: its entry and its audio.
async fn discard(app: &AppHandle, transcript_id: i64) {
    let deleted = async {
        let repo =
            TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
        repo.delete_by_id(transcript_id)
            .await
            .map_err(|e| e.to_string())?;
        app.state::<Recordings>().remove(transcript_id)
    };
    if let Err(e) = deleted.await {
        log::warn!("could not discard the import of entry {transcript_id}: {e}");
    }
}

/// The whole job, on a blocking thread. `Ok(None)` is a cancel.
fn run(
    app: &AppHandle,
    job: &Job,
    stop: &Stop,
    canceller: &Canceller,
) -> Result<Option<Done>, Failure> {
    let samples = app
        .state::<Recordings>()
        .read(job.transcript_id)
        .map_err(|e| {
            log::warn!(
                "could not read the audio of entry {}: {e}",
                job.transcript_id
            );
            Failure("The saved audio couldn't be read.".to_string())
        })?;
    let language = job.language.as_deref();
    let Some(engine) = load_engine(app, job.model, stop, canceller)? else {
        return Ok(None);
    };
    let Some(text) = transcribe(app, &engine, &samples, language, stop, canceller)? else {
        return Ok(None);
    };
    let output = engine.output_language(language);
    let text = remove_fillers(strip_leading_dashes(&text), output.as_deref());
    if text.is_empty() {
        return Err(Failure("No speech was found in that audio.".to_string()));
    }
    if !stop.commit() {
        return Ok(None);
    }
    tauri::async_runtime::block_on(save(app, job, &text, &engine.entry.id)).map(Some)
}

/// Write the job's text, dictionary-corrected, into its entry.
async fn save(app: &AppHandle, job: &Job, text: &str, model_id: &str) -> Result<Done, Failure> {
    let pool = app.state::<Database>().pool().await.map_err(|e| e.0)?;
    let text = app.state::<Dictionary>().correct(&pool, text).await;
    let repo = TranscriptRepository::new(pool);
    let deleted = || Failure("That entry was deleted.".to_string());
    let before = repo
        .get_by_id(job.transcript_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(deleted)?;
    repo.set_text(job.transcript_id, &text, model_id)
        .await
        .map_err(|e| e.to_string())?;
    // No longer pending, an import's audio counts toward the limit now.
    // Trimmed before the entry is read, so it never offers a deleted file.
    if let Err(e) = crate::recordings::apply_limit(app).await {
        log::warn!("could not apply the saved audio limit: {e}");
    }
    let entry = repo
        .get_by_id(job.transcript_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(deleted)?;
    Ok(Done {
        previous: (job.kind == JobKind::Retranscribe).then_some(Previous {
            text: before.content,
            model: before.model,
        }),
        entry: TranscriptResponse::new(entry, &app.state::<Recordings>()),
    })
}

/// An engine of the job's own for `model`, so the job never holds dictation's.
/// `None` when the job was cancelled.
fn load_engine(
    app: &AppHandle,
    model: &ModelEntry,
    stop: &Stop,
    canceller: &Canceller,
) -> Result<Option<TranscriptionEngine>, Failure> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let models_dir = app.state::<ModelLibrary>().dir().to_path_buf();
    let loaded = yielding(app, stop, canceller, |mark| {
        TranscriptionEngine::new(
            &models_dir,
            &app_data_dir,
            Some(&model.id),
            canceller.clone(),
            mark,
        )
    });
    loaded.transpose().map_err(Failure)
}

/// Decode chunk by chunk. `None` when cancelled.
fn transcribe(
    app: &AppHandle,
    engine: &TranscriptionEngine,
    samples: &[f32],
    language: Option<&str>,
    stop: &Stop,
    canceller: &Canceller,
) -> Result<Option<String>, Failure> {
    let mut text = String::new();
    let mut start = 0;
    for end in chunks::ends(samples) {
        let chunk = &samples[start..end];
        start = end;
        if contains_speech(chunk, RATE) {
            let pcm = normalize_level(chunk);
            let decoded = yielding(app, stop, canceller, |mark| {
                engine.transcribe_segments(mark, &pcm, "", Pass::Final, language)
            });
            let Some(decoded) = decoded.transpose()? else {
                return Ok(None);
            };
            // A cut-short chunk would lose words without saying so.
            if decoded.truncated {
                return Err(Failure(
                    "Part of the audio couldn't be transcribed, so nothing was changed."
                        .to_string(),
                ));
            }
            let part = join_words(decoded.segments.iter().flat_map(|s| &s.words));
            if !text.is_empty() && !part.is_empty() {
                text.push(' ');
            }
            text.push_str(&part);
        } else if stop.cancelled() {
            return Ok(None);
        }
        #[allow(clippy::cast_precision_loss)] // sample counts fit f64 exactly
        let progress = end as f64 / samples.len() as f64;
        Jobs::update(app, |snapshot| snapshot.progress = progress);
    }
    Ok(Some(text))
}

/// Run `work` while no recording is: one that starts cuts it
/// ([`Jobs::yield_to_dictation`]), and it runs again once the recording is
/// done. `None` when the job was cancelled.
fn yielding<T>(
    app: &AppHandle,
    stop: &Stop,
    canceller: &Canceller,
    mut work: impl FnMut(CancelMark) -> Result<T, String>,
) -> Option<Result<T, String>> {
    loop {
        // Taken before the recording check, so a recording starting after it
        // still cuts the work.
        let mark = canceller.mark();
        wait_for_recording(app, stop);
        if stop.cancelled() {
            return None;
        }
        let result = work(mark);
        if stop.cancelled() {
            return None;
        }
        if result.is_err() && canceller.mark() != mark {
            log::info!("a recording cut background work short; running it again");
            continue;
        }
        return Some(result);
    }
}

/// Hold here while a recording is in progress or being finished.
fn wait_for_recording(app: &AppHandle, stop: &Stop) {
    let busy = || app.state::<Recorder>().status() != RecordingStatus::Idle;
    if !busy() {
        return;
    }
    Jobs::update(app, |snapshot| snapshot.paused = true);
    while busy() && !stop.cancelled() {
        std::thread::sleep(RECORDING_POLL);
    }
    Jobs::update(app, |snapshot| snapshot.paused = false);
}

#[cfg(test)]
#[path = "../../tests/unit/jobs/mod.rs"]
mod tests;
