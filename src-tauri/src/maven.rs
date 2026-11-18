//! Maven 多模块项目探测。
//!
//! `mvn spring-boot:run` 的前缀 `spring-boot` 只会在**当前工程**的 `<build><plugins>`
//! 中解析；如果工作目录是聚合根（packaging=pom），根本就找不到该插件，于是出现
//! `No plugin found for prefix 'spring-boot'`。本模块负责在聚合根下找出真正声明了
//! `spring-boot-maven-plugin` 的可启动子模块，供启动命令通过 `-pl` 指定。

use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// 递归扫描时跳过的目录，避免遍历 node_modules / target 等体积巨大的目录。
const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    ".idea",
    ".mvn",
    "dist",
    "build",
    "out",
    "logs",
    "docs",
    "prototypes",
    "script",
    "public",
    ".zcode",
];

const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MavenProjectInfo {
    pub is_maven_project: bool,
    pub root_declares_spring_boot_plugin: bool,
    pub has_modules: bool,
    /// 声明了 spring-boot-maven-plugin 的子模块（相对工程根的路径，使用 `/` 分隔）
    pub modules: Vec<String>,
    /// 推荐使用的启动模块（层级最浅的候选模块）
    pub suggested_module: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePlan {
    /// 需要通过 `-pl` 指定的启动模块；None 表示直接在工程根目录运行
    pub module: Option<String>,
    /// 需要写入控制台提示的说明
    pub note: Option<String>,
}

/// 扫描工作目录，返回 Maven 工程结构信息。
pub fn inspect(work_dir: &str) -> MavenProjectInfo {
    let root = Path::new(work_dir);
    let root_pom = root.join("pom.xml");
    if !root_pom.is_file() {
        return MavenProjectInfo::default();
    }

    let root_text = fs::read_to_string(&root_pom).unwrap_or_default();
    let root_declares = declares_spring_boot_plugin(&root_text);
    let has_modules = has_modules_section(&root_text);

    let mut poms: Vec<PathBuf> = Vec::new();
    collect_poms(root, 0, &mut poms);

    let mut modules: Vec<String> = poms
        .iter()
        .filter(|p| p.as_path() != root_pom.as_path())
        .filter_map(|pom| {
            let text = fs::read_to_string(pom).ok()?;
            if !declares_spring_boot_plugin(&text) {
                return None;
            }
            let dir = pom.parent()?;
            let rel = dir.strip_prefix(root).ok()?;
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("/"))
            }
        })
        .collect();

    // 层级越浅越可能是主启动模块（例如 ry-admin 优先于 ry-extend/ry-monitor-admin）
    modules.sort_by(|a, b| {
        a.matches('/')
            .count()
            .cmp(&b.matches('/').count())
            .then_with(|| a.cmp(b))
    });
    modules.dedup();

    let suggested = if root_declares {
        None
    } else {
        modules.first().cloned()
    };

    MavenProjectInfo {
        is_maven_project: true,
        root_declares_spring_boot_plugin: root_declares,
        has_modules,
        modules,
        suggested_module: suggested,
    }
}

/// 解析本次启动实际需要使用的模块。
///
/// - 用户显式配置了模块：校验后直接使用；
/// - 工程根目录本身声明了插件：直接使用根目录；
/// - 聚合根且存在可启动子模块：自动选择层级最浅的模块；
/// - 其余情况：保持原来的根目录启动（由 Maven 给出具体错误）。
pub fn plan_module(work_dir: &str, configured: Option<&str>) -> Result<ModulePlan, String> {
    let info = inspect(work_dir);

    if let Some(configured) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        let normalized = configured.replace('\\', "/").trim_matches('/').to_string();
        let module_pom = Path::new(work_dir).join(&normalized).join("pom.xml");
        if !module_pom.is_file() {
            return Err(format!(
                "配置的 Maven 启动模块不存在：{}（缺少 pom.xml）",
                normalized
            ));
        }
        return Ok(ModulePlan {
            module: Some(normalized),
            note: None,
        });
    }

    if !info.is_maven_project {
        return Err(format!(
            "工作目录下未找到 pom.xml，无法以 Maven 方式启动：{}",
            work_dir
        ));
    }

    if info.root_declares_spring_boot_plugin {
        return Ok(ModulePlan {
            module: None,
            note: None,
        });
    }

    match info.suggested_module.clone() {
        Some(module) => {
            let note = if info.modules.len() > 1 {
                let others = info
                    .modules
                    .iter()
                    .filter(|m| **m != module)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、");
                format!(
                    "检测到多模块工程，已自动选择启动模块 \"{}\"（其他可启动模块：{}）。如不符合预期，请在「编辑配置」中调整 Maven 启动模块。",
                    module, others
                )
            } else {
                format!("检测到多模块工程，已自动选择启动模块 \"{}\"", module)
            };
            Ok(ModulePlan {
                module: Some(module),
                note: Some(note),
            })
        }
        None => Ok(ModulePlan {
            module: None,
            note: None,
        }),
    }
}

fn collect_poms(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            collect_poms(&path, depth + 1, out);
        } else if name.eq_ignore_ascii_case("pom.xml") {
            out.push(path);
        }
    }
}

fn strip_comments(text: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").expect("valid comment regex"));
    re.replace_all(text, "").into_owned()
}

