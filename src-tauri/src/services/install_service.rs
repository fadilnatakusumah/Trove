//! Install service (plan 06, tech-plan §17/§18/§19/§22/§23).
//!
//! Flow: validate → temp clone/copy → inspect → validate → compute
//! destination → ensure_within → conflict check → copy → cleanup.
//!
//! Security invariants (§23):
//! - Nothing from a cloned repo is ever executed.
//! - Every destination passes `ensure_within` against the approved skills
//!   root — traversal (including paths discovered inside the repo) is
//!   rejected with `PATH_ESCAPE`.
//! - Symlinks inside sources are skipped by the copy layer (plan 03).
//! - Clone output/URLs are sanitized before logging (§31).
//!
//! Repo layouts (verified, plan 06 "Format findings"):
//! - Single-skill repo: SKILL.md at root → whole repo installs as one skill.
//! - Collection repo: one-level-deep `<dir>/SKILL.md` → user selects skills.
//! - Deeper nesting is not scanned (documented ceiling, Rule 12).

use std::path::Path;

use serde::Serialize;

use crate::errors::{AppError, ErrorCode};
use crate::models::{Skill, SkillScope, SkillSource};
use crate::providers::AgentProvider;
use crate::services::filesystem_service as fs;
use crate::services::skill_service::parse_frontmatter;

/// How long a git clone may run before being abandoned (§18/§6.4).
const CLONE_TIMEOUT_SECS: u32 = 60;

/// One discovered skill in a source (pre-install inspection, §19).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSkill {
    /// Directory name — the skill's install identity.
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Path of the skill directory **relative to the source root** (`.` for a
    /// single-skill repo). Relative, not absolute: inspection and install run
    /// separate stagings in different temp dirs, so absolute paths from the
    /// first staging would be stale by the time install re-clones.
    pub dir: String,
}

/// Result of `inspect_source` — drives the install dialog (§19 messaging).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceInspection {
    /// One skill at the source root.
    Single(DiscoveredSkill),
    /// Multiple skills one level deep; user selects which to install.
    Collection { skills: Vec<DiscoveredSkill> },
}

impl SourceInspection {
    pub fn skills(&self) -> Vec<&DiscoveredSkill> {
        match self {
            SourceInspection::Single(s) => vec![s],
            SourceInspection::Collection { skills } => skills.iter().collect(),
        }
    }
}

/// A validated, ready-to-install source staged in a temp directory.
/// The TempDir binding keeps the staging alive until this value is dropped.
#[derive(Debug)]
pub struct StagedSource {
    _temp: tempfile::TempDir,
    inspection: SourceInspection,
}

impl StagedSource {
    pub fn inspection(&self) -> &SourceInspection {
        &self.inspection
    }

    /// Root of the staged copy. Install resolves relative skill paths here.
    pub fn root(&self) -> &Path {
        self._temp.path()
    }
}

/// Validate input and stage the source in a temp dir.
pub fn stage_source(source: &SkillSource) -> Result<StagedSource, AppError> {
    let temp = tempfile::tempdir().map_err(|e| {
        AppError::new(ErrorCode::InstallFailed).with_detail(format!("temp dir: {e}"))
    })?;

    let staged_root = match source {
        SkillSource::Git { url, ref_, subpath } => {
            let loc = parse_git_location(url, ref_.as_deref())?;
            clone_shallow(&loc.repo_url, loc.ref_.as_deref(), temp.path())?;
            // Browser blob/tree links carry the skill's directory; resolve it
            // inside the clone, guarded against traversal (§22).
            match subpath.clone().or(loc.subpath) {
                Some(sub) => {
                    let candidate = temp.path().join(sub.replace('\\', "/"));
                    let resolved = fs::ensure_within(temp.path(), &candidate)?;
                    if !resolved.is_dir() {
                        return Err(AppError::new(ErrorCode::InvalidSkill).with_detail(
                            format!("the repository has no directory '{sub}'"),
                        ));
                    }
                    resolved
                }
                None => temp.path().to_path_buf(),
            }
        }
        SkillSource::Local { path } => {
            let resolved = fs::resolve_home_relative(path);
            if !resolved.is_dir() {
                return Err(AppError::new(ErrorCode::InvalidSkill).with_detail(format!(
                    "local folder '{}' does not exist",
                    resolved.display()
                )));
            }
            // Copy the source INTO the temp root (mirroring git clone's
            // behavior: the repo root = staged root), so single-skill repos
            // keep their root SKILL.md at the staged root.
            copy_contents(&resolved, temp.path())?;
            temp.path().to_path_buf()
        }
    };

    let inspection = inspect_staged(&staged_root)?;
    Ok(StagedSource { _temp: temp, inspection })
}

