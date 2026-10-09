pub mod commands;
pub mod errors;
pub mod models;
pub mod providers;
pub mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      let app_data_dir = app
        .path()
        .app_data_dir()
        .expect("app data dir unavailable");
      app.manage(crate::services::project_service::ProjectRegistry::new(
        app_data_dir,
      ));
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
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
      commands::pick_folder,
      commands::list_projects,
      commands::add_project,
      commands::remove_project,
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
