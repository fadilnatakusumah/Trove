//! Performance verification (plan 08 §8.3, tech-plan §33).
//!
//! §33 sets budgets and asks that discovery not re-scan on every render. The
//! render half is a frontend concern (TanStack Query caches each combo list
//! with a 30s stale time and invalidates after mutations), so what's left to
//! verify here is that the scan itself fits the "filesystem refresh < 1s"
//! budget on a realistically-sized tree.
//!
//! These are budgets, not benchmarks: the assertion is deliberately loose
//! enough to survive CI noise while still failing if discovery becomes
//! accidentally quadratic or starts walking the whole filesystem.

#![allow(non_snake_case)]

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use app_lib::models::SkillScope;
use app_lib::providers::claude::ClaudeProvider;
use app_lib::providers::{self, SKILL_FILE};

/// §33: "Filesystem refresh: < 1 second for normal project sizes".
const REFRESH_BUDGET: Duration = Duration::from_secs(1);

/// A deliberately generous ceiling. Real developer machines have well under
/// this many skills; the point is to catch a regression that changes the
/// complexity class, not to police microseconds.
const SKILL_COUNT: usize = 200;

fn build_tree(root: &Path, count: usize) {
    for i in 0..count {
        let dir = root.join(format!("skill-{i:03}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(SKILL_FILE),
            format!("---\nname: skill-{i:03}\ndescription: fixture {i}\n---\nbody"),
        )
        .unwrap();
    }
}

#[test]
fn discovery_of_a_large_tree_fits_the_refresh_budget() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project = tmp.path();
    build_tree(&project.join(".claude").join("skills"), SKILL_COUNT);

    let scope = SkillScope::Project {
        project_path: project.to_string_lossy().into_owned(),
    };

    // Warm the filesystem cache once; §33 is about the steady-state refresh a
    // user triggers, not a cold first read after boot.
    let warm = providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    assert_eq!(warm.len(), SKILL_COUNT, "all fixtures discovered");

    let started = Instant::now();
    let skills = providers::skills_for_scope(&ClaudeProvider, &scope).unwrap();
    let elapsed = started.elapsed();

    assert_eq!(skills.len(), SKILL_COUNT);
    assert!(
        elapsed < REFRESH_BUDGET,
        "discovering {SKILL_COUNT} skills took {elapsed:?}, over the §33 budget of {REFRESH_BUDGET:?}"
    );
}

/// Detection runs on every app start and must not walk skill trees at all —
/// it only asks whether each agent's config root exists (§33 startup budget).
#[test]
fn detection_does_not_scan_skill_trees() {
    // A large tree that detection must not read. If `detect_all` ever starts
    // enumerating skills, this test's timing regresses hard.
    let tmp = tempfile::TempDir::new().unwrap();
    build_tree(&tmp.path().join(".claude").join("skills"), SKILL_COUNT);

    let started = Instant::now();
    for _ in 0..50 {
        let results = providers::detect_all();
        assert_eq!(results.len(), providers::all_providers().len());
    }
    let elapsed = started.elapsed();

    // 50 detection sweeps. Existence checks only — should be trivially fast.
    assert!(
        elapsed < REFRESH_BUDGET,
        "50 detection sweeps took {elapsed:?}; detection must not scan skill trees"
    );
}
