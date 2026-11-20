pub mod commands;
pub mod config;
mod embedded_terminal;
pub mod health;
pub mod logs;
pub mod maven;
pub mod maven_tasks;
pub mod metrics;
pub mod port;
pub mod process;
pub mod profiles;
pub mod terminal;

use commands::AppState;
use config::ConfigManager;
use logs::LogManager;
use maven_tasks::MavenTaskManager;
use process::{ProcessManager, ServiceStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::NotificationExt;

/// 首次隐藏到托盘时是否已弹过提示（每个进程生命周期只提示一次）
static TRAY_HINT_SHOWN: AtomicBool = AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_manager = ConfigManager::new();
    let initial_services = config_manager.load_services();
    let log_manager = LogManager::new();
    let process_manager = ProcessManager::new(log_manager.clone());
    let maven_task_manager = MavenTaskManager::new(log_manager.clone());

    let app_state = AppState {
        config_manager,
        process_manager: process_manager.clone(),
        maven_task_manager: maven_task_manager.clone(),
        log_manager,
        services: Arc::new(Mutex::new(initial_services)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // 单实例互斥：再次启动时唤起已有实例的窗口，而不是开出第二个进程/托盘
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(app_state)
        .manage(embedded_terminal::TerminalManager::default())
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
            commands::run_maven_task,
            commands::stop_maven_task,
            commands::get_maven_task_status,
            commands::detect_main_classes,
            commands::locate_built_jar,
            commands::open_service_terminal,
            commands::start_embedded_terminal,
            commands::write_embedded_terminal,
            commands::resize_embedded_terminal,
            commands::close_embedded_terminal,
            commands::get_service_health,
            commands::get_service_metrics,
            commands::export_services,
            commands::import_services,
            commands::clone_service,
            commands::open_url,
            commands::path_is_dir,
        ])
        .setup(|app| {
            // 系统托盘：常驻状态菜单
            let menu = build_tray_menu(app.handle())?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().expect("app icon required").clone())
                .tooltip("Local Service Runner — Spring Boot 服务运行管理器")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
                .build(app)?;

            // 服务状态变化 → 刷新托盘菜单 + 异常退出时发系统通知
            let app_for_status = app.handle().clone();
            app.listen_any("service-status-change", move |event| {
                on_status_changed(&app_for_status, event.payload());
            });

            // 配置增删改（保存/删除/克隆/导入）→ 刷新托盘菜单
            let app_for_config = app.handle().clone();
            app.listen_any("services-changed", move |_event| {
                refresh_tray(&app_for_config);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗驻留：关闭按钮隐藏到托盘，进程治理与日志继续在后台运行
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
                // 首次进托盘时轻提示一次，避免用户误以为程序已退出
                if !TRAY_HINT_SHOWN.swap(true, Ordering::SeqCst) {
                    let _ = window
                        .app_handle()
                        .notification()
                        .builder()
                        .title("Local Service Runner")
                        .body("窗口已最小化到系统托盘，服务仍在后台受管运行；右键托盘图标可退出应用。")
                        .show();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                // 应用退出时清理所有受管理进程与构建任务
                process_manager.cleanup_all();
                maven_task_manager.cleanup_all();
                app_handle.state::<embedded_terminal::TerminalManager>().close_all();
            }
        });
}

/// 托盘菜单中的服务状态标记字符。
fn tray_status_char(status: &ServiceStatus) -> &'static str {
    match status {
        ServiceStatus::Running { .. } => "●",
        ServiceStatus::Starting | ServiceStatus::Stopping => "◐",
        ServiceStatus::Building { .. } => "⚙",
        ServiceStatus::Failed { .. } => "✕",
        ServiceStatus::Stopped => "○",
    }
}

/// 构建托盘菜单：显示窗口 / 各服务（启动·停止） / 退出。
fn build_tray_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let services = state.services.lock().unwrap().clone();

    let mut builder = MenuBuilder::new(app).text("win-show", "显示主窗口").separator();

    for svc in &services {
        let status = state.process_manager.get_status(&svc.id);
        let label = format!("{}  {}", tray_status_char(&status), svc.name);
        let submenu = SubmenuBuilder::new(app, label)
            .text(format!("svc-start:{}", svc.id), "启动")
            .text(format!("svc-stop:{}", svc.id), "停止")
            .build()?;
        builder = builder.item(&submenu);
    }

    builder = builder.separator().text("app-exit", "退出应用");
    builder.build()
}

/// 托盘菜单整体重建（服务数与状态都不多，整棵重建最简单可靠）。
fn refresh_tray(app: &AppHandle) {
    let Ok(menu) = build_tray_menu(app) else {
        return;
    };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_menu(Some(menu));
    }
}

/// 托盘菜单事件分发。
fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "win-show" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "app-exit" => {
            app.exit(0);
        }
        _ => {
            if let Some(svc_id) = id.strip_prefix("svc-start:") {
                let state = app.state::<AppState>();
                let Some(svc) = state
                    .services
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|s| s.id == svc_id)
                    .cloned()
                else {
                    return;
                };
                // 正在跑 Maven 工具箱任务时不允许启动（与前端按钮一致）
                if state.maven_task_manager.is_running(svc_id) {
                    return;
                }
                let _ = state.process_manager.start_service(Some(app), &svc);
            } else if let Some(svc_id) = id.strip_prefix("svc-stop:") {
                let state = app.state::<AppState>();
                let timeout = state
                    .services
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|s| s.id == svc_id)
                    .map(|s| s.stop_timeout_secs)
                    .unwrap_or(5);
                let _ = state.process_manager.stop_service(Some(app), svc_id, timeout);
            }
        }
    }
}

/// 服务状态变化：刷新托盘 + 异常退出时发系统通知。
fn on_status_changed(app: &AppHandle, payload: &str) {
    refresh_tray(app);

    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    if value["status"]["type"].as_str() != Some("failed") {
        return;
    }
    let Some(service_id) = value["serviceId"].as_str() else {
        return;
    };

    let state = app.state::<AppState>();
    let name = state
        .services
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == service_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| service_id.to_string());
    let error = value["status"]["error"].as_str().unwrap_or_default();

    let body = if error.is_empty() {
        format!("服务「{}」异常退出", name)
    } else {
        format!("服务「{}」异常退出：{}", name, error)
    };
    let _ = app
        .notification()
        .builder()
        .title("Local Service Runner")
        .body(body)
        .show();
}
