#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod audio;
mod database;
mod events;
mod focus;
mod hardware;
mod inference;
mod injection;
mod llm;
mod platform;
mod postprocess;
mod preprocess;
mod remote;
mod runtime;
mod state;
mod transcribe;
mod transcription;
mod ui;

use platform::paths;

fn main() {
    platform::logging::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !platform::single_instance::acquire(&args) {
        return;
    }

    // Route transcribe.cpp/GGML's verbose stderr output through `log` so our
    // level filter controls it instead of it flooding stdout.
    transcribe_cpp::init_logging();

    // Debug only: release unwinds so `catch_unwind` on the recording path can
    // recover instead.
    #[cfg(debug_assertions)]
    {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("PANIC: {info}");
            default_hook(info);
        }));
    }

    if let Err(e) = boot() {
        log::error!("startup failed: {e}");
        return;
    }

    ui::run();

    shutdown();
}

/// Bring the backend up. Only directory creation blocks; the database, model
/// warmup and hardware probe all start in the background.
fn boot() -> std::io::Result<()> {
    let app_data_dir = paths::app_data_dir().clone();
    std::fs::create_dir_all(&app_data_dir)?;
    init_compute_backends(&app_data_dir);

    let models_dir = paths::shared_models_dir();
    std::fs::create_dir_all(&models_dir)?;
    // Nothing can be mid-download at startup, so any .part file is debris from
    // a crash or a quit during a download.
    inference::downloader::clean_stale_parts(&models_dir);

    let state = state::install(state::AppState::new(
        app_data_dir.clone(),
        app_data_dir.join("hotkeys.json"),
        app_data_dir.join("model_override"),
        app_data_dir.join("language"),
        app_data_dir.join("format_config.json"),
        app_data_dir.join("input_device"),
        models_dir,
    ));

    let _guard = runtime::handle().enter();
    transcription::controller::spawn();
    tokio::spawn(open_database(state));
    tokio::spawn(log_hardware());
    // So the first transcription is instant. With no model downloaded yet this
    // is a no-op, and recording start kicks the load off again.
    tokio::spawn(async move {
        match state.get_or_load_engine().await {
            Ok(_) => log::info!("transcription engine pre-loaded and warmed up"),
            Err(e) => log::info!("transcription engine pre-load skipped: {e}"),
        }
    });
    inference::idle::spawn_watcher();

    // Global shortcuts cannot be registered under Wayland, so dictation is
    // also reachable by signal for users who bind it themselves.
    #[cfg(target_os = "linux")]
    remote::listen_for_signals();

    Ok(())
}

/// Register compute backends before any model loads, the CUDA pack first so ggml prefers it.
fn init_compute_backends(app_data_dir: &std::path::Path) {
    inference::cuda::activate(app_data_dir);
    match transcribe_cpp::init_backends_default() {
        Ok(()) => {
            let devices = transcribe_cpp::devices();
            log::info!(
                "transcribe.cpp: {} compute device(s) [{}]",
                devices.len(),
                devices
                    .iter()
                    .map(|d| format!("{} ({})", d.name, d.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Err(e) => log::warn!("transcribe.cpp backend init failed: {e}"),
    }
}

/// Open the database (migrations are the slow part), then fill the dictionary
/// cache. Either outcome is published, so waiting calls are released.
async fn open_database(state: &'static state::AppState) {
    use database::repositories::dictionary::DictionaryRepository;

    let result = database::connection::open_database(&state.app_data_dir.join("nexusvoice.db")).await;
    state.set_database(result.clone());
    let pool = match result {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("database init failed: {e}");
            return;
        }
    };
    log::info!("database ready");

    let entries = DictionaryRepository::new(pool)
        .list_all()
        .await
        .unwrap_or_default();
    *state.dict_cache.write().await = entries.into_iter().map(|e| (e.term.clone(), e)).collect();
}

/// Hardware detection makes blocking syscalls, so it runs off the async workers.
async fn log_hardware() {
    let (hw, recommended) = tokio::task::spawn_blocking(|| {
        let hw = hardware::cached_profile().clone();
        (hw, inference::provider::recommend_model())
    })
    .await
    .unwrap_or_else(|_| {
        // Detection failed; fall back to the catalog's entry level.
        (
            hardware::profile::HardwareProfile::default(),
            &inference::catalog::all()[0],
        )
    });

    log::info!("NexusVoice v{} starting", env!("CARGO_PKG_VERSION"));
    log::info!("OS: {}", std::env::consts::OS);
    log::info!("RAM: {:.1} GB", hw.ram_gb);
    log::info!(
        "GPU: {} ({}, {:.1} GB VRAM)",
        hw.gpu_type,
        hw.execution_provider,
        hw.vram_gb
    );
    log::info!("Recommended model: {}", recommended.display_name);
}

/// Stop any recording and release the microphone before the process exits.
fn shutdown() {
    let state = state::get();
    state
        .transcription_running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    state.reset_recording_session();
    state.mic.shutdown();
    log::logger().flush();
}
