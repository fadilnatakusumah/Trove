//! End-to-end skill lifecycle integration test (plan 08 §8.4).
//!
//! Walks the §30 critical flow at the service layer — install → list →
//! disable → enable → uninstall — against fixture trees, then covers the
//! scope-independence and safety cases the tech-plan calls out.
//!
//! Scope choice: everything here uses **project** scope. Global scope resolves
//! through the real `$HOME`, so a test that mutated it would delete the
//! developer's actual skills. Project scope exercises the identical code path
//! (the only difference is which root `skill_roots` returns), so this is the
//! same coverage without the hazard.

#![allow(non_snake_case)]

use std::fs;
use std::path::{Path, PathBuf};

use app_lib::models::{ProviderId, SkillScope, SkillSource};
use app_lib::providers::claude::ClaudeProvider;
use app_lib::providers::{DISABLED_SKILL_FILE, SKILL_FILE, is_skill_dir};
use app_lib::services::install_service;
use app_lib::services::lifecycle_service;

/// Build a single-skill "repo" fixture (SKILL.md at the root).
fn source_repo(name: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    fs::write(
        dir.path().join(SKILL_FILE),
        format!("---\nname: {name}\ndescription: fixture for {name}\n---\nbody"),
    )
    .unwrap();
    dir
}

fn project_scope(project: &Path) -> SkillScope {
    SkillScope::Project {
        project_path: project.to_string_lossy().into_owned(),
    }
}

fn skills_dir(project: &Path) -> PathBuf {
    project.join(".claude").join("skills")
}

/// The §30 critical flow, start to finish, on one project.
#[test]
fn full_lifecycle_install_list_toggle_uninstall() {
    let project = tempfile::TempDir::new().unwrap();
    let repo = source_repo("react-expert");
    let source = SkillSource::Local {
        path: repo.path().to_string_lossy().into_owned(),
    };
    let scope = project_scope(project.path());

    // --- install ---------------------------------------------------------
    let staged = install_service::stage_source(&source).unwrap();
    let installed = install_service::install_skills(
        &ClaudeProvider,
        &source,
        &staged,
        &[".".to_string()],
        &scope,
        false,
    )
    .unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].id, "react-expert");
    assert!(installed[0].enabled, "a freshly installed skill is enabled");

    let dir = skills_dir(project.path()).join("react-expert");
    assert!(dir.join(SKILL_FILE).is_file());

    // --- list ------------------------------------------------------------
    let listed = app_lib::providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    assert_eq!(listed.len(), 1, "the installed skill appears in discovery");
    assert_eq!(listed[0].id, "react-expert");
    assert_eq!(listed[0].name, "react-expert");
    assert!(listed[0].enabled);

    // --- disable ---------------------------------------------------------
    lifecycle_service::set_skill_enabled(ProviderId::Claude, "react-expert", &scope, false)
        .unwrap();
    assert!(!dir.join(SKILL_FILE).exists());
    assert!(dir.join(DISABLED_SKILL_FILE).is_file());

    let listed = app_lib::providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    assert_eq!(listed.len(), 1, "a disabled skill stays listed, not hidden");
    assert!(!listed[0].enabled, "and is reported as disabled");

    // --- enable ----------------------------------------------------------
    lifecycle_service::set_skill_enabled(ProviderId::Claude, "react-expert", &scope, true).unwrap();
    assert!(dir.join(SKILL_FILE).is_file());
    assert!(!dir.join(DISABLED_SKILL_FILE).exists());

    let listed = app_lib::providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    assert!(listed[0].enabled, "re-enabled");

    // --- uninstall -------------------------------------------------------
    lifecycle_service::uninstall_skill(ProviderId::Claude, "react-expert", &scope, true).unwrap();
    assert!(!dir.exists(), "skill directory is gone");

    let listed = app_lib::providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    assert!(listed.is_empty(), "and no longer discovered");
}

/// Disabling must not make the skill unreachable: the user has to be able to
/// turn it back on. This is the exact failure mode that would strand a skill.
#[test]
fn disabled_skill_can_still_be_enabled_again() {
    let project = tempfile::TempDir::new().unwrap();
    let dir = skills_dir(project.path()).join("stranded");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(SKILL_FILE), "---\nname: stranded\n---\nbody").unwrap();
    let scope = project_scope(project.path());

    lifecycle_service::set_skill_enabled(ProviderId::Claude, "stranded", &scope, false).unwrap();

    // Resolution must find a directory whose SKILL.md was renamed.
    assert!(lifecycle_service::resolve_skill_dir(ProviderId::Claude, "stranded", &scope).is_ok());
    lifecycle_service::set_skill_enabled(ProviderId::Claude, "stranded", &scope, true).unwrap();
    assert!(dir.join(SKILL_FILE).is_file());
}

