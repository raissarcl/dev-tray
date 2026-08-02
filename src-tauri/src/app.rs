use crate::commands;
use crate::config::ConfigManager;
use crate::state::AppState;
use crate::tray::TrayManager;
use crate::window::WindowManager;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, RunEvent};

/// Set when the user chooses Quit from the tray so ExitRequested is not cancelled.
pub static FORCE_EXIT: AtomicBool = AtomicBool::new(false);

pub fn run() {
    let config = ConfigManager::init().unwrap_or_else(|err| {
        eprintln!("[dev-tray] failed to load config: {err:#}");
        std::process::exit(1);
    });
    let log_capacity = config.repository().settings().log_lines;
    let state = AppState::new(config, log_capacity);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::get_settings,
            commands::get_logs,
            commands::start_project,
            commands::stop_project,
            commands::restart_project,
            commands::open_browser,
            commands::open_editor,
            commands::open_folder,
            commands::reload_config,
            commands::open_dashboard,
            commands::close_dashboard,
        ])
        .setup(|app| {
            WindowManager::ensure_no_window_at_boot(app.handle())?;
            TrayManager::create(app.handle())?;

            // Block until hosts + proxy are ready so Open Browser uses aliases, not localhost:port.
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let state = handle.state::<AppState>();
                state.apply_alias_infra().await;
                let status = state.proxy.status();
                match status.listen_port {
                    Some(port) => println!(
                        "[dev-tray] aliases ready (proxy :{port}, hosts_active={})",
                        status.hosts_active
                    ),
                    None => eprintln!(
                        "[dev-tray] alias proxy is not listening — Open Browser will use *.localhost or fallback URLs"
                    ),
                }
            });

            println!(
                "[dev-tray] running in tray. Config: {}",
                ConfigManager::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "(unknown)".into())
            );
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Dev Tray")
        .run(|app_handle, event| match event {
            // Tray-resident: closing the optional window must not quit the app.
            RunEvent::ExitRequested { api, .. } => {
                if !FORCE_EXIT.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
            RunEvent::Exit => {
                let state = app_handle.state::<AppState>();
                state.cleanup_aliases_on_exit();
                let app = app_handle.clone();
                tauri::async_runtime::block_on(async move {
                    state.processes.stop_all(&app).await;
                });
            }
            _ => {}
        });
}