fn strip_plugin_management(text: &str) -> String {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?s)<pluginManagement\b.*?</pluginManagement>")
            .expect("valid pluginManagement regex")
    });
    re.replace_all(text, "").into_owned()
}

/// 判断 POM 的 `<build><plugins>` 中是否直接声明了 spring-boot-maven-plugin。
///
/// 只统计 `<plugins>`，不统计 `<pluginManagement>` —— 因为 Maven 解析
/// `spring-boot:run` 前缀时只看前者。
fn declares_spring_boot_plugin(pom_text: &str) -> bool {
    let cleaned = strip_plugin_management(&strip_comments(pom_text));

    static PLUGIN_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static ARTIFACT_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let plugin_re = PLUGIN_RE.get_or_init(|| {
        Regex::new(r"(?s)<plugin\b[^>]*>(.*?)</plugin>").expect("valid plugin regex")
    });
    let artifact_re = ARTIFACT_RE.get_or_init(|| {
        Regex::new(r"<artifactId>\s*spring-boot-maven-plugin\s*</artifactId>")
            .expect("valid artifact regex")
    });

    plugin_re
        .captures_iter(&cleaned)
        .any(|caps| artifact_re.is_match(&caps[1]))
}

fn has_modules_section(pom_text: &str) -> bool {
    static MODULES_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static MODULE_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let cleaned = strip_comments(pom_text);
    let modules_re = MODULES_RE.get_or_init(|| {
        Regex::new(r"(?s)<modules\b[^>]*>(.*?)</modules>").expect("valid modules regex")
    });
    let module_re = MODULE_RE.get_or_init(|| {
        Regex::new(r"<module>\s*[^<\s][^<]*</module>").expect("valid module regex")
    });
    modules_re
        .captures(&cleaned)
        .map(|caps| module_re.is_match(&caps[1]))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("lsr-maven-tests").join(format!(
            "{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pom(dir: &Path, content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("pom.xml"), content).unwrap();
    }

    const AGGREGATOR: &str = r#"<project>
        <groupId>org.example</groupId>
        <artifactId>demo-parent</artifactId>
        <packaging>pom</packaging>
        <modules>
            <module>app</module>
            <module>monitor</module>
        </modules>
    </project>"#;

    const BOOT_MODULE: &str = r#"<project>
        <parent><groupId>org.example</groupId><artifactId>demo-parent</artifactId></parent>
        <artifactId>app</artifactId>
        <build>
            <plugins>
                <plugin>
                    <groupId>org.springframework.boot</groupId>
                    <artifactId>spring-boot-maven-plugin</artifactId>
                </plugin>
            </plugins>
        </build>
    </project>"#;

    #[test]
    fn test_detects_module_in_aggregator() {
        let dir = temp_project("aggregator");
        write_pom(&dir, AGGREGATOR);
        write_pom(&dir.join("app"), BOOT_MODULE);

        let info = inspect(dir.to_str().unwrap());
        assert!(info.is_maven_project);
        assert!(!info.root_declares_spring_boot_plugin);
        assert!(info.has_modules);
        assert_eq!(info.modules, vec!["app".to_string()]);
        assert_eq!(info.suggested_module.as_deref(), Some("app"));

        let plan = plan_module(dir.to_str().unwrap(), None).unwrap();
        assert_eq!(plan.module.as_deref(), Some("app"));
        assert!(plan.note.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_prefers_shallow_module_and_ignores_plugin_management() {
        let dir = temp_project("shallow");
        write_pom(&dir, AGGREGATOR);
        write_pom(&dir.join("app"), BOOT_MODULE);

        // 嵌套更深且只有 pluginManagement 的模块不应被当作可启动模块
        let nested = r#"<project>
            <build>
                <pluginManagement>
                    <plugins>
                        <plugin><artifactId>spring-boot-maven-plugin</artifactId></plugin>
                    </plugins>
                </pluginManagement>
            </build>
        </project>"#;
        write_pom(&dir.join("monitor"), nested);

        let info = inspect(dir.to_str().unwrap());
        assert_eq!(info.modules, vec!["app".to_string()]);
        assert_eq!(info.suggested_module.as_deref(), Some("app"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_root_with_plugin_needs_no_module() {
        let dir = temp_project("single");
        write_pom(&dir, BOOT_MODULE);

        let info = inspect(dir.to_str().unwrap());
        assert!(info.root_declares_spring_boot_plugin);
        assert!(info.modules.is_empty());

        let plan = plan_module(dir.to_str().unwrap(), None).unwrap();
        assert_eq!(plan.module, None);
        assert!(plan.note.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_configured_module_is_validated() {
        let dir = temp_project("configured");
        write_pom(&dir, AGGREGATOR);
        write_pom(&dir.join("app"), BOOT_MODULE);

        let plan = plan_module(dir.to_str().unwrap(), Some("app")).unwrap();
        assert_eq!(plan.module.as_deref(), Some("app"));

        let err = plan_module(dir.to_str().unwrap(), Some("missing")).unwrap_err();
        assert!(err.contains("missing"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_non_maven_dir_is_reported() {
        let dir = temp_project("plain");
        let info = inspect(dir.to_str().unwrap());
        assert!(!info.is_maven_project);

        let err = plan_module(dir.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("pom.xml"));

        let _ = fs::remove_dir_all(&dir);
    }
}
