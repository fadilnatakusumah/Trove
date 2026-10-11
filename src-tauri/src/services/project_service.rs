//! Project registration service (plan 05, tech-plan §15/§27/§28).
//!
//! Registered projects persist to one JSON file in the app-data dir. The
//! application stores only the registration; it never copies or moves
//! project content. No database (Rule 8).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, ErrorCode};
use crate::models::ProviderId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredProject {
    pub id: String,
    pub name: String,
    /// The real path. Used for IPC calls and identity — never for display.
    pub path: String,
    /// Display form (`~`-shortened, forward slashes).
    ///
    /// Computed here because only the backend knows the home directory; the
    /// frontend cannot shorten it itself. `default` keeps stores written before
    /// this field existed loadable, and `list()` refreshes it on every read so
    /// it can never be stale.
    #[serde(default)]
    pub display_path: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectStore {
    projects: Vec<RegisteredProject>,
}

impl ProjectStore {
    fn file_path(app_data_dir: &PathBuf) -> PathBuf {
        app_data_dir.join("projects.json")
    }

    fn load(app_data_dir: &PathBuf) -> Self {
        std::fs::read_to_string(Self::file_path(app_data_dir))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, app_data_dir: &PathBuf) -> Result<(), AppError> {
        let path = Self::file_path(app_data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::from_io(&e, parent))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| AppError::from_io(&e, &path))
    }
}

/// Ids are derived from the path (stable across restarts, no extra state).
pub fn project_id_for(path: &str) -> String {
    // Normalize separators and lowercase on Windows-insensitive parts, then
    // hash-free slug: take the last segment + short hash of the full path.
    let normalized = path.replace('\\', "/");
    let slug: String = normalized
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("project")
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    // Short digest disambiguates same-named projects in different parents.
    let mut hash: u64 = 5381;
    for b in normalized.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(b));
    }
    format!("{slug}-{:x}", hash & 0xffff)
}

/// Project name detection: last non-empty path segment.
pub fn detect_project_name(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("Project")
        .to_string()
}

/// Providers detected inside a project's agent directories.
pub fn detect_project_providers(project: &str) -> Vec<ProviderId> {
    let root = crate::services::filesystem_service::resolve_home_relative(project);
    let mut found = Vec::new();
    for (id, dirs) in [
        (
            ProviderId::Claude,
            vec![root.join(".claude").join("skills")],
        ),
        (
            ProviderId::Cursor,
            vec![
                root.join(".cursor").join("skills"),
                root.join(".agents").join("skills"),
            ],
        ),
        (ProviderId::Codex, vec![root.join(".agents").join("skills")]),
        (ProviderId::Opencode, vec![root.join(".opencode").join("skills")]),
    ] {
        if dirs.iter().any(|d| d.is_dir()) && !found.contains(&id) {
            found.push(id);
        }
    }
    found
}

/// Handle to the persisted project store, held in Tauri managed state.
pub struct ProjectRegistry {
    app_data_dir: PathBuf,
    cache: Mutex<ProjectStore>,
}

impl ProjectRegistry {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let store = ProjectStore::load(&app_data_dir);
        Self {
            app_data_dir,
            cache: Mutex::new(store),
        }
    }

    pub fn list(&self) -> Vec<RegisteredProject> {
        // Refresh the display form on read rather than trusting what was
        // persisted (the home directory can move between runs).
        self.cache
            .lock()
            .expect("project cache")
            .projects
            .iter()
            .map(|p| RegisteredProject {
                display_path: crate::services::filesystem_service::display_path(
                    Path::new(&p.path),
                ),
                ..p.clone()
            })
            .collect()
    }

    pub fn add(&self, path: &str) -> Result<RegisteredProject, AppError> {
        let resolved =
            crate::services::filesystem_service::resolve_home_relative(path);
        if !resolved.is_dir() {
            return Err(AppError::new(ErrorCode::SkillNotFound).with_detail(format!(
                "'{}' is not a directory",
                crate::services::filesystem_service::display_path(&resolved)
            )));
        }
        let path_str = resolved.to_string_lossy().into_owned();
        let id = project_id_for(&path_str);
        let project = RegisteredProject {
            id: id.clone(),
            name: detect_project_name(&path_str),
            display_path: crate::services::filesystem_service::display_path(&resolved),
            path: path_str,
        };

        let mut store = self.cache.lock().expect("project cache");
        if let Some(existing) = store.projects.iter_mut().find(|p| p.id == id) {
            // Re-adding refreshes name + provider detection, no duplicate.
            existing.name = project.name.clone();
        } else {
            store.projects.push(project.clone());
        }
        store.save(&self.app_data_dir)?;
        Ok(project)
    }

    pub fn remove(&self, id: &str) -> Result<(), AppError> {
        let mut store = self.cache.lock().expect("project cache");
        let before = store.projects.len();
        store.projects.retain(|p| p.id != id);
        if store.projects.len() == before {
            return Err(AppError::new(ErrorCode::SkillNotFound)
                .with_detail(format!("no registered project with id '{id}'")));
        }
        store.save(&self.app_data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_stable_and_disambiguates_paths() {
        let a1 = project_id_for("~/Projects/prophero");
        let a2 = project_id_for("~/Projects/prophero");
        let b = project_id_for("~/Work/prophero");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert!(a1.starts_with("prophero-"));
    }

    #[test]
    fn name_from_last_segment() {
        assert_eq!(detect_project_name("~/Projects/my-saas/"), "my-saas");
        assert_eq!(detect_project_name("C:\\Dev\\AI Wallpaper"), "AI Wallpaper");
    }

    #[test]
    fn registry_round_trips_and_rejects_missing_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let registry = ProjectRegistry::new(tmp.path().to_path_buf());

        // Missing dir rejected.
        assert!(registry.add("/definitely/not/here").is_err());

        let project_dir = tmp.path().join("proj");
        std::fs::create_dir_all(&project_dir).unwrap();
        let added = registry
            .add(project_dir.to_str().unwrap())
            .expect("add real dir");

        // Re-add doesn't duplicate.
        registry.add(project_dir.to_str().unwrap()).unwrap();
        assert_eq!(registry.list().len(), 1);

        // Persistence survives a fresh registry instance.
        let reloaded = ProjectRegistry::new(tmp.path().to_path_buf());
        assert_eq!(reloaded.list(), vec![added.clone()]);

        reloaded.remove(&added.id).unwrap();
        assert!(reloaded.list().is_empty());
        // Removing unknown id errors.
        assert!(reloaded.remove(&added.id).is_err());
    }
}
