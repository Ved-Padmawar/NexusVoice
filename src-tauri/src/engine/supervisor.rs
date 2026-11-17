//! Parent side: [`Supervisor`], the owner of one model's worker process.
//! Dropping it ends the worker, freeing the model's CPU and GPU memory.
//!
//! Calls take turns on the owner lock. Each call to the worker has a deadline
//! scaled to its audio; a worker that misses it is killed as hung. A crashed or
//! hung transcription is retried once in a fresh worker, on the CPU if the GPU
//! failed. [`Supervisor::cancel`] bypasses the lock and kills the busy worker.

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn, Level};
use transcribe_cpp::{Backend, RunOptions, StreamOptions, StreamText, StreamUpdate, Transcript};

use super::protocol::{encode_message, read_message, LoadedInfo, Request, Response};
use super::worker::LOG_LINE_PREFIX;
use super::{APP_DATA_FLAG, CPU_ONLY_FLAG, LOG_LEVEL_ENV, WORKER_FLAG};
use crate::sync::lock_recovering;

/// Spawn and backend init, which on a first GPU use may compile shaders.
const HELLO_TIMEOUT: Duration = Duration::from_secs(60);
/// Model loads stay generous: a slow load is slow, not hung.
const LOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// Floor for stream calls that do no decoding (begin, reset).
const CALL_FLOOR: Duration = Duration::from_secs(30);
/// How long a worker gets to exit after its stdin closes before it's killed.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
/// Native stderr lines kept for crash reports (e.g. a `GGML_ASSERT` message).
const STDERR_TAIL_LINES: usize = 64;
const SAMPLE_RATE: f64 = 16_000.0;

/// Set once a GPU worker crashes or hangs: later workers this run are CPU-only.
static GPU_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// How long decoding may take before a worker counts as hung. A GPU hang is
/// retried on CPU, so it is caught soon; a CPU hang is not, so its deadline
/// leaves room for a slow machine.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Deadlines {
    /// Floor for a stream feed or finalize (short chunks, shader compiles).
    call_floor: Duration,
    /// Floor for a batch run: Whisper encodes a full 30 s window however
    /// short the clip.
    run_floor: Duration,
    /// Work on N seconds of audio may take up to this many times N.
    audio_factor: f64,
}

const GPU_DEADLINES: Deadlines = Deadlines {
    call_floor: Duration::from_secs(30),
    run_floor: Duration::from_secs(60),
    audio_factor: 10.0,
};

/// A large model on an older CPU runs slower than real time.
const CPU_DEADLINES: Deadlines = Deadlines {
    call_floor: Duration::from_secs(120),
    run_floor: Duration::from_secs(300),
    audio_factor: 20.0,
};

impl Deadlines {
    fn for_device(on_gpu: bool) -> Self {
        if on_gpu {
            GPU_DEADLINES
        } else {
            CPU_DEADLINES
        }
    }

    /// Deadline for stream work on `samples` of 16 kHz audio.
    #[allow(clippy::cast_precision_loss)] // audio-sized counts fit f64 exactly enough
    fn audio(&self, samples: usize) -> Duration {
        self.call_floor.max(Duration::from_secs_f64(
            samples as f64 / SAMPLE_RATE * self.audio_factor,
        ))
    }

    /// Deadline for a batch run on `samples` of 16 kHz audio.
    fn run(&self, samples: usize) -> Duration {
        self.run_floor.max(self.audio(samples))
    }
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped non-negative
fn ms_to_samples(ms: i64) -> usize {
    (ms.max(0) as usize) * (SAMPLE_RATE as usize / 1000)
}

/// How to start a worker, kept so a dead one can be replaced.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    /// The executable to relaunch as a worker; see [`worker_exe`].
    pub exe: PathBuf,
    pub model: PathBuf,
    /// Where the CUDA pack lives.
    pub app_data_dir: PathBuf,
}

