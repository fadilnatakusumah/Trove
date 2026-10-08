/**
 * Compile-time contract check (plan 02 §2.4): a fixture with the exact JSON
 * shape the Rust contract tests assert must type-check as `Skill` / `AppError`.
 * Run with `tsc --noEmit` — type errors here mean the Rust/TS mirrors drifted.
 */
import type { AppError } from "./error";
import type { Project, Skill, SkillScope, SkillSource } from "./skill";

// Mirrors the Rust test `skill_serializes_to_ts_contract`.
const gitSkill: Skill = {
  id: "react-expert",
  name: "React Expert",
  description: "React development assistance",
  source: {
    type: "git",
    url: "https://github.com/example/react-skill",
    ref: "main",
  },
  provider: "claude",
  scope: { type: "global" },
  path: "~/.claude/skills/react-expert",
  enabled: true,
  version: "1.0.0",
  installedAt: "2026-10-08T00:00:00Z",
  metadata: { license: "Apache-2.0" },
};

// Mirrors the local/project-scope variant.
const localSkill: Skill = {
  id: "react-expert",
  name: "React Expert",
  source: { type: "local", path: "/tmp/skill" },
  provider: "codex",
  scope: { type: "project", projectPath: "~/Projects/prophero" },
  path: "~/Projects/prophero/.codex/skills/react-expert",
  enabled: false,
};

const project: Project = {
  id: "prophero",
  name: "PropHero",
  path: "~/Projects/prophero",
  providerIds: ["claude", "codex"],
};

const err: AppError = {
  code: "PERMISSION_DENIED",
  message: "Agent Manager could not access the skill directory.",
  detail: "os error 13",
};

// Discriminant exhaustiveness: these must narrow correctly.
function scopeLabel(scope: SkillScope): string {
  return scope.type === "global" ? "Global" : `Project: ${scope.projectPath}`;
}
function sourceUrl(source: SkillSource): string | undefined {
  return source.type === "git" ? source.url : undefined;
}

// Referenced to avoid unused-symbol lint noise; the value is the type check.
export const CONTRACT_FIXTURES = { gitSkill, localSkill, project, err, scopeLabel, sourceUrl };
