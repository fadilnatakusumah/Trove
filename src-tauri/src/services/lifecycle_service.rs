//! Skill lifecycle: enable/disable and uninstall (plan 07, §20/§21).
//!
//! Both operations are agent-agnostic, so they live here once instead of as
//! per-provider code:
//!
//! - **Disable** renames `SKILL.md` to `SKILL.md.disabled`. Every agent
//!   discovers skills by that exact filename (Agent Skills spec), so the
//!   rename takes the skill out of the agent's reach while destroying nothing
//!   — enabling it again is the exact inverse. It needs no knowledge of any
//!   agent's config format, which is why it works for all of them.
//! - **Uninstall** removes the skill directory: to the OS trash by default
//!   (§21), or permanently when the user explicitly asks for that.
//!
//! Every target must pass `ensure_within` against the provider's skills root,
//! so a crafted path or id cannot touch anything else.

use std::path::PathBuf;

use crate::errors::{AppError, ErrorCode};
use crate::models::{ProviderId, SkillScope};
use crate::providers::{
    is_skill_dir, provider_by_id, AgentProvider, DISABLED_SKILL_FILE, SKILL_FILE,
};
use crate::services::filesystem_service as fs;

/// Candidate skills roots for a scope. Project scope can have more than one
/// (Cursor reads both `.cursor/skills` and `.agents/skills`), so callers must
/// consider all of them — resolving only the first would fail to find skills
/// that `get_skills` legitimately listed from the second.
fn skill_roots(
    provider: &dyn AgentProvider,
    scope: &SkillScope,
) -> Result<Vec<PathBuf>, AppError> {
    let roots: Vec<PathBuf> = match scope {
        SkillScope::Global => provider.global_skills_dir().into_iter().collect(),
        SkillScope::Project { project_path } => {
            provider.project_skills_dirs(&fs::resolve_home_relative(project_path))
        }
    };
    if roots.is_empty() {
        return Err(AppError::new(ErrorCode::ProviderNotFound));
    }
    Ok(roots)
}

/// Resolve the on-disk directory for a skill in a given scope, guarded.
///
/// Mirrors discovery (`skills_for_scope`): a skill is a directory named
/// `skill_id` containing a `SKILL.md`, either directly under a root or one
/// level deeper inside a category folder (plan 06 §4). Searching only the root
/// would make nested skills un-uninstallable — and, worse, a same-named
/// top-level skill would win and get trashed instead of the intended one.
///
/// With several candidate roots, the one that actually holds the skill wins;
/// if none does, the first candidate is returned so the not-found error names
/// a real location instead of a guess.
fn skill_dir(
    provider: &dyn AgentProvider,
    skill_id: &str,
    scope: &SkillScope,
) -> Result<(PathBuf, PathBuf), AppError> {
    // A skill id is a single directory name — reject anything with separators
    // so a crafted id can't walk out of the root before ensure_within even runs.
    if skill_id.is_empty()
        || skill_id.contains('/')
        || skill_id.contains('\\')
        || skill_id == "."
        || skill_id == ".."
    {
        return Err(AppError::new(ErrorCode::InvalidSkill)
            .with_detail(format!("'{skill_id}' is not a valid skill id")));
    }

    let roots = skill_roots(provider, scope)?;
    let mut fallback: Option<(PathBuf, PathBuf)> = None;

    for root in &roots {
        // Direct child — the common case.
        let direct = fs::ensure_within(root, &root.join(skill_id))?;
        if direct.is_dir() && is_skill_dir(&direct) {
            return Ok((root.clone(), direct));
        }
        if fallback.is_none() {
            fallback = Some((root.clone(), direct));
        }

        // One level deeper: Cursor-style category folders.
        let Ok(entries) = fs::list_dir_entries(root) else {
            continue;
        };
        for entry in entries.iter().filter(|e| e.is_dir) {
            let nested = fs::ensure_within(root, &root.join(&entry.name).join(skill_id))?;
            if nested.is_dir() && is_skill_dir(&nested) {
                return Ok((root.clone(), nested));
            }
        }
    }

    Ok(fallback.expect("skill_roots returned at least one root"))
}