/// Copy `src`'s entries into `dest` (dir contents merge, not the dir itself).
/// Entries are individually verified against `dest` as root.
fn copy_contents(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::ensure_dir(dest)?;
    for entry in fs::list_dir_entries(src)? {
        let from = src.join(&entry.name);
        let to = dest.join(&entry.name);
        if entry.is_dir {
            fs::copy_dir_checked(&from, &to, dest, fs::OverwritePolicy::Fail)?;
        } else {
            fs::ensure_dir(dest)?;
            std::fs::copy(&from, &to).map_err(|e| crate::errors::AppError::from_io(&e, &from))?;
        }
    }
    Ok(())
}

/// Strip credentials from a URL before it appears in logs or errors (§31).
/// `https://user:token@github.com/a/b` → `https://github.com/a/b`.
fn sanitize_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    match rest.split_once('@') {
        // Only treat '@' as credentials when it precedes the first path
        // separator; otherwise it's part of the path and must be kept.
        Some((creds, host_and_path))
            if !creds.contains('/') && !host_and_path.is_empty() =>
        {
            format!("{scheme}://{host_and_path}")
        }
        _ => url.to_string(),
    }
}

/// A cloneable repository plus where inside it the skill(s) live.
#[derive(Debug, Clone, PartialEq)]
struct GitLocation {
    repo_url: String,
    ref_: Option<String>,
    subpath: Option<String>,
}

/// Accept the URL shapes users actually paste and reduce them to a cloneable
/// repository. Copying a SKILL.md link from the browser (a `blob`/`tree`
/// link) is the common mistake — it's not cloneable, but it *does* name the
/// exact skill directory, so we extract the repo + subpath from it.
fn parse_git_location(input: &str, explicit_ref: Option<&str>) -> Result<GitLocation, AppError> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(
            AppError::new(ErrorCode::InstallFailed).with_detail("enter a repository URL")
        );
    }

    // scp-style ssh URL — git clones it verbatim.
    if trimmed.starts_with("git@") {
        return Ok(GitLocation {
            repo_url: trimmed.to_string(),
            ref_: explicit_ref.map(str::to_string),
            subpath: None,
        });
    }

    // Assume https for schemeless input (`github.com/owner/repo`).
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let Some((scheme, rest)) = with_scheme.split_once("://") else {
        return Err(AppError::new(ErrorCode::InstallFailed).with_detail(format!(
            "'{}' is not a valid repository URL",
            sanitize_url(trimmed)
        )));
    };

    // rest = host/owner/repo[/blob|tree/ref/path...]
    let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    let host = segments.first().copied().unwrap_or_default();
    // Everything after the host: for a repo URL this is exactly owner + repo.
    let path_segments = &segments[1.min(segments.len())..];

    // Browser link: host/owner/repo/(blob|tree|raw|edit|blame)/ref/path...
    let is_web_link = path_segments.len() >= 4
        && matches!(path_segments[2], "blob" | "tree" | "raw" | "edit" | "blame");

    if is_web_link {
        let repo_url = format!("{scheme}://{host}/{}/{}", path_segments[0], path_segments[1]);
        let ref_ = explicit_ref
            .map(str::to_string)
            .or_else(|| path_segments.get(3).map(|s| (*s).to_string()));
        let path = path_segments.get(4..).map(|p| p.join("/")).unwrap_or_default();
        // `blob` names a file (typically SKILL.md) → its parent is the skill
        // dir. `tree` already names a directory.
        let subpath = if path.is_empty() {
            None
        } else if matches!(path_segments[2], "blob" | "blame") {
            path.rsplit_once('/').map(|(dir, _)| dir.to_string())
        } else {
            Some(path)
        };
        return Ok(GitLocation {
            repo_url,
            ref_,
            subpath,
        });
    }

    // Plain repo URL needs owner + repo; a profile page (`/owner`) is not
    // enough to clone.
    if path_segments.len() < 2 {
        return Err(AppError::new(ErrorCode::InstallFailed).with_detail(format!(
            "'{}' is not a repository URL — use https://github.com/<owner>/<repo>",
            sanitize_url(trimmed)
        )));
    }

    Ok(GitLocation {
        repo_url: format!("{scheme}://{host}/{}/{}", path_segments[0], path_segments[1]),
        ref_: explicit_ref.map(str::to_string),
        subpath: None,
    })
}

