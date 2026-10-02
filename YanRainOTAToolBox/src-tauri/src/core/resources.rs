use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// 从 exe_dir 向上回溯查找项目根目录（存在 package.json + src-tauri）
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    for _ in 0..10 {
        if current.join("package.json").exists() && current.join("src-tauri").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// 定位 resources 目录
/// 1. 生产模式：<exe_dir>/resources/
/// 2. 开发模式：向上回溯项目根目录 + resources/
fn resolve_resources_dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))?;

    let prod = exe_dir.join("resources");
    if prod.exists() && prod.is_dir() {
        debug!("Resources dir (prod): {:?}", prod);
        return Some(prod);
    }

    find_project_root(&exe_dir).map(|r| r.join("resources"))
}

/// Windows x64 必须存在的资源文件（相对 resources/ 的路径）
fn essential_files() -> [&'static str; 4] {
    [
        "platform-tools/windows/adb.exe",
        "platform-tools/windows/fastboot.exe",
        "scrcpy/windows/x86_64/scrcpy.exe",
        "scrcpy/windows/x86_64/scrcpy-server",
    ]
}

/// 遍历 essential_files，返回缺失文件列表（空 = 完整）
fn check_resources_integrity(resources_dir: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    for rel in &essential_files() {
        let full = resources_dir.join(rel);
        if !full.exists() {
            debug!("Missing resource: {}", rel);
            missing.push(rel.to_string());
        }
    }
    missing
}

/// 顶层入口：检查 resources/ 完整性
///
/// - 生产模式：检查 exe 同级 resources/ 下的所有必需文件
/// - 开发模式：resources/ 不存在于项目根时跳过检查，避免误报
///
/// 返回缺失的文件列表（相对路径），空列表 = 通过
pub fn verify_resources() -> Vec<String> {
    match resolve_resources_dir() {
        Some(dir) => check_resources_integrity(&dir),
        None => {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_default();

            if find_project_root(&exe_dir).is_some() {
                debug!("Dev mode, resources dir not found — skipping check");
                vec![]
            } else {
                warn!("Resources directory not found in production build");
                essential_files().iter().map(|s| s.to_string()).collect()
            }
        }
    }
}
