use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchType {
    Jar,
    MavenWrapper,
    GradleWrapper,
    Custom,
}

impl Default for LaunchType {
    fn default() -> Self {
        LaunchType::Jar
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub work_dir: String,
    pub launch_type: LaunchType,
    #[serde(default)]
    pub jar_path: Option<String>,
    #[serde(default)]
    pub custom_program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub selected_profiles: Vec<String>,
    /// 多模块 Maven 工程中的启动模块（相对工作目录，如 `ry-admin`）；
    /// 留空表示自动探测。
    #[serde(default)]
    pub maven_module: Option<String>,
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_secs: u64,
    #[serde(default)]
    pub inject_profile_custom: bool,
}

fn default_stop_timeout() -> u64 {
    10
}

pub struct ConfigManager {
    config_file_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let app_dir = dirs_fallback_local_appdata();
        let config_file_path = app_dir.join("services.json");
        Self { config_file_path }
    }

    pub fn load_services(&self) -> Vec<ServiceConfig> {
        if !self.config_file_path.exists() {
            return Vec::new();
        }
        match fs::read_to_string(&self.config_file_path) {
            Ok(content) => match serde_json::from_str::<Vec<ServiceConfig>>(&content) {
                Ok(services) => services,
                Err(err) => {
                    eprintln!("Failed to parse services.json: {}", err);
                    Vec::new()
                }
            },
            Err(err) => {
                eprintln!("Failed to read services.json: {}", err);
                Vec::new()
            }
        }
    }

    pub fn save_services(&self, services: &[ServiceConfig]) -> Result<(), String> {
        if let Some(parent) = self.config_file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let json = serde_json::to_string_pretty(services).map_err(|e| e.to_string())?;
        let temp_path = self.config_file_path.with_extension("tmp");

        {
            let mut file = File::create(&temp_path).map_err(|e| e.to_string())?;
            file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
        }

        // Atomic replace
        if self.config_file_path.exists() {
            let _ = fs::remove_file(&self.config_file_path);
        }
        fs::rename(&temp_path, &self.config_file_path).map_err(|e| e.to_string())?;

        Ok(())
    }
}

fn dirs_fallback_local_appdata() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        Path::new(&local_app_data).join("LocalServiceRunner")
    } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
        Path::new(&user_profile).join(".local_service_runner")
    } else {
        PathBuf::from("data")
    }
}