/// Shallow clone with fixed argv — no shell, timeout-guarded (§22 rule 7).
/// Git's stderr is captured so the user sees the real reason (bad URL,
/// private repo, network) instead of a generic failure (§25).
fn clone_shallow(url: &str, ref_: Option<&str>, into: &Path) -> Result<(), AppError> {
    if !crate::services::git_service::probe_git()?.available {
        return Err(AppError::new(ErrorCode::GitMissing));
    }

    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(r) = ref_ {
        // A branch/tag name is required here; a commit SHA would fail a
        // shallow clone (documented ceiling — plan 06 §4).
        cmd.arg("--branch").arg(r);
    }
    cmd.arg(url).arg(into);

    // Timeout guard: a hanging remote must not wedge the flow (§6.4).
    let (status, stderr) = run_with_timeout(&mut cmd, CLONE_TIMEOUT_SECS)?;
    if !status.success() {
        let reason = git_error_summary(&stderr, url);
        log::warn!("clone failed for {}: {}", sanitize_url(url), reason);
        return Err(AppError::new(ErrorCode::InstallFailed).with_detail(reason));
    }
    log::info!("cloned {}", sanitize_url(url));
    Ok(())
}

/// Turn git's stderr into one actionable line, credential-sanitized (§31).
fn git_error_summary(stderr: &str, url: &str) -> String {
    // Scrub credentials that git may have echoed back.
    let sanitized = stderr.replace(url, &sanitize_url(url));
    let lines: Vec<&str> = sanitized
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    let pick = |needle: &str| lines.iter().copied().find(|l| l.contains(needle));

    // git's own summary line is the most informative; the server's `remote:`
    // lines are vaguer ("Repository not found.") and never carry the URL.
    let line = pick("fatal:")
        .or_else(|| pick("error:"))
        .or_else(|| pick("could not"))
        .or_else(|| pick("unable"))
        .or_else(|| pick("denied"))
        .or_else(|| lines.first().copied())
        .unwrap_or("git clone failed");

    // Drop git's prefixes; the message reads better without them.
    let cleaned = line
        .trim_start_matches("remote:")
        .trim_start_matches("fatal:")
        .trim_start_matches("error:")
        .trim();

    // git quotes an empty repo name when it can't parse the URL — say what we
    // actually tried so the failure is actionable (§25).
    if cleaned.contains("repository '' not found") || cleaned.contains("repository ' not found") {
        return format!("repository not found: {}", sanitize_url(url));
    }
    cleaned.to_string()
}

/// Run a command with a timeout, returning its exit status and captured
/// stderr. `try_wait` polling keeps the child handle local so it can be
/// killed on timeout (no orphaned git processes).
fn run_with_timeout(
    cmd: &mut std::process::Command,
    timeout_secs: u32,
) -> Result<(std::process::ExitStatus, String), AppError> {
    use std::io::Read;
    use std::time::{Duration, Instant};

    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            AppError::new(ErrorCode::InstallFailed).with_detail(format!("git spawn: {e}"))
        })?;

    // Drain stderr on its own thread so a chatty clone can't fill the pipe
    // buffer and deadlock the wait loop.
    let mut stderr_pipe = child.stderr.take().expect("stderr piped");
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(u64::from(timeout_secs));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::new(ErrorCode::InstallFailed).with_detail(format!(
                        "git clone timed out after {timeout_secs}s"
                    )));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(AppError::new(ErrorCode::InstallFailed)
                    .with_detail(format!("git wait: {e}")))
            }
        }
    };

    let stderr = stderr_handle.join().unwrap_or_default();
    Ok((status, stderr))
}

