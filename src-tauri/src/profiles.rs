use crate::config::LaunchType;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// 扫描项目资源目录中的 Profile 配置文件。
///
/// 多模块工程中配置文件通常位于启动模块下（如 `ry-admin/src/main/resources`），
/// 因此优先扫描配置的 Maven 模块目录，再回退到工程根目录。
pub fn detect_profiles(work_dir: &str, module: Option<&str>) -> Vec<String> {
    let mut detected = Vec::new();
    let mut seen = HashSet::new();

    // 正则匹配 application-{profile}.yml/yaml/properties
    let re = match Regex::new(r"^application-([a-zA-Z0-9_\-\.]+)\.(ya?ml|properties)$") {
        Ok(r) => r,
        Err(_) => return detected,
    };

    let mut resources_dirs = Vec::new();
    if let Some(module) = module.map(str::trim).filter(|m| !m.is_empty()) {
        resources_dirs.push(
            Path::new(work_dir)
                .join(module)
                .join("src")
                .join("main")
                .join("resources"),
        );
    }
    resources_dirs.push(
        Path::new(work_dir)
            .join("src")
            .join("main")
            .join("resources"),
    );

    for resources_dir in resources_dirs {
        if !resources_dir.is_dir() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&resources_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();

                // 规则1：基础配置文件排除
                if file_name == "application.yml"
                    || file_name == "application.yaml"
                    || file_name == "application.properties"
                {
                    continue;
                }

                if let Some(caps) = re.captures(&file_name) {
                    if let Some(profile_match) = caps.get(1) {
                        let profile_name = profile_match.as_str().to_string();
                        if seen.insert(profile_name.clone()) {
                            detected.push(profile_name);
                        }
                    }
                }
            }
        }
    }

    // 保持相对确定性的排序
    detected.sort();
    detected
}

/// 格式化用户选中的多个 Profile 为单一连接字符串（如 dev,local）
pub fn format_active_profiles(selected_profiles: &[String]) -> Option<String> {
    let mut valid_profiles = Vec::new();
    for p in selected_profiles {
        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
            continue;
        }
        // 支持用户直接输入逗号分隔的情况，并保留输入顺序
        for sub in trimmed.split(',') {
            let sub_trim = sub.trim();
            if !sub_trim.is_empty() && !sub_trim.eq_ignore_ascii_case("default") {
                if !valid_profiles.contains(&sub_trim.to_string()) {
                    valid_profiles.push(sub_trim.to_string());
                }
            }
        }
    }

    if valid_profiles.is_empty() {
        None
    } else {
        Some(valid_profiles.join(","))
    }
}

/// 构建针对各启动方式的 Profile 启动参数
pub fn build_profile_args(
    launch_type: &LaunchType,
    selected_profiles: &[String],
    existing_args: &[String],
    inject_profile_custom: bool,
) -> Vec<String> {
    let active_str = match format_active_profiles(selected_profiles) {
        Some(s) => s,
        None => return Vec::new(),
    };

    // 检查原有参数中是否已有 Profile 参数，避免静默重复覆盖
    let has_spring_profile_arg = existing_args.iter().any(|arg| {
        arg.starts_with("--spring.profiles.active=")
            || arg.starts_with("-Dspring.profiles.active=")
            || arg.starts_with("-Dspring-boot.run.profiles=")
    });

    if has_spring_profile_arg {
        // 如果用户已经在参数中显式指定了 Profile，则不重复追加冲突参数
        return Vec::new();
    }

    match launch_type {
        LaunchType::Jar => {
            vec![format!("--spring.profiles.active={}", active_str)]
        }
        LaunchType::MavenWrapper => {
            // Spring Boot Maven Plugin 规范使用 -Dspring-boot.run.profiles=...
            vec![format!("-Dspring-boot.run.profiles={}", active_str)]
        }
        LaunchType::GradleWrapper => {
            // Spring Boot Gradle Plugin bootRun 规范使用 --args
            vec![format!("--args=--spring.profiles.active={}", active_str)]
        }
        LaunchType::Custom => {
            if inject_profile_custom {
                vec![format!("--spring.profiles.active={}", active_str)]
            } else {
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_active_profiles() {
        assert_eq!(format_active_profiles(&["default".to_string()]), None);
        assert_eq!(
            format_active_profiles(&["dev".to_string(), "local".to_string()]),
            Some("dev,local".to_string())
        );
        assert_eq!(
            format_active_profiles(&["dev,local".to_string()]),
            Some("dev,local".to_string())
        );
        assert_eq!(
            format_active_profiles(&["default".to_string(), "prod".to_string()]),
            Some("prod".to_string())
        );
    }

    #[test]
    fn test_build_profile_args_jar() {
        let profiles = vec!["dev".to_string(), "local".to_string()];
        let args = build_profile_args(&LaunchType::Jar, &profiles, &[], false);
        assert_eq!(args, vec!["--spring.profiles.active=dev,local"]);
    }

    #[test]
    fn test_build_profile_args_maven() {
        let profiles = vec!["prod".to_string()];
        let args = build_profile_args(&LaunchType::MavenWrapper, &profiles, &[], false);
        assert_eq!(args, vec!["-Dspring-boot.run.profiles=prod"]);
    }

    #[test]
    fn test_build_profile_args_gradle() {
        let profiles = vec!["test".to_string()];
        let args = build_profile_args(&LaunchType::GradleWrapper, &profiles, &[], false);
        assert_eq!(args, vec!["--args=--spring.profiles.active=test"]);
    }

    #[test]
    fn test_detect_profiles_in_maven_module() {
        let dir = std::env::temp_dir()
            .join("lsr-profile-tests")
            .join(format!("module-{}", std::process::id()));
        let resources = dir.join("app").join("src").join("main").join("resources");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&resources).unwrap();
        fs::write(resources.join("application.yml"), "server:\n  port: 1\n").unwrap();
        fs::write(resources.join("application-dev.yml"), "").unwrap();
        fs::write(resources.join("application-local.properties"), "").unwrap();

        assert_eq!(
            detect_profiles(dir.to_str().unwrap(), Some("app")),
            vec!["dev".to_string(), "local".to_string()]
        );
        // 未指定模块时回退到工程根目录，根目录没有配置文件则为空
        assert!(detect_profiles(dir.to_str().unwrap(), None).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
