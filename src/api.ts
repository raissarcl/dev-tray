import { invoke } from "@tauri-apps/api/core";
import type { ProjectView } from "./types";

export function listProjects(): Promise<ProjectView[]> {
  return invoke<ProjectView[]>("list_projects");
}

export function startProject(id: string): Promise<void> {
  return invoke("start_project", { id });
}

export function stopProject(id: string): Promise<void> {
  return invoke("stop_project", { id });
}

export function restartProject(id: string): Promise<void> {
  return invoke("restart_project", { id });
}

export function openBrowser(id: string): Promise<void> {
  return invoke("open_browser", { id });
}

export function openEditor(id: string): Promise<void> {
  return invoke("open_editor", { id });
}

export function openFolder(id: string): Promise<void> {
  return invoke("open_folder", { id });
}

export function getLogs(id: string): Promise<string> {
  return invoke<string>("get_logs", { id });
}

export function reloadConfig(): Promise<void> {
  return invoke("reload_config");
}
