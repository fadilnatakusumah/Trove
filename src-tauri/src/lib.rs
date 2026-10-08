pub mod commands;
pub mod errors;
pub mod models;
pub mod providers;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .setup(|app| {
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
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
