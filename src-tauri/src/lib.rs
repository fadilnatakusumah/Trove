pub mod commands;
pub mod errors;
pub mod models;
pub mod providers;
pub mod services;

use tauri::Manager;

/// Log file name (the LogDir target appends the platform's log directory).
const LOG_FILE_NAME: &str = "agent-manager";

/// Rotate at ~1 MB so the file can't grow without bound (§31).
const LOG_MAX_FILE_BYTES: u128 = 1_000_000;

/// Logging is configured for release builds too, not just debug: a packaged
/// app is exactly where a user needs logs they can send back (§31).
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
  tauri_plugin_log::Builder::default()
    .level(log::LevelFilter::Info)
    .max_file_size(LOG_MAX_FILE_BYTES)
    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
    .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
    .targets([
      // Stdout keeps `npm run tauri dev` useful; LogDir is what the user can
      // actually find afterwards via Settings → Open Logs.
      tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
      tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
        file_name: Some(LOG_FILE_NAME.to_string()),
      }),
    ])
    .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(log_plugin())
    .setup(|app| {
      let app_data_dir = app
        .path()
        .app_data_dir()
        .expect("app data dir unavailable");
      app.manage(crate::services::project_service::ProjectRegistry::new(
        app_data_dir,
      ));
      app.manage(commands::StagingRegistry::default());
      log::info!("agent-manager {} started", env!("CARGO_PKG_VERSION"));
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::open_folder,
      commands::check_git,
      commands::detect_providers,
      commands::get_skills,
      commands::list_skill_files,
      commands::inspect_source,
      commands::install_skill,
      commands::set_skill_enabled,
      commands::uninstall_skill,
      commands::open_logs,
      commands::pick_folder,
      commands::list_projects,
      commands::add_project,
      commands::remove_project,
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
