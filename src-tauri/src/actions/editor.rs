use crate::domain::Settings;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct EditorManager;

impl EditorManager {
    pub fn open_project(path: &Path, settings: &Settings) -> Result<()> {
        if !path.exists() {
            bail!("project path does not exist: {}", path.display());
        }

        let program = resolve_editor(settings)?;
        println!(
            "[dev-tray] Open Cursor → {} ({})",
            path.display(),
            program.display()
        );

        spawn_editor(&program, path).with_context(|| {
            format!(
                "failed to open '{}' with editor '{}'",
                path.display(),
                program.display()
            )
        })?;
        Ok(())
    }
}

fn resolve_editor(settings: &Settings) -> Result<PathBuf> {
    if let Some(custom) = settings.editor_path.as_deref() {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
        bail!("editor_path is not a file: {custom}");
    }

    let name = settings.editor.trim();
    if name.is_empty() {
        bail!("settings.editor is empty");
    }

    let as_path = PathBuf::from(name);
    if as_path.is_file() {
        return Ok(as_path);
    }

    #[cfg(windows)]
    {
        if let Some(found) = resolve_windows_editor(name) {
            return Ok(found);
        }
        bail!(
            "could not find editor '{name}'. Install Cursor/VS Code or set settings.editor_path to the .exe"
        );
    }

    #[cfg(not(windows))]
    Ok(PathBuf::from(name))
}

#[cfg(windows)]
fn resolve_windows_editor(name: &str) -> Option<PathBuf> {
    let key = name.to_ascii_lowercase();
    let candidates: Vec<PathBuf> = match key.as_str() {
        "cursor" => cursor_candidates(),
        "code" | "vscode" | "vs code" => vscode_candidates(),
        _ => Vec::new(),
    };

    candidates.into_iter().find(|p| p.is_file()).or_else(|| {
        // Prefer real .exe/.cmd on PATH — never the extensionless Unix shim (`cursor`).
        find_on_path(&[
            &format!("{name}.exe"),
            &format!("{name}.cmd"),
            &format!("{name}.bat"),
        ])
    })
}

#[cfg(windows)]
fn cursor_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(local).join(r"Programs\cursor\Cursor.exe"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        out.push(PathBuf::from(pf).join(r"Cursor\Cursor.exe"));
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        out.push(PathBuf::from(pf86).join(r"Cursor\Cursor.exe"));
    }
    out
}

#[cfg(windows)]
fn vscode_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(local).join(r"Programs\Microsoft VS Code\Code.exe"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        out.push(PathBuf::from(pf).join(r"Microsoft VS Code\Code.exe"));
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        out.push(PathBuf::from(pf86).join(r"Microsoft VS Code\Code.exe"));
    }
    out
}

#[cfg(windows)]
fn find_on_path(file_names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in file_names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn spawn_editor(program: &Path, project: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let ext = program
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // .cmd/.bat shims need cmd.exe; GUI .exe can be spawned directly.
        if matches!(ext.as_str(), "cmd" | "bat") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", "call"])
                .arg(program)
                .arg(project)
                .creation_flags(CREATE_NO_WINDOW);
            cmd.spawn()?;
            return Ok(());
        }

        let mut cmd = Command::new(program);
        cmd.arg(project).creation_flags(CREATE_NO_WINDOW);
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(primary) => {
                // Fallback: VS Code CLI if Cursor.exe failed and we weren't already on code.
                let code = vscode_candidates()
                    .into_iter()
                    .find(|p| p.is_file())
                    .or_else(|| find_on_path(&["code.cmd", "code.exe"]));
                let Some(code) = code else {
                    return Err(primary).context("no VS Code fallback found");
                };
                if code == program {
                    return Err(primary).context("editor spawn failed");
                }
                eprintln!(
                    "[dev-tray] editor spawn failed ({primary}); falling back to {}",
                    code.display()
                );
                let mut fallback = Command::new(&code);
                fallback.arg(project).creation_flags(CREATE_NO_WINDOW);
                fallback.spawn()?;
                Ok(())
            }
        }
    }

    #[cfg(not(windows))]
    {
        Command::new(program).arg(project).spawn()?;
        Ok(())
    }
}
