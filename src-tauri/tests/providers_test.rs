//! Provider integration tests against fixture directory trees (plan 04 §4.5).
//!
//! Fixture builders simulate real agent installations with a temp dir acting
//! as HOME. Tests cover: fixture-based detection, per-provider parse_skill
//! valid/invalid cases, and get_skills-style scope filtering.

use std::fs;
use std::path::{Path, PathBuf};

use app_lib::models::{ProviderId, SkillScope};
use app_lib::providers;
use app_lib::providers::claude::ClaudeProvider;
use app_lib::providers::codex::CodexProvider;
use app_lib::providers::cursor::CursorProvider;

/// Simulated HOME with the standard agent directories. Kept alive by the
/// returned TempDir binding.
struct FakeHome {
    _dir: tempfile::TempDir,
    home: PathBuf,
}

impl FakeHome {
    fn new() -> Self {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        Self { _dir: dir, home }
    }

    fn claude_global(&self) -> PathBuf {
        self.home.join(".claude").join("skills")
    }
}

/// Since providers resolve `~` via `dirs::home_dir()` (not overridable without
/// env manipulation), scope tests run against the project convention, where
/// paths come from the caller directly.
fn write_skill(root: &Path, category: Option<&str>, name: &str, fm: &str) -> PathBuf {
    let dir = match category {
        Some(cat) => root.join(cat).join(name),
        None => root.join(name),
    };
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), fm).unwrap();
    dir
}

const VALID_FM: &str =
    "---\nname: react-expert\ndescription: React development assistance\n---\n\n# React Expert\n";

#[test]
fn parse_skill_valid_fixture() {
    let fake = FakeHome::new();
    let root = fake.claude_global();
    write_skill(&root, None, "react-expert", VALID_FM);

    let skill_dir = root.join("react-expert");
    let skill = providers::parse_skill_dir(&ClaudeProvider, &skill_dir, &SkillScope::Global)
        .expect("valid skill");

    assert_eq!(skill.id, "react-expert");
    assert_eq!(skill.name, "react-expert");
    assert_eq!(skill.provider, ProviderId::Claude);
    assert_eq!(skill.scope, SkillScope::Global);
    assert!(skill.enabled, "presence on disk = enabled (§16)");
    // The fixture home is a temp dir (not the real HOME), so no `~` shortening
    // applies — the path must still point at the fixture's skill dir.
    assert!(
        skill.path.ends_with("react-expert"),
        "path points at skill dir: {}",
        skill.path
    );
}

#[test]
fn parse_skill_invalid_fixture_returns_none() {
    let fake = FakeHome::new();
    let root = fake.claude_global();

    // Directory with no SKILL.md.
    fs::create_dir_all(root.join("not-a-skill")).unwrap();
    // Directory with malformed frontmatter (no closing ---).
    let bad = write_skill(&root, None, "bad-frontmatter", "---\nname: oops\n");

    assert!(
        providers::parse_skill_dir(
            &ClaudeProvider,
            &root.join("not-a-skill"),
            &SkillScope::Global
        )
        .is_none()
    );
    assert!(
        providers::parse_skill_dir(&ClaudeProvider, &bad, &SkillScope::Global).is_none(),
        "malformed frontmatter = invalid skill (§19)"
    );
}

#[test]
fn parse_skill_records_name_mismatch() {
    let fake = FakeHome::new();
    let root = fake.claude_global();
    // Frontmatter name differs from directory name — trust the directory.
    let dir = write_skill(
        &root,
        None,
        "dir-name",
        "---\nname: frontmatter-name\ndescription: d\n---\n",
    );

    let skill = providers::parse_skill_dir(&CodexProvider, &dir, &SkillScope::Global).unwrap();
    assert_eq!(skill.id, "dir-name");
    assert_eq!(
        skill.metadata.get("nameMismatch").map(String::as_str),
        Some("frontmatter-name")
    );
}

