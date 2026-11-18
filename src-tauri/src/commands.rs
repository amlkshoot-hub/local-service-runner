use crate::config::{ConfigManager, ServiceConfig};
use crate::logs::{LogEntry, LogManager};
use crate::maven::{self, MavenProjectInfo};
use crate::process::{ProcessManager, ServiceStatus};
use crate::profiles;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    pub config_manager: ConfigManager,
    pub process_manager: ProcessManager,
    pub log_manager: LogManager,
    pub services: Arc<Mutex<Vec<ServiceConfig>>>,
}

#[tauri::command]
pub fn get_services(state: State<'_, AppState>) -> Vec<ServiceConfig> {
    let services = state.services.lock().unwrap();
    services.clone()
}

#[tauri::command]
pub fn save_service(
    _app: AppHandle,
    state: State<'_, AppState>,
    config: ServiceConfig,
) -> Result<(), String> {
    let mut services = state.services.lock().unwrap();
    if let Some(pos) = services.iter().position(|s| s.id == config.id) {
        services[pos] = config.clone();
    } else {
        services.push(config.clone());
    }
    state.config_manager.save_services(&services)?;
    Ok(())
}

#[tauri::command]
pub fn delete_service(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    // 如果正在运行，先停止
    let status = state.process_manager.get_status(&id);
    if let ServiceStatus::Running { .. } = status {
        state.process_manager.stop_service(Some(&app), &id, 5)?;
    }

    let mut services = state.services.lock().unwrap();
    services.retain(|s| s.id != id);
    state.config_manager.save_services(&services)?;
    state.log_manager.clear_logs(&id);
    Ok(())
}

/// 打开系统目录选择对话框（虚拟文件夹/资源管理器样式），返回所选目录。
#[tauri::command]
pub async fn pick_directory(
    app: AppHandle,
    title: String,
    initial_dir: String,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut builder = app.dialog().file().set_title(if title.trim().is_empty() {
        "选择项目目录".to_string()
    } else {
        title
    });

    let initial = initial_dir.trim();
    if !initial.is_empty() {
        let path = std::path::Path::new(initial);
        if path.is_dir() {
            builder = builder.set_directory(path);
        } else if let Some(parent) = path.parent().filter(|p| p.is_dir()) {
            builder = builder.set_directory(parent);
        }
    }

    if let Some(window) = app.get_webview_window("main") {
        builder = builder.set_parent(&window);
    }

    builder.pick_folder(move |picked| {
        let _ = tx.send(picked.map(|p| p.to_string()));
    });

    rx.await.map_err(|_| "目录选择窗口异常关闭".to_string())
}

#[tauri::command]
pub fn detect_maven_modules(work_dir: String) -> MavenProjectInfo {
    maven::inspect(&work_dir)
}

#[tauri::command]
pub fn detect_profiles(work_dir: String, module: String) -> Vec<String> {
    let module = if module.trim().is_empty() {
        None
    } else {
        Some(module.as_str())
    };
    profiles::detect_profiles(&work_dir, module)
}

#[tauri::command]
pub fn start_service(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let services = state.services.lock().unwrap();
    let config = services
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Service not found".to_string())?
        .clone();
    drop(services);

    state.process_manager.start_service(Some(&app), &config)
}

#[tauri::command]
pub fn stop_service(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let services = state.services.lock().unwrap();
    let timeout = services
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.stop_timeout_secs)
        .unwrap_or(10);
    drop(services);

    state.process_manager.stop_service(Some(&app), &id, timeout)
}

#[tauri::command]
pub fn restart_service(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let services = state.services.lock().unwrap();
    let config = services
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Service not found".to_string())?
        .clone();
    drop(services);

    state.process_manager.restart_service(Some(&app), &config)
}

#[tauri::command]
pub fn get_service_status(state: State<'_, AppState>, id: String) -> ServiceStatus {
    state.process_manager.get_status(&id)
}

#[tauri::command]
pub fn get_all_service_statuses(state: State<'_, AppState>) -> HashMap<String, ServiceStatus> {
    state.process_manager.get_all_statuses()
}

#[tauri::command]
pub fn get_service_logs(state: State<'_, AppState>, id: String) -> Vec<LogEntry> {
    state.log_manager.get_logs(&id)
}

#[tauri::command]
pub fn clear_service_logs(state: State<'_, AppState>, id: String) {
    state.log_manager.clear_logs(&id);
}
