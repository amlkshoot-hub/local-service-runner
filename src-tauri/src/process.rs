use crate::config::{LaunchType, ServiceConfig};
use crate::logs::LogManager;
use crate::maven;
use crate::profiles::build_profile_args;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::windows::io::FromRawHandle;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, FALSE, HANDLE, TRUE,
};
#[cfg(windows)]
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
#[cfg(windows)]
use windows_sys::Win32::System::Pipes::CreatePipe;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, GetCurrentProcess, GetExitCodeProcess,
    PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOW, WaitForSingleObject,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running {
        pid: u32,
        // 注意：枚举上的 rename_all 只作用于变体名，不会重命名变体内的字段，
        // 必须显式指定，否则前端拿到的会是 active_profiles / started_at。
        #[serde(rename = "activeProfiles")]
        active_profiles: Vec<String>,
        #[serde(rename = "startedAt")]
        started_at: String,
    },
    Stopping,
    Failed {
        #[serde(rename = "exitCode")]
        exit_code: Option<i32>,
        error: String,
    },
}

#[allow(dead_code)]
struct ActiveInstance {
    job_handle: usize,
    process_handle: usize,
    pid: u32,
    active_profiles: Vec<String>,
}

#[derive(Clone)]
pub struct ProcessManager {
    statuses: Arc<Mutex<HashMap<String, ServiceStatus>>>,
    instances: Arc<Mutex<HashMap<String, ActiveInstance>>>,
    log_manager: LogManager,
}

impl ProcessManager {
    pub fn new(log_manager: LogManager) -> Self {
        Self {
            statuses: Arc::new(Mutex::new(HashMap::new())),
            instances: Arc::new(Mutex::new(HashMap::new())),
            log_manager,
        }
    }

    pub fn get_status(&self, service_id: &str) -> ServiceStatus {
        let statuses = self.statuses.lock().unwrap();
        statuses
            .get(service_id)
            .cloned()
            .unwrap_or(ServiceStatus::Stopped)
    }

    pub fn get_all_statuses(&self) -> HashMap<String, ServiceStatus> {
        let statuses = self.statuses.lock().unwrap();
        statuses.clone()
    }

    fn set_status(&self, app_handle: Option<&AppHandle>, service_id: &str, status: ServiceStatus) {
        {
            let mut map = self.statuses.lock().unwrap();
            map.insert(service_id.to_string(), status.clone());
        }
        if let Some(app) = app_handle {
            #[derive(Serialize, Clone)]
            #[serde(rename_all = "camelCase")]
            struct StatusChangeEvent {
                service_id: String,
                status: ServiceStatus,
            }
            let _ = app.emit(
                "service-status-change",
                StatusChangeEvent {
                    service_id: service_id.to_string(),
                    status,
                },
            );
        }
    }

