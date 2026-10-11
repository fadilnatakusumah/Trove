//! Tauri command layer.
//!
//! Thin, explicitly-typed IPC entry points. Commands validate input, delegate
//! to services, and return structured `AppError`s. No business logic here
//! (tech-plan §24).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::errors::{AppError, ErrorCode};
use crate::services::filesystem_service as fs_service;
use crate::services::git_service;
use crate::services::install_service::{StagedInspection, StagedSource};

/// Staged sources kept alive between `inspect_source` and `install_skill`.
///
/// Inspect and install are two IPC calls but one operation: the user reviews
/// what inspect found, then installs it. Re-staging for the install step would
/// clone the same source a second time — and for a pasted GitHub link, re-parse
/// the URL, which resolves the subpath differently depending on what the fresh
/// clone contains. That mismatch is how a whole repository ended up copied into
/// a skills directory under a skill's name.
///
/// Entries are keyed by a server-issued id, never by the request contents: an
/// id cannot drift out of sync with the other language's serialization.
#[derive(Default)]
pub struct StagingRegistry {
    next_id: AtomicU64,
    staged: Mutex<HashMap<String, Arc<StagedSource>>>,
}

impl StagingRegistry {
    /// Park a staging and return the handle that claims it.
    ///
    /// Previous entries are dropped first: only one install dialog exists at a
    /// time, so anything still parked is an abandoned inspection whose temp dir
    /// would otherwise linger.
    fn park(&self, staged: StagedSource) -> String {
        let id = format!("staging-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let mut map = self.staged.lock().expect("staging registry");
        map.clear();
        map.insert(id.clone(), Arc::new(staged));
        id
    }

    /// Borrow a staging. Deliberately does **not** remove it: installing can
    /// run twice for one inspection — the first attempt is refused with a
    /// CONFLICT, and the user then confirms a replace. Consuming the staging on
    /// the first attempt would strand that retry.
    ///
    /// An `Arc` keeps the temp dir alive for as long as any in-flight install
    /// still references it, even though the map entry may be replaced.
    fn claim(&self, staging_id: &str) -> Option<Arc<StagedSource>> {
        self.staged
            .lock()
            .expect("staging registry")
            .get(staging_id)
            .cloned()
    }
}

/// Log a command failure once, at the IPC boundary (§31).
///
/// Errors already carry a machine code plus human copy, and nothing sensitive
/// is ever put in them (credentials are stripped before they reach a message),
/// so this is a safe single choke point instead of scattered logging.
fn logged<T>(what: &str, result: Result<T, AppError>) -> Result<T, AppError> {
    if let Err(e) = &result {
        log::error!("{what} failed: {:?} — {}", e.code, e.message);
    }
    result
}

/// Reveal the application log directory in the OS file manager (§31).
/// Returns the path so the UI can show the user where their logs live.
#[tauri::command]
pub async fn open_logs(app: tauri::AppHandle) -> Result<String, AppError> {
    let dir = app.path().app_log_dir().map_err(|e| {
        AppError::new(ErrorCode::PermissionDenied).with_detail(e.to_string())
    })?;
    if !dir.is_dir() {
        // Not an error state worth alarming the user with — just nothing yet.
        return Err(AppError::new(ErrorCode::SkillNotFound).with_detail(format!(
            "no logs have been written yet at '{}'",
            fs_service::display_path(&dir)
        )));
    }
    let shown = fs_service::display_path(&dir);
    app.opener()
        .open_path(shown.clone(), None::<&str>)
        .map_err(|e| {
            AppError::new(ErrorCode::PermissionDenied)
                .with_detail(format!("failed to open '{shown}': {e}"))
        })?;
    log::info!("opened log directory {shown}");
    Ok(shown)
}

/// Opens a skill/project directory with the OS file manager
/// (Finder/Explorer/xdg-open, tech-plan §29). Only *reveals* — never executes.
#[tauri::command]
pub async fn open_folder(
    app: tauri::AppHandle,
    path: String,
) -> Result<(), AppError> {
    let resolved = fs_service::resolve_home_relative(&path);
    if !resolved.is_dir() {
        return Err(AppError::new(ErrorCode::SkillNotFound)
            .with_detail(format!("'{}' is not a directory", fs_service::display_path(&resolved))));
    }
    log::info!("opening folder {}", fs_service::display_path(&resolved));
    app.opener()
        .open_path(resolved.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| {
            AppError::new(ErrorCode::PermissionDenied)
                .with_detail(format!("failed to open '{}': {e}", fs_service::display_path(&resolved)))
        })
}

/// Probe whether Git is installed (plan 03).
#[tauri::command]
pub async fn check_git() -> Result<git_service::GitProbe, AppError> {
    tauri::async_runtime::spawn_blocking(git_service::probe_git)
        .await
        .map_err(|e| {
            AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string())
        })?
}

