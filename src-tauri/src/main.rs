#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod bridge;
mod data;
mod models;
mod platform;
mod session;
mod settings;
mod sync;
mod text;

fn main() {
    app::run();
}
