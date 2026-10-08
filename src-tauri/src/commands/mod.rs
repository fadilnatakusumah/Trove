//! Tauri command layer.
//!
//! Thin, explicitly-typed IPC entry points. Commands validate input, delegate
//! to services, and return structured `AppError`s. No business logic here
//! (tech-plan §24).

use tauri_plugin_opener::OpenerExt;

use crate::errors::{AppError, ErrorCode};
use crate::services::filesystem_service as fs_service;
use crate::services::git_service;

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
            .with_detail(format!("'{}' is not a directory", resolved.display())));
    }
    app.opener()
        .open_path(resolved.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| {
            AppError::new(ErrorCode::PermissionDenied)
                .with_detail(format!("failed to open '{}': {e}", resolved.display()))
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
    tauri::async_runtime::spawn_blocking(move || {
        let provider = crate::providers::provider_by_id(provider_id)?;
        crate::providers::skills_for_scope(provider.as_ref(), &scope)
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?
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