// ---------------------------------------------------------------------------
// Providers (plan 04)
// ---------------------------------------------------------------------------

/// Detect every registered agent provider on this machine.
#[tauri::command]
pub async fn detect_providers(
) -> Result<Vec<crate::providers::ProviderDetectionResult>, AppError> {
    // Filesystem reads only; block briefly rather than spawning.
    Ok(crate::providers::detect_all())
}

/// List skills for one provider in one scope (global or a registered project).
#[tauri::command]
pub async fn get_skills(
    provider_id: crate::models::ProviderId,
    scope: crate::models::SkillScope,
) -> Result<Vec<crate::models::Skill>, AppError> {
    logged(
        "load skills",
        tauri::async_runtime::spawn_blocking(move || {
            let provider = crate::providers::provider_by_id(provider_id)?;
            crate::providers::skills_for_scope(provider.as_ref(), &scope)
        })
        .await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?,
    )
}

/// Read-only file listing for the skill detail view (§14). The path must be a
/// directory; entries are name + is_dir only — no content crosses the IPC
/// boundary from this command.
#[tauri::command]
pub async fn list_skill_files(
    path: String,
) -> Result<Vec<crate::services::filesystem_service::DirEntry>, AppError> {
    let resolved = fs_service::resolve_home_relative(&path);
    tauri::async_runtime::spawn_blocking(move || fs_service::list_dir_entries(&resolved))
        .await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?
}

// ---------------------------------------------------------------------------
// Installation (plan 06)
// ---------------------------------------------------------------------------

/// Inspect a source (git URL or local folder) without installing: returns the
/// discovered skill(s) for the install dialog (§19 pre-flight check), plus the
/// handle that `install_skill` must present to copy from this very staging.
#[tauri::command]
pub async fn inspect_source(
    registry: tauri::State<'_, StagingRegistry>,
    source: crate::models::SkillSource,
) -> Result<StagedInspection, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let staged = crate::services::install_service::stage_source(&source)?;
        let inspection = staged.inspection().clone();
        Ok::<_, AppError>((staged, inspection))
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?;

    let (staged, inspection) = logged("inspect source", result)?;
    let staging_id = registry.park(staged);
    Ok(StagedInspection {
        staging_id,
        inspection,
    })
}

/// Install selected skill directories from a previously inspected source.
///
/// `replace` is false unless the user explicitly confirmed replacing an
/// occupied destination; when false a conflict changes nothing (Rule 14).
#[tauri::command]
pub async fn install_skill(
    registry: tauri::State<'_, StagingRegistry>,
    staging_id: String,
    source: crate::models::SkillSource,
    provider_id: crate::models::ProviderId,
    scope: crate::models::SkillScope,
    skill_dirs: Vec<String>,
    replace: bool,
) -> Result<Vec<crate::models::Skill>, AppError> {
    let staged = registry.claim(&staging_id).ok_or_else(|| {
        AppError::new(ErrorCode::SkillNotFound).with_detail(
            "that source is no longer staged — run Inspect again before installing",
        )
    });
    // Logged here as well as through `logged` below: a lookup miss returns
    // before reaching the inner call, and an unlogged failure is how this bug
    // stayed invisible in the log file.
    let staged = logged("claim staging", staged)?;

    logged(
        "install skill",
        tauri::async_runtime::spawn_blocking(move || {
            let provider = crate::providers::provider_by_id(provider_id)?;
            crate::services::install_service::install_skills(
                provider.as_ref(),
                &source,
                &staged,
                &skill_dirs,
                &scope,
                replace,
            )
        })
        .await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?,
    )
}

// ---------------------------------------------------------------------------
// Lifecycle (plan 07)
// ---------------------------------------------------------------------------