#[derive(Debug)]
pub enum EngineError {
    /// [`Supervisor::cancel`] stopped the work.
    Cancelled,
    /// A transcribe.cpp error, or a worker failure that could not be recovered.
    Failed(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("transcription was cancelled"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

/// A finished transcription.
pub struct Ran {
    pub transcript: Transcript,
    /// The decode stopped early, so the transcript may be missing its end.
    pub truncated: bool,
}

/// A finished stream.
pub struct Finalized {
    pub text: StreamText,
    pub truncated: bool,
}

/// State a cancel reaches without waiting for the owner lock.
#[derive(Default)]
struct Shared {
    /// Bumped by every [`Supervisor::cancel`]. Work begun before the bump
    /// counts as cancelled.
    cancel_epoch: AtomicU64,
    /// The worker answering the call in flight, if any. A cancel takes it
    /// and kills the worker.
    in_flight: Mutex<Option<Arc<Control>>>,
    /// Device kind of the latest worker, which a CPU fallback changes.
    backend: Mutex<String>,
}

impl Shared {
    fn epoch(&self) -> u64 {
        self.cancel_epoch.load(Ordering::SeqCst)
    }

    fn cancelled_since(&self, epoch: u64) -> bool {
        self.epoch() != epoch
    }

    fn cancel(&self) {
        self.cancel_epoch.fetch_add(1, Ordering::SeqCst);
        // Taking the call tells the owner its worker was killed.
        if let Some(control) = lock_recovering(&self.in_flight).take() {
            info!(
                "cancelling by stopping the transcription worker (pid {})",
                control.pid
            );
            control.kill_for_cancel();
        }
    }
}

/// Cancels one supervisor's work, its model load included. Made before the
/// supervisor, so a cancel can reach the load too.
#[derive(Clone, Default)]
pub struct Canceller(Arc<Shared>);

impl Canceller {
    pub fn mark(&self) -> CancelMark {
        CancelMark(self.0.epoch())
    }

    /// See [`Supervisor::cancel`].
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

/// A point to cancel from: work given it counts as cancelled by any
/// [`Supervisor::cancel`] after it was taken, even work that starts later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelMark(u64);

/// Owns one transcribe.cpp worker for one model.
pub struct Supervisor {
    owner: Mutex<Owner>,
    shared: Arc<Shared>,
}

impl Supervisor {
    /// Start a worker and load the model in it. A cancel through `canceller`
    /// after `mark` stops the load.
    pub fn start(
        spec: WorkerSpec,
        canceller: Canceller,
        mark: CancelMark,
    ) -> Result<(Self, LoadedInfo), EngineError> {
        let Canceller(shared) = canceller;
        let mut owner = Owner {
            spec,
            worker: None,
            gpu_backend_error: false,
        };
        let info = owner.start(&shared, mark.0)?;
        let supervisor = Self {
            owner: Mutex::new(owner),
            shared,
        };
        Ok((supervisor, info))
    }

    /// The device kind the model runs on now: "cuda", "vulkan" or "cpu".
    pub fn backend(&self) -> String {
        lock_recovering(&self.shared.backend).clone()
    }

    pub fn cancel_mark(&self) -> CancelMark {
        CancelMark(self.shared.epoch())
    }

    /// Transcribe 16 kHz mono PCM. A worker crash or hang is retried once in
    /// a fresh worker (on CPU if it was on the GPU).
    pub fn transcribe(
        &self,
        mark: CancelMark,
        pcm: &[f32],
        run: &RunOptions,
    ) -> Result<Ran, EngineError> {
        lock_recovering(&self.owner).transcribe(&self.shared, pcm, run, mark.0)
    }

    /// Begin a live stream. Other calls wait until its handle ends.
    pub fn stream(
        &self,
        mark: CancelMark,
        run: &RunOptions,
        stream: &StreamOptions,
    ) -> Result<StreamHandle<'_>, EngineError> {
        let epoch = mark.0;
        let mut owner = lock_recovering(&self.owner);
        owner.ensure_worker(&self.shared, epoch)?;
        let request = Request::StreamBegin {
            run: run.clone(),
            stream: stream.clone(),
        };
        match owner.call_work(&self.shared, &request, None, CALL_FLOOR, epoch) {
            Ok(Response::Ok) => Ok(StreamHandle {
                owner,
                shared: &self.shared,
                epoch,
                pending_ms: 0,
                open: true,
            }),
            Ok(other) => Err(unexpected(&other).into()),
            Err(failure) => {
                if failure.worker_lost() {
                    owner.worker_lost("stream start", &failure);
                }
                Err(failure.into())
            }
        }
    }

    /// Stop the work in flight and any waiting its turn, by killing the busy
    /// worker. The next call starts a fresh one.
    pub fn cancel(&self) {
        self.shared.cancel();
    }
}

/// A live stream in the worker. Dropping it unfinalized resets the stream.
pub struct StreamHandle<'a> {
    owner: MutexGuard<'a, Owner>,
    shared: &'a Shared,
    /// Cancel epoch when the stream began; a later cancel stops it.
    epoch: u64,
    /// Audio fed but not yet committed; sizes the deadlines.
    pending_ms: i64,
    /// The worker still holds this stream.
    open: bool,
}

impl StreamHandle<'_> {
    /// Feed 16 kHz mono PCM. `Ok(Some(text))` when the text changed.
    pub fn feed(&mut self, pcm: &[f32]) -> Result<Option<StreamText>, EngineError> {
        if !self.open {
            return Err(EngineError::Failed("the stream was lost".to_string()));
        }
        let deadline = self
            .owner
            .deadlines()
            .audio(pcm.len() + ms_to_samples(self.pending_ms));
        match self
            .owner
            .call_work(self.shared, &Request::Feed, Some(pcm), deadline, self.epoch)
        {
            Ok(Response::Fed { update, text }) => {
                self.pending_ms = pending_ms(&update);
                Ok(text)
            }
            Ok(other) => Err(unexpected(&other).into()),
            Err(failure) => {
                if failure.worker_lost() {
                    self.owner.worker_lost("stream feed", &failure);
                }
                // Gone after a crash, a hang or a cancel's kill.
                self.open = self.owner.worker.is_some();
                Err(failure.into())
            }
        }
    }

    /// Flush the stream and return its final text. `None` if it was lost,
    /// failed or cancelled.
    pub fn finalize(mut self) -> Option<Finalized> {
        if !self.open || self.shared.cancelled_since(self.epoch) {
            return None;
        }
        let deadline = self.owner.deadlines().audio(ms_to_samples(self.pending_ms));
        let result =
            self.owner
                .call_work(self.shared, &Request::Finalize, None, deadline, self.epoch);
        // Finalize ends the stream even when it fails; only a cancel may have
        // kept it from being sent, and dropping then resets it.
        self.open = matches!(result, Err(Failure::Cancelled));
        match result {
            Ok(Response::Finalized { text, truncated }) => {
                self.owner.run_succeeded();
                Some(Finalized { text, truncated })
            }
            Ok(other) => {
                error!("stream finalize failed: {}", unexpected(&other));
                None
            }
            Err(Failure::Cancelled) => None,
            Err(failure) => {
                error!("stream finalize failed: {failure}");
                if failure.worker_lost() {
                    self.owner.worker_lost("stream finalize", &failure);
                }
                None
            }
        }
    }
}

impl Drop for StreamHandle<'_> {
    fn drop(&mut self) {
        if !self.open || self.owner.worker.is_none() {
            return;
        }
        let reset = self.owner.call_work(
            self.shared,
            &Request::StreamReset,
            None,
            CALL_FLOOR,
            self.epoch,
        );
        match reset {
            Ok(_) => {}
            // Expected when the stream already ended in the worker.
            Err(Failure::Remote(message)) => debug!("stream reset: {message}"),
            // The worker may still hold the stream; a fresh one won't.
            Err(Failure::Cancelled) => self.owner.worker = None,
            Err(failure) => {
                self.owner.worker_lost("stream reset", &failure);
            }
        }
    }
}

