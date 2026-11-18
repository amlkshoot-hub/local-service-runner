use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub const DEFAULT_MAX_LOG_LINES: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub content: String,
    pub source: String, // "stdout", "stderr", "system"
}

#[derive(Clone)]
pub struct LogManager {
    logs: Arc<Mutex<HashMap<String, VecDeque<LogEntry>>>>,
    max_lines: usize,
}

impl LogManager {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(HashMap::new())),
            max_lines: DEFAULT_MAX_LOG_LINES,
        }
    }

    pub fn append_log(
        &self,
        app_handle: Option<&AppHandle>,
        service_id: &str,
        content: String,
        source: &str,
    ) {
        let timestamp = Local::now().format("%H:%M:%S%.3f").to_string();
        let level = detect_log_level(&content);

        let entry = LogEntry {
            timestamp,
            level,
            content: mask_sensitive_info(&content),
            source: source.to_string(),
        };

        {
            let mut map = self.logs.lock().unwrap();
            let queue = map.entry(service_id.to_string()).or_default();
            if queue.len() >= self.max_lines {
                queue.pop_front();
            }
            queue.push_back(entry.clone());
        }

        // 向前端发射 Tauri 事件
        if let Some(app) = app_handle {
            let payload = ServiceLogEvent {
                service_id: service_id.to_string(),
                entry,
            };
            let _ = app.emit("service-log", payload);
        }
    }

    pub fn get_logs(&self, service_id: &str) -> Vec<LogEntry> {
        let map = self.logs.lock().unwrap();
        if let Some(queue) = map.get(service_id) {
            queue.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn clear_logs(&self, service_id: &str) {
        let mut map = self.logs.lock().unwrap();
        if let Some(queue) = map.get_mut(service_id) {
            queue.clear();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLogEvent {
    pub service_id: String,
    pub entry: LogEntry,
}

fn detect_log_level(line: &str) -> String {
    let upper = line.to_uppercase();
    if upper.contains(" ERROR ")
        || upper.contains("[ERROR]")
        || upper.starts_with("ERROR")
        || upper.contains("FATAL")
    {
        "ERROR".to_string()
    } else if upper.contains(" WARN ")
        || upper.contains("[WARN")
        || upper.starts_with("WARN")
        || upper.contains("WARNING")
    {
        "WARN".to_string()
    } else if upper.contains(" INFO ") || upper.contains("[INFO]") || upper.starts_with("INFO") {
        "INFO".to_string()
    } else if upper.contains(" DEBUG ") || upper.contains("[DEBUG]") || upper.starts_with("DEBUG") {
        "DEBUG".to_string()
    } else {
        "DEFAULT".to_string()
    }
}

fn mask_sensitive_info(text: &str) -> String {
    // 对常见的密码、密钥做简单脱敏保护
    let mut sanitized = text.to_string();
    let sensitive_keys = ["password", "secret", "private_key", "token", "pwd"];
    for key in sensitive_keys {
        if let Some(pos) = sanitized.to_lowercase().find(key) {
            // 简单安全脱敏，避免泄露完整密码
            if let Some(eq_pos) = sanitized[pos..].find('=') {
                let val_start = pos + eq_pos + 1;
                let val_end = sanitized[val_start..]
                    .find(|c: char| c.is_whitespace() || c == '&' || c == ',' || c == ';')
                    .map(|idx| val_start + idx)
                    .unwrap_or(sanitized.len());
                if val_end > val_start {
                    sanitized.replace_range(val_start..val_end, "******");
                }
            }
        }
    }
    sanitized
}
