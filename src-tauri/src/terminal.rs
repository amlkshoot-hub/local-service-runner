//! 在指定目录唤起系统终端。
//!
//! 优先使用 Windows Terminal（`wt.exe -d <dir>`，Win11 自带；wt.exe 是转发器，
//! 立即返回，窗口由 WindowsTerminal 进程托管）；未安装时回退到新开的 cmd.exe 窗口。
//!
//! 终端进程通过 std::process::Command 独立启动，不进入本应用的 Job Object：
//! 用户关闭 Runner 不会连带关闭终端窗口，终端的生命周期完全由用户掌握。

use std::path::Path;

/// 在 work_dir 打开一个新终端窗口。
pub fn open_terminal_at(work_dir: &str) -> Result<(), String> {
    let dir = Path::new(work_dir);
    if !dir.is_dir() {
        return Err(format!("目录不存在: {}", work_dir));
    }

    // 1. 优先 Windows Terminal
    if std::process::Command::new("wt.exe")
        .arg("-d")
        .arg(dir)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }

    // 2. 回退：cmd.exe 新窗口（GUI 父进程无控制台，子进程自动获得新控制台窗口）
    match std::process::Command::new("cmd.exe")
        .current_dir(dir)
        .args(["/D", "/K"])
        .spawn()
    {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("无法打开终端: {}", e)),
    }
}