/// Toggle a skill by renaming its `SKILL.md` (§20).
///
/// Works for every agent because discovery is by that exact filename, and it
/// is lossless: the directory and all supporting files stay put. Idempotent —
/// asking for the state a skill already has is a no-op, not an error.
pub fn set_skill_enabled(
    provider_id: ProviderId,
    skill_id: &str,
    scope: &SkillScope,
    enabled: bool,
) -> Result<(), AppError> {
    let provider = provider_by_id(provider_id)?;
    let (_root, dir) = skill_dir(provider.as_ref(), skill_id, scope)?;
    if !dir.is_dir() {
        return Err(AppError::new(ErrorCode::SkillNotFound)
            .with_detail(format!("'{}' does not exist", fs::display_path(&dir))));
    }

    let live = dir.join(SKILL_FILE);
    let parked = dir.join(DISABLED_SKILL_FILE);

    let (from, to) = if enabled {
        if live.is_file() {
            return Ok(()); // already enabled
        }
        (&parked, &live)
    } else {
        if parked.is_file() {
            return Ok(()); // already disabled
        }
        (&live, &parked)
    };

    if !from.is_file() {
        return Err(AppError::new(ErrorCode::SkillNotFound).with_detail(format!(
            "'{}' has no SKILL.md to rename",
            fs::display_path(&dir)
        )));
    }

    fs::rename_checked(&dir, from, to)?;
    log::info!("skill '{skill_id}' enabled={enabled}");
    Ok(())
}

/// Remove a skill directory (§21).
///
/// Defaults to the OS trash so the action is recoverable; `permanent` deletes
/// it outright, which the UI only ever asks for on explicit confirmation.
pub fn uninstall_skill(
    provider_id: ProviderId,
    skill_id: &str,
    scope: &SkillScope,
    permanent: bool,
) -> Result<(), AppError> {
    let provider = provider_by_id(provider_id)?;
    let (_root, dir) = skill_dir(provider.as_ref(), skill_id, scope)?;

    if !dir.is_dir() {
        return Err(AppError::new(ErrorCode::SkillNotFound)
            .with_detail(format!("'{}' does not exist", fs::display_path(&dir))));
    }

    if permanent {
        fs::delete_dir_permanent(&dir)?;
        log::info!("uninstalled '{skill_id}' (deleted permanently)");
    } else {
        fs::move_to_trash(&dir)?;
        log::info!("uninstalled '{skill_id}' (moved to trash)");
    }
    Ok(())
}

/// Guarded existence check used by tests and callers that need the path.
pub fn resolve_skill_dir(
    provider_id: ProviderId,
    skill_id: &str,
    scope: &SkillScope,
) -> Result<PathBuf, AppError> {
    let provider = provider_by_id(provider_id)?;
    let (_root, dir) = skill_dir(provider.as_ref(), skill_id, scope)?;
    Ok(dir)
}

#[cfg(test)]
#[allow(non_snake_case)] // REGRESSION_* names are deliberate: greppable in CI.
mod tests {
    use super::*;
    use crate::providers::claude::ClaudeProvider;

    #[test]
    fn rejects_traversal_in_skill_id() {
        for evil in ["../secrets", "a/b", "a\\b", "..", ".", ""] {
            let err = skill_dir(&ClaudeProvider, evil, &SkillScope::Global).unwrap_err();
            assert_eq!(
                err.code,
                ErrorCode::InvalidSkill,
                "'{evil}' must be rejected before touching the filesystem"
            );
        }
    }

    /// Disable must be lossless and reversible: the directory and every
    /// supporting file stay put, only `SKILL.md` is renamed.
    #[test]
    fn disable_and_enable_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill = tmp.path().join(".claude").join("skills").join("tdd");
        std::fs::create_dir_all(skill.join("scripts")).unwrap();
        std::fs::write(skill.join(SKILL_FILE), "---\nname: tdd\n---\n").unwrap();
        std::fs::write(skill.join("scripts").join("run.sh"), "echo hi").unwrap();

        let scope = SkillScope::Project {
            project_path: tmp.path().to_string_lossy().into_owned(),
        };

        set_skill_enabled(ProviderId::Claude, "tdd", &scope, false).unwrap();
        assert!(!skill.join(SKILL_FILE).exists(), "SKILL.md renamed away");
        assert!(skill.join(DISABLED_SKILL_FILE).is_file(), "marker written");
        assert!(
            skill.join("scripts").join("run.sh").is_file(),
            "supporting files must survive the toggle"
        );

        // A disabled skill is still resolvable, or it could never be re-enabled.
        assert!(resolve_skill_dir(ProviderId::Claude, "tdd", &scope).is_ok());

