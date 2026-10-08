//! Agent provider layer.
//!
//! One module per supported agent (claude, cursor, codex). Each provider owns
//! only its own filesystem conventions; providers are assembled behind the
//! `AgentProvider` trait in plan 04 (tech-plan §10 — no abstraction before two
//! real implementations exist).

pub mod claude;
pub mod codex;
pub mod cursor;

use std::path::{Path, PathBuf};

use crate::errors::{AppError, ErrorCode};
use crate::models::{ProviderId, Skill, SkillScope};

/// Whether an agent installation was detected on this machine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDetectionResult {
    pub provider_id: ProviderId,
    pub name: &'static str,
    pub detected: bool,
    /// Global skills root, `~`-shortened for display. Present even when the
    /// dir doesn't exist yet (shows where skills *would* live).
    pub global_skills_dir: Option<String>,
    /// True when the provider supports a real disable mechanism (§20).
    pub supports_disable: bool,
}

/// A provider exposes skill locations and parses skills; it contains no UI
/// concepts (Rule 5/6 boundary).
pub trait AgentProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn name(&self) -> &'static str;

    /// Global skills root (resolved, no `~`). `None` = convention absent.
    fn global_skills_dir(&self) -> Option<PathBuf>;

    /// Project-level skills roots. A Vec because some agents honor multiple
    /// conventions (e.g. Cursor reads `.cursor/skills` and `.agents/skills`).
    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf>;

    /// Honest capability flag — the UI must not fake toggles (§20, Rule 15).
    fn supports_disable(&self) -> bool;

    /// Apply the enabled state (plan 07 consumes this). Default: unsupported.
    fn set_enabled(&self, _skill: &Skill, _enabled: bool) -> Result<(), AppError> {
        Err(AppError::new(ErrorCode::Unsupported))
    }
}

/// All providers, in stable display order.
pub fn all_providers() -> Vec<Box<dyn AgentProvider>> {
    vec![
        Box::new(claude::ClaudeProvider),
        Box::new(cursor::CursorProvider),
        Box::new(codex::CodexProvider),
    ]
}

pub fn provider_by_id(id: ProviderId) -> Result<Box<dyn AgentProvider>, AppError> {
    all_providers()
        .into_iter()
        .find(|p| p.id() == id)
        .ok_or_else(|| AppError::new(ErrorCode::ProviderNotFound).with_detail(format!("no provider with id '{id}'")))
}

/// Detection across every registered provider.
pub fn detect_all() -> Vec<ProviderDetectionResult> {
    all_providers()
        .into_iter()
        .map(|p| {
            let global = p.global_skills_dir();
            ProviderDetectionResult {
                provider_id: p.id(),
                name: p.name(),
                detected: global.as_ref().is_some_and(|d| d.is_dir()),
                global_skills_dir: global.as_ref().map(|d| display_home_path(d)),
                supports_disable: p.supports_disable(),
            }
        })
        .collect()
}

/// Resolve skills for a scope by walking the provider's skill directories.
///
/// `enabled` derives from actual filesystem state — Phase 1 displays reality,
/// no virtual inheritance (tech-plan §16). A directory counts as a skill when
/// it directly contains a readable `SKILL.md`; a directory without one is
/// checked one level deeper for nested skills (Cursor-style category folders —
/// one level is the documented Phase 1 ceiling, plan 06 §4).
pub fn skills_for_scope(
    provider: &dyn AgentProvider,
    scope: &SkillScope,
) -> Result<Vec<Skill>, AppError> {
    let dirs: Vec<PathBuf> = match scope {
        SkillScope::Global => provider.global_skills_dir().into_iter().collect(),
        SkillScope::Project { project_path } => provider
            .project_skills_dirs(&crate::services::filesystem_service::resolve_home_relative(
                project_path,
            )),
    };

    let mut skills = Vec::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        for entry in crate::services::filesystem_service::list_dir_entries(&dir)? {
            if !entry.is_dir {
                continue;
            }
            let skill_dir = dir.join(&entry.name);
            match parse_skill_dir(provider, &skill_dir, scope) {
                Some(skill) => skills.push(skill),
                // No SKILL.md here — look one level down for category folders.
                None => skills.extend(nested_skills(provider, &skill_dir, scope)),
            }
        }
    }
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(skills)
}

/// Parse one skill directory. `None` when the dir has no readable SKILL.md.
pub fn parse_skill_dir(
    provider: &dyn AgentProvider,
    skill_dir: &Path,
    scope: &SkillScope,
) -> Option<Skill> {
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.is_file() {
        return None;
    }
    let content =
        crate::services::filesystem_service::read_text_file_capped(&skill_md).ok()?;
    let frontmatter = crate::services::skill_service::parse_frontmatter(&content)?;

    let id = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if id.is_empty() {
        return None;
    }

    // Trust the directory name (agents resolve skills by directory); record a
    // frontmatter `name` mismatch in metadata instead of failing (plan 06 §4).
    let mut metadata = frontmatter.metadata;
    if let Some(name) = &frontmatter.name {
        if *name != id {
            metadata.insert("nameMismatch".to_string(), name.clone());
        }
    }

    Some(Skill {
        name: frontmatter.name.unwrap_or_else(|| id.clone()),
        id,
        description: frontmatter.description,
        source: crate::models::SkillSource::Local {
            path: skill_dir.to_string_lossy().into_owned(),
        },
        provider: provider.id(),
        scope: scope.clone(),
        path: display_home_path(skill_dir),
        // Phase 1: present on disk = enabled. Provider-specific disable state
        // lands in plan 07.
        enabled: true,
        version: frontmatter.version,
        installed_at: None,
        metadata,
    })
}

/// One-level-deep nested skills (category folders).
fn nested_skills(
    provider: &dyn AgentProvider,
    category_dir: &Path,
    scope: &SkillScope,
) -> Vec<Skill> {
    let Ok(entries) = crate::services::filesystem_service::list_dir_entries(category_dir) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|e| e.is_dir)
        .filter_map(|e| parse_skill_dir(provider, &category_dir.join(&e.name), scope))
        .collect()
}

/// Display path with `~` for the home prefix; full path otherwise.
pub fn display_home_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            let rel = rel.to_string_lossy();
            if !rel.is_empty() {
                return format!("~/{rel}");
            }
        }
    }
    path.to_string_lossy().into_owned()
}
