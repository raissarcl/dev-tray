import { listen } from "@tauri-apps/api/event";
import * as api from "./api";
import { render, showLogs } from "./ui";

const projectsEl = document.querySelector<HTMLElement>("#projects")!;
const logsPanel = document.querySelector<HTMLElement>("#logs-panel")!;
const logsEl = document.querySelector<HTMLElement>("#logs")!;
const logsTitle = document.querySelector<HTMLElement>("#logs-title")!;

async function refresh() {
  const projects = await api.listProjects();
  render(projectsEl, projects);
}

async function onAction(id: string, action: string) {
  switch (action) {
    case "start":
      await api.startProject(id);
      break;
    case "stop":
      await api.stopProject(id);
      break;
    case "restart":
      await api.restartProject(id);
      break;
    case "browser":
      await api.openBrowser(id);
      break;
    case "editor":
      await api.openEditor(id);
      break;
    case "folder":
      await api.openFolder(id);
      break;
    case "logs": {
      const logs = await api.getLogs(id);
      showLogs(logsPanel, logsTitle, logsEl, id, logs);
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
  await api.reloadConfig();
  await refresh();
});

document.querySelector("#btn-close-logs")?.addEventListener("click", () => {
  logsPanel.hidden = true;
});

void listen("project://status", () => {
  void refresh();
});

void refresh();
