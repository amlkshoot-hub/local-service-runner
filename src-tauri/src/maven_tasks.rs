//! Maven 工具箱任务管理。
//!
//! 面向单个服务执行一次性的 `mvn clean / compile / package / install` 构建，
//! 输出写入该服务的日志视窗；任务独立于服务运行状态机，可随时中断。
//! 与服务运行共用 `spawn_managed_process` 底层，因此同样受 Job Object 约束：
//! 停止任务时内核级终结整个 Maven 进程树（mvn.cmd → cmd.exe → java 编译器），
//! 不会残留任何孤儿进程；Runner 崩溃时系统也会自动回收。

use crate::config::MavenGoal;
use crate::logs::LogManager;
use crate::process::ProcessManager;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::TerminateJobObject;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::WaitForSingleObject;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MavenTaskStatus {
    Idle,
    Running {
        pid: u32,
        goal: String,
    },
    Success {
        goal: String,
        #[serde(rename = "exitCode")]
        exit_code: i32,
    },
    Failed {
        goal: String,
        #[serde(rename = "exitCode")]
        exit_code: i32,
    },
}

#[allow(dead_code)]
struct TaskHandle {
    job_handle: usize,
    process_handle: usize,
    pid: u32,
    goal: String,
}

#[derive(Clone)]
pub struct MavenTaskManager {
    statuses: Arc<Mutex<HashMap<String, MavenTaskStatus>>>,
    tasks: Arc<Mutex<HashMap<String, TaskHandle>>>,
    log_manager: LogManager,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TaskStatusEvent {
    service_id: String,
    status: MavenTaskStatus,
}

const TASK_EVENT: &str = "maven-task-status-change";

impl MavenTaskManager {
    pub fn new(log_manager: LogManager) -> Self {
        Self {
            statuses: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            log_manager,
        }
    }

    pub fn get_status(&self, service_id: &str) -> MavenTaskStatus {
        self.statuses
            .lock()
            .unwrap()
            .get(service_id)
            .cloned()
            .unwrap_or(MavenTaskStatus::Idle)
    }

    fn set_status(&self, app: Option<&AppHandle>, service_id: &str, status: MavenTaskStatus) {
        {
            let mut map = self.statuses.lock().unwrap();
            map.insert(service_id.to_string(), status.clone());
        }
        if let Some(app) = app {
            let _ = app.emit(
                TASK_EVENT,
                TaskStatusEvent {
                    service_id: service_id.to_string(),
                    status,
                },
            );
        }
    }

    /// 是否有构建任务正在进行（供服务启动前互斥校验）。
    pub fn is_running(&self, service_id: &str) -> bool {
        matches!(self.get_status(service_id), MavenTaskStatus::Running { .. })
    }

