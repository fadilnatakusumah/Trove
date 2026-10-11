//! Agent provider layer.
//!
//! One module per supported agent (claude, cursor, codex). Each provider owns
//! only its own filesystem conventions; providers are assembled behind the
//! `AgentProvider` trait in plan 04 (tech-plan §10 — no abstraction before two
//! real implementations exist).

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;

use std::path::{Path, PathBuf};

use crate::errors::{AppError, ErrorCode};
use crate::models::{ProviderId, Skill, SkillScope};

/// The file that makes a directory a skill. The Agent Skills spec requires
/// `SKILL.md` at the skill root, and every agent we support discovers skills
/// by that exact filename.
pub const SKILL_FILE: &str = "SKILL.md";

/// A disabled skill's `SKILL.md`, renamed.
///
/// Because discovery is by exact filename, renaming takes the skill out of
/// every agent's reach while leaving the directory and its supporting files
/// untouched — so enabling it again is the exact inverse, and nothing is ever
/// lost. Deliberately does not end in `.md` so a `*.md` sweep can't pick it up.
pub const DISABLED_SKILL_FILE: &str = "SKILL.md.disabled";

/// Is `dir` a skill directory — enabled or disabled?
///
/// The **single** definition of "a skill" in this codebase: discovery, the
/// lifecycle service, and the installer all call it. When they each rolled
/// their own check, the installer could insist "a skill with this name already
/// exists" about a directory the Skills list simultaneously reported as not
/// containing any skill.
pub fn is_skill_dir(dir: &Path) -> bool {
    dir.join(SKILL_FILE).is_file() || dir.join(DISABLED_SKILL_FILE).is_file()
}

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
}

/// A provider answers "where do this agent's skills live?" and nothing else.
///
/// Enable/disable and uninstall are deliberately NOT trait methods: they work
/// identically for every agent (rename `SKILL.md`, or remove the directory),
/// so they live in `lifecycle_service` as one implementation rather than four
/// (Rule 5/6 boundary — providers hold no UI or lifecycle concepts).
pub trait AgentProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn name(&self) -> &'static str;

    /// Global skills root (resolved, no `~`). `None` = convention absent.
    fn global_skills_dir(&self) -> Option<PathBuf>;

    /// The agent's config root (e.g. `~/.config/opencode`), used to decide
    /// whether the agent is installed. Defaults to the parent of the skills
    /// dir, which is right for every agent here.
    ///
    /// Detection must NOT require the skills dir to exist: an installed agent
    /// that has never had a skill installed still has its config root, and
    /// "not detected" would be a lie the user can disprove with `agent -v`.
    fn config_root(&self) -> Option<PathBuf> {
        self.global_skills_dir()
            .and_then(|d| d.parent().map(Path::to_path_buf))
    }

    /// Project-level skills roots. A Vec because some agents honor multiple
    /// conventions (e.g. Cursor reads `.cursor/skills` and `.agents/skills`).
    fn project_skills_dirs(&self, project: &Path) -> Vec<PathBuf>;
}

/// All providers, in stable display order.
pub fn all_providers() -> Vec<Box<dyn AgentProvider>> {
    vec![
        Box::new(claude::ClaudeProvider),
        Box::new(cursor::CursorProvider),
        Box::new(codex::CodexProvider),
        Box::new(opencode::OpencodeProvider),
    ]
}

pub fn provider_by_id(id: ProviderId) -> Result<Box<dyn AgentProvider>, AppError> {
    all_providers()
        .into_iter()
        .find(|p| p.id() == id)
        .ok_or_else(|| AppError::new(ErrorCode::ProviderNotFound).with_detail(format!("no provider with id '{id}'")))
}

/// Detection across every registered provider.
///
/// "Detected" = the agent's config root exists (installed/configured), NOT
/// that the skills dir exists. An agent with no skills yet is still installed;
/// reporting it as undetected would contradict `agent --version`.
pub fn detect_all() -> Vec<ProviderDetectionResult> {
    all_providers()
        .into_iter()
        .map(|p| {
            let global = p.global_skills_dir();
            let detected = p.config_root().as_ref().is_some_and(|d| d.is_dir());
            if detected {
                log::info!(
                    "detected {} provider at {}",
                    p.name(),
                    global
                        .as_ref()
                        .map(|d| display_home_path(d))
                        .unwrap_or_else(|| "(no skills dir)".to_string())
                );
            }
            ProviderDetectionResult {
                provider_id: p.id(),
                name: p.name(),
                detected,
                global_skills_dir: global.as_ref().map(|d| display_home_path(d)),
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
                // No skill file here — look one level down for category folders.
                None => skills.extend(nested_skills(provider, &skill_dir, scope)),
            }
        }
    }
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    log::info!(
        "loaded {} skill(s) for {} ({})",
        skills.len(),
        provider.name(),
        match scope {
            SkillScope::Global => "global".to_string(),
            SkillScope::Project { project_path } => {
                format!("project {project_path}")
            }
        }
    );
    Ok(skills)
}

/// Parse one skill directory. `None` when it holds no skill file at all.
pub fn parse_skill_dir(
    provider: &dyn AgentProvider,
    skill_dir: &Path,
    scope: &SkillScope,
) -> Option<Skill> {
    // An enabled skill has `SKILL.md`; a disabled one has it renamed. Reading
    // the renamed file keeps a disabled skill listed — with its real name and
    // description — so the UI can offer to enable it again (§16).
    let (skill_md, enabled) = if skill_dir.join(SKILL_FILE).is_file() {
        (skill_dir.join(SKILL_FILE), true)
    } else if skill_dir.join(DISABLED_SKILL_FILE).is_file() {
        (skill_dir.join(DISABLED_SKILL_FILE), false)
    } else {
        return None;
    };
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
        // Enabled state IS on-disk state: `SKILL.md` present vs renamed (§16).
        enabled,
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
/// Separators are normalized to `/` so Windows doesn't produce mixed
/// paths like `~/.agents/skills\eslint-import-sort`.
///
/// Re-exported from the path module: there is one implementation, in
/// `filesystem_service`, so every path the user sees is formatted the same
/// way (§8.1).
pub use crate::services::filesystem_service::display_path as display_home_path;
