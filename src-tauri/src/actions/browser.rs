use anyhow::{bail, Context, Result};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct BrowserManager;

impl BrowserManager {
    pub fn open_url(url: &str) -> Result<()> {
        if url.trim().is_empty() {
            bail!("project has no URL configured");
        }
        open::that(url).with_context(|| format!("failed to open browser for {url}"))
    }

    /// Fallback via Windows `start` if the `open` crate fails.
    #[allow(dead_code)]
    pub fn open_url_shell(url: &str) -> Result<()> {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .with_context(|| format!("failed to open URL via shell: {url}"))?;
        Ok(())
    }
}