    /// 启动一次 Maven 构建任务。goal 由 `MavenGoal::parse` 白名单校验。
    ///
    /// 构建始终在工程根目录以全工程方式执行（不带 `-pl`），
    /// 与命令行里手动敲 `mvn package` 的行为保持一致。
    pub fn run_task(
        &self,
        app_handle: Option<&AppHandle>,
        service_id: &str,
        work_dir: &str,
        goal: &str,
    ) -> Result<(), String> {
        let goal = MavenGoal::parse(goal)
            .ok_or_else(|| format!("不支持的 Maven 目标: {}（仅支持 clean / compile / package / install）", goal))?
            .as_str()
            .to_string();

        if self.is_running(service_id) {
            return Err("该服务已有 Maven 构建任务正在进行中".to_string());
        }

        let work_path = std::path::Path::new(work_dir);
        if !work_path.is_dir() {
            return Err(format!("工作目录不存在: {}", work_dir));
        }

        let maven_cmd = maven_cmd_path(work_path);

        let log_manager = self.log_manager.clone();
        self.log_manager.append_log(
            app_handle,
            service_id,
            format!("=== Maven 任务开始: mvn {} ===", goal),
            "system",
        );
        self.log_manager.append_log(
            app_handle,
            service_id,
            format!("Command: cmd.exe /c {} {}", maven_cmd, goal),
            "system",
        );

        let args = vec![goal.clone()];

        // 退出回调：释放句柄并迁移任务终态
        let statuses = Arc::clone(&self.statuses);
        let tasks = Arc::clone(&self.tasks);
        let log_mgr_cb = log_manager.clone();
        let app_for_cb = app_handle.cloned();
        let service_id_cb = service_id.to_string();
        let goal_cb = goal.clone();

        let on_exit = Box::new(move |code_opt: Option<i32>| {
            // 归属判断：若任务已被 stop_task 移除，说明停止流程接管了句柄，这里跳过
            let removed = {
                let mut map = tasks.lock().unwrap();
                map.remove(&service_id_cb)
                    .map(|t| (t.job_handle as HANDLE, t.process_handle as HANDLE, t.pid))
            };

            if let Some((h_job, h_proc, _pid)) = removed {
                unsafe {
                    CloseHandle(h_job);
                    CloseHandle(h_proc);
                }
            } else {
                return; // 已被 stop 流程接管
            }

            let (next_status, msg) = match code_opt {
                Some(code) if code == 0 => (
                    MavenTaskStatus::Success {
                        goal: goal_cb.clone(),
                        exit_code: 0,
                    },
                    format!("=== Maven {} 完成 ===", goal_cb),
                ),
                Some(code) => (
                    MavenTaskStatus::Failed {
                        goal: goal_cb.clone(),
                        exit_code: code,
                    },
                    format!("=== Maven {} 失败，退出码: {} ===", goal_cb, code),
                ),
                None => (
                    MavenTaskStatus::Failed {
                        goal: goal_cb.clone(),
                        exit_code: -1,
                    },
                    format!("=== Maven {} 进程退出，未能获取退出码 ===", goal_cb),
                ),
            };

            log_mgr_cb.append_log(app_for_cb.as_ref(), &service_id_cb, msg, "system");

            {
                let mut map = statuses.lock().unwrap();
                map.insert(service_id_cb.clone(), next_status.clone());
            }
            if let Some(app) = app_for_cb.as_ref() {
                let _ = app.emit(
                    TASK_EVENT,
                    TaskStatusEvent {
                        service_id: service_id_cb,
                        status: next_status,
                    },
                );
            }
        });

        let handles = ProcessManager::spawn_managed_process(
            app_handle,
            &log_manager,
            service_id,
            work_dir,
            "cmd.exe",
            &{
                let mut wrapped = vec!["/c".to_string(), maven_cmd];
                wrapped.extend(args.iter().cloned());
                wrapped
            },
            on_exit,
        )?;

        {
            let mut map = self.tasks.lock().unwrap();
            map.insert(
                service_id.to_string(),
                TaskHandle {
                    job_handle: handles.job_handle,
                    process_handle: handles.process_handle,
                    pid: handles.pid,
                    goal: goal.clone(),
                },
            );
        }

        self.set_status(
            app_handle,
            service_id,
            MavenTaskStatus::Running {
                pid: handles.pid,
                goal,
            },
        );

        Ok(())
    }

    /// 中断正在进行的构建任务（内核级终结整个进程树）。
    pub fn stop_task(&self, app_handle: Option<&AppHandle>, service_id: &str) -> Result<(), String> {
        let removed = {
            let mut map = self.tasks.lock().unwrap();
            map.remove(service_id)
        };

        let Some(task) = removed else {
            return Ok(());
        };

        #[cfg(windows)]
        unsafe {
            TerminateJobObject(task.job_handle as HANDLE, 1);
            WaitForSingleObject(task.process_handle as HANDLE, 5000);
            CloseHandle(task.job_handle as HANDLE);
            CloseHandle(task.process_handle as HANDLE);
        }

        self.log_manager.append_log(
            app_handle,
            service_id,
            "=== Maven 任务已手动停止 ===".to_string(),
            "system",
        );
        self.set_status(app_handle, service_id, MavenTaskStatus::Idle);

        Ok(())
    }

    /// 应用退出时清理所有构建任务进程。
    /// 即使不调用，Job Object 的 KILL_ON_JOB_CLOSE 也会在句柄关闭后由内核回收。
    pub fn cleanup_all(&self) {
        let drained: Vec<(String, TaskHandle)> = {
            let mut map = self.tasks.lock().unwrap();
            map.drain().collect()
        };

        #[cfg(windows)]
        for (_id, task) in drained {
            unsafe {
                TerminateJobObject(task.job_handle as HANDLE, 1);
                CloseHandle(task.job_handle as HANDLE);
                CloseHandle(task.process_handle as HANDLE);
            }
        }
    }
}

fn maven_cmd_path(work_path: &std::path::Path) -> String {
    let mvnw_local = work_path.join("mvnw.cmd");
    if mvnw_local.exists() {
        mvnw_local.to_string_lossy().to_string()
    } else {
        "mvn.cmd".to_string()
    }
}
