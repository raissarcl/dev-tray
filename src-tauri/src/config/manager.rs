use crate::config::ProjectRepository;
use crate::domain::AppConfig;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const APP_DIR_NAME: &str = "tray-for-projects";
const CONFIG_FILE_NAME: &str = "projects.json";
const EXAMPLE_FILE_NAME: &str = "projects.example.json";

pub struct ConfigManager {
    repository: ProjectRepository,
}

impl ConfigManager {
    pub fn init() -> Result<Self> {
        let config_path = Self::resolve_config_path()?;
        if !config_path.exists() {
            Self::seed_default_config(&config_path)?;
        }
        let repository = ProjectRepository::load(config_path)?;
        Ok(Self { repository })
    }

    /// Public helper used by startup logs / tray.
    pub fn config_path() -> Result<PathBuf> {
        Self::resolve_config_path()
    }

    pub fn repository(&self) -> &ProjectRepository {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut ProjectRepository {
        &mut self.repository
    }

    pub fn reload(&mut self) -> Result<()> {
        let path = self.repository.config_path().to_path_buf();
        self.repository = ProjectRepository::load(path)?;
        Ok(())
    }

    /// Resolution order (first existing `projects.json` wins; otherwise first writable seed target):
    /// 1. `DEV_TRAY_CONFIG` env var
    /// 2. `./projects.json` (current working directory)
    /// 3. next to the executable
    /// 4. app/repo root discovered via `projects.example.json` (or `projects.json`) nearby
    /// 5. `%APPDATA%\tray-for-projects\projects.json` (legacy fallback)
    fn resolve_config_path() -> Result<PathBuf> {
        if let Ok(custom) = env::var("DEV_TRAY_CONFIG") {
            let path = PathBuf::from(custom);
            if path.exists() || path.parent().map(Path::exists).unwrap_or(false) {
                return Ok(path);
            }
        }

        for candidate in Self::existing_config_candidates()? {
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Nothing exists yet — seed into the portable app/repo root when possible.
        if let Some(root) = Self::discover_app_root() {
            return Ok(root.join(CONFIG_FILE_NAME));
        }

        if let Some(exe_dir) = Self::executable_dir() {
            return Ok(exe_dir.join(CONFIG_FILE_NAME));
        }

        Self::appdata_config_path()
    }

    fn existing_config_candidates() -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        if let Ok(cwd) = env::current_dir() {
            paths.push(cwd.join(CONFIG_FILE_NAME));
        }

        if let Some(exe_dir) = Self::executable_dir() {
            paths.push(exe_dir.join(CONFIG_FILE_NAME));
            for ancestor in exe_dir.ancestors().take(6) {
                paths.push(ancestor.join(CONFIG_FILE_NAME));
            }
        }

        if let Some(root) = Self::discover_app_root() {
            paths.push(root.join(CONFIG_FILE_NAME));
        }

        paths.push(Self::appdata_config_path()?);

        // De-duplicate while preserving order.
        let mut unique = Vec::new();
        for path in paths {
            if !unique.iter().any(|p: &PathBuf| p == &path) {
                unique.push(path);
            }
        }
        Ok(unique)
    }

    fn discover_app_root() -> Option<PathBuf> {
        let mut roots = Vec::new();

        roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));

        if let Ok(cwd) = env::current_dir() {
            roots.push(cwd.clone());
            if let Some(parent) = cwd.parent() {
                roots.push(parent.to_path_buf());
            }
        }

        if let Some(exe_dir) = Self::executable_dir() {
            for ancestor in exe_dir.ancestors().take(6) {
                roots.push(ancestor.to_path_buf());
            }
        }

        for root in roots {
            let example = root.join(EXAMPLE_FILE_NAME);
            let config = root.join(CONFIG_FILE_NAME);
            if example.exists() || config.exists() {
                return root.canonicalize().ok().or(Some(root));
            }
        }

        None
    }

    fn executable_dir() -> Option<PathBuf> {
        env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    fn appdata_config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("could not resolve config directory")?;
        Ok(base.join(APP_DIR_NAME).join(CONFIG_FILE_NAME))
    }

    fn seed_default_config(config_path: &PathBuf) -> Result<()> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        for candidate in Self::example_candidates() {
            if candidate.exists() {
                fs::copy(&candidate, config_path).with_context(|| {
                    format!("failed to seed config from {}", candidate.display())
                })?;
                return Ok(());
            }
        }

        // Empty starter config — the user fills in their own projects.
        let default = AppConfig {
            version: 1,
            settings: crate::domain::Settings::default(),
            projects: Vec::new(),
        };
        let raw = serde_json::to_string_pretty(&default)?;
        fs::write(config_path, raw)?;
        Ok(())
    }

    fn example_candidates() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(root) = Self::discover_app_root() {
            paths.push(root.join(EXAMPLE_FILE_NAME));
        }

        paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(EXAMPLE_FILE_NAME));
        paths.push(PathBuf::from(EXAMPLE_FILE_NAME));
        paths.push(PathBuf::from("..").join(EXAMPLE_FILE_NAME));

        if let Ok(cwd) = env::current_dir() {
            paths.push(cwd.join(EXAMPLE_FILE_NAME));
        }

        if let Some(exe_dir) = Self::executable_dir() {
            paths.push(exe_dir.join(EXAMPLE_FILE_NAME));
            for ancestor in exe_dir.ancestors().take(6) {
                paths.push(ancestor.join(EXAMPLE_FILE_NAME));
            }
        }

        paths
    }
}