/// How a call to the worker failed.
#[derive(Debug)]
enum Failure {
    /// The worker process could not be started.
    Spawn(io::Error),
    /// transcribe.cpp returned an error; the worker is fine.
    Remote(String),
    /// The worker exited: its exit status and last native output.
    Crashed(String),
    /// The worker missed its deadline and was killed.
    Hung(Duration),
    /// A backend error (e.g. a lost GPU device) that may fail every later run.
    Backend(String),
    Cancelled,
}

impl Failure {
    /// Whether the worker is gone, or can't be trusted with another run.
    fn worker_lost(&self) -> bool {
        matches!(self, Self::Crashed(_) | Self::Hung(_) | Self::Backend(_))
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "failed to start the transcription worker: {e}"),
            Self::Remote(message) => f.write_str(message),
            Self::Crashed(detail) => write!(f, "transcription worker crashed ({detail})"),
            Self::Hung(t) => write!(f, "transcription worker did not respond within {t:?}"),
            Self::Backend(message) => write!(f, "transcription backend failed ({message})"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl From<Failure> for EngineError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Cancelled => Self::Cancelled,
            other => Self::Failed(other.to_string()),
        }
    }
}

fn unexpected(response: &Response) -> Failure {
    Failure::Remote(format!(
        "unexpected transcription worker response: {response:?}"
    ))
}

