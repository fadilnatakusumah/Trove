//! Codex provider (plan 04).
//!
//! Format verified from official docs (Rule 13, 2026-10):
//! - Global: `~/.agents/skills/` (USER scope). (Historically also `~/.codex/`;
//!   current docs name `~/.agents/skills` as the user location.)
//! - Project: `.agents/skills/` scanned from CWD up to the repo root. Phase 1
//!   models the project root only (the registration unit is one project).
//! - Disable: Codex DOES support disabling via `[[skills.config]]` entries
//!   (`path = ".../SKILL.md"`, `enabled = false`) in `~/.codex/config.toml`.
//!   The mechanism itself is implemented in plan 07; capability is declared
//!   here so the UI can show a real toggle.
//!
//! NOT modeled in Phase 1: plugins, admin (/etc/codex/skills) and system
//! scopes, `agents/openai.yaml` metadata.

use std::path::{Path, PathBuf};

use super::AgentProvider;
use crate::models::ProviderId;

pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn global_skills_dir(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.agents/skills",
        ))
    }

    /// Skills-dir parent is `~` (always exists) — use Codex's own dir.
    fn config_root(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.codex",
        ))
    }

    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf> {
        vec![project.join(".agents").join("skills")]
    }

    fn supports_disable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_dir_is_home_agents_skills() {
        let dir = CodexProvider.global_skills_dir().expect("some");
        assert!(dir.ends_with(".agents/skills") || dir.ends_with(".agents\\skills"));
    }

    #[test]
    fn project_dir_follows_agents_convention() {
        let dirs = CodexProvider.project_skills_dirs(Path::new("/tmp/proj"));
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with(".agents/skills") || dirs[0].ends_with(".agents\\skills"));
    }

    #[test]
    fn codex_supports_disable() {
        assert!(CodexProvider.supports_disable());
    }
}
