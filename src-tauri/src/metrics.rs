//! 服务资源监控：基于 Job Object 枚举受管进程树，汇总 CPU 时间与工作集内存。
//!
//! CPU 百分比需要两次采样差分：ProcessManager 维护每个服务上次采样的
//! (累计 CPU 时间, 时刻)，前端轮询调用时自动计算区间利用率。

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    JobObjectBasicProcessIdList, QueryInformationJobObject, JOBOBJECT_BASIC_PROCESS_ID_LIST,
};
#[cfg(windows)]
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
#[cfg(windows)]
use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMetrics {
    pub pid: u32,
    /// 进程树 CPU 利用率（%），首次采样无基线时为 None
    pub cpu_percent: Option<f64>,
    /// 进程树工作集内存合计（MB）
    pub memory_mb: f64,
    /// 受管进程数（含 mvn.cmd / cmd.exe 等派生进程）
    pub process_count: usize,
}

/// 每服务上次采样的累计 CPU 时间（100ns 单位）与采样时刻。
#[derive(Debug, Clone, Copy)]
struct CpuSample {
    total_100ns: u64,
    at: Instant,
}

#[derive(Clone, Default)]
pub struct MetricsManager {
    last_samples: Arc<Mutex<HashMap<String, CpuSample>>>,
}

impl MetricsManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 采样指定 Job Object 的资源占用；`job_handle` 为 0 视为服务未运行。
    #[cfg(windows)]
    pub fn query(&self, service_id: &str, job_handle: usize) -> Option<ServiceMetrics> {
        if job_handle == 0 {
            return None;
        }
        let pids = enumerate_job_pids(job_handle)?;
        let first_pid = pids.first().copied()?;

        let mut total_cpu_100ns: u64 = 0;
        let mut total_working_set: u64 = 0;
        let mut alive = 0usize;

        for pid in &pids {
            if let Some((cpu_100ns, working_set)) = query_process(*pid) {
                total_cpu_100ns = total_cpu_100ns.saturating_add(cpu_100ns);
                total_working_set = total_working_set.saturating_add(working_set);
                alive += 1;
            }
        }

        let cpu_percent = {
            let mut samples = self.last_samples.lock().unwrap();
            let now = Instant::now();
            let computed = match samples.get(service_id) {
                Some(prev) => {
                    let wall = now.duration_since(prev.at).as_secs_f64();
                    if wall > 0.2 {
                        let delta = total_cpu_100ns.saturating_sub(prev.total_100ns);
                        Some(delta as f64 / 100_000_00.0 / wall) // 100ns → 秒
                    } else {
                        None
                    }
                }
                None => None,
            };
            samples.insert(
                service_id.to_string(),
                CpuSample {
                    total_100ns: total_cpu_100ns,
                    at: now,
                },
            );
            computed.map(|v| v.min(999.0) * 100.0 / num_cpus()).map(|v| (v * 10.0).round() / 10.0)
        };

        Some(ServiceMetrics {
            pid: first_pid,
            cpu_percent,
            memory_mb: (total_working_set as f64 / 1024.0 / 1024.0 * 10.0).round() / 10.0,
            process_count: alive,
        })
    }

    #[cfg(not(windows))]
    pub fn query(&self, _service_id: &str, _job_handle: usize) -> Option<ServiceMetrics> {
        None
    }

    /// 服务停止后清除采样基线。
    pub fn clear(&self, service_id: &str) {
        self.last_samples.lock().unwrap().remove(service_id);
    }
}

/// 逻辑核心数（GetSystemInfo，避免引入额外 crate）。
#[cfg(windows)]
fn num_cpus() -> f64 {
    unsafe {
        let mut info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut info);
        (info.dwNumberOfProcessors as f64).max(1.0)
    }
}

/// 枚举 Job Object 内全部进程 PID。
#[cfg(windows)]
fn enumerate_job_pids(job_handle: usize) -> Option<Vec<u32>> {
    unsafe {
        let mut buffer: Vec<u8> =
            vec![0; std::mem::size_of::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() + 8 * 1024];
        let mut returned: u32 = 0;
        let rc = QueryInformationJobObject(
            job_handle as HANDLE,
            JobObjectBasicProcessIdList,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
        );
        if rc == 0 {
            return None;
        }
        let list = &*(buffer.as_ptr() as *const JOBOBJECT_BASIC_PROCESS_ID_LIST);
        let count = list.NumberOfProcessIdsInList as usize;
        if count == 0 {
            return Some(Vec::new());
        }
        // ProcessIdList 是 ULONG_PTR（usize）数组
        let raw = std::slice::from_raw_parts(list.ProcessIdList.as_ptr(), count);
        Some(raw.iter().map(|&p| p as u32).collect())
    }
}

/// 查询单个进程的（累计 CPU 时间 100ns，工作集字节）。
#[cfg(windows)]
fn query_process(pid: u32) -> Option<(u64, u64)> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();

        let times_ok = GetProcessTimes(
            handle,
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        ) != 0;

        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let mem_ok = GetProcessMemoryInfo(handle, &mut counters, cb) != 0;

        CloseHandle(handle);

        let cpu_100ns = if times_ok {
            let k = filetime_to_u64(kernel);
            let u = filetime_to_u64(user);
            k.saturating_add(u)
        } else {
            0
        };
        let working_set = if mem_ok { counters.WorkingSetSize as u64 } else { 0 };

        if !times_ok && !mem_ok {
            return None;
        }
        Some((cpu_100ns, working_set))
    }
}

#[cfg(windows)]
fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}
