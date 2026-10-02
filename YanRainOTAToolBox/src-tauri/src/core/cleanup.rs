//! 应用退出时的进程清理模块
//!
//! 在应用关闭时自动清理所有由本应用启动的子进程，
//! 包括 adb server、scrcpy 等，防止僵尸进程和端口占用。
//!
//! 使用 `sysinfo` crate 直接遍历系统进程并强制 kill，
//! 比 taskkill/pkill 更可靠，不会弹出控制台窗口。

use sysinfo::System;
use tracing::{info, warn};

/// 需要在退出时清理的进程名列表
const PROCESS_NAMES_TO_KILL: &[&str] = &[
    "adb",
    "scrcpy",
    "fastboot",
];

/// 执行全局进程清理
///
/// 使用 sysinfo 遍历系统进程，按名称匹配并强制 kill。
/// 该函数在应用主窗口关闭时被调用。
pub fn cleanup_all_processes() {
    info!("🧹 App exit cleanup: killing child processes via sysinfo...");

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    for name in PROCESS_NAMES_TO_KILL {
        kill_by_sysinfo(&sys, name);
    }

    // 额外执行 adb kill-server 确保 ADB server 彻底停止
    kill_adb_server();

    info!("🧹 App exit cleanup completed");
}

/// 使用 sysinfo 按进程名查找并强制 kill
fn kill_by_sysinfo(sys: &System, name: &str) {
    let target = name.to_lowercase();
    let mut killed = 0;

    for (pid, process) in sys.processes() {
        let pname = process.name().to_string_lossy().to_lowercase();
        // 匹配进程名（去掉 .exe 后缀比较）
        let pname_no_ext = pname.strip_suffix(".exe").unwrap_or(&pname);
        if pname_no_ext == target {
            if process.kill() {
                killed += 1;
                info!("  ✅ Killed {} (PID {})", name, pid);
            } else {
                warn!("  ⚠️ Failed to kill {} (PID {})", name, pid);
            }
        }
    }

    if killed > 0 {
        info!("  Killed {} instance(s) of '{}'", killed, name);
    }
}

/// 执行 adb kill-server 以确保 ADB server 停止
fn kill_adb_server() {
    // 尝试用系统 PATH 中的 adb
    let adb_cmd = if cfg!(target_os = "windows") {
        "adb.exe"
    } else {
        "adb"
    };

    let mut cmd = std::process::Command::new(adb_cmd);
    cmd.arg("kill-server");

    #[cfg(target_os = "windows")]
    crate::infra::process::hide_console_window_std(&mut cmd);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                info!("  ✅ ADB server killed");
            }
        }
        Err(_) => {
            // adb 不在 PATH 中也无所谓，前面 taskkill/pkill 已经尝试过了
        }
    }
}