/// A GPU worker failed this run, so every new worker is CPU-only.
pub fn gpu_unavailable() -> bool {
    GPU_UNAVAILABLE.load(Ordering::Acquire)
}

/// `reason` is logged when this changes the state.
fn mark_gpu_unavailable(reason: impl fmt::Display) {
    if !GPU_UNAVAILABLE.swap(true, Ordering::AcqRel) {
        warn!("transcription GPU marked unavailable: {reason}");
    }
}

// --- Owner -----------------------------------------------------------------

struct Owner {
    spec: WorkerSpec,
    /// Empty after a crash, hang or cancel; the next call starts a fresh one.
    worker: Option<Worker>,
    /// A GPU backend error since the last good run. The first only restarts
    /// the worker (a lost device is often a one-off driver reset); a second
    /// marks the GPU unavailable.
    gpu_backend_error: bool,
}

impl Owner {
    /// Start a worker and load the model in it; a cancel stops either step.
    fn start_worker(
        &self,
        shared: &Shared,
        epoch: u64,
        cpu_only: bool,
    ) -> Result<(Worker, LoadedInfo), Failure> {
        let mut worker = Worker::spawn(&self.spec, cpu_only).map_err(Failure::Spawn)?;
        match call_on(
            &mut worker,
            shared,
            &Request::Hello,
            None,
            HELLO_TIMEOUT,
            epoch,
        )? {
            Response::Hello => {}
            other => return Err(unexpected(&other)),
        }
        let load = Request::Load {
            path: self.spec.model.clone(),
            backend: if cpu_only {
                Backend::Cpu
            } else {
                Backend::Auto
            },
        };
        match call_on(&mut worker, shared, &load, None, LOAD_TIMEOUT, epoch)? {
            Response::Loaded(info) => {
                worker.info = Some(info.clone());
                Ok((worker, info))
            }
            other => Err(unexpected(&other)),
        }
    }

    /// Fill the slot with a loaded worker. A GPU worker lost on the way marks
    /// the GPU unavailable and is retried once CPU-only.
    fn start(&mut self, shared: &Shared, epoch: u64) -> Result<LoadedInfo, EngineError> {
        let started = Instant::now();
        let cpu_only = gpu_unavailable();
        let (worker, info) = match self.start_worker(shared, epoch, cpu_only) {
            Ok(started) => started,
            Err(failure) if failure.worker_lost() && !cpu_only => {
                mark_gpu_unavailable(format!(
                    "a worker with GPU backends {failure} while loading '{}'",
                    self.spec.model.display()
                ));
                warn!("retrying in a CPU-only worker");
                self.start_worker(shared, epoch, true)?
            }
            Err(failure) => return Err(failure.into()),
        };
        debug!(
            "worker (pid {}) spawned and loaded the model in {}ms",
            worker.control.pid,
            started.elapsed().as_millis()
        );
        self.worker = Some(worker);
        lock_recovering(&shared.backend).clone_from(&info.backend);
        Ok(info)
    }

    /// A fresh worker if the last one died or was cancelled.
    fn ensure_worker(&mut self, shared: &Shared, epoch: u64) -> Result<(), EngineError> {
        if shared.cancelled_since(epoch) {
            return Err(EngineError::Cancelled);
        }
        if self.worker.is_none() {
            info!("starting a fresh transcription worker for the loaded model");
            self.start(shared, epoch)?;
        }
        Ok(())
    }

