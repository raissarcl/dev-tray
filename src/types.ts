export type ProjectStatus = "stopped" | "running" | "starting" | "stopping";

export interface ProjectView {
  id: string;
  name: string;
  path: string;
  command: string;
  port?: number | null;
  url?: string | null;
  icon?: string | null;
  status: ProjectStatus;
}
