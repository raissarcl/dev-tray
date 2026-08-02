use anyhow::{bail, Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

const BEGIN_MARKER: &str = "# BEGIN dev-tray";
const END_MARKER: &str = "# END dev-tray";

pub struct HostsManager;

impl HostsManager {
    pub fn hosts_path() -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
        }
        #[cfg(not(windows))]
        {
            PathBuf::from("/etc/hosts")
        }
    }

    /// Sync the managed block so it contains exactly `aliases` (127.0.0.1 each).
    pub fn sync(aliases: &[String]) -> Result<()> {
        let path = Self::hosts_path();
        let desired = build_block(aliases);
        let current = fs::read_to_string(&path).unwrap_or_default();
        let next = replace_or_append_block(&current, &desired);

        if next == current {
            return Ok(());
        }

        match write_hosts(&path, &next) {
            Ok(()) => {
                println!(
                    "[dev-tray] hosts updated ({} alias{})",
                    aliases.len(),
                    if aliases.len() == 1 { "" } else { "es" }
                );
                Ok(())
            }
            Err(err) if is_access_denied(&err) => {
                println!("[dev-tray] hosts write needs elevation; requesting UAC…");
                write_hosts_elevated(&path, &next)?;
                println!(
                    "[dev-tray] hosts updated via elevation ({} alias{})",
                    aliases.len(),
                    if aliases.len() == 1 { "" } else { "es" }
                );
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Remove the managed block entirely.
    pub fn clear() -> Result<()> {
        let path = Self::hosts_path();
        let current = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(err).with_context(|| format!("failed to read {}", path.display()))
            }
        };

        let next = remove_block(&current);
        if next == current {
            return Ok(());
        }

        match write_hosts(&path, &next) {
            Ok(()) => {
                println!("[dev-tray] hosts block removed");
                Ok(())
            }
            Err(err) if is_access_denied(&err) => {
                println!("[dev-tray] hosts clear needs elevation; requesting UAC…");
                write_hosts_elevated(&path, &next)?;
                println!("[dev-tray] hosts block removed via elevation");
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

fn build_block(aliases: &[String]) -> String {
    let mut lines = Vec::with_capacity(aliases.len() + 2);
    lines.push(BEGIN_MARKER.to_string());
    for alias in aliases {
        let host = alias.trim();
        if !host.is_empty() {
            lines.push(format!("127.0.0.1 {host}"));
        }
    }
    lines.push(END_MARKER.to_string());
    lines.join("\n")
}

fn replace_or_append_block(current: &str, block: &str) -> String {
    if let Some(next) = replace_block(current, block) {
        return ensure_trailing_newline(&next);
    }

    let trimmed = current.trim_end();
    if trimmed.is_empty() {
        ensure_trailing_newline(block)
    } else {
        ensure_trailing_newline(&format!("{trimmed}\n\n{block}"))
    }
}

fn replace_block(current: &str, block: &str) -> Option<String> {
    let start = current.find(BEGIN_MARKER)?;
    let end_rel = current[start..].find(END_MARKER)?;
    let end = start + end_rel + END_MARKER.len();

    // Consume a single trailing newline after the end marker if present.
    let mut end = end;
    if current[end..].starts_with('\r') {
        end += 1;
    }
    if current[end..].starts_with('\n') {
        end += 1;
    }

    let mut next = String::with_capacity(current.len() + block.len());
    next.push_str(&current[..start]);
    next.push_str(block);
    if !block.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&current[end..]);
    Some(next)
}

fn remove_block(current: &str) -> String {
    let Some(start) = current.find(BEGIN_MARKER) else {
        return current.to_string();
    };
    let Some(end_rel) = current[start..].find(END_MARKER) else {
        return current.to_string();
    };
    let mut end = start + end_rel + END_MARKER.len();
    if current[end..].starts_with('\r') {
        end += 1;
    }
    if current[end..].starts_with('\n') {
        end += 1;
    }

    // Also drop a blank line immediately before the block when we created one.
    let mut start = start;
    if start >= 2 && &current[start - 2..start] == "\n\n" {
        start -= 1;
    } else if start >= 1 && current.as_bytes()[start - 1] == b'\n' {
        // keep a single newline separator by removing only the block
    }

    let mut next = String::new();
    next.push_str(&current[..start]);
    next.push_str(&current[end..]);
    ensure_trailing_newline(next.trim_end())
}

fn ensure_trailing_newline(s: &str) -> String {
    if s.is_empty() || s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{s}\n")
    }
}

fn write_hosts(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn is_access_denied(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            if io.kind() == ErrorKind::PermissionDenied {
                return true;
            }
            #[cfg(windows)]
            if io.raw_os_error() == Some(5) {
                return true;
            }
        }
    }
    false
}

/// Elevate via PowerShell UAC prompt and overwrite hosts with the prepared contents.
#[cfg(windows)]
fn write_hosts_elevated(path: &Path, contents: &str) -> Result<()> {
    let temp = std::env::temp_dir().join("dev-tray-hosts.txt");
    fs::write(&temp, contents)
        .with_context(|| format!("failed to write temp hosts file {}", temp.display()))?;

    let path_str = path.display().to_string();
    let temp_str = temp.display().to_string();
    // -Wait so we know whether the elevated copy succeeded.
    let script = format!(
        "Start-Process -FilePath 'cmd.exe' -ArgumentList '/c copy /Y \"{temp}\" \"{path}\"' -Verb RunAs -Wait",
        temp = temp_str.replace('\'', "''"),
        path = path_str.replace('\'', "''"),
    );

    let status = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .status()
        .context("failed to launch elevated hosts writer")?;

    let _ = fs::remove_file(&temp);

    if !status.success() {
        bail!(
            "elevated hosts update failed (exit {}). Run Dev Tray as Administrator once, or set manage_hosts to false.",
            status.code().unwrap_or(-1)
        );
    }

    // Verify the write landed (UAC cancel still returns confusing codes sometimes).
    let written = fs::read_to_string(path).unwrap_or_default();
    if !written.contains(BEGIN_MARKER) && contents.contains(BEGIN_MARKER) {
        bail!("hosts update did not apply (UAC cancelled?)");
    }
    if contents.contains(BEGIN_MARKER) {
        // sync case — block should match aliases; soft check
        if !written.contains(BEGIN_MARKER) {
            bail!("hosts update did not apply");
        }
    } else if written.contains(BEGIN_MARKER) {
        // clear case
        bail!("hosts clear did not apply (UAC cancelled?)");
    }

    Ok(())
}

#[cfg(not(windows))]
fn write_hosts_elevated(path: &Path, contents: &str) -> Result<()> {
    let _ = (path, contents);
    bail!("elevated hosts updates are only supported on Windows");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_block() {
        let current = "127.0.0.1 localhost\n\n# BEGIN dev-tray\n127.0.0.1 old\n# END dev-tray\n";
        let block = build_block(&["new".into()]);
        let next = replace_or_append_block(current, &block);
        assert!(next.contains("127.0.0.1 new"));
        assert!(!next.contains("127.0.0.1 old"));
        assert!(next.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn removes_block() {
        let current = "127.0.0.1 localhost\n\n# BEGIN dev-tray\n127.0.0.1 app\n# END dev-tray\n";
        let next = remove_block(current);
        assert!(!next.contains("BEGIN dev-tray"));
        assert!(next.contains("127.0.0.1 localhost"));
    }
}
