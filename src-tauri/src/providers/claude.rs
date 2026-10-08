//! Claude Code provider (plan 04).
//!
//! Format verified from official docs (Rule 13, 2026-10):
//! - Personal/global: `~/.claude/skills/<skill-name>/SKILL.md`
//! - Project: `<project>/.claude/skills/<skill-name>/SKILL.md`
//! - Symlinked skill folders are supported by Claude Code and resolved to
//!   their target when loading.
//! - Disable: Claude Code has no on-disk disable mechanism for skills
//!   (visibility is controlled via settings/skillOverrides); Phase 1 reports
//!   disable as unsupported rather than inventing a convention (§20, Rule 15).
//!
//! Provider-specific behaviors NOT modeled in Phase 1 (documented, Rule 15):
//! nested/`--add-dir` locations, plugin/marketplace skills, claude.ai-synced
//! skills (`skills/synced/` is reserved and skipped by convention).

use std::path::{Path, PathBuf};

use super::AgentProvider;
use crate::models::ProviderId;

pub struct ClaudeProvider;

impl AgentProvider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn global_skills_dir(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.claude/skills",
        ))
    }

    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf> {
        vec![project.join(".claude").join("skills")]
    }

    fn supports_disable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_dir_is_home_claude_skills() {
        let dir = ClaudeProvider.global_skills_dir().expect("some");
        assert!(dir.ends_with(".claude/skills") || dir.ends_with(".claude\\skills"));
    }

    #[test]
    fn project_dir_follows_convention() {
        let dirs = ClaudeProvider.project_skills_dirs(Path::new("/tmp/proj"));
        assert_eq!(dirs.len(), 1);
        assert!(
            dirs[0].ends_with(".claude/skills") || dirs[0].ends_with(".claude\\skills"),
            "got {}",
            dirs[0].display()
        );
    }

    #[test]
    fn disable_is_unsupported_honestly() {
        assert!(!ClaudeProvider.supports_disable());
    }
}
