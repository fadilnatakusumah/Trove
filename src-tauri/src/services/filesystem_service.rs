//! Filesystem safety layer (plan 03).
//!
//! The ONLY module allowed to mutate the filesystem. Providers and the
//! install service go through these functions; nothing else performs raw
//! `std::fs` mutations (tech-plan §22, Rule 11).
//!
//! Safety invariants:
//! - Installation/uninstall targets must pass [`ensure_within`] against an
//!   approved agent directory root — path escapes are rejected.
//! - Copy never silently overwrites: the caller picks an explicit
//!   [`OverwritePolicy`] (Rule 14).
//! - Deletion goes to the OS trash (§21) unless explicitly permanent.
//! - Skill files are untrusted input: text reads are size-capped (§23).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, ErrorCode};

/// Maximum bytes read from a single skill file — skill content is untrusted
/// input and must not be able to exhaust memory (§23).
const MAX_TEXT_FILE_BYTES: u64 = 1_048_576; // 1 MB

/// What to do when the copy destination already exists (Rule 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwritePolicy {
    /// Fail with `CONFLICT`. The default; caller decides what to do.
    Fail,
    /// Move the existing destination to the OS trash, then copy.
    BackupAndReplace,
}

/// Resolve a `~`-prefixed path against the user's home directory,
/// cross-platform. Non-`~` paths are returned unchanged.
pub fn resolve_home_relative(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or(path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Canonicalize `candidate` and verify it is inside `root`.
///
/// Every mutation target must pass through this — it is the path-traversal
/// guard (§22). `candidate` need not exist; the nearest existing ancestor is
/// canonicalized so not-yet-created directories are still checked.
pub fn ensure_within(root: &Path, candidate: &Path) -> Result<PathBuf, AppError> {
    let root = canonicalize_best_effort(root)?;
    let candidate = canonicalize_best_effort(candidate)?;

    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err(AppError::new(ErrorCode::PathEscape).with_detail(format!(
            "'{}' is outside approved root '{}'",
            candidate.display(),
            root.display()
        )))
    }
}

/// Canonicalize what exists, returning the logical path for the remainder.
/// Unlike `fs::canonicalize`, tolerates (and resolves through symlinks only
/// for the part that exists) paths whose tail does not exist yet.
fn canonicalize_best_effort(path: &Path) -> Result<PathBuf, AppError> {
    let mut missing: Vec<PathBuf> = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        match fs::canonicalize(&current) {
            Ok(resolved) => {
                let mut resolved = resolved;
                for part in missing.iter().rev() {
                    if let Some(name) = part.file_name() {
                        resolved.push(name);
                    }
                }
                return Ok(resolved);
            }
            Err(_) => {
                if current.file_name().is_some() {
                    missing.push(current.clone());
                    current = current.parent().unwrap_or(Path::new("/")).to_path_buf();
                } else {
                    // Reached the filesystem root without resolving.
                    return Err(AppError::new(ErrorCode::PathEscape)
                        .with_detail(format!("cannot resolve '{}'", path.display())));
                }
            }
        }
    }
}

pub fn dir_exists(path: &Path) -> bool {
    path.is_dir()
}

