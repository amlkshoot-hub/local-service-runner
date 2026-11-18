//! 服务健康探针：周期性 GET 健康检查端点，把「进程活着」升级为「真的能服务请求」。
//!
//! 刻意零 HTTP 依赖：本机 HTTP/1.1 GET 用 std TcpStream 手写即可（不涉及 TLS），
//! 避免为一次探测引入 reqwest 级别的重依赖。

use crate::config::ServiceConfig;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HealthState {
    /// 未配置健康检查 / 服务未运行
    Unknown,
    /// 检测中（服务刚启动，等待首次探测）
    Checking,
    /// 健康检查通过
    Up,
    /// 探测失败（连接拒绝 / 超时 / 非 200）
    Down,
}

const HEALTH_EVENT: &str = "service-health-change";
/// 服务启动后给 JVM 留出的首次探测缓冲
const FIRST_CHECK_DELAY_SECS: u64 = 4;
/// 常规探测周期
const CHECK_INTERVAL_SECS: u64 = 5;
/// 单次探测超时
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HealthEvent {
    service_id: String,
    health: HealthState,
}

/// 服务健康状态管理：状态表 + 运行中的检查器句柄（stop 标志）。
#[derive(Clone, Default)]
pub struct HealthManager {
    states: Arc<Mutex<HashMap<String, HealthState>>>,
    stop_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl HealthManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_state(&self, service_id: &str) -> HealthState {
        self.states
            .lock()
            .unwrap()
            .get(service_id)
            .copied()
            .unwrap_or(HealthState::Unknown)
    }

    fn set_state(&self, app: Option<&AppHandle>, service_id: &str, state: HealthState) {
        {
            let mut map = self.states.lock().unwrap();
            map.insert(service_id.to_string(), state);
        }
        if let Some(app) = app {
            let _ = app.emit(
                HEALTH_EVENT,
                HealthEvent {
                    service_id: service_id.to_string(),
                    health: state,
                },
            );
        }
    }

    /// 服务进入 Running 后启动周期探针；未配置健康检查 URL 则不做任何事。
    pub fn start_checker(
        &self,
        app_handle: Option<&AppHandle>,
        config: &ServiceConfig,
        service_id: String,
    ) {
        let Some(url) = resolve_health_url(config) else {
            return;
        };
        let stop = Arc::new(AtomicBool::new(false));

        {
            let mut flags = self.stop_flags.lock().unwrap();
            // 覆盖旧的（理论上不应存在，防御重复启动）
            if let Some(old) = flags.insert(service_id.clone(), stop.clone()) {
                old.store(true, Ordering::Relaxed);
            }
        }

        self.set_state(app_handle, &service_id, HealthState::Checking);

        // checker 持有自身克隆，与 ProcessManager 解耦
        let self_clone = self.clone();
        let app_for_checker = app_handle.cloned();
        let sid = service_id.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(FIRST_CHECK_DELAY_SECS));
            let mut tick: u64 = 0;
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                tick += 1;
                if tick % CHECK_INTERVAL_SECS == 0 {
                    let state = if http_get_ok(&url) {
                        HealthState::Up
                    } else {
                        HealthState::Down
                    };
                    self_clone.set_state(app_for_checker.as_ref(), &sid, state);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            // 检查器退出（服务停止或退出回调触发 stop）→ 状态归零
            self_clone.set_state(app_for_checker.as_ref(), &sid, HealthState::Unknown);
            let mut flags = self_clone.stop_flags.lock().unwrap();
            flags.remove(&sid);
        });
    }

    /// 停止服务的健康检查（服务停止 / 退出 / 重新启动前）。
    pub fn stop_checker(&self, service_id: &str) {
        if let Some(flag) = self.stop_flags.lock().unwrap().get(service_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// 应用退出时停止全部检查线程。
    pub fn stop_all(&self) {
        let flags: Vec<Arc<AtomicBool>> = {
            let mut map = self.stop_flags.lock().unwrap();
            map.drain().map(|(_, v)| v).collect()
        };
        for f in flags {
            f.store(true, Ordering::Relaxed);
        }
    }
}

/// 解析健康检查 URL：
/// - 绝对地址（http://...）原样使用；
/// - 相对路径（如 /actuator/health）拼接 `http://127.0.0.1:{port}`；
/// - 未配置或相对路径但无端口 → None（不做检测）。
pub fn resolve_health_url(config: &ServiceConfig) -> Option<String> {
    let raw = config
        .health_check_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;

    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_string());
    }

    // 相对路径：需要端口
    let port = config.port?;
    let path = if raw.starts_with('/') {
        raw.to_string()
    } else {
        format!("/{}", raw)
    };
    Some(format!("http://127.0.0.1:{}{}", port, path))
}

