pub mod commands;
pub mod config;
pub mod logs;
pub mod maven;
pub mod process;
pub mod profiles;

use commands::AppState;
use config::ConfigManager;
use logs::LogManager;
use process::ProcessManager;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_manager = ConfigManager::new();
    let initial_services = config_manager.load_services();
    let log_manager = LogManager::new();
    let process_manager = ProcessManager::new(log_manager.clone());

    let app_state = AppState {
        config_manager,
        process_manager: process_manager.clone(),
        log_manager,
        services: Arc::new(Mutex::new(initial_services)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_services,
            commands::save_service,
            commands::delete_service,
            commands::pick_directory,
            commands::detect_maven_modules,
            commands::detect_profiles,
            commands::start_service,
            commands::stop_service,
            commands::restart_service,
            commands::get_service_status,
            commands::get_all_service_statuses,
            commands::get_service_logs,
            commands::clear_service_logs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                // 应用退出时清理所有受管理进程
                process_manager.cleanup_all();
            }
        });
}
