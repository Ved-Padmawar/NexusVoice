//! The worker process: the only place transcribe.cpp native code runs. Serves
//! [`Request`]s until the parent closes its stdin, then exits at once, even if
//! busy.

use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use log::{LevelFilter, Log, Metadata, Record};
use transcribe_cpp::{
    Backend, BackendMask, DeviceType, ErrorKind, Model, ModelOptions, RunOptions, Session,
    SessionOptions, Stream, StreamOptions, StreamText,
};

use super::protocol::{read_message, write_message, LoadedInfo, Request, Response};
use super::{APP_DATA_FLAG, CPU_ONLY_FLAG, LOG_LEVEL_ENV};

/// Marks worker log lines apart from raw native output.
pub(super) const LOG_LINE_PREFIX: &str = "\u{1}";

struct Incoming {
    request: Request,
    pcm: Vec<f32>,
}

/// How a stream served by [`serve_stream`] ended.
enum StreamEnd {
    Finalized(StreamText),
    /// Answered with this instead (a reset, or a failed finalize).
    Reply(Response),
}

pub fn run() -> i32 {
    #[cfg(unix)]
    ignore_app_signals();
    #[cfg(target_os = "linux")]
    set_process_name();
    // Before any native code runs: ggml writes to stdout in places, which
    // would corrupt the protocol, so fd 1 goes to stderr.
    let mut output = match take_stdout_for_protocol() {
        Ok(file) => file,
        Err(e) => {
            eprintln!("transcribe worker: failed to claim stdout: {e}");
            return 2;
        }
    };
    init_logger();

    let (requests_tx, requests) = mpsc::channel();
    if let Err(e) = thread::Builder::new()
        .name("transcribe-worker-in".into())
        .spawn(move || read_requests(&requests_tx))
    {
        log::error!("failed to start the request reader: {e}");
        return 2;
    }

    let init_error = init_backends().err();
    let mut loaded: Option<Session> = None;

    while let Ok(Incoming { request, pcm }) = requests.recv() {
        let response = match request {
            Request::Hello => match &init_error {
                Some(message) => Response::Error(message.clone()),
                None => Response::Hello,
            },
            Request::Load { path, backend } => {
                // Free any previous model first to avoid holding two at once.
                loaded = None;
                match load(&path, backend) {
                    Ok((session, info)) => {
                        loaded = Some(session);
                        Response::Loaded(info)
                    }
                    Err(e) => Response::Error(e),
                }
            }
            Request::Run { options } => match loaded.as_mut() {
                Some(_) if inject_fault("run") => injected_backend_error(),
                Some(session) => match session.run(&pcm, &options) {
                    Ok(transcript) => Response::Transcript {
                        transcript,
                        truncated: session.was_truncated(),
                    },
                    // Stopped early: keep the text before it.
                    Err(e) => match e.partial() {
                        Some(partial) => {
                            log::warn!("decode stopped early, keeping the partial: {e}");
                            Response::Transcript {
                                transcript: partial.clone(),
                                truncated: true,
                            }
                        }
                        None => run_error(&e),
                    },
                },
                None => not_loaded(),
            },
            Request::StreamBegin { run, stream } => match loaded.as_mut() {
                Some(session) => {
                    match begin_stream(session, &run, &stream, &requests, &mut output) {
                        Ok(response) => response,
                        Err(e) => {
                            log::error!("protocol write failed during stream: {e}");
                            return 1;
                        }
                    }
                }
                None => not_loaded(),
            },
            Request::Feed | Request::Finalize | Request::StreamReset => {
                Response::Error("no active stream".to_string())
            }
        };
        if let Err(e) = write_message(&mut output, &response, None) {
            log::error!("protocol write failed: {e}");
            return 1;
        }
    }
    // Unreachable in practice: the reader exits the process on EOF.
    0
}