    /// Drop a failed worker. On a GPU, mark the GPU unavailable, except after
    /// a first backend error. Returns whether it was on a GPU.
    fn worker_lost(&mut self, during: &str, failure: &Failure) -> bool {
        // Dropping the handle reaps the process.
        let info = self.worker.take().and_then(|worker| worker.info.clone());
        match info {
            Some(info) if info.on_gpu => {
                if matches!(failure, Failure::Backend(_)) && !self.gpu_backend_error {
                    self.gpu_backend_error = true;
                    warn!(
                        "{failure} during {during} on GPU '{}' ({}); restarting the worker",
                        info.device, info.backend
                    );
                } else {
                    mark_gpu_unavailable(format!(
                        "{failure} during {during} on GPU '{}' ({})",
                        info.device, info.backend
                    ));
                }
                true
            }
            _ => false,
        }
    }

    /// Deadlines for the device the current worker's model runs on.
    fn deadlines(&self) -> Deadlines {
        Deadlines::for_device(self.worker_on_gpu())
    }

    fn worker_on_gpu(&self) -> bool {
        self.worker
            .as_ref()
            .and_then(|worker| worker.info.as_ref())
            .is_some_and(|info| info.on_gpu)
    }

    /// [`call_on`] the slot's worker, emptying the slot if it was killed.
    fn call_work(
        &mut self,
        shared: &Shared,
        request: &Request,
        pcm: Option<&[f32]>,
        deadline: Duration,
        epoch: u64,
    ) -> Result<Response, Failure> {
        let Some(worker) = self.worker.as_mut() else {
            return Err(Failure::Remote("the worker is gone".to_string()));
        };
        let result = call_on(worker, shared, request, pcm, deadline, epoch);
        if worker.control.killed() {
            self.worker = None;
        }
        result
    }

    /// A run succeeded; a later backend error on the GPU is a new one.
    fn run_succeeded(&mut self) {
        if self.worker_on_gpu() {
            self.gpu_backend_error = false;
        }
    }

    /// Retry once after a crash, hang or backend error, on CPU if the GPU
    /// failed; a first GPU backend error retries on the GPU for free. A hang
    /// or backend error on CPU is final.
    fn transcribe(
        &mut self,
        shared: &Shared,
        pcm: &[f32],
        run: &RunOptions,
        epoch: u64,
    ) -> Result<Ran, EngineError> {
        let request = Request::Run {
            options: run.clone(),
        };
        let mut retried = false;
        loop {
            self.ensure_worker(shared, epoch)?;
            let deadline = self.deadlines().run(pcm.len());
            match self.call_work(shared, &request, Some(pcm), deadline, epoch) {
                Ok(Response::Transcript {
                    transcript,
                    truncated,
                }) => {
                    self.run_succeeded();
                    return Ok(Ran {
                        transcript,
                        truncated,
                    });
                }
                Ok(other) => return Err(unexpected(&other).into()),
                Err(failure) if failure.worker_lost() => {
                    let on_gpu = self.worker_lost("transcription", &failure);
                    let failed_on_cpu =
                        !on_gpu && matches!(failure, Failure::Hung(_) | Failure::Backend(_));
                    // Still allowed after a first backend error on the GPU.
                    let fresh_gpu = on_gpu && !gpu_unavailable();
                    if failed_on_cpu || (retried && !fresh_gpu) {
                        return Err(failure.into());
                    }
                    if fresh_gpu {
                        warn!("retrying the transcription in a fresh GPU worker");
                    } else {
                        warn!("retrying the transcription in a fresh worker");
                        retried = true;
                    }
                }
                Err(failure) => return Err(failure.into()),
            }
        }
    }
}

