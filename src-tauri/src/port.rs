//! 端口预检：启动服务前探测目标端口是否已被占用。
//!
//! 两步走：
//! 1. `TcpListener::bind` 快速探测端口是否空闲（免费且零权限）；
//! 2. 若被占用，通过 `GetExtendedTcpTable`（IP Helper API）查出占用者 PID，
//!    错误信息直接告诉用户「谁占的」，而不是让 Spring Boot 起来后才报
//!    `Port already in use`。

#[cfg(windows)]
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
#[cfg(windows)]
use windows_sys::Win32::NetworkManagement::IpHelper::GetExtendedTcpTable;
#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::AF_INET;
#[cfg(windows)]
use windows_sys::Win32::NetworkManagement::IpHelper::{
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};

/// MIB_TCP_STATE_LISTEN
#[cfg(windows)]
const MIB_TCP_STATE_LISTEN: u32 = 2;

/// 探测端口是否被占用。
/// - 返回 `Ok(None)`：端口空闲；
/// - 返回 `Ok(Some(owner_pid))`：被占用，`owner_pid` 为占用者进程（查不到时为 None）；
/// - 返回 `Err`：探测本身失败。
pub fn check_port_availability(port: u16) -> Result<Option<Option<u32>>, String> {
    // 服务自身未运行时，端口能被 bind 即为空闲
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => Ok(None),
        Err(err) => {
            if err.kind() != std::io::ErrorKind::AddrInUse {
                // Windows 下非 AddrInUse 的错误（权限等）不阻塞启动，只作日志
                eprintln!("Port pre-check error on {}: {}", port, err);
                return Ok(None);
            }
            Ok(Some(lookup_listener_owner_pid(port)))
        }
    }
}

/// 遍历系统 TCP 表找出 LISTEN 态监听该端口的进程 PID。
#[cfg(windows)]
fn lookup_listener_owner_pid(port: u16) -> Option<u32> {
    unsafe {
        let mut size: u32 = 0;
        // 第一次调用取所需缓冲区大小
        let rc = GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0, // FALSE: 不按进程排序
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if rc != 0 && rc != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        if size == 0 {
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        let rc = GetExtendedTcpTable(
            buffer.as_mut_ptr() as *mut _,
            &mut size,
            0,
            AF_INET as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if rc != 0 {
            return None;
        }

        let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let count = table.dwNumEntries as usize;
        // 行数组紧跟在 MIB_TCPTABLE_OWNER_PID 头部（第一个条目位于表结构末尾）
        let rows_ptr = buffer.as_ptr().add(std::mem::size_of::<MIB_TCPTABLE_OWNER_PID>()
            - std::mem::size_of::<MIB_TCPROW_OWNER_PID>());
        let rows = std::slice::from_raw_parts(rows_ptr as *const MIB_TCPROW_OWNER_PID, count);

        for row in rows {
            let is_listener = row.dwState == MIB_TCP_STATE_LISTEN;
            if !is_listener {
                continue;
            }
            // dwLocalPort 为网络字节序
            let local_port = u16::from_be((row.dwLocalPort & 0xFFFF) as u16);
            if local_port == port {
                return Some(row.dwOwningPid);
            }
        }
        None
    }
}

#[cfg(not(windows))]
fn lookup_listener_owner_pid(_port: u16) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_port_reports_available() {
        // 绑定一个临时监听器拿一个确定空闲的端口，然后释放后再检查应为空闲
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let result = check_port_availability(port).unwrap();
        assert!(result.is_none(), "released port should be free");
    }

    #[test]
    fn test_occupied_port_reports_pid() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let result = check_port_availability(port).unwrap();
        assert!(result.is_some(), "bound port should be reported occupied");
        if let Some(owner) = result {
            // Windows 上应能解析出占用者 PID（就是当前测试进程或其子进程）
            assert!(owner.is_some(), "owner pid should be resolvable on Windows");
        }

        drop(listener);
    }
}