/// Register the compute backends: CPU only when asked, else the CUDA pack
/// first (ggml prefers it), then the bundled ones. A broken GPU driver
/// crashes or hangs here, before the hello is answered.
fn init_backends() -> Result<(), String> {
    transcribe_cpp::init_logging();
    inject_fault("init");
    let args: Vec<_> = std::env::args_os().collect();
    let cpu_only = args.iter().any(|arg| arg == CPU_ONLY_FLAG);
    let app_data = args
        .iter()
        .skip_while(|arg| *arg != APP_DATA_FLAG)
        .nth(1)
        .map(PathBuf::from);

    let init = if cpu_only {
        transcribe_cpp::init_backends_with(None::<&Path>, BackendMask::CPU)
    } else {
        if let Some(app_data) = &app_data {
            crate::models::cuda::register(app_data);
        }
        transcribe_cpp::init_backends_default()
    };
    // Fails only with no compute device at all; the hello reports it.
    init.map_err(|e| format!("failed to initialize transcribe.cpp backends: {e}"))?;
    let devices = transcribe_cpp::devices();
    log::info!(
        "{} compute device(s) [{}]",
        devices.len(),
        devices
            .iter()
            .map(|d| format!("{} ({})", d.name, d.kind))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

/// Forwards requests to the main loop, and ends the process when the parent
/// closes stdin. `_exit` skips C++ static destructors, so a live model can't
/// trip ggml's teardown asserts.
fn read_requests(requests: &mpsc::Sender<Incoming>) -> ! {
    let mut input = BufReader::new(io::stdin().lock());
    loop {
        match read_message::<Request>(&mut input) {
            Ok(Some((request, pcm))) => {
                if requests.send(Incoming { request, pcm }).is_err() {
                    exit_now(1);
                }
            }
            Ok(None) => exit_now(0),
            Err(e) => {
                log::error!("protocol read failed: {e}");
                exit_now(1);
            }
        }
    }
}

fn exit_now(code: i32) -> ! {
    let _ = io::stderr().flush();
    // SAFETY: `_exit` only ends the process; nothing runs afterwards.
    unsafe { libc::_exit(code) }
}

/// Begin a stream and serve it; returns the reply to its finalize or reset.
fn begin_stream(
    session: &mut Session,
    run: &RunOptions,
    options: &StreamOptions,
    requests: &mpsc::Receiver<Incoming>,
    output: &mut impl Write,
) -> io::Result<Response> {
    let end = match session.stream(run, options) {
        Ok(stream) => {
            write_message(output, &Response::Ok, None)?;
            serve_stream(stream, requests, output)?
        }
        Err(e) => return Ok(run_error(&e)),
    };
    // The stream is gone, so the session can say whether it was cut short.
    Ok(match end {
        StreamEnd::Finalized(text) => Response::Finalized {
            text,
            truncated: session.was_truncated(),
        },
        StreamEnd::Reply(response) => response,
    })
}

/// Serve requests against an active stream until it is finalized or reset.
fn serve_stream(
    mut stream: Stream<'_>,
    requests: &mpsc::Receiver<Incoming>,
    output: &mut impl Write,
) -> io::Result<StreamEnd> {
    while let Ok(Incoming { request, pcm }) = requests.recv() {
        let response = match request {
            Request::Feed if inject_fault("feed") => injected_backend_error(),
            Request::Feed => match stream.feed(&pcm) {
                Ok(update) => {
                    let text = (update.committed_changed || update.tentative_changed)
                        .then(|| stream.text());
                    Response::Fed { update, text }
                }
                Err(e) => run_error(&e),
            },
            Request::Finalize if inject_fault("finalize") => {
                return Ok(StreamEnd::Reply(injected_backend_error()))
            }
            // Finalize ends the stream even when it fails.
            Request::Finalize => {
                return Ok(match stream.finalize() {
                    Ok(_) => StreamEnd::Finalized(stream.text()),
                    Err(e) => StreamEnd::Reply(run_error(&e)),
                })
            }
            Request::StreamReset => {
                stream.reset();
                return Ok(StreamEnd::Reply(Response::Ok));
            }
            _ => Response::Error("a stream is active; finalize or reset it first".to_string()),
        };
        write_message(output, &response, None)?;
    }
    // Unreachable: the reader exits the process on EOF.
    Ok(StreamEnd::Reply(Response::Ok))
}

fn not_loaded() -> Response {
    Response::Error("no model loaded".to_string())
}

/// A failed run, feed or finalize. A backend error may break every later run,
/// so it gets its own response and the parent replaces the worker.
fn run_error(e: &transcribe_cpp::Error) -> Response {
    if e.kind() == ErrorKind::Backend {
        Response::BackendError(e.to_string())
    } else {
        Response::Error(e.to_string())
    }
}

fn load(path: &Path, backend: Backend) -> Result<(Session, LoadedInfo), String> {
    inject_fault("load");
    let model = Model::load_with(
        path,
        &ModelOptions {
            backend,
            ..Default::default()
        },
    )
    .map_err(|e| format!("failed to load the model: {e}"))?;

    // Half the cores, leaving headroom for the app's capture and UI threads.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let n_threads =
        (thread::available_parallelism().map_or(4, std::num::NonZero::get) / 2).clamp(1, 4) as i32;
    let session = model
        .session_with(&SessionOptions {
            n_threads,
            ..Default::default()
        })
        .map_err(|e| format!("failed to create a session: {e}"))?;

    let device = model.device().ok();
    let info = LoadedInfo {
        arch: model.arch(),
        // `Model::backend` names the device ("CUDA0"); the kind is what we show.
        backend: device
            .as_ref()
            .map_or_else(|| "cpu".to_string(), |d| d.kind.clone()),
        device: device
            .as_ref()
            .map_or_else(|| model.backend(), |d| d.name.clone()),
        on_gpu: device
            .as_ref()
            .is_some_and(|d| matches!(d.device_type, DeviceType::Gpu | DeviceType::Igpu)),
        capabilities: model
            .capabilities()
            .map_err(|e| format!("failed to read model capabilities: {e}"))?,
    };
    Ok((session, info))
}

fn injected_backend_error() -> Response {
    Response::BackendError("injected backend error".to_string())
}

/// Debug builds: fail on demand, to exercise recovery. [`FAULT_ENV`] names a
/// file holding `<abort|hang|backend>@<init|load|run|feed|finalize>`; the
/// first worker to reach that stage claims the file by deleting it, so each
/// fault fires once. Returns true for `backend`, which the caller answers.
#[cfg(debug_assertions)]
fn inject_fault(stage: &str) -> bool {
    let Some(path) = std::env::var_os(super::FAULT_ENV) else {
        return false;
    };
    let Ok(spec) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Some((kind, at)) = spec.trim().split_once('@') else {
        return false;
    };
    if at != stage || std::fs::remove_file(&path).is_err() {
        return false;
    }
    log::warn!("injected fault: {kind} at {stage}");
    match kind {
        "abort" => std::process::abort(),
        "hang" => loop {
            thread::sleep(std::time::Duration::from_secs(3600));
        },
        "backend" => true,
        _ => false,
    }
}

#[cfg(not(debug_assertions))]
fn inject_fault(_stage: &str) -> bool {
    false
}

/// Logs go to stderr as `\x01LEVEL\ttarget\tmessage` lines; the parent
/// re-logs them at the same level.
struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // transcribe.cpp/GGML per-token tracing is extremely verbose.
        let max = if metadata.target().starts_with("transcribe_cpp") {
            LevelFilter::Warn
        } else {
            log::max_level()
        };
        metadata.level() <= max
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        let mut stderr = io::stderr().lock();
        for line in message.lines() {
            let _ = writeln!(
                stderr,
                "{LOG_LINE_PREFIX}{}\t{}\t{}",
                record.level(),
                record.target(),
                line
            );
        }
    }

    fn flush(&self) {
        let _ = io::stderr().flush();
    }
}