/// A call [`Supervisor::cancel`] can stop: `Cancelled` if a cancel came after
/// `epoch`, with the worker marked killed if it may be dead.
fn call_on(
    worker: &mut Worker,
    shared: &Shared,
    request: &Request,
    pcm: Option<&[f32]>,
    deadline: Duration,
    epoch: u64,
) -> Result<Response, Failure> {
    let frame = encode_request(request, pcm)?;
    let sent = {
        // Under the lock cancel() takes after bumping the epoch: either this
        // sees the cancel, or the cancel sees this call and kills it.
        let mut in_flight = lock_recovering(&shared.in_flight);
        if shared.cancelled_since(epoch) {
            return Err(Failure::Cancelled);
        }
        let sent = worker.control.send(frame);
        *in_flight = Some(Arc::clone(&worker.control));
        sent
    };
    let result = if sent {
        worker.wait(request, deadline)
    } else {
        Err(worker.died())
    };
    // Gone if cancel() took the call, which means it killed the worker.
    let killed = lock_recovering(&shared.in_flight).take().is_none();
    if killed || shared.cancelled_since(epoch) {
        if result.as_ref().is_err_and(Failure::worker_lost) {
            worker.control.kill_for_cancel();
        }
        return Err(Failure::Cancelled);
    }
    result
}

fn pending_ms(update: &StreamUpdate) -> i64 {
    update
        .buffered_ms
        .max(update.input_received_ms - update.audio_committed_ms)
        .max(0)
}

// --- Worker process handle ------------------------------------------------

/// The parts of a worker other threads touch. All writes go through one
/// writer thread, so frames never interleave.
struct Control {
    pid: u32,
    child: Mutex<Child>,
    stdin: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    /// Killed by a cancel, so its exit is expected.
    killed: AtomicBool,
    /// Wakes the call waiting on this worker.
    wake: mpsc::Sender<Event>,
}

/// What a call waiting on a worker can receive.
enum Event {
    Response(Response),
    /// The worker's output closed: it exited or crashed.
    Closed,
    Cancelled,
}

impl Control {
    fn send(&self, frame: Vec<u8>) -> bool {
        lock_recovering(&self.stdin)
            .as_ref()
            .is_some_and(|stdin| stdin.send(frame).is_ok())
    }

    /// Closing stdin tells the worker to exit (it ends on EOF).
    fn close_stdin(&self) {
        lock_recovering(&self.stdin).take();
    }

    fn kill(&self) {
        let _ = lock_recovering(&self.child).kill();
    }

    /// Kill, and release the waiting call now: a process stuck in a driver
    /// may hold its output open long after the kill.
    fn kill_for_cancel(&self) {
        self.killed.store(true, Ordering::SeqCst);
        self.kill();
        let _ = self.wake.send(Event::Cancelled);
    }

    fn killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }
}

/// One worker process. Dropping it closes stdin, kills the worker if it
/// outlives [`SHUTDOWN_GRACE`], and reaps it.
struct Worker {
    control: Arc<Control>,
    events: mpsc::Receiver<Event>,
    stderr_done: mpsc::Receiver<()>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    /// Set once a model is loaded.
    info: Option<LoadedInfo>,
}

