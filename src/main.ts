import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type ProjectStatus = "stopped" | "running" | "starting" | "stopping";

interface ProjectView {
  id: string;
  name: string;
  path: string;
  command: string;
  port?: number | null;
  url?: string | null;
  icon?: string | null;
  status: ProjectStatus;
}

const projectsEl = document.querySelector<HTMLElement>("#projects")!;
const logsPanel = document.querySelector<HTMLElement>("#logs-panel")!;
const logsEl = document.querySelector<HTMLElement>("#logs")!;
const logsTitle = document.querySelector<HTMLElement>("#logs-title")!;

async function listProjects(): Promise<ProjectView[]> {
  return invoke<ProjectView[]>("list_projects");
}

function statusLabel(status: ProjectStatus): string {
  switch (status) {
    case "running":
      return "Running";
    case "starting":
      return "Starting";
    case "stopping":
      return "Stopping";
    default:
      return "Stopped";
  }
}

function render(projects: ProjectView[]) {
  if (projects.length === 0) {
    projectsEl.innerHTML = `<p class="empty">No projects configured.</p>`;
    return;
  }

  projectsEl.innerHTML = projects
    .map((p) => {
      const running = p.status === "running" || p.status === "starting";
      const port = p.port ? `:${p.port}` : "";
      return `
        <article class="project" data-id="${p.id}">
          <div>
            <h2>${escapeHtml(p.name)}</h2>
            <p class="meta">
              <span class="status ${running ? "running" : "stopped"}">${statusLabel(p.status)}</span>
              · ${escapeHtml(p.command)}${port}
            </p>
            <p class="meta">${escapeHtml(p.path)}</p>
          </div>
          <div class="actions">
            <button type="button" data-action="start" ${running ? "disabled" : ""}>Start</button>
            <button type="button" data-action="stop" ${running ? "" : "disabled"}>Stop</button>
            <button type="button" data-action="restart">Restart</button>
            <button type="button" data-action="browser">Browser</button>
            <button type="button" data-action="editor">Cursor</button>
            <button type="button" data-action="folder">Folder</button>
            <button type="button" data-action="logs">Logs</button>
          </div>
        </article>
      `;
    })
    .join("");
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function refresh() {
  const projects = await listProjects();
  render(projects);
}

async function onAction(id: string, action: string) {
  switch (action) {
    case "start":
      await invoke("start_project", { id });
      break;
    case "stop":
      await invoke("stop_project", { id });
      break;
    case "restart":
      await invoke("restart_project", { id });
      break;
    case "browser":
      await invoke("open_browser", { id });
      break;
    case "editor":
      await invoke("open_editor", { id });
      break;
    case "folder":
      await invoke("open_folder", { id });
      break;
    case "logs": {
      const logs = await invoke<string>("get_logs", { id });
      logsTitle.textContent = `Logs — ${id}`;
      logsEl.textContent = logs || "(no logs yet)";
      logsPanel.hidden = false;
      break;
    }
  }
  await refresh();
}

projectsEl.addEventListener("click", async (event) => {
  const target = event.target as HTMLElement;
  const button = target.closest<HTMLButtonElement>("button[data-action]");
  const article = target.closest<HTMLElement>("article.project");
  if (!button || !article) return;
  const id = article.dataset.id!;
  const action = button.dataset.action!;
  try {
    await onAction(id, action);
  } catch (err) {
    console.error(err);
    alert(String(err));
  }
});

document.querySelector("#btn-refresh")?.addEventListener("click", () => {
  void refresh();
});

document.querySelector("#btn-reload-config")?.addEventListener("click", async () => {
  await invoke("reload_config");
  await refresh();
});

document.querySelector("#btn-close-logs")?.addEventListener("click", () => {
  logsPanel.hidden = true;
});

void listen("project://status", () => {
  void refresh();
});

void refresh();
