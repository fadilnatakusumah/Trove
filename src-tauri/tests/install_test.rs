//! Installation integration tests (plan 06 §6.5).
//!
//! Uses local-fixture "repos" for the stage → inspect → install flow; the
//! git path is covered by the same code after clone (one branch point, tested
//! via the local path). Conflict, traversal, and selection behavior are all
//! exercised against temp dirs.

#![allow(non_snake_case)] // REGRESSION_* names are deliberate: greppable in CI.

use std::fs;
use std::path::Path;

use app_lib::models::{SkillScope, SkillSource};
use app_lib::providers::claude::ClaudeProvider;
use app_lib::services::install_service::{self, SourceInspection};

fn write_skill(root: &Path, name: &str, body: &str) -> String {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {body}\n---\ncontent"),
    )
    .unwrap();
    dir.to_string_lossy().into_owned()
}

#[test]
fn happy_path_local_global_and_project() {
    let src = tempfile::TempDir::new().unwrap();
    // Single-skill repo: SKILL.md at the source root.
    fs::write(
        src.path().join("SKILL.md"),
        "---\nname: happy\ndescription: A happy skill\n---\ncontent",
    )
    .unwrap();

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    assert!(
        matches!(staged.inspection(), SourceInspection::Single(_)),
        "root SKILL.md = single-skill repo"
    );

    // Project scope (global would write to the real HOME — avoided in tests).
    let project = tempfile::TempDir::new().unwrap();
    // Root layout: the discovered dir is "." (the staged root itself).
    let installed = install_service::install_skills(
        &ClaudeProvider,
        &SkillSource::Local { path: src.path().to_string_lossy().into_owned() },
        &staged,
        &[".".to_string()],
        &SkillScope::Project { project_path: project.path().to_string_lossy().into_owned() },
        false,
    )
    .unwrap();

    assert_eq!(installed.len(), 1);
    assert!(installed[0].installed_at.is_some());
    assert_eq!(installed[0].id, "happy", "id from frontmatter name");
    assert!(project.path().join(".claude/skills/happy/SKILL.md").exists());
}

#[test]
fn REGRESSION_stale_staging_paths_do_not_break_install() {
    // The reported bug: inspect and install stage SEPARATELY (two temp dirs),
    // so a discovered path from the first staging must not be used verbatim
    // against the second — otherwise install fails with "path not found" even
    // though the skill exists.
    let src = tempfile::TempDir::new().unwrap();
    write_skill(src.path(), "frontend-design", "design guidance");
    let source = SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    };

    // 1. Inspection staging — what the UI sees and echoes back.
    let inspected = install_service::stage_source(&source).unwrap();
    let selected: Vec<String> = inspected
        .inspection()
        .skills()
        .iter()
        .map(|s| s.dir.clone())
        .collect();
    let inspect_root = inspected.root().to_path_buf();

    // 2. Install staging — a DIFFERENT temp dir, exactly like the real flow.
    let install_staged = install_service::stage_source(&source).unwrap();
    assert_ne!(
        inspect_root,
        install_staged.root(),
        "stagings are independent temp dirs (the precondition of the bug)"
    );

    let project = tempfile::TempDir::new().unwrap();
    let installed = install_service::install_skills(
        &ClaudeProvider,
        &source,
        &install_staged,
        &selected,
        &SkillScope::Project {
            project_path: project.path().to_string_lossy().into_owned(),
        },
        false,
    )
    .expect("install must resolve selected paths against its own staging");

    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].id, "frontend-design");
    assert!(project
        .path()
        .join(".claude/skills/frontend-design/SKILL.md")
        .exists());
}

#[test]
fn invalid_repo_maps_to_invalid_skill() {
    let src = tempfile::TempDir::new().unwrap();
    // README only — no SKILL.md root or one level deep.
    fs::write(src.path().join("README.md"), "# not a skill").unwrap();

    let err = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap_err();
    assert_eq!(err.code, app_lib::errors::ErrorCode::InvalidSkill);
}

#[test]
fn collection_subset_selection_installs_only_selected() {
    let src = tempfile::TempDir::new().unwrap();
    write_skill(src.path(), "one", "first");
    write_skill(src.path(), "two", "second");
    write_skill(src.path(), "three", "third");

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    assert_eq!(staged.inspection().skills().len(), 3);

    let project = tempfile::TempDir::new().unwrap();
    // Relative dirs, as the frontend would send them back.
    let selected: Vec<String> = staged
        .inspection()
        .skills()
        .iter()
        .filter(|s| s.id == "one" || s.id == "three")
        .map(|s| s.dir.clone())
        .collect();

    let installed = install_service::install_skills(
        &ClaudeProvider,
        &SkillSource::Local { path: src.path().to_string_lossy().into_owned() },
        &staged,
        &selected,
        &SkillScope::Project { project_path: project.path().to_string_lossy().into_owned() },
        false,
    )
    .unwrap();

    assert_eq!(installed.len(), 2);
    let skills_dir = project.path().join(".claude/skills");
    assert!(skills_dir.join("one/SKILL.md").exists());
    assert!(skills_dir.join("three/SKILL.md").exists());
    assert!(!skills_dir.join("two").exists());
}

