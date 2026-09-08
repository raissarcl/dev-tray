use anyhow::{bail, Context, Result};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Manages the optional dashboard window.
/// MVP: window is created only on demand and destroyed on close (frees WebView2).
#[allow(dead_code)] // helpers reserved for tray / future UI wiring
pub struct WindowManager;

impl WindowManager {
    pub const WINDOW_LABEL: &'static str = "main";

    pub fn is_open(app: &AppHandle) -> bool {
        app.get_webview_window(Self::WINDOW_LABEL).is_some()
    }

    /// Show the dashboard, creating it on first use.
    ///
    /// On Windows, `WebviewWindowBuilder::build` must not run on the UI / tray
    /// event thread (WebView2 deadlock). Creation always happens on a fresh OS
    /// thread; focusing an existing window is scheduled onto the main thread.
    pub fn show(app: &AppHandle) -> Result<()> {
        if let Some(window) = app.get_webview_window(Self::WINDOW_LABEL) {
            let window = window.clone();
            app.run_on_main_thread(move || {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            })?;
            return Ok(());
        }

        let app = app.clone();
        std::thread::Builder::new()
            .name("dev-tray-dashboard".into())
            .spawn(move || {
                if let Err(err) = Self::create_window(&app) {
                    eprintln!("[dev-tray] failed to open dashboard: {err:#}");
                }
            })
            .context("failed to spawn dashboard window thread")?;

        Ok(())
    }

    fn create_window(app: &AppHandle) -> Result<()> {
        // Another click may have won the race while this thread was starting.
        if let Some(window) = app.get_webview_window(Self::WINDOW_LABEL) {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.unminimize();
            return Ok(());
        }

        let window = WebviewWindowBuilder::new(
            app,
            Self::WINDOW_LABEL,
            WebviewUrl::App("index.html".into()),
        )
        .title("Dev Tray")
        .inner_size(720.0, 520.0)
        .resizable(true)
        .visible(true)
        .focused(true)
        .build()
        .context("WebviewWindowBuilder::build failed")?;

        let _ = window.show();
        let _ = window.set_focus();

        let label = Self::WINDOW_LABEL.to_string();
        let app_handle = app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Some(win) = app_handle.get_webview_window(&label) {
                    let _ = win.destroy();
                }
            }
        });

        Ok(())
    }

    pub fn hide(app: &AppHandle) -> Result<()> {
        if let Some(window) = app.get_webview_window(Self::WINDOW_LABEL) {
            window.hide()?;
        }
        Ok(())
    }

    pub fn destroy(app: &AppHandle) -> Result<()> {
        if let Some(window) = app.get_webview_window(Self::WINDOW_LABEL) {
            window.destroy()?;
        }
        Ok(())
    }

    pub fn toggle(app: &AppHandle) -> Result<()> {
        if Self::is_open(app) {
            Self::destroy(app)
        } else {
            Self::show(app)
        }
    }

    /// Stub used at startup — intentionally does nothing so no WebView is loaded.
    pub fn ensure_no_window_at_boot(_app: &AppHandle) -> Result<()> {
        Ok(())
    }

    pub fn unsupported_message() -> Result<()> {
        bail!("dashboard window is available via tray → Open Dashboard")
    }
}
