use crate::config::ConfigManager;
use crate::domain::{Project, ProjectView, Settings};
use crate::process::ProcessManager;
use anyhow::{bail, Result};
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AppState {
    pub config: Mutex<ConfigManager>,
    pub processes: Arc<ProcessManager>,
}

impl AppState {
    pub fn new(config: ConfigManager, log_capacity: usize) -> Self {
        Self {
            config: Mutex::new(config),
            processes: Arc::new(ProcessManager::new(log_capacity)),
        }
    }

    pub fn settings(&self) -> Settings {
        self.config.lock().repository().settings().clone()
    }

    pub fn projects(&self) -> Vec<Project> {
        self.config.lock().repository().list().to_vec()
    }

    pub fn get_project(&self, id: &str) -> Result<Project> {
        self.config
            .lock()
            .repository()
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown project '{id}'"))
    }

    pub fn project_views(&self) -> Vec<ProjectView> {
        self.projects()
            .into_iter()
            .map(|p| {
                let status = self.processes.status(&p.id);
                ProjectView {
                    id: p.id,
                    name: p.name,
                    path: p.path,
                    command: p.command,
                    port: p.port,
                    url: p.url,
                    icon: p.icon,
                    status,
                }
            })
            .collect()
    }

    pub fn require_url(&self, id: &str) -> Result<String> {
        let project = self.get_project(id)?;
        match project.url {
            Some(url) if !url.is_empty() => Ok(url),
            _ => bail!("project '{id}' has no URL configured"),
        }
    }
}
