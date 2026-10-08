/**
 * Skill domain types (plan 02). Hand-maintained mirror of the Rust model in
 * `src-tauri/src/models/mod.rs` — the Rust shapes are the source of truth.
 * Changes must be applied to both sides; the Rust contract tests pin the JSON.
 */

export type ProviderId = "claude" | "cursor" | "codex";

export type SkillSource =
  | { type: "local"; path: string }
  | { type: "git"; url: string; ref?: string };

export type SkillScope =
  | { type: "global" }
  | { type: "project"; projectPath: string };

export interface Skill {
  id: string;
  name: string;
  description?: string;
  source: SkillSource;
  provider: ProviderId;
  scope: SkillScope;
  path: string;
  enabled: boolean;
  version?: string;
  /** RFC 3339 timestamp, when known. */
  installedAt?: string;
  metadata?: Record<string, string>;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  /** Providers detected inside this project's agent directories. */
  providerIds: ProviderId[];
}