/// 极简 HTTP/1.1 GET：只关心响应状态码是否 200。
fn http_get_ok(url: &str) -> bool {
    let Some((host, port, path)) = parse_http_url(url) else {
        return false;
    };

    let Ok(mut stream) = TcpStream::connect((host.as_str(), port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(PROBE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(PROBE_TIMEOUT));

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: LocalServiceRunner/1.0\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, host
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = Vec::with_capacity(512);
    let mut buf = [0u8; 512];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buf[..n]);
                // 状态行完整（首个 CRLF）即可判定，无需等待对端关闭连接——
                // 服务器忽略 Connection: close 时避免白等满整个超时
                if response.windows(2).any(|w| w == b"\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let text = String::from_utf8_lossy(&response);
    // HTTP/1.1 200 OK
    text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200") || text.starts_with("HTTP/2 200")
}

/// 解析 http://host:port/path（不处理 https —— 本机健康检查不需要 TLS）。
fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(idx) => {
            let p: u16 = authority[idx + 1..].parse().ok()?;
            (authority[..idx].to_string(), p)
        }
        None => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return None;
    }
    Some((host, port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LaunchType;
    use std::collections::HashMap;

    fn sample_config(port: Option<u16>, url: Option<&str>) -> ServiceConfig {
        ServiceConfig {
            id: "svc-h".to_string(),
            name: "H".to_string(),
            work_dir: r"D:\proj\h".to_string(),
            launch_type: LaunchType::Jar,
            jar_path: None,
            custom_program: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            port,
            java_path: None,
            selected_profiles: Vec::new(),
            maven_module: None,
            stop_timeout_secs: 5,
            inject_profile_custom: false,
            build_goal: None,
            skip_tests: false,
            main_class: None,
            group: None,
            health_check_url: url.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_resolve_health_url_variants() {
        // 未配置
        assert_eq!(resolve_health_url(&sample_config(Some(8080), None)), None);

        // 绝对地址原样
        assert_eq!(
            resolve_health_url(&sample_config(None, Some("http://10.0.0.5:9000/probe"))),
            Some("http://10.0.0.5:9000/probe".to_string())
        );

        // 相对路径 + 有端口
        assert_eq!(
            resolve_health_url(&sample_config(Some(8080), Some("/actuator/health"))),
            Some("http://127.0.0.1:8080/actuator/health".to_string())
        );

        // 相对路径缺斜杠自动补
        assert_eq!(
            resolve_health_url(&sample_config(Some(9090), Some("actuator/health"))),
            Some("http://127.0.0.1:9090/actuator/health".to_string())
        );

        // 相对路径但没配置端口 → 无法拼接
        assert_eq!(resolve_health_url(&sample_config(None, Some("/actuator/health"))), None);
    }

    #[test]
    fn test_parse_http_url() {
        assert_eq!(
            parse_http_url("http://127.0.0.1:8080/actuator/health"),
            Some(("127.0.0.1".to_string(), 8080, "/actuator/health".to_string()))
        );
        assert_eq!(
            parse_http_url("http://localhost/x"),
            Some(("localhost".to_string(), 80, "/x".to_string()))
        );
        assert_eq!(parse_http_url("http://example.com"), Some(("example.com".to_string(), 80, "/".to_string())));
        assert_eq!(parse_http_url("https://secure/x"), None);
    }

    #[test]
    fn test_http_get_against_real_listener() {
        // 起一个最小 HTTP 服务：只服务一次连接，写完即 shutdown → 客户端读到 EOF 立即返回
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = std::thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
                let _ = conn.shutdown(std::net::Shutdown::Both);
            }
        });

        // 给服务端线程一点时间进入 accept，规避并行测试满载时的调度竞态
        std::thread::sleep(Duration::from_millis(150));

        assert!(http_get_ok(&format!("http://127.0.0.1:{}/actuator/health", port)));

        // 无服务端口应失败
        let free_port = {
            let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        std::thread::sleep(Duration::from_millis(50));
        assert!(!http_get_ok(&format!("http://127.0.0.1:{}/", free_port)));

        server.join().expect("server thread should exit after one connection");
    }
}