fn init_logger() {
    static LOGGER: StderrLogger = StderrLogger;
    let level = std::env::var(LOG_LEVEL_ENV)
        .ok()
        .and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}

/// `pkill -USR2 nexusvoice` (dictation toggle) can reach this worker too, and
/// the default action would kill it.
#[cfg(unix)]
fn ignore_app_signals() {
    // SAFETY: setting a signal's disposition to SIG_IGN has no preconditions.
    unsafe {
        libc::signal(libc::SIGUSR1, libc::SIG_IGN);
        libc::signal(libc::SIGUSR2, libc::SIG_IGN);
    }
}

/// Not `nexusvoice`, so `pkill -n nexusvoice` keeps reaching the app.
#[cfg(target_os = "linux")]
fn set_process_name() {
    // SAFETY: PR_SET_NAME copies a NUL-terminated name of up to 16 bytes.
    unsafe {
        libc::prctl(libc::PR_SET_NAME, c"NexusVoice-wkr".as_ptr());
    }
}

#[cfg(unix)]
fn take_stdout_for_protocol() -> io::Result<File> {
    use std::os::fd::FromRawFd;
    // SAFETY: plain fd duplication; the new fd is owned by the returned File.
    unsafe {
        let fd = libc::dup(libc::STDOUT_FILENO);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO) < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(File::from_raw_fd(fd))
    }
}

#[cfg(windows)]
fn take_stdout_for_protocol() -> io::Result<File> {
    use std::os::windows::io::FromRawHandle;
    // Native code writes through the CRT's fd 1, so redirect that one. The
    // protocol uses the OS handle behind a CRT duplicate of the original.
    // SAFETY: CRT fd duplication; the duplicate's handle is owned by the File
    // for the rest of the process.
    unsafe {
        let fd = libc::dup(1);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::dup2(2, 1) < 0 {
            return Err(io::Error::last_os_error());
        }
        let handle = libc::get_osfhandle(fd);
        if handle == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(File::from_raw_handle(handle as _))
    }
}
