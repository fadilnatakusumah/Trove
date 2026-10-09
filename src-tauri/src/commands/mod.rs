//! Tauri command layer.
//!
//! Thin, explicitly-typed IPC entry points. Commands validate input, delegate
//! to services, and return structured `AppError`s. No business logic here
//! (tech-plan §24).

use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::errors::{AppError, ErrorCode};
use crate::services::filesystem_service as fs_service;
use crate::services::git_service;

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
            dir.display()
        )));
    }
    let shown = dir.to_string_lossy().into_owned();
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
            .with_detail(format!("'{}' is not a directory", resolved.display())));
    }
    log::info!("opening folder {}", resolved.display());
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
/// discovered skill(s) for the install dialog (§19 pre-flight check).
#[tauri::command]
pub async fn inspect_source(
    source: crate::models::SkillSource,
) -> Result<crate::services::install_service::SourceInspection, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let staged = crate::services::install_service::stage_source(&source)?;
        Ok(staged.inspection().clone())
    })
    .await
    .map_err(|e| AppError::new(ErrorCode::InstallFailed).with_detail(e.to_string()))?
}

/// Install selected skill directories from a staged source.
///
/// `replace` is false unless the user explicitly confirmed replacing an
/// occupied destination; when false a conflict changes nothing (Rule 14).
#[tauri::command]
pub async fn install_skill(
    source: crate::models::SkillSource,
    provider_id: crate::models::ProviderId,
    scope: crate::models::SkillScope,
    skill_dirs: Vec<String>,
    replace: bool,
) -> Result<Vec<crate::models::Skill>, AppError> {
    logged(
        "install skill",
        tauri::async_runtime::spawn_blocking(move || {
            let provider = crate::providers::provider_by_id(provider_id)?;
            let staged = crate::services::install_service::stage_source(&source)?;
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
