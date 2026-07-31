use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct FolderManager;

impl FolderManager {
    pub fn open(path: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            let mut cmd = Command::new("explorer");
            cmd.arg(path);
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.spawn()
                .with_context(|| format!("failed to open folder {}", path.display()))?;
            return Ok(());
        }

        #[cfg(not(windows))]
        {
            open::that(path)
                .with_context(|| format!("failed to open folder {}", path.display()))
        }
    }
}
