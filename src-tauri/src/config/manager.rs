use crate::config::ProjectRepository;
use crate::domain::AppConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "tray-for-projects";
const CONFIG_FILE_NAME: &str = "projects.json";

pub struct ConfigManager {
    repository: ProjectRepository,
}

impl ConfigManager {
    pub fn init() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            Self::seed_default_config(&config_path)?;
        }
        let repository = ProjectRepository::load(config_path)?;
        Ok(Self { repository })
    }

    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("could not resolve config directory")?;
        Ok(base.join(APP_DIR_NAME).join(CONFIG_FILE_NAME))
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

    fn seed_default_config(config_path: &PathBuf) -> Result<()> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Prefer the example shipped with the repo (resolved from crate / CWD).
        let manifest_example =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../projects.example.json");
        let candidates = [
            manifest_example,
            PathBuf::from("projects.example.json"),
            PathBuf::from("../projects.example.json"),
            std::env::current_dir()?.join("projects.example.json"),
        ];

        for candidate in candidates {
            if candidate.exists() {
                fs::copy(&candidate, config_path).with_context(|| {
                    format!(
                        "failed to seed config from {}",
                        candidate.display()
                    )
                })?;
                return Ok(());
            }
        }

        let default = AppConfig {
            version: 1,
            settings: crate::domain::Settings::default(),
            projects: vec![crate::domain::Project {
                id: "visualize-git".into(),
                name: "PR Network".into(),
                path: PathBuf::from(r"C:\Users\rairc\Projects\visualize-git"),
                command: "npm run dev".into(),
                port: Some(5173),
                url: Some("http://localhost:5173".into()),
                icon: Some("git".into()),
                env: Default::default(),
                stop_command: None,
                kind: None,
            }],
        };

        let raw = serde_json::to_string_pretty(&default)?;
        fs::write(config_path, raw)?;
        Ok(())
    }
}
