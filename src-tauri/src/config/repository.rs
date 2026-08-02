use crate::domain::{AppConfig, Project, ProjectId};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)] // public persistence API used as the config grows
pub struct ProjectRepository {
    config_path: PathBuf,
    config: AppConfig,
}

impl ProjectRepository {
    pub fn load(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            let raw = fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            serde_json::from_str::<AppConfig>(&raw)
                .with_context(|| format!("failed to parse {}", config_path.display()))?
        } else {
            AppConfig::default()
        };

        validate_projects(&config)?;

        Ok(Self {
            config_path,
            config,
        })
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn settings(&self) -> &crate::domain::Settings {
        &self.config.settings
    }

    pub fn list(&self) -> &[Project] {
        &self.config.projects
    }

    pub fn get(&self, id: &str) -> Option<&Project> {
        self.config.projects.iter().find(|p| p.id == id)
    }

    pub fn upsert(&mut self, project: Project) -> Result<()> {
        if let Some(existing) = self
            .config
            .projects
            .iter_mut()
            .find(|p| p.id == project.id)
        {
            *existing = project;
        } else {
            self.config.projects.push(project);
        }
        validate_projects(&self.config)?;
        self.persist()
    }

    pub fn remove(&mut self, id: &str) -> Result<bool> {
        let before = self.config.projects.len();
        self.config.projects.retain(|p| p.id != id);
        let removed = self.config.projects.len() != before;
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    pub fn replace_all(&mut self, config: AppConfig) -> Result<()> {
        if config.version == 0 {
            bail!("invalid config version");
        }
        validate_projects(&config)?;
        self.config = config;
        self.persist()
    }

    pub fn persist(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, raw)
            .with_context(|| format!("failed to write {}", self.config_path.display()))?;
        Ok(())
    }

    pub fn project_ids(&self) -> Vec<ProjectId> {
        self.config.projects.iter().map(|p| p.id.clone()).collect()
    }
}

/// Reject duplicate `id` values and duplicate hostname aliases (`alias` or `id`).
pub fn validate_projects(config: &AppConfig) -> Result<()> {
    let mut seen_ids = HashSet::new();
    let mut seen_hosts = HashSet::new();

    for project in &config.projects {
        if project.id.trim().is_empty() {
            bail!("project id must not be empty");
        }
        if !seen_ids.insert(project.id.as_str()) {
            bail!(
                "duplicate project id '{}': each project needs a unique id",
                project.id
            );
        }

        let host = project.alias_key();
        if host.trim().is_empty() {
            bail!("project '{}' has an empty alias", project.id);
        }
        if host.contains('/') || host.contains(':') || host.contains(' ') {
            bail!(
                "project '{}' has invalid alias '{}': use a plain hostname (no spaces, ports, or paths)",
                project.id,
                host
            );
        }
        if !seen_hosts.insert(host) {
            bail!(
                "duplicate project hostname '{}': id/alias values must be unique for proxy routing",
                host
            );
        }
    }

    Ok(())
}
