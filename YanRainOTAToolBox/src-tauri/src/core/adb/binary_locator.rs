use std::path::{Path, PathBuf};
use tracing::{debug, trace, warn};

use crate::infra::config::{AdbPathSource, AppConfig};
use crate::infra::errors::AppError;
use crate::infra::permissions::ensure_executable;

/// ADB 二进制文件名（按平台区分）
#[cfg(target_os = "windows")]
const ADB_BINARY: &str = "adb.exe";
#[cfg(not(target_os = "windows"))]
const ADB_BINARY: &str = "adb";

#[cfg(target_os = "windows")]
const FASTBOOT_BINARY: &str = "fastboot.exe";
#[cfg(not(target_os = "windows"))]
const FASTBOOT_BINARY: &str = "fastboot";

/// 获取当前操作系统对应的 platform-tools 子目录名
fn platform_dir_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
}

/// 定位 ADB 可执行文件
///
/// 按优先级查找：
/// 1. 用户设置的自定义路径
/// 2. 应用内置路径（resources/platform-tools/<os>/）
/// 3. 系统 PATH
pub fn locate_adb(config: &AppConfig, app_dir: &Path) -> Result<PathBuf, AppError> {
    match &config.adb_path_source {
        AdbPathSource::Custom => locate_custom_adb(config),
        AdbPathSource::Builtin => locate_builtin_adb(app_dir),
        AdbPathSource::SystemPath => locate_system_adb(),
    }
}

/// 定位 Fastboot 可执行文件
pub fn locate_fastboot(config: &AppConfig, app_dir: &Path) -> Result<PathBuf, AppError> {
    match &config.adb_path_source {
        AdbPathSource::Custom => locate_custom_fastboot(config),
        AdbPathSource::Builtin => locate_builtin_fastboot(app_dir),
        AdbPathSource::SystemPath => locate_system_fastboot(),
    }
}

/// 自动查找：按优先级逐个尝试
pub fn auto_locate_adb(config: &AppConfig, app_dir: &Path) -> Result<PathBuf, AppError> {
    // 1. 尝试自定义路径
    if config.custom_adb_path.is_some() {
        if let Ok(path) = locate_custom_adb(config) {
            return Ok(path);
        }
    }
    // 2. 尝试内置路径
    if let Ok(path) = locate_builtin_adb(app_dir) {
        return Ok(path);
    }
    // 3. 尝试系统 PATH
    if let Ok(path) = locate_system_adb() {
        return Ok(path);
    }

    Err(AppError::AdbNotFound(
        "ADB not found in any location. Please install platform-tools or set a custom path."
            .to_string(),
    ))
}

/// 自动查找 Fastboot：按优先级逐个尝试
pub fn auto_locate_fastboot(config: &AppConfig, app_dir: &Path) -> Result<PathBuf, AppError> {
    // 1. 尝试自定义路径
    if config.custom_adb_path.is_some() {
        if let Ok(path) = locate_custom_fastboot(config) {
            return Ok(path);
        }
    }
    // 2. 尝试内置路径
    if let Ok(path) = locate_builtin_fastboot(app_dir) {
        return Ok(path);
    }
    // 3. 尝试系统 PATH
    if let Ok(path) = locate_system_fastboot() {
        return Ok(path);
    }

    Err(AppError::AdbNotFound(
        "Fastboot not found in any location. Please install platform-tools or set a custom path."
            .to_string(),
    ))
}

fn locate_custom_adb(config: &AppConfig) -> Result<PathBuf, AppError> {
    if let Some(ref custom_path) = config.custom_adb_path {
        let path = PathBuf::from(custom_path);
        // 如果给出的是目录，拼接 adb 文件名
        let adb_path = if path.is_dir() {
            path.join(ADB_BINARY)
        } else {
            path
        };
        if adb_path.exists() {
            debug!("Found ADB at custom path: {:?}", adb_path);
            return Ok(adb_path);
        }
        warn!("Custom ADB path does not exist: {:?}", adb_path);
    }
    Err(AppError::AdbNotFound(
        "Custom ADB path not set or not found".to_string(),
    ))
}

fn locate_custom_fastboot(config: &AppConfig) -> Result<PathBuf, AppError> {
    if let Some(ref custom_path) = config.custom_adb_path {
        let path = PathBuf::from(custom_path);
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        let fastboot_path = dir.join(FASTBOOT_BINARY);
        if fastboot_path.exists() {
            return Ok(fastboot_path);
        }
    }
    Err(AppError::AdbNotFound(
        "Custom Fastboot path not found".to_string(),
    ))
}