/// Enable/disable a skill by renaming its `SKILL.md`. Agent-agnostic and
/// lossless, so there is no provider that cannot be toggled (§20).
#[tauri::command]
pub async fn set_skill_enabled(
    provider_id: crate::models::ProviderId,
    skill_id: String,
    scope: crate::models::SkillScope,
    enabled: bool,
) -> Result<(), AppError> {
    logged(
        "toggle skill",
        tauri::async_runtime::spawn_blocking(move || {
            crate::services::lifecycle_service::set_skill_enabled(
                provider_id,
                &skill_id,
                &scope,
                enabled,
            )
        })
        .await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?,
    )
}

/// Remove a skill. Moves the directory to the OS trash unless `permanent`,
/// which the UI only sets after explicit confirmation (§21).
#[tauri::command]
pub async fn uninstall_skill(
    provider_id: crate::models::ProviderId,
    skill_id: String,
    scope: crate::models::SkillScope,
    permanent: bool,
) -> Result<(), AppError> {
    logged(
        "uninstall skill",
        tauri::async_runtime::spawn_blocking(move || {
            crate::services::lifecycle_service::uninstall_skill(
                provider_id,
                &skill_id,
                &scope,
                permanent,
            )
        })
        .await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?,
    )
}

// ---------------------------------------------------------------------------
// Projects (plan 05)
// ---------------------------------------------------------------------------

/// Native folder picker (Tauri dialog). `None` = user cancelled.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose a project folder")
        .pick_folder(move |path| {
            let _ = tx.send(path.map(|p| p.to_string()));
        });
    rx.await
        .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))
}

#[tauri::command]
pub async fn list_projects(
    registry: tauri::State<'_, crate::services::project_service::ProjectRegistry>,
) -> Result<Vec<crate::services::project_service::RegisteredProject>, AppError> {
    Ok(registry.list())
}

/// Add a project directory to the registry. Registry calls are cheap
/// (in-memory + small JSON write), so no spawn_blocking/State-lifetime dance.
#[tauri::command]
pub async fn add_project(
    registry: tauri::State<'_, crate::services::project_service::ProjectRegistry>,
    path: String,
) -> Result<crate::services::project_service::RegisteredProject, AppError> {
    registry.add(&path)
}

#[tauri::command]
pub async fn remove_project(
    registry: tauri::State<'_, crate::services::project_service::ProjectRegistry>,
    id: String,
) -> Result<(), AppError> {
    registry.remove(&id)
}

#[cfg(test)]
mod staging_tests {
    use super::*;
    use crate::models::SkillSource;

    fn staged() -> StagedSource {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("SKILL.md"),
            "---\nname: staged\n---\nbody",
        )
        .unwrap();
        crate::services::install_service::stage_source(&SkillSource::Local {
            path: tmp.path().to_string_lossy().into_owned(),
        })
        .unwrap()
    }

    /// The reported bug: after a CONFLICT the user confirmed "move aside and
    /// install", and that retry failed with "could not find that skill".
    ///
    /// Cause: the first (refused) attempt consumed the staging, so the retry
    /// had nothing to claim. Both attempts come from one inspection, so
    /// claiming must not consume it.
    #[test]
    fn staging_can_be_claimed_more_than_once() {
        let registry = StagingRegistry::default();
        let id = registry.park(staged());

        assert!(registry.claim(&id).is_some(), "first attempt (refused)");
        assert!(
            registry.claim(&id).is_some(),
            "retry after the user confirms the replace"
        );
    }

    /// A stale handle from an earlier inspection must not resolve to a newer
    /// staging, or install would copy something the user never reviewed.
    #[test]
    fn a_new_inspection_invalidates_the_previous_handle() {
        let registry = StagingRegistry::default();
        let old = registry.park(staged());
        let new = registry.park(staged());

        assert_ne!(old, new, "each inspection gets a fresh handle");
        assert!(registry.claim(&new).is_some());
        assert!(
            registry.claim(&old).is_none(),
            "the superseded staging is gone, so its handle cannot resolve"
        );
    }

    #[test]
    fn an_unknown_handle_does_not_resolve() {
        let registry = StagingRegistry::default();
        assert!(registry.claim("staging-999").is_none());
    }
}