pub fn ensure_dir(path: &Path) -> Result<(), AppError> {
    fs::create_dir_all(path).map_err(|e| io_to_app_error(&e, path))
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

pub fn list_dir_entries(path: &Path) -> Result<Vec<DirEntry>, AppError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| io_to_app_error(&e, path))? {
        let entry = entry.map_err(|e| io_to_app_error(&e, path))?;
        let file_type = entry.file_type().map_err(|e| io_to_app_error(&e, path))?;
        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Read a text file, capped at [`MAX_TEXT_FILE_BYTES`]. Files larger than the
/// cap are rejected rather than truncated — a corrupted/huge file should fail
/// validation loudly (§23).
pub fn read_text_file_capped(path: &Path) -> Result<String, AppError> {
    let meta = fs::metadata(path).map_err(|e| io_to_app_error(&e, path))?;
    if meta.len() > MAX_TEXT_FILE_BYTES {
        return Err(AppError::new(ErrorCode::InvalidSkill).with_detail(format!(
            "file '{}' is {} bytes, exceeding the {} byte limit",
            path.display(),
            meta.len(),
            MAX_TEXT_FILE_BYTES
        )));
    }
    fs::read_to_string(path).map_err(|e| io_to_app_error(&e, path))
}

/// Recursively copy `src` into `dest` under the given overwrite policy.
///
/// Every copied path is verified with [`ensure_within`] against `dest_root`
/// (the approved skills directory), so a malicious source tree containing
/// entries like `../...` can never write outside it.
pub fn copy_dir_checked(
    src: &Path,
    dest: &Path,
    dest_root: &Path,
    policy: OverwritePolicy,
) -> Result<(), AppError> {
    if dest.exists() {
        match policy {
            OverwritePolicy::Fail => {
                return Err(AppError::new(ErrorCode::Conflict).with_detail(format!(
                    "destination '{}' already exists",
                    dest.display()
                )))
            }
            OverwritePolicy::BackupAndReplace => {
                move_to_trash(dest)?;
            }
        }
    }

    copy_recursive(src, dest, dest_root).map_err(|(path, e)| io_to_app_error(&e, &path))
}

fn copy_recursive(
    src: &Path,
    dest: &Path,
    dest_root: &Path,
) -> Result<(), (PathBuf, io::Error)> {
    // Guard the destination, not the source: we only ever write under dest_root.
    ensure_within(dest_root, dest).map_err(|e| (dest.to_path_buf(), io::Error::other(e)))?;

    let meta = fs::symlink_metadata(src).map_err(|e| (src.to_path_buf(), e))?;
    if meta.file_type().is_symlink() {
        // Never follow or recreate symlinks from untrusted sources (§22): a
        // symlink could point anywhere; recreating it could too. Skip them.
        log::warn!("skipping symlink during copy: {}", src.display());
        return Ok(());
    }

    if meta.is_dir() {
        fs::create_dir_all(dest).map_err(|e| (dest.to_path_buf(), e))?;
        for entry in fs::read_dir(src).map_err(|e| (src.to_path_buf(), e))? {
            let entry = entry.map_err(|e| (src.to_path_buf(), e))?;
            copy_recursive(&entry.path(), &dest.join(entry.file_name()), dest_root)?;
        }
        Ok(())
    } else {
        fs::copy(src, dest).map(|_| ()).map_err(|e| (src.to_path_buf(), e))
    }
}

/// Move a path to the OS trash/recycle bin (§21). Recoverable by the user.
pub fn move_to_trash(path: &Path) -> Result<(), AppError> {
    trash::delete(path).map_err(|e| {
        AppError::new(ErrorCode::PermissionDenied).with_detail(format!(
            "failed to move '{}' to trash: {e}",
            path.display()
        ))
    })
}

/// Rename `from` to `to`, guarding both ends against `root`.
///
/// Enable/disable uses this, where both paths are siblings inside one skill
/// directory — the guard keeps that a property of the code, not an assumption.
pub fn rename_checked(root: &Path, from: &Path, to: &Path) -> Result<(), AppError> {
    ensure_within(root, from)?;
    ensure_within(root, to)?;
    fs::rename(from, to).map_err(|e| io_to_app_error(&e, from))
}

/// Permanently delete a directory tree. Only for explicit user-confirmed
/// actions on paths that already passed [`ensure_within`].
pub fn delete_dir_permanent(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::new(ErrorCode::SkillNotFound)
            .with_detail(format!("'{}' does not exist", path.display())));
    }
    fs::remove_dir_all(path).map_err(|e| io_to_app_error(&e, path))
}