impl Worker {
    /// Start a worker process. A `cpu_only` one runs no GPU driver code.
    fn spawn(spec: &WorkerSpec, cpu_only: bool) -> io::Result<Self> {
        let mut command = Command::new(&spec.exe);
        #[cfg(target_os = "linux")]
        if let Some(arg0) = std::env::args_os().next() {
            // Keep `ps` showing the app's own path rather than /proc/self/exe.
            use std::os::unix::process::CommandExt;
            command.arg0(arg0);
        }
        command.arg(WORKER_FLAG);
        if cpu_only {
            command.arg(CPU_ONLY_FLAG);
        }
        command
            .arg(APP_DATA_FLAG)
            .arg(&spec.app_data_dir)
            .env(LOG_LEVEL_ENV, log::max_level().as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn()?;
        let pid = child.id();
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");

        let (stdin_tx, frames) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let (stderr_done_tx, stderr_done) = mpsc::channel();
        // Built before the threads so a failure below still reaps the child.
        let worker = Self {
            control: Arc::new(Control {
                pid,
                child: Mutex::new(child),
                stdin: Mutex::new(Some(stdin_tx)),
                killed: AtomicBool::new(false),
                wake: event_tx.clone(),
            }),
            events,
            stderr_done,
            stderr_tail: Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES))),
            info: None,
        };

        thread::Builder::new()
            .name("transcribe-worker-in".into())
            .spawn(move || write_frames(stdin, &frames))?;
        thread::Builder::new()
            .name("transcribe-worker-out".into())
            .spawn(move || read_responses(stdout, &event_tx))?;
        // Always drain stderr: a full pipe would block the worker.
        let tail = Arc::clone(&worker.stderr_tail);
        thread::Builder::new()
            .name("transcribe-worker-err".into())
            .spawn(move || {
                // Raw bytes: native code may write non-UTF-8, which would end
                // `lines()` and lose the abort message.
                for line in BufReader::new(stderr).split(b'\n') {
                    let Ok(line) = line else { break };
                    let line = String::from_utf8_lossy(&line);
                    let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
                    forward_worker_log(&line);
                    let mut tail = lock_recovering(&tail);
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
                let _ = stderr_done_tx.send(());
            })?;

        debug!(
            "started {}transcription worker (pid {pid})",
            if cpu_only { "CPU-only " } else { "" },
        );
        Ok(worker)
    }

    /// Wait for the response to `request`, already sent.
    fn wait(&mut self, request: &Request, deadline: Duration) -> Result<Response, Failure> {
        match self.events.recv_timeout(deadline) {
            Ok(Event::Response(Response::Error(message))) => Err(Failure::Remote(message)),
            Ok(Event::Response(Response::BackendError(message))) => Err(Failure::Backend(message)),
            Ok(Event::Response(response)) => Ok(response),
            Ok(Event::Cancelled) => Err(Failure::Cancelled),
            Ok(Event::Closed) | Err(mpsc::RecvTimeoutError::Disconnected) => Err(self.died()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.control.kill();
                let tail = self.native_tail();
                error!(
                    "transcription worker (pid {}) did not answer {} within {deadline:?}; killed it{}",
                    self.control.pid,
                    request_name(request),
                    if tail.is_empty() {
                        String::new()
                    } else {
                        format!(". Native output before the hang:\n{}", tail.join("\n"))
                    }
                );
                Err(Failure::Hung(deadline))
            }
        }
    }

    /// The worker's stdout closed: collect its exit status and last output.
    fn died(&mut self) -> Failure {
        if self.control.killed() {
            return Failure::Cancelled;
        }
        // Let the stderr thread catch the final lines (the abort message).
        let _ = self.stderr_done.recv_timeout(Duration::from_secs(1));
        let status = {
            let mut child = lock_recovering(&self.control.child);
            if let Some(status) = wait_exit(&mut child, Duration::from_secs(1)) {
                status.to_string()
            } else {
                let _ = child.kill();
                "unresponsive after closing its output; killed".to_string()
            }
        };
        let tail = self.native_tail();
        if tail.is_empty() {
            error!(
                "transcription worker (pid {}) died: {status}",
                self.control.pid
            );
        } else {
            error!(
                "transcription worker (pid {}) died: {status}. Native output before exit:\n{}",
                self.control.pid,
                tail.join("\n")
            );
        }
        Failure::Crashed(match tail.last() {
            Some(last) => format!("{status}: {last}"),
            None => status,
        })
    }

    /// Recent raw native output, without the already re-logged log lines.
    fn native_tail(&self) -> Vec<String> {
        lock_recovering(&self.stderr_tail)
            .iter()
            .filter(|line| !line.starts_with(LOG_LINE_PREFIX))
            .cloned()
            .collect()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.control.close_stdin();
        let mut child = lock_recovering(&self.control.child);
        // A cancelled worker was already killed, so it gets no grace.
        if !self.control.killed() {
            if wait_exit(&mut child, SHUTDOWN_GRACE).is_some() {
                return;
            }
            warn!(
                "transcription worker (pid {}) did not exit within {SHUTDOWN_GRACE:?}; killing it",
                self.control.pid
            );
            let _ = child.kill();
        }
        if !matches!(child.try_wait(), Ok(Some(_))) {
            // It may be stuck in a driver call: reap it off-thread.
            drop(child);
            let control = Arc::clone(&self.control);
            let _ = thread::Builder::new()
                .name("transcribe-worker-reap".into())
                .spawn(move || {
                    let _ = lock_recovering(&control.child).wait();
                });
        }
    }
}