/// Two projects can each hold a skill of the same name. Acting on one must
/// never touch the other (§16: scopes are independent copies).
#[test]
fn scopes_are_independent_copies() {
    let a = tempfile::TempDir::new().unwrap();
    let b = tempfile::TempDir::new().unwrap();
    let scope_a = project_scope(a.path());
    let scope_b = project_scope(b.path());

    for (project, scope) in [(a.path(), &scope_a), (b.path(), &scope_b)] {
        let repo = source_repo("shared-name");
        let source = SkillSource::Local {
            path: repo.path().to_string_lossy().into_owned(),
        };
        let staged = install_service::stage_source(&source).unwrap();
        install_service::install_skills(
            &ClaudeProvider,
            &source,
            &staged,
            &[".".to_string()],
            scope,
            false,
        )
        .unwrap();
        assert!(skills_dir(project).join("shared-name").is_dir());
    }

    // Uninstall in A only.
    lifecycle_service::uninstall_skill(ProviderId::Claude, "shared-name", &scope_a, true).unwrap();

    assert!(
        !skills_dir(a.path()).join("shared-name").exists(),
        "A removed"
    );
    assert!(
        skills_dir(b.path()).join("shared-name").is_dir(),
        "B must be untouched — the other project's copy is a different skill"
    );
    let in_b = app_lib::providers::skills_for_scope(&ClaudeProvider, &scope_b).unwrap();
    assert_eq!(in_b.len(), 1, "B still discovers its own copy");
}

/// A skill id is a directory name. Anything with a separator is an attempt to
/// address a path, and must be refused before the filesystem is touched (§22).
#[test]
fn traversal_ids_are_refused_without_touching_the_filesystem() {
    let project = tempfile::TempDir::new().unwrap();
    let scope = project_scope(project.path());

    // A sibling directory that must survive the attempt.
    let victim = project.path().join("victim");
    fs::create_dir_all(&victim).unwrap();
    fs::write(victim.join("keep.txt"), "must not be touched").unwrap();

    for evil in ["../victim", "..", ".", "", "a/b", "a\\b"] {
        let err =
            lifecycle_service::uninstall_skill(ProviderId::Claude, evil, &scope, true).unwrap_err();
        assert_eq!(
            err.code,
            app_lib::errors::ErrorCode::InvalidSkill,
            "'{evil}' must be rejected as an invalid skill id"
        );
    }

    assert!(victim.join("keep.txt").is_file(), "nothing was deleted");
}

/// Reinstalling over an occupied destination is refused by default (Rule 14),
/// and only an explicit `replace` moves the occupant aside.
#[test]
fn reinstall_requires_explicit_confirmation() {
    let project = tempfile::TempDir::new().unwrap();
    let scope = project_scope(project.path());

    let install = |body: &str, replace: bool| {
        let repo = tempfile::TempDir::new().unwrap();
        fs::write(
            repo.path().join(SKILL_FILE),
            format!("---\nname: twice\ndescription: {body}\n---\n{body}"),
        )
        .unwrap();
        let source = SkillSource::Local {
            path: repo.path().to_string_lossy().into_owned(),
        };
        let staged = install_service::stage_source(&source).unwrap();
        install_service::install_skills(
            &ClaudeProvider,
            &source,
            &staged,
            &[".".to_string()],
            &scope,
            replace,
        )
    };

    install("first", false).unwrap();

    // Refused: the original content survives untouched.
    let err = install("second", false).unwrap_err();
    assert_eq!(err.code, app_lib::errors::ErrorCode::Conflict);
    let body =
        fs::read_to_string(skills_dir(project.path()).join("twice").join(SKILL_FILE)).unwrap();
    assert!(body.contains("first"), "zero-overwrite guarantee");

    // Confirmed: the replacement lands.
    install("second", true).unwrap();
    let body =
        fs::read_to_string(skills_dir(project.path()).join("twice").join(SKILL_FILE)).unwrap();
    assert!(body.contains("second"), "explicit replace took effect");
}

/// Uninstalling something that isn't there is a structured not-found, not a
/// panic and not a silent success.
#[test]
fn uninstall_of_missing_skill_is_not_found() {
    let project = tempfile::TempDir::new().unwrap();
    let err = lifecycle_service::uninstall_skill(
        ProviderId::Claude,
        "never-existed",
        &project_scope(project.path()),
        false,
    )
    .unwrap_err();
    assert_eq!(err.code, app_lib::errors::ErrorCode::SkillNotFound);
}

/// The invariant every one of the operations above depends on: a directory is
/// a skill only if it holds a skill file (enabled or disabled).
#[test]
fn is_skill_dir_is_the_single_source_of_truth() {
    let tmp = tempfile::TempDir::new().unwrap();

    let plain = tmp.path().join("not-a-skill");
    fs::create_dir_all(&plain).unwrap();
    fs::write(plain.join("README.md"), "a repo clone, not a skill").unwrap();
    assert!(!is_skill_dir(&plain), "a directory alone is not a skill");

    let enabled = tmp.path().join("enabled");
    fs::create_dir_all(&enabled).unwrap();
    fs::write(enabled.join(SKILL_FILE), "---\nname: enabled\n---\n").unwrap();
    assert!(is_skill_dir(&enabled));

    let disabled = tmp.path().join("disabled");
    fs::create_dir_all(&disabled).unwrap();
    fs::write(
        disabled.join(DISABLED_SKILL_FILE),
        "---\nname: disabled\n---\n",
    )
    .unwrap();
    assert!(is_skill_dir(&disabled), "a disabled skill is still a skill");
}