fn io_to_app_error(err: &io::Error, path: &Path) -> AppError {
    let code = match err.kind() {
        io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        io::ErrorKind::NotFound => ErrorCode::SkillNotFound,
        _ => ErrorCode::InstallFailed,
    };
    AppError::new(code).with_detail(format!("'{}': {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_root() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path().join("skills");
        fs::create_dir_all(&root).unwrap();
        // Keep TempDir alive via the returned binding at the call site.
        (dir, root)
    }

    #[test]
    fn rejects_path_escape() {
        let (_dir, root) = temp_root();
        let evil = root.join("../../../").join("Documents");
        let result = ensure_within(&root, &evil);
        assert!(matches!(result, Err(ref e) if e.code == ErrorCode::PathEscape));
    }

    #[test]
    fn accepts_path_inside_root() {
        let (_dir, root) = temp_root();
        let ok = root.join("react-expert");
        assert!(ensure_within(&root, &ok).is_ok());
    }

    #[test]
    fn resolves_home_relative_paths() {
        let p = resolve_home_relative("~/.claude/skills");
        assert!(!p.starts_with("~"));
        assert!(p.ends_with(".claude/skills") || p.ends_with(".claude\\skills"));
        // Non-tilde paths pass through unchanged.
        assert_eq!(resolve_home_relative("/tmp/x"), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn copy_fails_when_destination_exists_under_fail_policy() {
        let (dir, root) = temp_root();
        let src = dir.path().join("src-skill");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "hello").unwrap();

        let dest = root.join("react-expert");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("SKILL.md"), "existing").unwrap();

        let result = copy_dir_checked(&src, &dest, &root, OverwritePolicy::Fail);
        assert!(matches!(result, Err(ref e) if e.code == ErrorCode::Conflict));
        // Existing content untouched.
        assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "existing");
    }

    #[test]
    fn copy_recursive_copies_tree() {
        let (dir, root) = temp_root();
        let src = dir.path().join("src-skill");
        fs::create_dir_all(src.join("references")).unwrap();
        fs::write(src.join("SKILL.md"), "body").unwrap();
        fs::write(src.join("references/api.md"), "docs").unwrap();

        let dest = root.join("copied");
        copy_dir_checked(&src, &dest, &root, OverwritePolicy::Fail).unwrap();

        assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "body");
        assert_eq!(
            fs::read_to_string(dest.join("references/api.md")).unwrap(),
            "docs"
        );
    }

    #[test]
    fn copy_skips_symlinks_pointing_outside() {
        let (dir, root) = temp_root();
        let src = dir.path().join("src-skill");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "body").unwrap();

        // Symlink pointing outside the root entirely.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", src.join("steal")).unwrap();
        }
        #[cfg(windows)]
        {
            // Windows symlink creation may require privileges; write a normal
            // file instead and rely on the unix test for symlink semantics.
            fs::write(src.join("steal"), "x").unwrap();
        }

        let dest = root.join("with-symlink");
        copy_dir_checked(&src, &dest, &root, OverwritePolicy::Fail).unwrap();
        assert!(dest.join("SKILL.md").exists());
        #[cfg(unix)]
        assert!(!dest.join("steal").exists(), "symlink must not be copied");
    }

    #[test]
    fn read_text_file_caps_size() {
        let (dir, _root) = temp_root();
        let big = dir.path().join("big.md");
        fs::write(&big, vec![b'a'; MAX_TEXT_FILE_BYTES as usize + 1]).unwrap();
        let result = read_text_file_capped(&big);
        assert!(matches!(result, Err(ref e) if e.code == ErrorCode::InvalidSkill));

        let small = dir.path().join("small.md");
        fs::write(&small, "ok").unwrap();
        assert_eq!(read_text_file_capped(&small).unwrap(), "ok");
    }

    #[test]
    fn list_dir_entries_sorted_with_kinds() {
        let (dir, _root) = temp_root();
        // Use a dedicated subdir so the `skills/` dir created by temp_root
        // doesn't leak into the expected listing.
        let listing_dir = dir.path().join("listing");
        fs::create_dir_all(&listing_dir).unwrap();
        fs::create_dir_all(listing_dir.join("subdir")).unwrap();
        fs::write(listing_dir.join("a.md"), "x").unwrap();
        let entries = list_dir_entries(&listing_dir).unwrap();
        assert_eq!(
            entries,
            vec![
                DirEntry { name: "a.md".into(), is_dir: false },
                DirEntry { name: "subdir".into(), is_dir: true },
            ]
        );
    }

    #[test]
    fn delete_missing_returns_skill_not_found() {
        let (dir, _root) = temp_root();
        let result = delete_dir_permanent(&dir.path().join("nope"));
        assert!(matches!(result, Err(ref e) if e.code == ErrorCode::SkillNotFound));
    }
}
