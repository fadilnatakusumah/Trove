//! OpenCode provider.
//!
//! Format verified from official docs (Rule 13, 2026-10):
//! - Global: `~/.config/opencode/skills/<name>/SKILL.md` (primary)
//! - Project: `.opencode/skills/<name>/SKILL.md`
//! - Frontmatter: `name` + `description` required; `license`, `compatibility`,
//!   `metadata` optional. Unknown fields ignored. `name` must match the
//!   containing directory (the Agent Skills spec rule, same as the others).
//!
//! OpenCode *also* reads Claude-compatible (`.claude/skills`, `~/.claude/skills`)
//! and agent-compatible (`.agents/skills`, `~/.agents/skills`) locations. Those
//! are owned by the Claude and Codex providers here, so listing them again
//! would double-count the same on-disk skills under a second provider.
//!
//! Disable: OpenCode controls skill access with pattern-based permissions
//! (`permission.skill` in `opencode.json`: allow/deny/ask). That is a
//! permission policy with wildcards, not a per-skill boolean toggle — Phase 1
//! reports disable as unsupported rather than writing wildcard permission rules
//! into the user's config (Rule 15: don't half-implement).

use std::path::{Path, PathBuf};

use super::AgentProvider;
use crate::models::ProviderId;

pub struct OpencodeProvider;

impl AgentProvider for OpencodeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Opencode
    }

    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn global_skills_dir(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.config/opencode/skills",
        ))
    }

    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf> {
        vec![project.join(".opencode").join("skills")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_dir_is_home_config_opencode_skills() {
        let dir = OpencodeProvider.global_skills_dir().expect("some");
        // Not `~/.opencode` — OpenCode's primary location is under ~/.config.
        assert!(
            dir.ends_with(".config/opencode/skills") || dir.ends_with(".config\\opencode\\skills"),
            "got {}",
            dir.display()
        );
    }

    #[test]
    fn project_dir_follows_opencode_convention() {
        let dirs = OpencodeProvider.project_skills_dirs(Path::new("/tmp/proj"));
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with(".opencode/skills") || dirs[0].ends_with(".opencode\\skills"));
    }
}
