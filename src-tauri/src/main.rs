#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod bridge;
mod data;
mod engine;
mod jobs;
mod models;
mod platform;
mod recordings;
mod session;
mod settings;
mod sync;
mod text;

fn main() {
    // Before Tauri and single-instance handling: a worker is not the app.
    if engine::is_worker_invocation() {
        std::process::exit(engine::run_worker());
    }
    engine::record_launched_exe();
    app::run();
}
