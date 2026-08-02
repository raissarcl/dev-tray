import type { ProjectStatus, ProjectView } from "./types";

export function statusLabel(status: ProjectStatus): string {
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

export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function render(projectsEl: HTMLElement, projects: ProjectView[]) {
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

export function showLogs(
  logsPanel: HTMLElement,
  logsTitle: HTMLElement,
  logsEl: HTMLElement,
  id: string,
  logs: string,
) {
  logsTitle.textContent = `Logs — ${id}`;
  logsEl.textContent = logs || "(no logs yet)";
  logsPanel.hidden = false;
}