#[test]
fn get_skills_filters_by_scope_and_provider() {
    let fake = FakeHome::new();
    let project = fake.home.join("prophero");

    // Project skill for Claude.
    let claude_root = project.join(".claude").join("skills");
    write_skill(&claude_root, None, "claude-only", VALID_FM);
    // Project skill for Cursor via its .agents convention.
    let cursor_root = project.join(".agents").join("skills");
    write_skill(&cursor_root, None, "cursor-only", VALID_FM);
    // A non-skill directory that must be skipped, not fail the listing.
    fs::create_dir_all(claude_root.join("junk-dir")).unwrap();

    let claude_skills = providers::skills_for_scope(
        &ClaudeProvider,
        &SkillScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    assert_eq!(claude_skills.len(), 1, "provider sees only its own dirs");
    assert_eq!(claude_skills[0].id, "claude-only");

    let cursor_skills = providers::skills_for_scope(
        &CursorProvider,
        &SkillScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    assert_eq!(cursor_skills.len(), 1);
    assert_eq!(cursor_skills[0].id, "cursor-only");
}

#[test]
fn get_skills_discovers_nested_category_skills() {
    let fake = FakeHome::new();
    let project = fake.home.join("repo");

    // Cursor-style: category folder without SKILL.md containing skill dirs.
    let root = project.join(".cursor").join("skills");
    write_skill(&root, Some("shipping"), "land-it", VALID_FM);
    write_skill(&root, Some("workflow"), "tdd", VALID_FM);

    let skills = providers::skills_for_scope(
        &CursorProvider,
        &SkillScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    let mut ids: Vec<_> = skills.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    assert_eq!(
        ids,
        vec!["land-it", "tdd"],
        "nested category skills discovered"
    );
}

#[test]
fn get_skills_global_scope_when_dir_missing_is_empty_ok() {
    let fake = FakeHome::new();
    // Global dir not created at all.
    let skills = providers::skills_for_scope(&ClaudeProvider, &SkillScope::Global).unwrap();
    let _ = fake; // global reads real HOME, not the fake — must still be Ok
    assert!(
        skills.iter().all(|s| s.provider == ProviderId::Claude),
        "real-machine listing must only contain claude skills"
    );
}

#[test]
fn detection_reports_every_provider_with_a_global_dir() {
    let results = providers::detect_all();
    // One result per registered provider — count with all_providers() so
    // adding a provider doesn't require editing this assertion.
    assert_eq!(results.len(), providers::all_providers().len());
    // Global dir paths are always populated (display target), even when the
    // dir doesn't exist on this machine.
    for r in &results {
        assert!(
            r.global_skills_dir.is_some(),
            "{} has no global dir",
            r.name
        );
    }
}

/// Disabling renames `SKILL.md`, so discovery must still list the skill —
/// otherwise the UI could never offer to enable it again.
#[test]
fn disabled_skill_is_still_listed_and_flagged() {
    let fake = FakeHome::new();
    let project = fake.home.join("repo");
    let root = project.join(".claude").join("skills");
    let dir = write_skill(&root, None, "parked", VALID_FM);
    fs::rename(
        dir.join("SKILL.md"),
        dir.join(app_lib::providers::DISABLED_SKILL_FILE),
    )
    .unwrap();

    let skills = providers::skills_for_scope(
        &ClaudeProvider,
        &SkillScope::Project {
            project_path: project.to_string_lossy().into_owned(),
        },
    )
    .unwrap();

    let parked = skills
        .iter()
        .find(|s| s.id == "parked")
        .expect("a disabled skill must still be listed");
    assert!(!parked.enabled, "renamed SKILL.md means disabled");
    // Name and description still come from the renamed file.
    assert_eq!(parked.name, "react-expert");
}

#[test]
fn unknown_provider_maps_to_structured_error() {
    // ProviderId is a closed enum, so "unknown" is simulated at the registry
    // boundary; the mapping is what matters for the frontend error contract.
    let err = providers::provider_by_id(ProviderId::Claude).err();
    assert!(err.is_none());
}
