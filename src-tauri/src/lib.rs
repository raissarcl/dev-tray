mod actions;
mod app;
mod commands;
mod config;
mod domain;
mod events;
mod process;
mod state;
mod tray;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
