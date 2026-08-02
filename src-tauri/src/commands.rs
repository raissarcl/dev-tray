//! Tauri commands for the optional dashboard window (and future UI).
//! Tray menu talks to Rust directly; these commands are the IPC surface for TypeScript.

use crate::actions::{BrowserManager, EditorManager, FolderManager};
use crate::domain::{ProjectView, Settings};
use crate::state::AppState;
use crate::tray::TrayManager;
use crate::window::WindowManager;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectView>, String> {
    Ok(state.project_views())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings())
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>, id: String) -> Result<String, String> {
    Ok(state.processes.logs_snapshot(&id))
}

#[tauri::command]
pub async fn start_project(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let project = state.get_project(&id).map_err(|e| e.to_string())?;
    let refresh = refresh_callback(&app);
    state
        .processes
        .start(app.clone(), project, refresh)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_project(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let refresh = refresh_callback(&app);
    state
        .processes
        .stop(&app, &id, refresh)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_project(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let project = state.get_project(&id).map_err(|e| e.to_string())?;
    let refresh = refresh_callback(&app);
    state
        .processes
        .restart(app.clone(), project, refresh)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_browser(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.ensure_alias_infra().await;
    let url = state.require_url(&id).map_err(|e| e.to_string())?;
    println!("[dev-tray] Open Browser → {url}");
    BrowserManager::open_url(&url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_editor(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let project = state.get_project(&id).map_err(|e| e.to_string())?;
    let settings = state.settings();
    EditorManager::open_project(&project.path, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let project = state.get_project(&id).map_err(|e| e.to_string())?;
    FolderManager::open(&project.path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reload_config(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        state.config.lock().reload().map_err(|e| e.to_string())?;
    }
    {
        let state = app.state::<AppState>();
        state.apply_alias_infra().await;
    }
    TrayManager::rebuild_menu(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_dashboard(app: AppHandle) -> Result<(), String> {
    WindowManager::show(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_dashboard(app: AppHandle) -> Result<(), String> {
    WindowManager::destroy(&app).map_err(|e| e.to_string())
}

fn refresh_callback(app: &AppHandle) -> Arc<dyn Fn() + Send + Sync> {
    let app = app.clone();
    Arc::new(move || {
        let app = app.clone();
        let app_for_menu = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Err(err) = TrayManager::rebuild_menu(&app_for_menu) {
                eprintln!("[dev-tray] failed to rebuild tray menu: {err:#}");
            }
        });
    })
}