#[test]
fn existing_skill_conflict_returns_conflict_without_overwrite() {
    let src = tempfile::TempDir::new().unwrap();
    let _dir = write_skill(src.path(), "clash", "new version");

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();

    let project = tempfile::TempDir::new().unwrap();
    let existing = project.path().join(".claude/skills/clash");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("SKILL.md"), "---\nname: clash\ndescription: original\n---\nORIGINAL").unwrap();

    let err = install_service::install_skills(
        &ClaudeProvider,
        &SkillSource::Local { path: src.path().to_string_lossy().into_owned() },
        &staged,
        // Discovered dirs are relative to the staged root.
        &["clash".to_string()],
        &SkillScope::Project { project_path: project.path().to_string_lossy().into_owned() },
        false,
    )
    .unwrap_err();

    assert_eq!(err.code, app_lib::errors::ErrorCode::Conflict);
    let body = fs::read_to_string(existing.join("SKILL.md")).unwrap();
    assert!(body.contains("ORIGINAL"), "zero-overwrite guarantee (Rule 14)");
}

/// The reported bug: a destination occupied by a directory that holds no
/// skill. Discovery lists no skill there, so "a skill with this name already
/// exists" was a contradiction the user could see was false.
#[test]
fn conflict_on_non_skill_directory_explains_itself() {
    let src = tempfile::TempDir::new().unwrap();
    write_skill(src.path(), "frontend-design", "body");

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();

    let project = tempfile::TempDir::new().unwrap();
    // A full repo clone dumped at the destination name — no SKILL.md at its
    // root, so it is not a skill, exactly like a manual `git clone`.
    let occupied = project.path().join(".claude/skills/frontend-design");
    fs::create_dir_all(occupied.join("plugins")).unwrap();
    fs::write(occupied.join("README.md"), "a repo, not a skill").unwrap();

    let err = install_service::install_skills(
        &ClaudeProvider,
        &SkillSource::Local { path: src.path().to_string_lossy().into_owned() },
        &staged,
        &["frontend-design".to_string()],
        &SkillScope::Project { project_path: project.path().to_string_lossy().into_owned() },
        false,
    )
    .unwrap_err();

    assert_eq!(err.code, app_lib::errors::ErrorCode::Conflict);
    let detail = err.detail.unwrap_or_default();
    assert!(
        detail.contains("does not contain a SKILL.md"),
        "must say the directory is not a skill, got: {detail}"
    );
    assert!(
        !err.message.contains("already exists at the destination"),
        "must not claim a skill exists there"
    );
    // Confirming without `replace` changed nothing (Rule 14).
    assert!(occupied.join("README.md").exists());
}

/// `replace: true` (only ever set after explicit confirmation) swaps the
/// occupant out, and trashes it rather than unlinking it.
#[test]
fn replace_confirmed_swaps_the_occupant_into_the_trash() {
    let src = tempfile::TempDir::new().unwrap();
    write_skill(src.path(), "frontend-design", "new body");

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();

    let project = tempfile::TempDir::new().unwrap();
    let occupied = project.path().join(".claude/skills/frontend-design");
    fs::create_dir_all(occupied.join("plugins")).unwrap();
    fs::write(occupied.join("README.md"), "a repo, not a skill").unwrap();

    let installed = install_service::install_skills(
        &ClaudeProvider,
        &SkillSource::Local { path: src.path().to_string_lossy().into_owned() },
        &staged,
        &["frontend-design".to_string()],
        &SkillScope::Project { project_path: project.path().to_string_lossy().into_owned() },
        true,
    )
    .unwrap();

    assert_eq!(installed.len(), 1);
    assert!(occupied.join("SKILL.md").is_file(), "skill is installed now");
    assert!(
        !occupied.join("README.md").exists(),
        "the previous occupant is gone (moved to the trash, not unlinked)"
    );
}

#[test]
fn git_missing_source_path_is_structured_error() {
    let err = install_service::stage_source(&SkillSource::Local {
        path: "/no/such/dir/anywhere".to_string(),
    })
    .unwrap_err();
    assert_eq!(err.code, app_lib::errors::ErrorCode::InvalidSkill);
}

#[test]
fn temp_dir_cleaned_up_after_stage_drop() {
    let src = tempfile::TempDir::new().unwrap();
    write_skill(src.path(), "tempcheck", "x");

    let staged = install_service::stage_source(&SkillSource::Local {
        path: src.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    // The staged root must exist while the staging is alive.
    let staged_root = staged.root().to_path_buf();
    assert!(staged_root.exists(), "staged while alive");
    assert!(staged_root.join("tempcheck/SKILL.md").exists());

    drop(staged);
    assert!(
        !staged_root.exists(),
        "temp cleaned up on drop (§18 step 7)"
    );
}
