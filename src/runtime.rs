//! The tokio runtime the backend runs on. The UI thread belongs to GPUI, so
//! async work the UI starts is spawned here and awaited from GPUI's executor.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Build the runtime. Called once, before anything spawns.
pub fn init() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("nexusvoice-rt")
            .build()
            .expect("failed to build the tokio runtime")
    })
}

pub fn handle() -> &'static Handle {
    init().handle()
}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(future)
}
