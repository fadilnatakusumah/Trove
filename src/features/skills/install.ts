/**
 * Skill installation API (plan 06, §17/§18/§19).
 */
import { invokeCommand } from "@/lib/tauri";
import { isAppError } from "@/types/error";
import type { ProviderId, Skill, SkillScope, SkillSource } from "@/types/skill";

/**
 * Best-available explanation for a failed command (§25). Accepts `unknown`
 * because React Query types mutation errors as `Error` even though Tauri
 * hands back a structured `AppError`.
 */
export function errorDetail(err: unknown): string {
  if (isAppError(err)) {
    return err.detail ? `${err.message} ${err.detail}` : err.message;
  }
  if (err instanceof Error) return err.message;
  return String(err);
}

/** One discovered skill in an inspected source (§19). */
export interface DiscoveredSkill {
  id: string;
  name?: string;
  description?: string;
  dir: string;
}

export type SourceInspection =
  | { kind: "single"; id: string; name?: string; description?: string; dir: string }
  | { kind: "collection"; skills: DiscoveredSkill[] };

/** Flatten an inspection into its discovered skills (mirrors the Rust
 * `SourceInspection::skills()` helper). */
export function skillsOf(inspection: SourceInspection): DiscoveredSkill[] {
  return inspection.kind === "single" ? [inspection] : inspection.skills;
}

export async function inspectSource(source: SkillSource): Promise<SourceInspection> {
  return invokeCommand<SourceInspection>("inspect_source", { source });
}

export async function installSkills(args: {
  source: SkillSource;
  providerId: ProviderId;
  scope: SkillScope;
  skillDirs: string[];
  /**
   * Replace an occupied destination. Only ever true after the user confirmed
   * it — the backend refuses to overwrite otherwise (Rule 14).
   */
  replace?: boolean;
}): Promise<Skill[]> {
  return invokeCommand<Skill[]>("install_skill", {
    ...args,
    replace: args.replace ?? false,
  });
}