/// Locate skill definitions in a staged source (both layouts).
fn inspect_staged(root: &Path) -> Result<SourceInspection, AppError> {
    // Layout 1: root SKILL.md. The install id comes from the frontmatter
    // `name` (spec: name must match the directory the skill will live in);
    // the staged root's own basename is meaningless for git clones.
    if root.join("SKILL.md").is_file() {
        let mut skill = discover_skill(root, root)?;
        let fm = fs::read_text_file_capped(&root.join("SKILL.md"))
            .ok()
            .and_then(|c| parse_frontmatter(&c));
        skill.id = fm
            .and_then(|f| f.name)
            .ok_or_else(|| {
                AppError::new(ErrorCode::InvalidSkill).with_detail(
                    "root SKILL.md has no `name` in frontmatter — cannot derive install name",
                )
            })?;
        return Ok(SourceInspection::Single(skill));
    }

    // Layout 2: one level deep.
    let entries = fs::list_dir_entries(root)?;
    let mut skills = Vec::new();
    for entry in entries {
        if !entry.is_dir {
            continue;
        }
        let dir = root.join(&entry.name);
        if dir.join("SKILL.md").is_file() {
            skills.push(discover_skill(root, &dir)?);
        }
    }
    if skills.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidSkill).with_detail(
            "no SKILL.md at the source root and none in immediate subdirectories",
        ));
    }
    Ok(SourceInspection::Collection { skills })
}

/// Read frontmatter of one discovered skill directory. `root` is the staged
/// source root; `dir` is recorded relative to it (`.` for the root itself).
fn discover_skill(root: &Path, dir: &Path) -> Result<DiscoveredSkill, AppError> {
    let content = fs::read_text_file_capped(&dir.join("SKILL.md"))?;
    let fm = parse_frontmatter(&content);
    let (name, description) = match &fm {
        Some(f) => (f.name.clone(), f.description.clone()),
        None => (None, None),
    };
    let rel = dir
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    Ok(DiscoveredSkill {
        id: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        name,
        description,
        dir: if rel.is_empty() { ".".to_string() } else { rel },
    })
}

/// Install previously discovered skills into the provider's skills dir.
///
/// `skill_dirs` selects which discovered directories to install (root layout
/// passes one, collections pass the user's selection). Conflicts are checked
/// per-skill: one conflicting skill doesn't block the others (plan 06 §6.3).
pub fn install_skills(
    provider: &dyn AgentProvider,
    source: &SkillSource,
    staged: &StagedSource,
    skill_dirs: &[String],
    scope: &SkillScope,
) -> Result<Vec<Skill>, AppError> {
    let dest_root = match scope {
        SkillScope::Global => provider
            .global_skills_dir()
            .ok_or_else(|| AppError::new(ErrorCode::ProviderNotFound))?,
        SkillScope::Project { project_path } => {
            let dirs = provider.project_skills_dirs(&fs::resolve_home_relative(project_path));
            let first = dirs.first().ok_or_else(|| {
                AppError::new(ErrorCode::ProviderNotFound)
            })?;
            // Primary convention dir for installs; ensure it exists.
            fs::ensure_dir(first)?;
            first.clone()
        }
    };
    fs::ensure_dir(&dest_root)?;

    // For the root layout, the selected "dir" is the staged root; its install
    // name is the discovered frontmatter name (set by inspect_staged).
    let discovered: std::collections::HashMap<String, &DiscoveredSkill> = staged
        .inspection()
        .skills()
        .into_iter()
        .map(|s| (s.dir.clone(), s))
        .collect();

    let mut installed = Vec::new();
    for rel in skill_dirs {
        // `rel` is relative to the staged root and was produced by inspection.
        // Inspection and install stage SEPARATELY (different temp dirs), so the
        // path must be re-resolved against THIS staging — an absolute path from
        // the inspection temp dir would already be gone.
        let candidate = if rel == "." {
            staged.root().to_path_buf()
        } else {
            staged.root().join(rel.replace('\\', "/"))
        };
        let src = fs::ensure_within(staged.root(), &candidate)?;
        if !src.is_dir() {
            return Err(AppError::new(ErrorCode::SkillNotFound).with_detail(format!(
                "'{rel}' does not exist in the repository"
            )));
        }

        let id = discovered
            .get(rel)
            .map(|s| s.id.clone())
            .or_else(|| {
                src.file_name().map(|n| n.to_string_lossy().into_owned())
            })
            .ok_or_else(|| {
                AppError::new(ErrorCode::InvalidSkill).with_detail(format!("bad dir '{rel}'"))
            })?;
        // Rule 14: the destination must not exist. Fail policy — the UI
        // resolves conflicts explicitly per skill before calling again.
        let dest = dest_root.join(&id);
        fs::copy_dir_checked(&src, &dest, &dest_root, fs::OverwritePolicy::Fail)?;

        installed.push(build_installed_skill(provider, &dest, scope, source));
    }
    Ok(installed)
}

