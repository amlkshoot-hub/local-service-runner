use crate::config::{ConfigManager, ServiceConfig};
use crate::logs::{LogEntry, LogManager};
use crate::maven::{self, MainClassCandidate, MavenProjectInfo};
use crate::maven_tasks::{MavenTaskManager, MavenTaskStatus};
use crate::process::{ProcessManager, ServiceStatus};
use crate::profiles;
use crate::terminal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub config_manager: ConfigManager,
    pub process_manager: ProcessManager,
    pub log_manager: LogManager,
    pub maven_task_manager: MavenTaskManager,
    pub services: Arc<Mutex<Vec<ServiceConfig>>>,
}

#[tauri::command]
pub fn get_services(state: State<'_, AppState>) -> Vec<ServiceConfig> {
    let services = state.services.lock().unwrap();
    services.clone()
}

#[tauri::command]
pub fn save_service(
    app: AppHandle,
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
    drop(services);
    let _ = app.emit("services-changed", ());
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
    drop(services);
    state.log_manager.clear_logs(&id);
    let _ = app.emit("services-changed", ());
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

    // 服务启动与 Maven 工具箱任务互斥：避免构建替换 JAR 时与服务进程冲突
    if state.maven_task_manager.is_running(&id) {
        return Err("该服务的 Maven 构建任务正在进行中，请等待完成或先停止任务".to_string());
    }

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

    if state.maven_task_manager.is_running(&id) {
        return Err("该服务的 Maven 构建任务正在进行中，请等待完成或先停止任务".to_string());
    }

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

// ---------------- Maven 工具箱（clean / compile / package / install） ----------------

/// 对服务所在工程执行一次 Maven 构建（输出进该服务的日志视窗，可中断）。
#[tauri::command]
pub fn run_maven_task(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    goal: String,
) -> Result<(), String> {
    let services = state.services.lock().unwrap();
    let config = services
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Service not found".to_string())?
        .clone();
    drop(services);

    // 与服务运行状态互斥：Windows 下运行中的 JAR 会被进程锁定，mvn clean 会直接失败
    match state.process_manager.get_status(&id) {
        ServiceStatus::Stopped | ServiceStatus::Failed { .. } => {}
        _ => {
            return Err(
                "服务正在运行或启停中，无法执行 Maven 构建（运行中的 JAR 被进程锁定），请先停止服务".to_string(),
            );
        }
    }

    state
        .maven_task_manager
        .run_task(Some(&app), &id, &config.work_dir, &goal)
}

/// 中断正在执行的 Maven 构建任务（内核级终结 Maven 进程树）。
#[tauri::command]
pub fn stop_maven_task(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.maven_task_manager.stop_task(Some(&app), &id)
}

#[tauri::command]
pub fn get_maven_task_status(state: State<'_, AppState>, id: String) -> MavenTaskStatus {
    state.maven_task_manager.get_status(&id)
}

// ---------------- 源码主类探测 / 构建产物定位 ----------------

/// 扫描工程源码中的可执行主类（@SpringBootApplication 优先）。
#[tauri::command]
pub fn detect_main_classes(work_dir: String, module: String) -> Vec<MainClassCandidate> {
    let module = if module.trim().is_empty() {
        None
    } else {
        Some(module.as_str())
    };
    maven::detect_main_classes(&work_dir, module)
}

/// 定位工程（优先启动模块）target 下最新的可运行 JAR 产物，供前端展示与手动确认。
#[tauri::command]
pub fn locate_built_jar(work_dir: String, module: String) -> Result<Option<String>, String> {
    let module = if module.trim().is_empty() {
        None
    } else {
        Some(module.as_str())
    };
    maven::locate_built_jar(&work_dir, module)
}

// ---------------- 项目终端与链接 ----------------

/// 在浏览器中打开 http/https 链接（端口徽章一键直达 swagger/健康页）。
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("仅支持打开 http/https 链接".to_string());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// 判断路径是否为目录（拖拽建服务时校验项目文件夹）。
#[tauri::command]
pub fn path_is_dir(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

/// 在服务工作目录唤起系统终端（Windows Terminal 优先，cmd 回退）。
/// 终端独立于本应用生命周期：关闭 Runner 不会关闭终端。
#[tauri::command]
pub fn open_service_terminal(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let services = state.services.lock().unwrap();
    let work_dir = services
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.work_dir.clone())
        .ok_or_else(|| "Service not found".to_string())?;
    drop(services);

    terminal::open_terminal_at(&work_dir)
}

// ---------------- 健康检查与资源监控 ----------------

/// 查询服务的健康探针状态。
#[tauri::command]
pub fn get_service_health(state: State<'_, AppState>, id: String) -> crate::health::HealthState {
    state.process_manager.health_manager.get_state(&id)
}

/// 采样服务进程树的实时 CPU / 内存占用（服务运行中才有数据）。
#[tauri::command]
pub fn get_service_metrics(
    state: State<'_, AppState>,
    id: String,
) -> Option<crate::metrics::ServiceMetrics> {
    state.process_manager.get_metrics(&id)
}

// ---------------- 配置导出 / 导入 / 克隆 ----------------

/// 导出全部服务配置到 JSON 文件（换电脑 / 团队共享）。返回导出路径。
#[tauri::command]
pub async fn export_services(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let services = state.services.lock().unwrap().clone();
    let json =
        serde_json::to_string_pretty(&services).map_err(|e| format!("序列化配置失败: {}", e))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut builder = app
        .dialog()
        .file()
        .set_title("导出服务配置")
        .add_filter("JSON 配置文件", &["json"])
        .set_file_name("local-service-runner-services.json");

    if let Some(window) = app.get_webview_window("main") {
        builder = builder.set_parent(&window);
    }

    builder.save_file(move |picked| {
        let _ = tx.send(picked.map(|p| p.to_string()));
    });

    let Some(path) = rx.await.map_err(|_| "导出窗口异常关闭".to_string())? else {
        return Ok(None); // 用户取消
    };

    std::fs::write(&path, json).map_err(|e| format!("写入导出文件失败: {}", e))?;
    Ok(Some(path))
}

/// 从 JSON 文件导入服务配置：同 id 覆盖更新，新 id 追加。返回导入结果；取消返回 None。
#[tauri::command]
pub async fn import_services(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<crate::config::ImportSummary>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();

    let mut builder = app
        .dialog()
        .file()
        .set_title("导入服务配置")
        .add_filter("JSON 配置文件", &["json"]);

    if let Some(window) = app.get_webview_window("main") {
        builder = builder.set_parent(&window);
    }

    builder.pick_file(move |picked| {
        let _ = tx.send(picked.map(|p| p.to_string()));
    });

    let Some(path) = rx.await.map_err(|_| "导入窗口异常关闭".to_string())? else {
        return Ok(None); // 用户取消
    };

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("读取配置文件失败: {}", e))?;
    let incoming: Vec<ServiceConfig> =
        serde_json::from_str(&content).map_err(|e| format!("配置文件格式错误: {}", e))?;

    let mut services = state.services.lock().unwrap();
    let (merged, summary) = crate::config::merge_imported(services.clone(), incoming);
    state
        .config_manager
        .save_services(&merged)
        .map_err(|e| format!("保存配置失败: {}", e))?;
    *services = merged;
    drop(services);
    let _ = app.emit("services-changed", ());
    Ok(Some(summary))
}

/// 克隆一份服务配置（新 id + 「副本」后缀名），常用于起多实例调试。
#[tauri::command]
pub fn clone_service(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ServiceConfig, String> {
    let mut services = state.services.lock().unwrap();
    let source = services
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| "Service not found".to_string())?;

    let cloned = crate::config::clone_config(&source);
    services.push(cloned.clone());
    state.config_manager.save_services(&services)?;
    drop(services);
    let _ = app.emit("services-changed", ());
    Ok(cloned)
}
