//! Cursor provider (plan 04).
//!
//! Format verified from official docs (Rule 13, 2026-10):
//! - Global: `~/.cursor/skills/` (primary). Cursor also reads `~/.agents/skills/`.
//! - Project: `.cursor/skills/` (primary). Also `.agents/skills/`, and it
//!   additionally reads `.claude/skills`/`.codex/skills` for compatibility —
//!   those belong to the other providers here, so we don't list them twice.
//! - Discovery walks skill roots recursively: category folders are
//!   organizational; skill identity = folder containing SKILL.md. Phase 1
//!   supports one nesting level (documented ceiling, plan 06 §4).
//! - Disable: no on-disk disable mechanism (visibility via app settings);
//!   reported unsupported (§20, Rule 15).
//!
//! NOT modeled in Phase 1: plugins/team marketplaces, `paths` frontmatter
//! scoping, Custom Mode fields (icon/color).

use std::path::{Path, PathBuf};

use super::AgentProvider;
use crate::models::ProviderId;

pub struct CursorProvider;

impl AgentProvider for CursorProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Cursor
    }

    fn name(&self) -> &'static str {
        "Cursor"
    }

    fn global_skills_dir(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.cursor/skills",
        ))
    }

    /// Skills-dir parent is `~` (always exists) — use Cursor's own dir.
    fn config_root(&self) -> Option<PathBuf> {
        Some(crate::services::filesystem_service::resolve_home_relative(
            "~/.cursor",
        ))
    }

    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf> {
        vec![
            project.join(".cursor").join("skills"),
            project.join(".agents").join("skills"),
        ]
    }

    fn supports_disable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_dir_is_home_cursor_skills() {
        let dir = CursorProvider.global_skills_dir().expect("some");
        assert!(dir.ends_with(".cursor/skills") || dir.ends_with(".cursor\\skills"));
    }

    #[test]
    fn project_reads_both_conventions() {
        let dirs = CursorProvider.project_skills_dirs(Path::new("/tmp/proj"));
        assert_eq!(dirs.len(), 2);
        assert!(dirs[0].ends_with(".cursor/skills") || dirs[0].ends_with(".cursor\\skills"));
        assert!(dirs[1].ends_with(".agents/skills") || dirs[1].ends_with(".agents\\skills"));
    }

    #[test]
    fn disable_is_unsupported_honestly() {
        assert!(!CursorProvider.supports_disable());
    }
}
