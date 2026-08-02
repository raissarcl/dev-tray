use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub type ProjectId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Stopped,
    Running,
    Starting,
    Stopping,
}

impl ProjectStatus {
    pub fn label_prefix(self) -> &'static str {
        match self {
            Self::Running | Self::Starting => "🟢",
            Self::Stopped | Self::Stopping => "⚪",
        }
    }

    pub fn is_running(self) -> bool {
        matches!(self, Self::Running | Self::Starting)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub command: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub url: Option<String>,
    /// Hostname alias for the local reverse proxy / hosts file. Defaults to `id`.
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub stop_command: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl Project {
    /// Hostname used for hosts entries and Host-based proxy routing.
    pub fn alias_key(&self) -> &str {
        self.alias
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .unwrap_or(self.id.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_editor")]
    pub editor: String,
    #[serde(default = "default_log_lines")]
    pub log_lines: usize,
    #[serde(default)]
    pub editor_path: Option<String>,
    /// When true, Dev Tray runs a local reverse proxy for project aliases.
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    /// Forced proxy listen port. `null` tries 80, then falls back to 8787.
    #[serde(default)]
    pub proxy_port: Option<u16>,
    /// When true, sync project aliases into the Windows hosts file.
    #[serde(default = "default_true")]
    pub manage_hosts: bool,
    /// When true, leave hosts entries after Quit. Default clears them.
    #[serde(default)]
    pub hosts_persist: bool,
}

fn default_editor() -> String {
    "cursor".to_string()
}

fn default_log_lines() -> usize {
    500
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor: default_editor(),
            log_lines: default_log_lines(),
            editor_path: None,
            proxy_enabled: true,
            proxy_port: None,
            manage_hosts: true,
            hosts_persist: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub projects: Vec<Project>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            settings: Settings::default(),
            projects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    pub command: String,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub alias: Option<String>,
    pub icon: Option<String>,
    pub status: ProjectStatus,
}