/// Wait up to `timeout` for `child` to exit. `None` if it is still running.
fn wait_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => return None,
        }
    }
}

/// The file the app started from and its device and inode; see
/// [`worker_exe`]. `None` skips the check.
#[cfg(unix)]
static LAUNCHED_EXE: std::sync::OnceLock<Option<(PathBuf, (u64, u64))>> =
    std::sync::OnceLock::new();

#[cfg(unix)]
fn file_id(path: &std::path::Path) -> io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

/// Must run before an update can replace the file, so at startup.
pub fn record_launched_exe() {
    #[cfg(unix)]
    LAUNCHED_EXE.get_or_init(|| {
        std::env::current_exe()
            .and_then(|path| {
                let id = file_id(&path)?;
                Ok((path, id))
            })
            .inspect_err(|e| warn!("could not identify the app's executable: {e}"))
            .ok()
    });
}

/// The executable a worker runs: this very binary, so both agree on the
/// protocol and backend libraries. If an update replaced the file (Windows
/// locks it; Linux uses `/proc/self/exe`), refuse until a restart.
pub fn worker_exe() -> io::Result<PathBuf> {
    #[cfg(unix)]
    if let Some((path, launched)) = LAUNCHED_EXE.get().and_then(Option::as_ref) {
        if file_id(path).ok().as_ref() != Some(launched) {
            return Err(io::Error::other(
                "NexusVoice was updated or moved while it was running; restart it",
            ));
        }
    }
    #[cfg(target_os = "linux")]
    return Ok(PathBuf::from("/proc/self/exe"));
    #[cfg(not(target_os = "linux"))]
    std::env::current_exe()
}

fn encode_request(request: &Request, pcm: Option<&[f32]>) -> Result<Vec<u8>, Failure> {
    encode_message(request, pcm)
        .map_err(|e| Failure::Remote(format!("failed to encode request: {e}")))
}

fn write_frames(mut stdin: ChildStdin, frames: &mpsc::Receiver<Vec<u8>>) {
    // Ends when the handle closes stdin (dropping it here sends EOF) or the
    // worker is gone.
    for frame in frames {
        if stdin
            .write_all(&frame)
            .and_then(|()| stdin.flush())
            .is_err()
        {
            break;
        }
    }
}

fn read_responses(stdout: ChildStdout, events: &mpsc::Sender<Event>) {
    let mut stdout = BufReader::new(stdout);
    loop {
        match read_message::<Response>(&mut stdout) {
            Ok(Some((response, _))) => {
                if events.send(Event::Response(response)).is_err() {
                    return;
                }
            }
            Ok(None) => break,
            Err(e) => {
                error!("transcription worker protocol error: {e}");
                break;
            }
        }
    }
    // Explicit: the control's waker keeps the channel itself open.
    let _ = events.send(Event::Closed);
}

fn request_name(request: &Request) -> &'static str {
    match request {
        Request::Hello => "hello",
        Request::Load { .. } => "model load",
        Request::Run { .. } => "transcription",
        Request::StreamBegin { .. } => "stream begin",
        Request::Feed => "stream feed",
        Request::Finalize => "stream finalize",
        Request::StreamReset => "stream reset",
    }
}

/// Re-log a worker stderr line in the parent. Structured lines keep their
/// level; anything else is raw native output, kept for crash reports.
fn forward_worker_log(line: &str) {
    const TARGET: &str = "transcribe_worker";
    if let Some(rest) = line.strip_prefix(LOG_LINE_PREFIX) {
        let mut parts = rest.splitn(3, '\t');
        if let (Some(level), Some(target), Some(message)) =
            (parts.next(), parts.next(), parts.next())
        {
            let level = level.parse::<Level>().unwrap_or(Level::Info);
            log::log!(target: TARGET, level, "[{target}] {message}");
            return;
        }
    }
    debug!(target: TARGET, "{line}");
}

#[cfg(test)]
#[path = "../../tests/unit/engine/supervisor.rs"]
mod tests;
