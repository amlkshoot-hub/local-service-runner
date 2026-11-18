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
    /// 先执行 mvn install/package 构建，成功后自动定位并运行 target 下的 JAR
    MavenBuildJar,
    /// 直接通过源码启动类运行（mvn compile exec:java）
    SourceMainClass,
}

impl Default for LaunchType {
    fn default() -> Self {
        LaunchType::Jar
    }
}

/// Maven 构建目标
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MavenGoal {
    Clean,
    Compile,
    Package,
    Install,
}

impl MavenGoal {
    pub fn as_str(&self) -> &'static str {
        match self {
            MavenGoal::Clean => "clean",
            MavenGoal::Compile => "compile",
            MavenGoal::Package => "package",
            MavenGoal::Install => "install",
        }
    }

    /// 把前端传入的任意 goal 字符串安全转换为受支持的 Maven 生命周期目标。
    pub fn parse(goal: &str) -> Option<Self> {
        match goal.trim().to_ascii_lowercase().as_str() {
            "clean" => Some(MavenGoal::Clean),
            "compile" => Some(MavenGoal::Compile),
            "package" => Some(MavenGoal::Package),
            "install" => Some(MavenGoal::Install),
            _ => None,
        }
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
    /// maven-build-jar：构建目标（package / install），留空默认 package
    #[serde(default)]
    pub build_goal: Option<String>,
    /// maven-build-jar：构建时是否追加 -DskipTests
    #[serde(default = "default_skip_tests")]
    pub skip_tests: bool,
    /// source-main-class：主类全限定名（如 com.example.DemoApplication）
    #[serde(default)]
    pub main_class: Option<String>,
    /// 服务分组名（业务域），留空归入「未分组」
    #[serde(default)]
    pub group: Option<String>,
    /// 健康检查 URL：绝对地址（http://...）直接探测；相对路径（如 /actuator/health）
    /// 自动拼接 http://127.0.0.1:{port}；留空表示不检测。
    #[serde(default)]
    pub health_check_url: Option<String>,
}

fn default_stop_timeout() -> u64 {
    10
}

/// 构建+运行场景下默认跳过测试，保证"点启动→能快速跑起来"的体验
fn default_skip_tests() -> bool {
    true
}

/// 导入合并结果
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// 本次导入新增的服务数
    pub added: usize,
    /// 覆盖更新（同 id 已存在）的服务数
    pub updated: usize,
    /// 导入后服务总数
    pub total: usize,
}

/// 导入合并的纯函数：同 id 覆盖更新，新 id 追加，保持原有顺序在前、新增在后。
/// 返回合并后的完整列表，便于单测验证。
pub fn merge_imported(
    existing: Vec<ServiceConfig>,
    incoming: Vec<ServiceConfig>,
) -> (Vec<ServiceConfig>, ImportSummary) {
    let mut merged: Vec<ServiceConfig> = existing;
    let mut index_by_id: HashMap<String, usize> =
        merged.iter().enumerate().map(|(i, s)| (s.id.clone(), i)).collect();

    let mut added = 0usize;
    let mut updated = 0usize;

    for svc in incoming {
        match index_by_id.get(&svc.id).copied() {
            Some(idx) => {
                merged[idx] = svc;
                updated += 1;
            }
            None => {
                index_by_id.insert(svc.id.clone(), merged.len());
                merged.push(svc);
                added += 1;
            }
        }
    }

    let total = merged.len();
    (merged, ImportSummary { added, updated, total })
}

/// 服务克隆：生成一份配置副本（新 id、新名称）。
pub fn clone_config(source: &ServiceConfig) -> ServiceConfig {
    let mut cloned = source.clone();
    cloned.id = format!("svc-{}", uuid::Uuid::new_v4());
    cloned.name = format!("{} (副本)", source.name);
    cloned
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, name: &str) -> ServiceConfig {
        ServiceConfig {
            id: id.to_string(),
            name: name.to_string(),
            work_dir: format!(r"D:\\proj\\{id}"),
            launch_type: LaunchType::Jar,
            jar_path: None,
            custom_program: None,
            args: Vec::new(),
            env_vars: HashMap::new(),
            port: None,
            java_path: None,
            selected_profiles: Vec::new(),
            maven_module: None,
            stop_timeout_secs: 10,
            inject_profile_custom: false,
            build_goal: None,
            skip_tests: false,
            main_class: None,
            group: None,
            health_check_url: None,
        }
    }

    #[test]
    fn test_merge_imported_appends_and_overwrites() {
        let existing = vec![sample("a", "Alpha"), sample("b", "Beta")];
        let mut updated_a = sample("a", "Alpha-v2");
        updated_a.port = Some(8080);
        let incoming = vec![updated_a.clone(), sample("c", "Gamma")];

        let (merged, summary) = merge_imported(existing, incoming);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.total, 3);
        assert_eq!(merged.len(), 3);
        // a 被覆盖更新，顺序保持，c 追加在末尾
        assert_eq!(merged[0].name, "Alpha-v2");
        assert_eq!(merged[0].port, Some(8080));
        assert_eq!(merged[1].name, "Beta");
        assert_eq!(merged[2].name, "Gamma");
    }

    #[test]
    fn test_merge_imported_empty_existing() {
        let incoming = vec![sample("x", "X")];
        let (merged, summary) = merge_imported(Vec::new(), incoming);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.updated, 0);
        assert_eq!(merged[0].name, "X");
    }

    #[test]
    fn test_clone_config_gets_new_identity() {
        let source = sample("a", "Alpha");
        let cloned = clone_config(&source);
        assert_ne!(cloned.id, "a");
        assert_eq!(cloned.name, "Alpha (副本)");
        assert_eq!(cloned.work_dir, source.work_dir);
        assert_eq!(cloned.launch_type, source.launch_type);
        assert_ne!(cloned.id, source.id);
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
