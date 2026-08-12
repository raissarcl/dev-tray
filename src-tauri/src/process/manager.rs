use crate::domain::{Project, ProjectId, ProjectStatus};
use crate::events::AppEvent;
use crate::process::LogBuffer;
use anyhow::{bail, Context, Result};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

#[cfg(windows)]
use crate::process::job_windows::JobObject;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct ManagedHandle {
    #[cfg(windows)]
    job: JobObject,
    /// Signalled by `stop` so the waiter can finish promptly.
    stop_tx: Option<oneshot::Sender<()>>,
}

type ChangeCallback = Arc<dyn Fn() + Send + Sync>;

pub struct ProcessManager {
    handles: Mutex<HashMap<ProjectId, ManagedHandle>>,
    statuses: Mutex<HashMap<ProjectId, ProjectStatus>>,
    logs: Mutex<HashMap<ProjectId, LogBuffer>>,
    log_capacity: usize,
}

impl ProcessManager {
    pub fn new(log_capacity: usize) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
            log_capacity,
        }
    }

    pub fn status(&self, id: &str) -> ProjectStatus {
        self.statuses
            .lock()
            .get(id)
            .copied()
            .unwrap_or(ProjectStatus::Stopped)
    }

    pub fn statuses_snapshot(&self) -> HashMap<ProjectId, ProjectStatus> {
        self.statuses.lock().clone()
    }

    pub fn logs_snapshot(&self, id: &str) -> String {
        self.logs
            .lock()
            .get(id)
            .map(|b| b.snapshot())
            .unwrap_or_default()
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.status(id).is_running()
    }

    fn set_status(&self, id: &str, status: ProjectStatus) {
        self.statuses.lock().insert(id.to_string(), status);
    }

    fn emit(app: &AppHandle, event: AppEvent) {
        let _ = app.emit(crate::events::PROJECT_STATUS_EVENT, &event);
    }

    fn append_log(&self, id: &str, line: impl Into<String>) {
        let mut logs = self.logs.lock();
        let buffer = logs
            .entry(id.to_string())
            .or_insert_with(|| LogBuffer::new(self.log_capacity));
        buffer.append(line);
    }

    pub async fn start(
        self: &Arc<Self>,
        app: AppHandle,
        project: Project,
        on_change: ChangeCallback,
    ) -> Result<()> {
        let id = project.id.clone();

        if self.is_running(&id) {
            bail!("project '{}' is already running", id);
        }

        self.set_status(&id, ProjectStatus::Starting);
        on_change();
        Self::emit(
            &app,
            AppEvent::ProjectStatusChanged {
                id: id.clone(),
                status: ProjectStatus::Starting,
            },
        );

        {
            let mut logs = self.logs.lock();
            let buffer = logs
                .entry(id.clone())
                .or_insert_with(|| LogBuffer::new(self.log_capacity));
            buffer.clear();
            buffer.append(format!("$ {}", project.command));
        }

        if !project.path.is_dir() {
            let err = format!("project path does not exist: {}", project.path.display());
            self.fail_start(&app, &id, err.clone(), &on_change);
            bail!(err);
        }

        #[cfg(windows)]
        let job = JobObject::new().context("failed to create Windows Job Object")?;

        let mut command = Command::new("cmd");
        command
            .args(["/C", &project.command])
            .current_dir(&project.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(false);

        for (key, value) in &project.env {
            command.env(key, value);
        }

        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let msg = format!("failed to spawn '{}': {err}", project.command);
                self.fail_start(&app, &id, msg.clone(), &on_change);
                bail!(msg);
            }
        };

        #[cfg(windows)]
        {
            let pid = child.id().context("spawned process has no PID")?;
            if let Err(err) = job.assign_pid(pid) {
                let _ = child.kill().await;
                let msg = format!("failed to assign process to job: {err}");
                self.fail_start(&app, &id, msg.clone(), &on_change);
                bail!(msg);
            }
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();

        {
            let mut handles = self.handles.lock();
            handles.insert(
                id.clone(),
                ManagedHandle {
                    #[cfg(windows)]
                    job,
                    stop_tx: Some(stop_tx),
                },
            );
        }

        self.set_status(&id, ProjectStatus::Running);
        on_change();
        Self::emit(&app, AppEvent::ProjectStarted { id: id.clone() });
        Self::emit(
            &app,
            AppEvent::ProjectStatusChanged {
                id: id.clone(),
                status: ProjectStatus::Running,
            },
        );

        let manager = Arc::clone(self);
        if let Some(out) = stdout {
            let mgr = Arc::clone(&manager);
            let pid = id.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    mgr.append_log(&pid, line);
                }
            });
        }

        if let Some(err_pipe) = stderr {
            let mgr = Arc::clone(&manager);
            let pid = id.clone();
            tauri::async_runtime::spawn(async move {
                let mut reader = BufReader::new(err_pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    mgr.append_log(&pid, line);
                }
            });
        }

        let watch_id = id.clone();
        let watch_app = app.clone();
        let on_change_watch = Arc::clone(&on_change);
        tauri::async_runtime::spawn(async move {
            let (success, code) = tokio::select! {
                status = child.wait() => {
                    match status {
                        Ok(s) => (s.success(), s.code()),
                        Err(_) => (false, None),
                    }
                }
                _ = stop_rx => {
                    // Job termination from stop() should reap the tree; wait for child.
                    match child.wait().await {
                        Ok(s) => (s.success(), s.code()),
                        Err(_) => (false, None),
                    }
                }
            };

            {
                let mut handles = manager.handles.lock();
                handles.remove(&watch_id);
            }

            let was_stopping = manager.status(&watch_id) == ProjectStatus::Stopping;
            manager.set_status(&watch_id, ProjectStatus::Stopped);
            manager.append_log(
                &watch_id,
                format!(
                    "[exited] success={success} code={}",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                ),
            );
            on_change_watch();

            if was_stopping {
                Self::emit(
                    &watch_app,
                    AppEvent::ProjectStopped {
                        id: watch_id.clone(),
                    },
                );
            } else {
                Self::emit(
                    &watch_app,
                    AppEvent::ProjectExited {
                        id: watch_id.clone(),
                        success,
                        code,
                    },
                );
            }

            Self::emit(
                &watch_app,
                AppEvent::ProjectStatusChanged {
                    id: watch_id,
                    status: ProjectStatus::Stopped,
                },
            );
        });

        Ok(())
    }

    fn fail_start(&self, app: &AppHandle, id: &str, error: String, on_change: &ChangeCallback) {
        self.set_status(id, ProjectStatus::Stopped);
        self.append_log(id, format!("[error] {error}"));
        on_change();
        Self::emit(
            app,
            AppEvent::ProjectStartFailed {
                id: id.to_string(),
                error,
            },
        );
        Self::emit(
            app,
            AppEvent::ProjectStatusChanged {
                id: id.to_string(),
                status: ProjectStatus::Stopped,
            },
        );
    }

    pub async fn stop(&self, app: &AppHandle, id: &str, on_change: ChangeCallback) -> Result<()> {
        if !self.is_running(id) {
            return Ok(());
        }

        self.set_status(id, ProjectStatus::Stopping);
        on_change();
        Self::emit(
            app,
            AppEvent::ProjectStatusChanged {
                id: id.to_string(),
                status: ProjectStatus::Stopping,
            },
        );

        let stop_tx = {
            let mut handles = self.handles.lock();
            if let Some(managed) = handles.get_mut(id) {
                #[cfg(windows)]
                {
                    if let Err(err) = managed.job.terminate() {
                        self.append_log(id, format!("[warn] terminate job: {err}"));
                    }
                }
                managed.stop_tx.take()
            } else {
                None
            }
        };

        if let Some(tx) = stop_tx {
            let _ = tx.send(());
        }

        // Waiter task emits ProjectStopped / status update when the child exits.
        // If the handle is already gone, ensure local state is clean.
        if !self.handles.lock().contains_key(id) {
            self.set_status(id, ProjectStatus::Stopped);
            on_change();
            Self::emit(
                app,
                AppEvent::ProjectStopped {
                    id: id.to_string(),
                },
            );
        }

        Ok(())
    }

    pub async fn restart(
        self: &Arc<Self>,
        app: AppHandle,
        project: Project,
        on_change: ChangeCallback,
    ) -> Result<()> {
        let id = project.id.clone();
        if self.is_running(&id) {
            self.stop(&app, &id, Arc::clone(&on_change)).await?;
            // Brief yield so the waiter can drop the Job Object before we start again.
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        self.start(app, project, on_change).await
    }

    pub async fn stop_all(&self, app: &AppHandle) {
        let ids: Vec<String> = {
            let handles = self.handles.lock();
            handles.keys().cloned().collect()
        };
        let noop: ChangeCallback = Arc::new(|| {});
        for id in ids {
            let _ = self.stop(app, &id, Arc::clone(&noop)).await;
        }
        // Allow waiters to finish.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    /// Terminate every managed job immediately (no await / sleep).
    /// Used on OS shutdown where blocking the event loop hangs reboot.
    pub fn kill_all_now(&self) {
        let drained: Vec<(ProjectId, ManagedHandle)> = {
            let mut handles = self.handles.lock();
            handles.drain().collect()
        };
        for (id, mut managed) in drained {
            #[cfg(windows)]
            {
                if let Err(err) = managed.job.terminate() {
                    self.append_log(&id, format!("[warn] terminate job: {err}"));
                }
            }
            if let Some(tx) = managed.stop_tx.take() {
                let _ = tx.send(());
            }
            self.set_status(&id, ProjectStatus::Stopped);
        }
    }
}
