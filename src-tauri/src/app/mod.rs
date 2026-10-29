//! Startup: logging, plugins, and wiring each owner into Tauri's managed state.

mod bindings;

use tauri::Manager;

use crate::data::Database;
use crate::models::{cache::EngineCache, cuda::CudaPack, library::ModelLibrary};
use crate::platform::{hardware, hotkeys::Hotkeys, pill, remote, tray};
use crate::session::Recorder;
use crate::settings::{Secrets, Settings};
use crate::text::dictionary::Dictionary;

const LOG_MAX_SIZE: u128 = 10 * 1024 * 1024;

/// Models live in the release data dir even in dev, so a dev build reuses
/// downloads. Everything else stays per-identifier. Dev's dir is the release
/// one plus a `.dev` suffix.
fn shared_models_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let shared_root = app_data_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".dev"))
        .map_or_else(
            || app_data_dir.to_path_buf(),
            |release| app_data_dir.with_file_name(release),
        );
    shared_root.join("models")
}

/// A short id for this run, on every log line, so interleaved runs can be told apart.
fn session_id() -> &'static str {
    use std::sync::OnceLock;
    static SESSION_ID: OnceLock<String> = OnceLock::new();
    SESSION_ID.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        #[allow(clippy::cast_possible_truncation)] // low 32 bits are all we want for an id
        let low = (nanos & 0xffff_ffff) as u64;
        format!("{low:08x}")
    })
}

/// Register compute backends before any model loads, the CUDA pack first so ggml prefers it.
fn init_compute_backends(app_data_dir: &std::path::Path) {
    crate::models::cuda::activate(app_data_dir);
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

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let file = tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
        file_name: Some("nexusvoice".into()),
    });
    #[cfg(debug_assertions)]
    let targets = vec![
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
        file,
    ];
    #[cfg(not(debug_assertions))]
    let targets = vec![file];

    tauri_plugin_log::Builder::new()
        .targets(targets)
        .level(log::LevelFilter::Info)
        // transcribe.cpp/GGML per-token tracing is extremely verbose; mute it.
        .level_for("transcribe_cpp", log::LevelFilter::Warn)
        .max_file_size(LOG_MAX_SIZE)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
        // One JSON object per line, so logs are queryable.
        .format(|out, message, record| {
            let line = serde_json::json!({
                "ts": chrono::Utc::now().to_rfc3339(),
                "level": record.level().as_str(),
                "target": record.target(),
                "session": session_id(),
                "msg": message.to_string(),
            });
            out.finish(format_args!("{line}"));
        })
        .build()
}

pub fn run() {
    // Route GGML's stderr through `log`, so the level filter applies to it.
    transcribe_cpp::init_logging();

    // Debug only: release unwinds, so `catch_unwind` on the recording path recovers.
    #[cfg(debug_assertions)]
    {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("PANIC: {info}");
            default_hook(info);
        }));
    }

    let specta_builder = bindings::command_bindings();

    // Debug only: regenerate the TS bindings. Release builds ship the committed file.
    #[cfg(debug_assertions)]
    bindings::export_bindings(&specta_builder);

    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let builder = builder.on_window_event(pill::window::on_window_event);
    builder
        // A second launch is either a dictation flag (the Wayland path, where
        // the desktop owns the shortcut) or the user reopening the window.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !remote::handle_args(app, &args) {
                tray::show_main_window(app);
            }
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            setup(app)?;
            tray::build(app)?;
            pill::window::spawn_create(app.handle().clone());
            #[cfg(windows)]
            pill::window::spawn_keep_on_top(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                app.state::<Recorder>().shutdown();
            }
        });
}

/// Manage every owner and start the background work they need.
fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
    std::fs::create_dir_all(&app_data_dir)?;
    init_compute_backends(&app_data_dir);

    let models_dir = shared_models_dir(&app_data_dir);
    std::fs::create_dir_all(&models_dir)?;
    // Nothing downloads yet, so any .part file is debris.
    crate::models::downloader::clean_stale_parts(&models_dir);

    // The database opens in the background; only the settings file is read here.
    let secrets = Secrets::new(app.config().identifier.clone());
    app.manage(Settings::load(&app_data_dir, secrets));
    app.manage(Database::default());
    app.manage(Dictionary::default());
    app.manage(EngineCache::new(models_dir.clone()));
    app.manage(ModelLibrary::new(models_dir));
    app.manage(CudaPack::new(app_data_dir.clone()));
    CudaPack::replace_unverified(app.handle());
    app.manage(Recorder::new());
    app.state::<Recorder>()
        .set_keep_mic_open(app.state::<Settings>().keep_mic_open());
    app.manage(Hotkeys::default());

    spawn_open_database(app.handle().clone(), app_data_dir.join("nexusvoice.db"));
    spawn_log_hardware();

    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        app_handle
            .state::<Hotkeys>()
            .restore(&app_handle, &app_handle.state::<Settings>())
            .await;
    });

    // Wayland refuses global shortcuts, so dictation is also reachable by signal.
    #[cfg(target_os = "linux")]
    remote::listen_for_signals(app.handle().clone());

    // Preload so the first transcription is instant; recording start retries.
    EngineCache::warm(app.handle());
    crate::models::idle::spawn_watcher(app.handle().clone());
    Ok(())
}

fn spawn_open_database(app: tauri::AppHandle, db_path: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        // May run migrations; this is the slow part.
        let result = crate::data::connection::open_database(&db_path).await;
        // Publish either outcome, so waiting commands are released.
        app.state::<Database>().set(result.clone());
        let pool = match result {
            Ok(pool) => pool,
            Err(e) => {
                log::error!("database init failed: {e}");
                return;
            }
        };
        log::info!("database ready");
        if let Err(e) = app.state::<Dictionary>().load(&pool).await {
            log::error!("could not load the dictionary: {e}");
        }
    });
}

/// Hardware detection makes blocking syscalls, so it must not run on the main thread.
fn spawn_log_hardware() {
    tauri::async_runtime::spawn(async move {
        // The cached profile, so recommend_model() reuses this probe.
        let (hw, recommended) = tokio::task::spawn_blocking(|| {
            let hw = hardware::cached_profile().clone();
            (hw, crate::models::provider::recommend_model())
        })
        .await
        .unwrap_or_else(|_| {
            // Detection failed; fall back to the catalog's entry level.
            (
                hardware::profile::HardwareProfile::default(),
                &crate::models::catalog::all()[0],
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
    });
}