    /// 启动服务
    pub fn start_service(
        &self,
        app_handle: Option<&AppHandle>,
        config: &ServiceConfig,
    ) -> Result<(), String> {
        // 1. 防重入与状态校验
        {
            let current = self.get_status(&config.id);
            match current {
                ServiceStatus::Running { .. } | ServiceStatus::Starting => {
                    return Err("Service is already running or starting".to_string());
                }
                ServiceStatus::Stopping => {
                    return Err("Service is currently stopping, please wait".to_string());
                }
                _ => {}
            }
        }

        self.set_status(app_handle, &config.id, ServiceStatus::Starting);
        self.log_manager.append_log(
            app_handle,
            &config.id,
            format!("=== Starting service: {} ===", config.name),
            "system",
        );

        // 2. 校验工作目录
        let work_path = Path::new(&config.work_dir);
        if !work_path.exists() {
            let err = format!("Work directory does not exist: {}", config.work_dir);
            self.set_status(
                app_handle,
                &config.id,
                ServiceStatus::Failed {
                    exit_code: None,
                    error: err.clone(),
                },
            );
            return Err(err);
        }

        // 3. 构建 Profile 参数与完整启动参数
        let profile_args = build_profile_args(
            &config.launch_type,
            &config.selected_profiles,
            &config.args,
            config.inject_profile_custom,
        );

        let active_profiles_for_record = config
            .selected_profiles
            .iter()
            .filter(|p| !p.trim().is_empty() && !p.eq_ignore_ascii_case("default"))
            .cloned()
            .collect::<Vec<_>>();

        let built = match build_command(config, &profile_args) {
            Ok(res) => res,
            Err(e) => {
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: e.clone(),
                    },
                );
                return Err(e);
            }
        };

        for note in &built.notes {
            self.log_manager
                .append_log(app_handle, &config.id, note.clone(), "system");
        }

        self.log_manager.append_log(
            app_handle,
            &config.id,
            format!("Command: {} {}", built.executable, built.args.join(" ")),
            "system",
        );

        #[cfg(windows)]
        {
            self.spawn_windows_process(
                app_handle,
                config,
                &built.executable,
                &built.args,
                active_profiles_for_record,
            )
        }

        #[cfg(not(windows))]
        {
            Err("Only Windows is supported".to_string())
        }
    }

    #[cfg(windows)]
    fn spawn_windows_process(
        &self,
        app_handle: Option<&AppHandle>,
        config: &ServiceConfig,
        executable: &str,
        args: &[String],
        active_profiles: Vec<String>,
    ) -> Result<(), String> {
        unsafe {
            // 1. 创建 Job Object
            let h_job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h_job.is_null() {
                let err = format!(
                    "Failed to create Job Object, Win32 error={}",
                    std::io::Error::last_os_error()
                );
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: err.clone(),
                    },
                );
                return Err(err);
            }

            // 2. 设置 JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let info_size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;

            let set_res = SetInformationJobObject(
                h_job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                info_size,
            );
            if set_res == FALSE {
                let err = format!(
                    "Failed to set Job Object information: {}",
                    std::io::Error::last_os_error()
                );
                CloseHandle(h_job);
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: err.clone(),
                    },
                );
                return Err(err);
            }

            // 3. 创建 stdout/stderr 重定向管道
            let mut sa: SECURITY_ATTRIBUTES = std::mem::zeroed();
            sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
            sa.bInheritHandle = TRUE;

            let mut h_read_pipe: HANDLE = std::ptr::null_mut();
            let mut h_write_pipe: HANDLE = std::ptr::null_mut();

            if CreatePipe(&mut h_read_pipe, &mut h_write_pipe, &sa, 0) == FALSE {
                let err = format!(
                    "Failed to create stdout pipe: {}",
                    std::io::Error::last_os_error()
                );
                CloseHandle(h_job);
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: err.clone(),
                    },
                );
                return Err(err);
            }

            // 4. 构建结构化 Windows 命令行
            let full_cmd_line = build_windows_command_line(executable, args);
            let mut cmd_utf16: Vec<u16> = OsStr::new(&full_cmd_line)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let work_dir_utf16: Vec<u16> = OsStr::new(&config.work_dir)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut si: STARTUPINFOW = std::mem::zeroed();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            si.dwFlags = STARTF_USESTDHANDLES;
            si.hStdOutput = h_write_pipe;
            si.hStdError = h_write_pipe; // 合并捕获 stderr

            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

            // 5. 挂起创建进程 CREATE_SUSPENDED，彻底杜绝子进程竞态逃逸！
            let create_res = CreateProcessW(
                std::ptr::null(),
                cmd_utf16.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                TRUE, // inherit handles
                CREATE_SUSPENDED | CREATE_NO_WINDOW,
                std::ptr::null(), // environment
                work_dir_utf16.as_ptr(),
                &si,
                &mut pi,
            );

            // 立即关闭写端，使得读端在写端全部退出后能读到 EOF
            CloseHandle(h_write_pipe);

            if create_res == FALSE {
                let err = format!(
                    "Failed to CreateProcess: {}",
                    std::io::Error::last_os_error()
                );
                CloseHandle(h_read_pipe);
                CloseHandle(h_job);
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: err.clone(),
                    },
                );
                return Err(err);
            }

            // 6. 关键步骤：在恢复主线程前将根进程加入 Job Object！
            let assign_res = AssignProcessToJobObject(h_job, pi.hProcess);
            if assign_res == FALSE {
                let err = format!(
                    "Failed to assign process to Job Object: {}",
                    std::io::Error::last_os_error()
                );
                ResumeThread(pi.hThread);
                CloseHandle(pi.hProcess);
                CloseHandle(pi.hThread);
                CloseHandle(h_read_pipe);
                CloseHandle(h_job);
                self.set_status(
                    app_handle,
                    &config.id,
                    ServiceStatus::Failed {
                        exit_code: None,
                        error: err.clone(),
                    },
                );
                return Err(err);
            }

            // 7. 恢复线程执行
            ResumeThread(pi.hThread);
            CloseHandle(pi.hThread);

            let pid = pi.dwProcessId;
            let h_process = pi.hProcess;

            // 8. 记录活跃实例与状态
            {
                let mut instances = self.instances.lock().unwrap();
                instances.insert(
                    config.id.clone(),
                    ActiveInstance {
                        job_handle: h_job as usize,
                        process_handle: h_process as usize,
                        pid,
                        active_profiles: active_profiles.clone(),
                    },
                );
            }

            let started_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            self.set_status(
                app_handle,
                &config.id,
                ServiceStatus::Running {
                    pid,
                    active_profiles,
                    started_at,
                },
            );

            // 9. 开启异步线程读取管道输出
            let log_mgr = self.log_manager.clone();
            let service_id_for_log = config.id.clone();
            let app_for_log = app_handle.cloned();
            let read_pipe_raw = h_read_pipe as usize;

            thread::spawn(move || {
                let file = File::from_raw_handle(read_pipe_raw as _);
                let mut reader = BufReader::new(file);
                let mut buffer = Vec::new();

                loop {
                    buffer.clear();
                    match reader.read_until(b'\n', &mut buffer) {
                        Ok(0) => break,
                        Ok(_) => {
                            let clean_line = decode_console_bytes(&buffer);
                            if !clean_line.is_empty() {
                                log_mgr.append_log(
                                    app_for_log.as_ref(),
                                    &service_id_for_log,
                                    clean_line,
                                    "stdout",
                                );
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // 10. 监听根进程生命周期
            let self_clone = self.clone();
            let service_id_for_watch = config.id.clone();
            let app_for_watch = app_handle.cloned();
            let proc_raw = h_process as usize;
            let watched_pid = pid;

            thread::spawn(move || {
                let h_proc = proc_raw as HANDLE;
                WaitForSingleObject(h_proc, u32::MAX);

                let mut exit_code: u32 = 0;
                let got_code = GetExitCodeProcess(h_proc, &mut exit_code);

                // 清理实例（只清理本次启动创建的实例，避免误删重启后的新实例）
                let (h_job_to_close, h_proc_to_close) = {
                    let mut instances = self_clone.instances.lock().unwrap();
                    let belongs_to_this_run = instances
                        .get(&service_id_for_watch)
                        .map(|inst| inst.pid == watched_pid)
                        .unwrap_or(false);
                    if belongs_to_this_run {
                        instances
                            .remove(&service_id_for_watch)
                            .map(|inst| (inst.job_handle as HANDLE, inst.process_handle as HANDLE))
                            .unwrap_or((std::ptr::null_mut(), std::ptr::null_mut()))
                    } else {
                        (std::ptr::null_mut(), std::ptr::null_mut())
                    }
                };

                if !h_job_to_close.is_null() {
                    CloseHandle(h_job_to_close);
                }
                if !h_proc_to_close.is_null() {
                    CloseHandle(h_proc_to_close);
                }

                let code_opt = if got_code != FALSE {
                    Some(exit_code as i32)
                } else {
                    None
                };

                // 判断是否是正常停止或异常退出
                let current_status = self_clone.get_status(&service_id_for_watch);
                if current_status == ServiceStatus::Stopping
                    || current_status == ServiceStatus::Stopped
                {
                    return;
                }

                let msg = format!(
                    "=== Service exited with code: {} ===",
                    code_opt
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                );
                self_clone.log_manager.append_log(
                    app_for_watch.as_ref(),
                    &service_id_for_watch,
                    msg,
                    "system",
                );

                let next_status = match code_opt {
                    Some(code) if code != 0 => ServiceStatus::Failed {
                        exit_code: Some(code),
                        error: format!("进程退出，退出码: {}", code),
                    },
                    _ => ServiceStatus::Stopped,
                };
                self_clone.set_status(app_for_watch.as_ref(), &service_id_for_watch, next_status);
            });

            Ok(())
        }
    }

    /// 停止服务
    pub fn stop_service(
        &self,
        app_handle: Option<&AppHandle>,
        service_id: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        // 先复制一份句柄：监听线程会在进程退出时关闭原始句柄，
        // 停止流程必须持有自己的副本，避免并发关闭 / 句柄复用造成的误关闭。
        #[cfg(windows)]
        let (h_job, h_process) = {
            let originals = {
                let instances = self.instances.lock().unwrap();
                instances
                    .get(service_id)
                    .map(|inst| (inst.job_handle as HANDLE, inst.process_handle as HANDLE))
            };
            let Some((job, process)) = originals else {
                self.set_status(app_handle, service_id, ServiceStatus::Stopped);
                return Ok(());
            };
            unsafe { (duplicate_handle(job), duplicate_handle(process)) }
        };

        self.set_status(app_handle, service_id, ServiceStatus::Stopping);
        self.log_manager.append_log(
            app_handle,
            service_id,
            "=== Stopping service... ===".to_string(),
            "system",
        );

        #[cfg(windows)]
        unsafe {
            let (h_job, h_process) = match (h_job, h_process) {
                (Some(job), Some(process)) => (job, process),
                (job, process) => {
                    if let Some(job) = job {
                        CloseHandle(job);
                    }
                    if let Some(process) = process {
                        CloseHandle(process);
                    }
                    self.set_status(app_handle, service_id, ServiceStatus::Stopped);
                    self.log_manager.append_log(
                        app_handle,
                        service_id,
                        "=== Failed to duplicate process handles ===".to_string(),
                        "system",
                    );
                    return Err("Failed to duplicate process handles".to_string());
                }
            };

            // 第一阶段：等待或尝试正常退出（可配置超时）
            let wait_res = WaitForSingleObject(h_process, (timeout_secs * 1000) as u32);
            if wait_res == 0 {
                // 已经在超时内自行正常退出
                CloseHandle(h_job);
                CloseHandle(h_process);
                self.set_status(app_handle, service_id, ServiceStatus::Stopped);
                self.log_manager.append_log(
                    app_handle,
                    service_id,
                    "=== Service stopped gracefully ===".to_string(),
                    "system",
                );
                return Ok(());
            }

            // 第二阶段：超时强制终止 Job Object 及其树下所有受管理子进程
            self.log_manager.append_log(
                app_handle,
                service_id,
                "=== Timeout reached. Terminating managed process tree via Job Object ==="
                    .to_string(),
                "system",
            );

            let term_ok = TerminateJobObject(h_job, 1);
            if term_ok == FALSE {
                let err = format!(
                    "Failed to terminate Job Object: {}",
                    std::io::Error::last_os_error()
                );
                self.log_manager.append_log(
                    app_handle,
                    service_id,
                    format!("Error terminating: {}", err),
                    "system",
                );
            }

            // 等待根进程句柄真正退出（最多 5 秒）
            WaitForSingleObject(h_process, 5000);

            // 移除实例；若监听线程已经回收，则不再重复关闭原始句柄
            let removed = {
                let mut instances = self.instances.lock().unwrap();
                instances.remove(service_id)
            };
            if let Some(inst) = removed {
                CloseHandle(inst.job_handle as HANDLE);
                CloseHandle(inst.process_handle as HANDLE);
            }

            CloseHandle(h_job);
            CloseHandle(h_process);

            self.set_status(app_handle, service_id, ServiceStatus::Stopped);
            self.log_manager.append_log(
                app_handle,
                service_id,
                "=== Service stopped (all managed processes terminated) ===".to_string(),
                "system",
            );

            Ok(())
        }

        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    /// 重启服务：严格保证先停后启
    pub fn restart_service(
        &self,
        app_handle: Option<&AppHandle>,
        config: &ServiceConfig,
    ) -> Result<(), String> {
        let current = self.get_status(&config.id);
        if let ServiceStatus::Running { .. } = current {
            self.stop_service(app_handle, &config.id, config.stop_timeout_secs)?;
        }

        // 确保实例完全移出后再启动
        let start_wait = Instant::now();
        while start_wait.elapsed() < Duration::from_secs(10) {
            if self.get_status(&config.id) == ServiceStatus::Stopped {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        self.start_service(app_handle, config)
    }

    /// 应用关闭时清理所有受管理进程
    pub fn cleanup_all(&self) {
        #[cfg(windows)]
        unsafe {
            let mut instances = self.instances.lock().unwrap();
            for (_id, inst) in instances.drain() {
                let h_job = inst.job_handle as HANDLE;
                let h_process = inst.process_handle as HANDLE;
                TerminateJobObject(h_job, 1);
                CloseHandle(h_job);
                CloseHandle(h_process);
            }
        }
    }
}

#[derive(Debug)]
struct BuiltCommand {
    executable: String,
    args: Vec<String>,
    /// 启动时写入控制台的说明（例如自动选择的 Maven 模块）
    notes: Vec<String>,
}

fn build_command(config: &ServiceConfig, profile_args: &[String]) -> Result<BuiltCommand, String> {
    let mut notes = Vec::new();

    match config.launch_type {
        LaunchType::Jar => {
            let java_cmd = config
                .java_path
                .as_ref()
                .filter(|p| !p.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| "java.exe".to_string());

            let jar = config
                .jar_path
                .as_ref()
                .filter(|p| !p.trim().is_empty())
                .ok_or_else(|| "JAR file path is required".to_string())?;

            let mut all_args = vec!["-jar".to_string(), jar.clone()];
            all_args.extend(profile_args.iter().cloned());
            all_args.extend(config.args.iter().cloned());

            Ok(BuiltCommand {
                executable: java_cmd,
                args: all_args,
                notes,
            })
        }
        LaunchType::MavenWrapper => {
            // 关键修复：`spring-boot` 前缀只在「当前工程」的 build/plugins 中解析。
            // 若工作目录是聚合根（多模块项目），必须通过 -pl 指定真正声明了
            // spring-boot-maven-plugin 的子模块，否则会报
            // "No plugin found for prefix 'spring-boot'"。
            let plan = maven::plan_module(&config.work_dir, config.maven_module.as_deref())?;
            if let Some(note) = plan.note {
                notes.push(note);
            }

            // 在 Windows 下检查项目目录下是否存在 mvnw.cmd
            let work_path = Path::new(&config.work_dir);
            let mvnw_local = work_path.join("mvnw.cmd");
            let cmd_path = if mvnw_local.exists() {
                mvnw_local.to_string_lossy().to_string()
            } else {
                "mvn.cmd".to_string()
            };

            let mut maven_args = Vec::new();
            if let Some(module) = plan.module {
                maven_args.push("-pl".to_string());
                maven_args.push(module);
            }
            maven_args.push("spring-boot:run".to_string());
            maven_args.extend(profile_args.iter().cloned());
            maven_args.extend(config.args.iter().cloned());

            let mut wrapped = vec!["/c".to_string(), cmd_path];
            wrapped.extend(maven_args);

            Ok(BuiltCommand {
                executable: "cmd.exe".to_string(),
                args: wrapped,
                notes,
            })
        }
        LaunchType::GradleWrapper => {
            let work_path = Path::new(&config.work_dir);
            let gradlew_local = work_path.join("gradlew.bat");
            let cmd_path = if gradlew_local.exists() {
                gradlew_local.to_string_lossy().to_string()
            } else {
                "gradle.bat".to_string()
            };

            let mut all_args = vec!["bootRun".to_string()];
            all_args.extend(profile_args.iter().cloned());
            all_args.extend(config.args.iter().cloned());

            let mut wrapped = vec!["/c".to_string(), cmd_path];
            wrapped.extend(all_args);

            Ok(BuiltCommand {
                executable: "cmd.exe".to_string(),
                args: wrapped,
                notes,
            })
        }
        LaunchType::Custom => {
            let program = config
                .custom_program
                .as_ref()
                .filter(|p| !p.trim().is_empty())
                .ok_or_else(|| "Custom program executable is required".to_string())?;

            let mut all_args = Vec::new();
            all_args.extend(config.args.iter().cloned());
            all_args.extend(profile_args.iter().cloned());

            Ok(BuiltCommand {
                executable: program.clone(),
                args: all_args,
                notes,
            })
        }
    }
}

/// 复制一个句柄，供停止流程独立持有。
#[cfg(windows)]
unsafe fn duplicate_handle(handle: HANDLE) -> Option<HANDLE> {
    let mut duplicated: HANDLE = std::ptr::null_mut();
    let ok = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut duplicated,
            0,
            FALSE,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if ok == FALSE { None } else { Some(duplicated) }
}

fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if arg.chars().any(|c| c.is_whitespace()) || arg.contains('"') {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// 构建 Windows 命令行。
///
/// 注意 `cmd /c` 的引号规则：当命令本身（如带空格的 mvnw.cmd 路径）需要引号时，
/// 必须再包一层引号，否则 cmd 会剥离首尾引号导致路径被截断。
fn build_windows_command_line(executable: &str, args: &[String]) -> String {
    let exe = quote_windows_arg(executable);

    if executable.eq_ignore_ascii_case("cmd.exe") {
        if let Some(pos) = args
            .iter()
            .position(|a| a.eq_ignore_ascii_case("/c") || a.eq_ignore_ascii_case("/k"))
        {
            if let Some(target) = args.get(pos + 1) {
                let needs_quotes =
                    target.chars().any(|c| c.is_whitespace()) || target.contains('"');
                if needs_quotes {
                    let mut inner = quote_windows_arg(target);
                    for arg in &args[pos + 2..] {
                        inner.push(' ');
                        inner.push_str(&quote_windows_arg(arg));
                    }
                    let mut line = exe;
                    for arg in &args[..=pos] {
                        line.push(' ');
                        line.push_str(&quote_windows_arg(arg));
                    }
                    line.push_str(" \"");
                    line.push_str(&inner);
                    line.push('"');
                    return line;
                }
            }
        }
    }

    let mut line = exe;
    for arg in args {
        line.push(' ');
        line.push_str(&quote_windows_arg(arg));
    }
    line
}

/// 把子进程输出解码为 UTF-8。
///
/// Windows 中文环境下（Java 17）Maven / Spring Boot 的输出是 GBK，
/// 直接按 UTF-8 读取会在第一行中文处报错并中断日志流，因此按行做退化解码。
fn decode_console_bytes(bytes: &[u8]) -> String {
    let mut end = bytes.len();
    while end > 0 && (bytes[end - 1] == b'\n' || bytes[end - 1] == b'\r') {
        end -= 1;
    }
    let body = &bytes[..end];
    if body.is_empty() {
        return String::new();
    }
    let decoded = match std::str::from_utf8(body) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(body);
            decoded.into_owned()
        }
    };
    strip_ansi_escapes(&decoded)
}

/// 去掉 Spring Boot / Maven 彩色输出中的 ANSI 转义序列，避免控制台出现 `[0;39m` 之类的乱码。
fn strip_ansi_escapes(input: &str) -> String {
    if !input.contains('\u{1b}') {
        return input.to_string();
    }
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // CSI 序列以 @ 到 ~ 之间的字节结束
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
        }
        output.push(ch);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn sample_config(work_dir: &str) -> ServiceConfig {
        ServiceConfig {
            id: "maven-cmd-test".to_string(),
            name: "Maven Cmd Test".to_string(),
            work_dir: work_dir.to_string(),
            launch_type: LaunchType::MavenWrapper,
            jar_path: None,
            custom_program: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            port: None,
            java_path: None,
            selected_profiles: Vec::new(),
            maven_module: None,
            stop_timeout_secs: 5,
            inject_profile_custom: false,
        }
    }

    /// 夹具目录相对仓库定位，避免测试依赖开发者本机的绝对路径。
    fn fixture_service_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri should have a parent directory")
            .join("fixtures")
            .join("demo-spring-service")
    }

    fn temp_maven_project(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("lsr-process-tests").join(format!(
            "{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("app")).unwrap();
        fs::write(
            dir.join("pom.xml"),
            r#"<project>
                <groupId>org.example</groupId>
                <artifactId>parent</artifactId>
                <packaging>pom</packaging>
                <modules><module>app</module></modules>
            </project>"#,
        )
        .unwrap();
        fs::write(
            dir.join("app").join("pom.xml"),
            r#"<project>
                <artifactId>app</artifactId>
                <build>
                    <plugins>
                        <plugin>
                            <groupId>org.springframework.boot</groupId>
                            <artifactId>spring-boot-maven-plugin</artifactId>
                        </plugin>
                    </plugins>
                </build>
            </project>"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_maven_command_uses_detected_module() {
        let dir = temp_maven_project("detected");
        let config = sample_config(dir.to_str().unwrap());

        let built = build_command(&config, &[]).unwrap();
        assert_eq!(built.executable, "cmd.exe");
        assert_eq!(built.args[0], "/c");
        assert_eq!(built.args[2], "-pl");
        assert_eq!(built.args[3], "app");
        assert_eq!(built.args[4], "spring-boot:run");
        assert!(built.notes.iter().any(|n| n.contains("\"app\"")));

        // 显式配置模块时不再输出自动探测提示
        let mut configured = sample_config(dir.to_str().unwrap());
        configured.maven_module = Some("app".to_string());
        let built = build_command(&configured, &[]).unwrap();
        assert_eq!(built.args[2], "-pl");
        assert_eq!(built.args[3], "app");
        assert!(built.notes.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_maven_command_rejects_missing_module() {
        let dir = temp_maven_project("missing");
        let mut config = sample_config(dir.to_str().unwrap());
        config.maven_module = Some("not-exists".to_string());

        let err = build_command(&config, &[]).unwrap_err();
        assert!(err.contains("not-exists"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cmd_command_line_quotes_command_with_spaces() {
        let args: Vec<String> = vec![
            "/c".to_string(),
            r"C:\Program Files\apache-maven\bin\mvn.cmd".to_string(),
            "spring-boot:run".to_string(),
        ];
        assert_eq!(
            build_windows_command_line("cmd.exe", &args),
            r#"cmd.exe /c ""C:\Program Files\apache-maven\bin\mvn.cmd" spring-boot:run""#
        );

        let plain: Vec<String> = vec![
            "/c".to_string(),
            "mvn.cmd".to_string(),
            "-pl".to_string(),
            "ry-admin".to_string(),
        ];
        assert_eq!(
            build_windows_command_line("cmd.exe", &plain),
            r#"cmd.exe /c mvn.cmd -pl ry-admin"#
        );
    }

    #[test]
    fn test_service_status_serializes_camel_case_fields() {
        let running = ServiceStatus::Running {
            pid: 42,
            active_profiles: vec!["dev".to_string(), "local".to_string()],
            started_at: "2026-01-01 00:00:00".to_string(),
        };
        let json = serde_json::to_value(&running).unwrap();
        assert_eq!(json["type"], "running");
        assert_eq!(json["pid"], 42);
        assert_eq!(json["activeProfiles"][0], "dev");
        assert_eq!(json["activeProfiles"][1], "local");
        assert_eq!(json["startedAt"], "2026-01-01 00:00:00");
        assert!(json.get("active_profiles").is_none());
        assert!(json.get("started_at").is_none());

        let failed = ServiceStatus::Failed {
            exit_code: Some(1),
            error: "boom".to_string(),
        };
        let json = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["type"], "failed");
        assert_eq!(json["exitCode"], 1);
        assert_eq!(json["error"], "boom");
        assert!(json.get("exit_code").is_none());
    }

    #[test]
    fn test_decode_console_bytes_handles_gbk_and_utf8() {
        let (gbk, _, _) = encoding_rs::GBK.encode("启动成功");
        assert_eq!(decode_console_bytes(&gbk), "启动成功");
        assert_eq!(
            decode_console_bytes("服务已停止\r\n".as_bytes()),
            "服务已停止"
        );
        assert_eq!(decode_console_bytes(b"\r\n"), "");

        // ANSI 颜色转义需要被剥离
        assert_eq!(
            decode_console_bytes(b"\x1b[0;39m - Project Started Successful\x1b[m\r\n"),
            " - Project Started Successful"
        );
    }

    #[test]
    fn test_real_jar_lifecycle_and_job_object() {
        let fixture_dir = fixture_service_dir();
        let jar_path = fixture_dir.join("target").join("demo-service.jar");
        if !jar_path.exists() {
            eprintln!("Fixture JAR does not exist, skipping real run test");
            return;
        }

        let log_mgr = LogManager::new();
        let proc_mgr = ProcessManager::new(log_mgr.clone());

        let service = ServiceConfig {
            id: "test-svc-1".to_string(),
            name: "Test Spring Service".to_string(),
            work_dir: fixture_dir.to_string(),
            launch_type: LaunchType::Jar,
            jar_path: Some(jar_path),
            custom_program: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            port: Some(8080),
            java_path: None,
            selected_profiles: vec!["dev".to_string(), "local".to_string()],
            maven_module: None,
            stop_timeout_secs: 5,
            inject_profile_custom: false,
        };

        // 1. 启动服务
        let start_res = proc_mgr.start_service(None, &service);
        assert!(start_res.is_ok(), "Service should start: {:?}", start_res);

        // 验证状态为 Running
        let mut is_running = false;
        for _ in 0..30 {
            if let ServiceStatus::Running {
                pid,
                active_profiles,
                ..
            } = proc_mgr.get_status(&service.id)
            {
                assert!(pid > 0);
                assert_eq!(
                    active_profiles,
                    vec!["dev".to_string(), "local".to_string()]
                );
                is_running = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert!(is_running, "Service should reach Running state");

        // 2. 防重入测试：在 Running 时再次调用 start 必须被拒绝
        let duplicate_start = proc_mgr.start_service(None, &service);
        assert!(duplicate_start.is_err(), "Duplicate start must be rejected");

        // 3. 验证日志捕获
        thread::sleep(Duration::from_secs(2));
        let logs = log_mgr.get_logs(&service.id);
        assert!(!logs.is_empty(), "Logs should not be empty");

        let has_active_profile_log = logs.iter().any(|l| {
            l.content
                .contains("The following 1 profile is active: \"dev,local\"")
        });
        assert!(
            has_active_profile_log,
            "Logs should record active profiles dev,local"
        );

        // 4. 停止服务
        let stop_res = proc_mgr.stop_service(None, &service.id, 5);
        assert!(stop_res.is_ok());

        // 验证状态变为 Stopped
        let mut is_stopped = false;
        for _ in 0..30 {
            if let ServiceStatus::Stopped = proc_mgr.get_status(&service.id) {
                is_stopped = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert!(is_stopped, "Service should reach Stopped state");
    }

    #[test]
    fn test_service_restart_flow() {
        let fixture_dir = fixture_service_dir();
        let jar_path = fixture_dir.join("target").join("demo-service.jar");
        if !jar_path.exists() {
            return;
        }

        let log_mgr = LogManager::new();
        let proc_mgr = ProcessManager::new(log_mgr.clone());

        let mut service = ServiceConfig {
            id: "test-restart-svc".to_string(),
            name: "Test Restart Service".to_string(),
            work_dir: fixture_dir.to_string(),
            launch_type: LaunchType::Jar,
            jar_path: Some(jar_path),
            custom_program: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            port: Some(8081),
            java_path: None,
            selected_profiles: vec!["dev".to_string()],
            maven_module: None,
            stop_timeout_secs: 5,
            inject_profile_custom: false,
        };

        // 启动初始为 dev
        proc_mgr.start_service(None, &service).unwrap();
        thread::sleep(Duration::from_secs(1));

        // 修改为 prod 并执行重启
        service.selected_profiles = vec!["prod".to_string()];
        let restart_res = proc_mgr.restart_service(None, &service);
        assert!(
            restart_res.is_ok(),
            "Restart must succeed: {:?}",
            restart_res
        );

        thread::sleep(Duration::from_secs(2));

        // 验证重启后的活跃 profile 已变更为 prod
        if let ServiceStatus::Running {
            active_profiles, ..
        } = proc_mgr.get_status(&service.id)
        {
            assert_eq!(active_profiles, vec!["prod".to_string()]);
        } else {
            panic!("Service should be running after restart");
        }

        // 清理停止
        proc_mgr.stop_service(None, &service.id, 5).unwrap();
    }
}
