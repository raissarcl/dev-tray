use crate::domain::Settings;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct EditorManager;

impl EditorManager {
    pub fn open_project(path: &Path, settings: &Settings) -> Result<()> {
        let editor = settings
            .editor_path
            .as_deref()
            .unwrap_or(settings.editor.as_str());

        let mut cmd = Command::new(editor);
        cmd.arg(path);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(err) if settings.editor != "code" && settings.editor_path.is_none() => {
                // Fallback to VS Code CLI if Cursor is unavailable.
                let mut fallback = Command::new("code");
                fallback.arg(path);
                #[cfg(windows)]
                fallback.creation_flags(CREATE_NO_WINDOW);
                fallback.spawn().with_context(|| {
                    format!(
                        "failed to open editor '{editor}' ({err}); also failed to open 'code'"
                    )
                })?;
                Ok(())
            }
            Err(err) => Err(err).with_context(|| format!("failed to open editor '{editor}'")),
        }
    }
}
