use crate::config::ConfigManager;
use crate::domain::{Project, ProjectView, Settings};
use crate::hosts::HostsManager;
use crate::process::ProcessManager;
use crate::proxy::ProxyManager;
use anyhow::Result;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct AppState {
    pub config: Mutex<ConfigManager>,
    pub processes: Arc<ProcessManager>,
    pub proxy: Arc<ProxyManager>,
}

impl AppState {
    pub fn new(config: ConfigManager, log_capacity: usize) -> Self {
        Self {
            config: Mutex::new(config),
            processes: Arc::new(ProcessManager::new(log_capacity)),
            proxy: ProxyManager::new(),
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
                    alias: p.alias,
                    icon: p.icon,
                    status,
                }
            })
            .collect()
    }

    /// Host → port map for the reverse proxy (lowercased alias keys).
    pub fn alias_routes(&self) -> HashMap<String, u16> {
        self.projects()
            .into_iter()
            .filter_map(|p| {
                let port = p.port?;
                Some((p.alias_key().to_ascii_lowercase(), port))
            })
            .collect()
    }

    pub fn alias_hostnames(&self) -> Vec<String> {
        self.projects()
            .into_iter()
            .filter_map(|p| p.port.map(|_| p.alias_key().to_string()))
            .collect()
    }

    /// Preferred public URL: hosts alias, then *.localhost, then configured `url`.
    pub fn public_url(&self, project: &Project) -> Option<String> {
        let settings = self.settings();
        if settings.proxy_enabled && project.port.is_some() {
            let status = self.proxy.status();
            let alias = project.alias_key();
            // Prefer the live listen port; if the proxy is still starting, assume 80
            // (apply_alias_infra / ensure_alias_infra should have run before Open Browser).
            let listen = status.listen_port.or(settings.proxy_port).unwrap_or(80);

            if status.hosts_active {
                return Some(if listen == 80 {
                    format!("http://{alias}")
                } else {
                    format!("http://{alias}:{listen}")
                });
            }

            // Hosts sync failed or disabled — *.localhost still resolves without hosts.
            return Some(if listen == 80 {
                format!("http://{alias}.localhost")
            } else {
                format!("http://{alias}.localhost:{listen}")
            });
        }
        project
            .url
            .clone()
            .filter(|u| !u.trim().is_empty())
    }

    pub fn require_url(&self, id: &str) -> Result<String> {
        let project = self.get_project(id)?;
        self.public_url(&project)
            .ok_or_else(|| anyhow::anyhow!("project '{id}' has no URL configured (set url or port)"))
    }

    /// Start hosts + proxy if enabled and the proxy is not listening yet.
    pub async fn ensure_alias_infra(&self) {
        if !self.settings().proxy_enabled {
            return;
        }
        if self.proxy.status().listen_port.is_some() {
            return;
        }
        self.apply_alias_infra().await;
    }

    /// Sync hosts file + reverse proxy routes from current config.
    pub async fn apply_alias_infra(&self) {
        let settings = self.settings();
        let routes = self.alias_routes();
        let hostnames = self.alias_hostnames();

        self.proxy.set_routes(routes);

        if settings.proxy_enabled && settings.manage_hosts {
            match HostsManager::sync(&hostnames) {
                Ok(()) => self.proxy.set_hosts_active(true),
                Err(err) => {
                    eprintln!("[dev-tray] hosts sync failed: {err:#}");
                    eprintln!(
                        "[dev-tray] falling back to *.localhost URLs (run as Administrator to enable bare hostnames)"
                    );
                    self.proxy.set_hosts_active(false);
                }
            }
        } else {
            self.proxy.set_hosts_active(false);
            if !settings.manage_hosts {
                if let Err(err) = HostsManager::clear(true) {
                    eprintln!("[dev-tray] hosts clear failed: {err:#}");
                }
            }
        }

        if settings.proxy_enabled {
            if let Err(err) = self.proxy.start(settings.proxy_port).await {
                eprintln!("[dev-tray] alias proxy failed to start: {err:#}");
            }
        } else {
            self.proxy.stop();
        }
    }

    /// Tear down proxy + optional hosts block.
    /// Pass `allow_elevate: false` on OS shutdown so we never block on a UAC prompt.
    pub fn cleanup_aliases_on_exit(&self, allow_elevate: bool) {
        self.proxy.stop();
        let settings = self.settings();
        if settings.manage_hosts && !settings.hosts_persist {
            if let Err(err) = HostsManager::clear(allow_elevate) {
                eprintln!("[dev-tray] hosts cleanup failed: {err:#}");
            }
        }
    }
}