fn locate_builtin_adb(app_dir: &Path) -> Result<PathBuf, AppError> {
    let platform = platform_dir_name();

    let mut candidates = vec![
        // 打包后的路径：<app_dir>/resources/platform-tools/<os>/adb
        app_dir
            .join("resources")
            .join("platform-tools")
            .join(platform)
            .join(ADB_BINARY),
        // 备选：直接在 app_dir 下
        app_dir
            .join("platform-tools")
            .join(platform)
            .join(ADB_BINARY),
    ];

    // 开发模式下 resource_dir 指向 src-tauri/target/debug/，
    // 需要向上回溯到项目根目录查找 resources/platform-tools/<os>/
    if let Some(project_root) = find_project_root(app_dir) {
        candidates.push(
            project_root
                .join("resources")
                .join("platform-tools")
                .join(platform)
                .join(ADB_BINARY),
        );
    }

    for candidate in &candidates {
        trace!("Checking builtin ADB path: {:?}", candidate);
        if candidate.exists() {
            trace!("Found builtin ADB at: {:?}", candidate);
            let _ = ensure_executable(candidate);
            return Ok(candidate.clone());
        }
    }

    Err(AppError::AdbNotFound(format!(
        "Builtin ADB not found for platform '{}'. Checked paths: {:?}",
        platform, candidates
    )))
}

fn locate_builtin_fastboot(app_dir: &Path) -> Result<PathBuf, AppError> {
    let platform = platform_dir_name();
    let mut candidates = vec![
        // 打包后的路径：<app_dir>/resources/platform-tools/<os>/fastboot
        app_dir
            .join("resources")
            .join("platform-tools")
            .join(platform)
            .join(FASTBOOT_BINARY),
        // 备选：直接在 app_dir 下
        app_dir
            .join("platform-tools")
            .join(platform)
            .join(FASTBOOT_BINARY),
    ];

    // 开发模式回溯到项目根目录
    if let Some(project_root) = find_project_root(app_dir) {
        candidates.push(
            project_root
                .join("resources")
                .join("platform-tools")
                .join(platform)
                .join(FASTBOOT_BINARY),
        );
    }

    for candidate in &candidates {
        if candidate.exists() {
            let _ = ensure_executable(candidate);
            return Ok(candidate.clone());
        }
    }

    Err(AppError::AdbNotFound(format!(
        "Builtin Fastboot not found for platform '{}'",
        platform
    )))
}

/// 从 app_dir 向上回溯查找项目根目录（通过检测 package.json + src-tauri）
/// 开发模式下 resource_dir 通常指向 src-tauri/target/debug/，
/// 需要向上回溯到包含 resources/platform-tools/ 的项目根目录
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    // 最多向上查找 10 层
    for _ in 0..10 {
        // 检查是否存在 package.json（项目根目录标志）
        if current.join("package.json").exists() && current.join("src-tauri").exists() {
            trace!("Found project root at: {:?}", current);
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn locate_system_adb() -> Result<PathBuf, AppError> {
    which_binary(ADB_BINARY)
}

fn locate_system_fastboot() -> Result<PathBuf, AppError> {
    which_binary(FASTBOOT_BINARY)
}

/// 在系统 PATH 中查找可执行文件
fn which_binary(name: &str) -> Result<PathBuf, AppError> {
    let path_var = std::env::var("PATH").unwrap_or_default();

    #[cfg(target_os = "windows")]
    let separator = ';';
    #[cfg(not(target_os = "windows"))]
    let separator = ':';

    for dir in path_var.split(separator) {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() {
            debug!("Found {} in system PATH: {:?}", name, candidate);
            return Ok(candidate);
        }
    }

    Err(AppError::AdbNotFound(format!(
        "{} not found in system PATH",
        name
    )))
}

/// 验证 ADB 可用性（执行 adb version）
pub async fn verify_adb(adb_path: &Path) -> Result<String, AppError> {
    let mut cmd = tokio::process::Command::new(adb_path);
    cmd.arg("version");

    // Linux/macOS: 注入 lib64 库搜索路径
    #[cfg(target_os = "linux")]
    if let Some(parent) = adb_path.parent() {
        let lib64 = parent.join("lib64");
        if lib64.exists() {
            cmd.env("LD_LIBRARY_PATH", lib64.to_string_lossy().as_ref());
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(parent) = adb_path.parent() {
        let lib64 = parent.join("lib64");
        if lib64.exists() {
            cmd.env("DYLD_LIBRARY_PATH", lib64.to_string_lossy().as_ref());
        }
    }

    crate::infra::process::hide_console_window(&mut cmd);
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::AdbNotFound(format!("Failed to execute adb: {}", e)))?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("ADB version: {}", version.trim());
        Ok(version.trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(AppError::AdbNotFound(format!(
            "ADB version check failed: {}",
            stderr
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_dir_name() {
        let name = platform_dir_name();
        assert!(["windows", "linux", "macos"].contains(&name));
    }
}
