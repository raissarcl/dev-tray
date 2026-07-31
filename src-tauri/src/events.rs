use crate::domain::{ProjectId, ProjectStatus};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    ProjectStarted { id: ProjectId },
    ProjectStopped { id: ProjectId },
    ProjectExited {
        id: ProjectId,
        success: bool,
        code: Option<i32>,
    },
    ProjectStatusChanged {
        id: ProjectId,
        status: ProjectStatus,
    },
    ProjectStartFailed {
        id: ProjectId,
        error: String,
    },
    ConfigReloaded,
}

pub const PROJECT_STATUS_EVENT: &str = "project://status";
