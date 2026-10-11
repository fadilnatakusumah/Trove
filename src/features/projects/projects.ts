/**
 * Registered projects (plan 05, tech-plan §15/§27/§28).
 *
 * Persistence = one JSON file in the app-data dir, written through Rust
 * commands. No database (Rule 8). Projects are references, never copied.
 */

import { invokeCommand } from "@/lib/tauri";

export interface RegisteredProject {
  id: string;
  name: string;
  /** The real path — use this for IPC calls and identity, never for display. */
  path: string;
  /** Display form (`~`-shortened, forward slashes). Computed by the backend. */
  displayPath: string;
}

export async function listProjects(): Promise<RegisteredProject[]> {
  return invokeCommand<RegisteredProject[]>("list_projects");
}

export async function addProject(path: string): Promise<RegisteredProject> {
  return invokeCommand<RegisteredProject>("add_project", { path });
}

export async function removeProject(id: string): Promise<void> {
  return invokeCommand<void>("remove_project", { id });
}

/** Native folder picker (Tauri dialog). Returns null when cancelled. */
export async function pickFolder(): Promise<string | null> {
  return invokeCommand<string | null>("pick_folder");
}