/// Build the returned `Skill` for a freshly installed directory.
fn build_installed_skill(
    provider: &dyn AgentProvider,
    dest: &Path,
    scope: &SkillScope,
    source: &SkillSource,
) -> Skill {
    let id = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fm = fs::read_text_file_capped(&dest.join("SKILL.md"))
        .ok()
        .and_then(|c| parse_frontmatter(&c));

    let mut metadata = fm.as_ref().map(|f| f.metadata.clone()).unwrap_or_default();
    if let Some(name) = fm.as_ref().and_then(|f| f.name.as_ref()) {
        if *name != id {
            metadata.insert("nameMismatch".to_string(), name.clone());
        }
    }

    Skill {
        id,
        name: fm.as_ref().and_then(|f| f.name.clone()).unwrap_or_else(|| "Skill".to_string()),
        description: fm.as_ref().and_then(|f| f.description.clone()),
        source: source.clone(),
        provider: provider.id(),
        scope: scope.clone(),
        path: crate::providers::display_home_path(dest),
        enabled: true,
        version: fm.as_ref().and_then(|f| f.version.clone()),
        installed_at: Some(now_rfc3339()),
        metadata,
    }
}

/// RFC 3339 UTC timestamp without pulling chrono (Rule 2): a Unix-epoch
/// formatter good enough for "when did I install this".
fn now_rfc3339() -> String {
    // ponytail: hand-rolled epoch→UTC conversion, no calendar edge cases
    // beyond what's needed for a display timestamp. Upgrade path: `time` crate.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Howard Hinnant's civil_from_days — small, correct, well-known.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::claude::ClaudeProvider;
    use std::path::PathBuf;

    fn write_skill(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: A test skill\n---\nbody"),
        )
        .unwrap();
        dir
    }

    #[test]
    fn stage_local_single_skill_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("SKILL.md"), "---\nname: root-skill\ndescription: d\n---\n").unwrap();

        let staged = stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap();
        match staged.inspection() {
            SourceInspection::Single(s) => {
                assert_eq!(s.id, "root-skill", "root layout: id = frontmatter name");
            }
            SourceInspection::Collection { .. } => panic!("expected single"),
        }
    }

    #[test]
    fn stage_local_collection_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_skill(tmp.path(), "alpha");
        write_skill(tmp.path(), "beta");

        let staged = stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap();
        let skills = staged.inspection().skills();
        assert_eq!(skills.len(), 2);
        let mut ids: Vec<_> = skills.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn stage_invalid_source_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No SKILL.md anywhere.
        std::fs::create_dir_all(tmp.path().join("nested/deeper")).unwrap();

        let err = stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidSkill);
    }

    #[test]
    fn stage_missing_local_path_errors() {
        let err = stage_source(&SkillSource::Local {
            path: "/definitely/not/here".to_string(),
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidSkill);
    }

    #[test]
    fn install_collection_subset_into_project_scope() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_skill(tmp.path(), "alpha");
        write_skill(tmp.path(), "beta");
        let staged = stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap();

        // Fake HOME not possible for provider dirs; install into a project
        // scope rooted in a temp project we control.
        let project = tempfile::TempDir::new().unwrap();
        let project_path = project.path().to_string_lossy().into_owned();
        let source = SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        };

        let dirs: Vec<String> = staged
            .inspection()
            .skills()
            .iter()
            .filter(|s| s.id == "alpha")
            .map(|s| s.dir.clone())
            .collect();

        let installed = install_skills(
            &ClaudeProvider,
            &source,
            &staged,
            &dirs,
            &SkillScope::Project { project_path },
        )
        .unwrap();

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "alpha");
        assert!(installed[0].installed_at.is_some());
        assert!(
            project
                .path()
                .join(".claude/skills/alpha/SKILL.md")
                .exists(),
            "skill copied into project .claude/skills"
        );
        assert!(!project.path().join(".claude/skills/beta").exists(), "unselected skill not installed");
    }

    #[test]
    fn install_conflict_does_not_overwrite() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_skill(tmp.path(), "dup");
        let staged = stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap();

        let project = tempfile::TempDir::new().unwrap();
        let project_path = project.path().to_string_lossy().into_owned();
        // Pre-existing skill with different content.
        let existing = project.path().join(".claude/skills/dup");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(existing.join("SKILL.md"), "---\nname: dup\ndescription: original\n---\nORIGINAL").unwrap();

        // Discovered dirs are relative to the staged root.
        let dirs: Vec<String> = staged.inspection().skills().iter().map(|s| s.dir.clone()).collect();

        let err = install_skills(
            &ClaudeProvider,
            &SkillSource::Local { path: tmp.path().to_string_lossy().into_owned() },
            &staged,
            &dirs,
            &SkillScope::Project { project_path },
        )
        .unwrap_err();

        assert_eq!(err.code, ErrorCode::Conflict);
        assert!(
            std::fs::read_to_string(existing.join("SKILL.md")).unwrap().contains("ORIGINAL"),
            "existing skill untouched (Rule 14)"
        );
    }

    #[test]
    fn sanitize_url_strips_credentials() {
        assert_eq!(
            sanitize_url("https://user:token123@github.com/example/skill"),
            "https://github.com/example/skill"
        );
        assert_eq!(
            sanitize_url("https://github.com/example/skill"),
            "https://github.com/example/skill"
        );
        // An '@' inside the path (not credentials) must survive.
        assert_eq!(
            sanitize_url("https://github.com/a/b@c"),
            "https://github.com/a/b@c"
        );
    }

    #[test]
    fn parses_repo_url() {
        let loc = parse_git_location("https://github.com/anthropics/skills", None).unwrap();
        assert_eq!(loc.repo_url, "https://github.com/anthropics/skills");
        assert_eq!(loc.subpath, None);
    }

    #[test]
    fn parses_blob_file_link_into_repo_and_subpath() {
        // The exact link shape users paste from a browser.
        let loc = parse_git_location(
            "https://github.com/anthropics/skills/blob/main/plugins/frontend-design/skills/frontend-design/SKILL.md",
            None,
        )
        .unwrap();
        assert_eq!(loc.repo_url, "https://github.com/anthropics/skills");
        assert_eq!(loc.ref_.as_deref(), Some("main"));
        assert_eq!(
            loc.subpath.as_deref(),
            Some("plugins/frontend-design/skills/frontend-design"),
            "SKILL.md link resolves to its containing directory"
        );
    }

    #[test]
    fn parses_tree_link_into_repo_and_subpath() {
        let loc = parse_git_location(
            "https://github.com/anthropics/skills/tree/main/plugins/frontend-design/skills/frontend-design",
            None,
        )
        .unwrap();
        assert_eq!(loc.repo_url, "https://github.com/anthropics/skills");
        assert_eq!(
            loc.subpath.as_deref(),
            Some("plugins/frontend-design/skills/frontend-design")
        );
    }

    #[test]
    fn accepts_schemeless_and_scp_urls() {
        let schemeless = parse_git_location("github.com/owner/repo", None).unwrap();
        assert_eq!(schemeless.repo_url, "https://github.com/owner/repo");

        let scp = parse_git_location("git@github.com:owner/repo.git", None).unwrap();
        assert_eq!(scp.repo_url, "git@github.com:owner/repo.git");
    }

    #[test]
    fn rejects_non_repo_urls() {
        assert!(parse_git_location("https://github.com/anthropics", None).is_err());
        assert!(parse_git_location("", None).is_err());
    }

    #[test]
    fn git_error_summary_reports_repo_name() {
        let stderr = "Cloning into '/tmp/x'...\nremote: Repository not found.\nfatal: repository 'https://github.com/a/b/' not found\n";
        let summary = git_error_summary(stderr, "https://github.com/a/b");
        assert!(
            summary.contains("https://github.com/a/b"),
            "message names the repo we tried: {summary}"
        );
        assert!(!summary.contains("repository ' not found"));
    }

    #[test]
    fn rfc3339_timestamp_shape() {
        let ts = now_rfc3339();
        // 2026-01-01T00:00:00Z shape check.
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