        set_skill_enabled(ProviderId::Claude, "tdd", &scope, true).unwrap();
        assert!(skill.join(SKILL_FILE).is_file(), "restored");
        assert!(!skill.join(DISABLED_SKILL_FILE).exists(), "marker removed");
    }

    #[test]
    fn toggle_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill = tmp.path().join(".claude").join("skills").join("twice");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join(SKILL_FILE), "---\nname: twice\n---\n").unwrap();
        let scope = SkillScope::Project {
            project_path: tmp.path().to_string_lossy().into_owned(),
        };

        // Already enabled -> enabling again is a no-op, not an error.
        set_skill_enabled(ProviderId::Claude, "twice", &scope, true).unwrap();
        set_skill_enabled(ProviderId::Claude, "twice", &scope, false).unwrap();
        set_skill_enabled(ProviderId::Claude, "twice", &scope, false).unwrap();
        assert!(skill.join(DISABLED_SKILL_FILE).is_file());
    }

    #[test]
    fn uninstall_permanent_deletes_the_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let skill = tmp.path().join(".claude").join("skills").join("gone");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join(SKILL_FILE), "---\nname: gone\n---\n").unwrap();
        let scope = SkillScope::Project {
            project_path: tmp.path().to_string_lossy().into_owned(),
        };

        uninstall_skill(ProviderId::Claude, "gone", &scope, true).unwrap();
        assert!(!skill.exists(), "directory deleted outright");
    }

    #[test]
    fn uninstall_missing_skill_is_not_found() {
        let err = uninstall_skill(
            ProviderId::Claude,
            "definitely-not-a-real-skill-xyz",
            &SkillScope::Global,
            false,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::SkillNotFound);
    }

    #[test]
    fn project_scope_resolves_inside_project_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().to_string_lossy().into_owned();
        let dir = resolve_skill_dir(
            ProviderId::Claude,
            "my-skill",
            &SkillScope::Project {
                project_path: project.clone(),
            },
        )
        .unwrap();
        // ensure_within canonicalizes, so compare against the canonical root
        // (Windows returns \\?\ paths / 8.3 short names otherwise).
        let canonical_root = std::fs::canonicalize(tmp.path()).unwrap();
        assert!(
            dir.starts_with(&canonical_root),
            "resolved inside the project: {} vs {}",
            dir.display(),
            canonical_root.display()
        );
        assert!(dir.ends_with("my-skill"));
    }

    /// Cursor reads BOTH `.cursor/skills` and `.agents/skills` in a project.
    /// Resolving only the first root would make a skill that `get_skills`
    /// listed from the second one un-uninstallable (SKILL_NOT_FOUND).
    #[test]
    fn REGRESSION_project_scope_finds_skill_in_secondary_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let second = tmp.path().join(".agents").join("skills").join("my-skill");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(second.join("SKILL.md"), "---\nname: my-skill\n---\n").unwrap();

        let dir = resolve_skill_dir(
            ProviderId::Cursor,
            "my-skill",
            &SkillScope::Project {
                project_path: tmp.path().to_string_lossy().into_owned(),
            },
        )
        .unwrap();

        let canonical = std::fs::canonicalize(&second).unwrap();
        assert_eq!(
            std::fs::canonicalize(&dir).unwrap(),
            canonical,
            "must resolve to the root that actually holds the skill"
        );
    }

    /// Nested (category) skills live at `<root>/<category>/<name>`, but the
    /// skill id is only the leaf name. Resolving `<root>/<name>` would report
    /// SKILL_NOT_FOUND for a skill the UI happily listed — and would trash a
    /// same-named top-level skill if one existed.
    #[test]
    fn REGRESSION_nested_skill_resolves_to_its_category_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nested = tmp
            .path()
            .join(".claude")
            .join("skills")
            .join("workflow")
            .join("tdd");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("SKILL.md"), "---\nname: tdd\n---\n").unwrap();

        let dir = resolve_skill_dir(
            ProviderId::Claude,
            "tdd",
            &SkillScope::Project {
                project_path: tmp.path().to_string_lossy().into_owned(),
            },
        )
        .unwrap();

        assert_eq!(
            std::fs::canonicalize(&dir).unwrap(),
            std::fs::canonicalize(&nested).unwrap(),
            "must resolve into the category folder, not <root>/tdd"
        );
    }
}
