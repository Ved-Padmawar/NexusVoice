//! Process isolation for transcribe.cpp.
//!
//! All transcribe.cpp native code (backend init, model load, inference,
//! streaming) runs in a child process: this executable relaunched with
//! [`WORKER_FLAG`]. A GPU driver fault, `GGML_ASSERT` abort or hang then
//! takes down only the worker, which the [`Supervisor`] kills and replaces.
//!
//! - `protocol`: the framed request/response protocol between the two.
//! - `supervisor`: the parent side, one [`Supervisor`] per loaded model.
//! - `worker`: the child process.
//!
//! Adapted from Handy's `engine_supervisor` (<https://github.com/cjpais/Handy>),
//! MIT License, Copyright (c) 2025 CJ Pais.

mod protocol;
mod supervisor;
mod worker;

pub use supervisor::{
    gpu_unavailable, record_launched_exe, worker_exe, CancelMark, EngineError, Finalized, Ran,
    StreamHandle, Supervisor, WorkerSpec,
};

/// Hidden first argument that turns the executable into a worker.
const WORKER_FLAG: &str = "--transcribe-worker";
/// Worker argument: register only the CPU backends.
const CPU_ONLY_FLAG: &str = "--cpu-only";
/// Worker argument: the app data dir, followed by its path, where the CUDA
/// pack lives.
const APP_DATA_FLAG: &str = "--app-data";
const LOG_LEVEL_ENV: &str = "NEXUSVOICE_WORKER_LOG";
/// Debug builds: a file naming a fault for the worker to inject.
#[cfg(debug_assertions)]
const FAULT_ENV: &str = "NEXUSVOICE_WORKER_FAULT";

/// Whether this process was launched as a transcription worker. Checked in
/// `main` before Tauri or single-instance handling.
pub fn is_worker_invocation() -> bool {
    std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == WORKER_FLAG)
}

/// Run the worker until the parent closes its stdin. Returns the exit code.
pub fn run_worker() -> i32 {
    worker::run()
}
