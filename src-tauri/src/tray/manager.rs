use crate::actions::{BrowserManager, EditorManager, FolderManager};
use crate::state::AppState;
use crate::window::WindowManager;
use anyhow::{Context, Result};
use std::fs;
use std::sync::Arc;
use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "main-tray";

pub struct TrayManager;

impl TrayManager {
    pub fn create(app: &AppHandle) -> Result<()> {
        let menu = Self::build_menu(app)?;
        let icon = Self::load_tray_icon(app)?;

        let _tray = TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .icon_as_template(false)
            .menu(&menu)
            .tooltip("Dev Tray — click for projects")
            .show_menu_on_left_click(true)
            .on_menu_event(move |app, event| {
                let id = event.id().as_ref().to_string();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = Self::handle_menu_event(&app, &id).await {
                        eprintln!("[dev-tray] menu action failed: {err:#}");
                    }
                });
            })
            .on_tray_icon_event(|_tray, event| {
                let _ = event;
            })
            .build(app)
            .context("failed to create system tray icon")?;

        println!("[dev-tray] tray icon registered (id={TRAY_ID})");
        Ok(())
    }

    fn load_tray_icon(app: &AppHandle) -> Result<tauri::image::Image<'static>> {
        const ICON_PNG: &[u8] = include_bytes!("../../icons/32x32.png");
        match tauri::image::Image::from_bytes(ICON_PNG) {
            Ok(icon) => Ok(icon),
            Err(primary) => {
                let _ = app;
                Err(primary).context("failed to decode tray icon PNG")
            }
        }
    }

    pub fn rebuild_menu(app: &AppHandle) -> Result<()> {
        let menu = Self::build_menu(app)?;
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            tray.set_menu(Some(menu))?;
        }
        Ok(())
    }

    fn build_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>> {
        let state = app.state::<AppState>();
        let views = state.project_views();
        let mut builder = MenuBuilder::new(app);

        if views.is_empty() {
            let empty = MenuItem::with_id(
                app,
                "no-projects",
                "No projects configured",
                false,
                None::<&str>,
            )?;
            builder = builder.item(&empty);
        } else {
            for project in &views {
                let label = format!("{} {}", project.status.label_prefix(), project.name);
                let running = project.status.is_running();
                let id = &project.id;

                let start = MenuItem::with_id(
                    app,
                    format!("{id}::start"),
                    "Start",
                    !running,
                    None::<&str>,
                )?;
                let stop = MenuItem::with_id(
                    app,
                    format!("{id}::stop"),
                    "Stop",
                    running,
                    None::<&str>,
                )?;
                let restart = MenuItem::with_id(
                    app,
                    format!("{id}::restart"),
                    "Restart",
                    true,
                    None::<&str>,
                )?;
                let sep = PredefinedMenuItem::separator(app)?;
                let browser = MenuItem::with_id(
                    app,
                    format!("{id}::browser"),
                    "Open Browser",
                    true,
                    None::<&str>,
                )?;
                let cursor = MenuItem::with_id(
                    app,
                    format!("{id}::cursor"),
                    "Open Cursor",
                    true,
                    None::<&str>,
                )?;
                let folder = MenuItem::with_id(
                    app,
                    format!("{id}::folder"),
                    "Open Folder",
                    true,
                    None::<&str>,
                )?;
                let logs = MenuItem::with_id(
                    app,
                    format!("{id}::logs"),
                    "Show Logs",
                    true,
                    None::<&str>,
                )?;

                let submenu = SubmenuBuilder::new(app, &label)
                    .item(&start)
                    .item(&stop)
                    .item(&restart)
                    .item(&sep)
                    .item(&browser)
                    .item(&cursor)
                    .item(&folder)
                    .item(&logs)
                    .build()?;

                builder = builder.item(&submenu);
            }
        }

        let sep = PredefinedMenuItem::separator(app)?;
        let dashboard = MenuItem::with_id(
            app,
            "open-dashboard",
            "Open Dashboard",
            true,
            None::<&str>,
        )?;
        let reload = MenuItem::with_id(
            app,
            "reload-config",
            "Reload Config",
            true,
            None::<&str>,
        )?;
        let sep2 = PredefinedMenuItem::separator(app)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        Ok(builder
            .item(&sep)
            .item(&dashboard)
            .item(&reload)
            .item(&sep2)
            .item(&quit)
            .build()?)
    }

    async fn handle_menu_event(app: &AppHandle, event_id: &str) -> Result<()> {
        match event_id {
            "quit" => {
                let state = app.state::<AppState>();
                state.processes.stop_all(app).await;
                // User-initiated quit may elevate to clear hosts; OS shutdown must not.
                state.cleanup_aliases_on_exit(true);
                app.exit(0);
                Ok(())
            }
            "open-dashboard" => {
                // WindowManager::show spawns its own OS thread for WebView2 creation.
                WindowManager::show(app)
            }
            "reload-config" => {
                {
                    let state = app.state::<AppState>();
                    state.config.lock().reload()?;
                }
                {
                    let state = app.state::<AppState>();
                    state.apply_alias_infra().await;
                }
                Self::rebuild_menu(app)?;
                Ok(())
            }
            "no-projects" => Ok(()),
            other => {
                let Some((project_id, action)) = other.split_once("::") else {
                    return Ok(());
                };
                Self::handle_project_action(app, project_id, action).await
            }
        }
    }

    async fn handle_project_action(app: &AppHandle, project_id: &str, action: &str) -> Result<()> {
        let state = app.state::<AppState>();
        let refresh = {
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
        };

        match action {
            "start" => {
                let project = state.get_project(project_id)?;
                state
                    .processes
                    .start(app.clone(), project, refresh)
                    .await?;
            }
            "stop" => {
                state.processes.stop(app, project_id, refresh).await?;
            }
            "restart" => {
                let project = state.get_project(project_id)?;
                state
                    .processes
                    .restart(app.clone(), project, refresh)
                    .await?;
            }
            "browser" => {
                state.ensure_alias_infra().await;
                let url = state.require_url(project_id)?;
                println!("[dev-tray] Open Browser → {url}");
                BrowserManager::open_url(&url)?;
            }
            "cursor" => {
                let project = state.get_project(project_id)?;
                let settings = state.settings();
                EditorManager::open_project(&project.path, &settings)?;
            }
            "folder" => {
                let project = state.get_project(project_id)?;
                FolderManager::open(&project.path)?;
            }
            "logs" => {
                Self::show_logs(app, project_id)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn show_logs(app: &AppHandle, project_id: &str) -> Result<()> {
        let state = app.state::<AppState>();
        let contents = state.processes.logs_snapshot(project_id);
        let project = state.get_project(project_id)?;

        let dir = std::env::temp_dir().join("dev-tray-logs");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{project_id}.log"));
        let header = format!(
            "# {} ({})\n# path: {}\n# command: {}\n\n",
            project.name,
            project.id,
            project.path.display(),
            project.command
        );
        fs::write(&path, format!("{header}{contents}\n"))?;

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use std::process::Command;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", "start", "", &path.to_string_lossy()]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.spawn()
                .with_context(|| format!("failed to open log file {}", path.display()))?;
        }

        #[cfg(not(windows))]
        {
            open::that(&path)?;
        }

        Ok(())
    }
}
