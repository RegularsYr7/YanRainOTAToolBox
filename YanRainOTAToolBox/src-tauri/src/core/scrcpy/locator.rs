use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::infra::errors::AppError;
use crate::infra::permissions::ensure_executable;

/// scrcpy 可执行文件名
#[cfg(target_os = "windows")]
const SCRCPY_BINARY: &str = "scrcpy.exe";
#[cfg(not(target_os = "windows"))]
const SCRCPY_BINARY: &str = "scrcpy";

/// 获取当前操作系统目录名
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

/// 获取当前 CPU 架构目录名
fn arch_dir_name() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "aarch64"
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        "x86_64"
    }
}

/// 从 app_dir 向上回溯查找项目根目录
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

/// 定位 scrcpy 可执行文件
///
/// 查找路径优先级：
/// 1. 应用内置路径 resources/scrcpy/<os>/<arch>/scrcpy
/// 2. 开发模式回溯项目根目录
/// 3. 系统 PATH
pub fn locate_scrcpy(app_dir: &Path) -> Result<PathBuf, AppError> {
    let platform = platform_dir_name();
    let arch = arch_dir_name();

    let mut candidates: Vec<PathBuf> = vec![
        // 打包后路径
        app_dir
            .join("resources")
            .join("scrcpy")
            .join(platform)
            .join(arch)
            .join(SCRCPY_BINARY),
        // 备选
        app_dir
            .join("scrcpy")
            .join(platform)
            .join(arch)
            .join(SCRCPY_BINARY),
    ];

    // 开发模式：回溯到项目根目录
    if let Some(project_root) = find_project_root(app_dir) {
        candidates.push(
            project_root
                .join("resources")
                .join("scrcpy")
                .join(platform)
                .join(arch)
                .join(SCRCPY_BINARY),
        );
    }

    for candidate in &candidates {
        debug!("Checking scrcpy path: {:?}", candidate);
        if candidate.exists() {
            info!("Found scrcpy at: {:?}", candidate);
            let _ = ensure_executable(candidate);
            return Ok(candidate.clone());
        }
    }

    // 最后尝试系统 PATH
    let mut cmd = std::process::Command::new(SCRCPY_BINARY);
    cmd.arg("--version");
    crate::infra::process::hide_console_window_std(&mut cmd);
    if let Ok(output) = cmd.output() {
        if output.status.success() {
            info!("Found scrcpy in system PATH");
            return Ok(PathBuf::from(SCRCPY_BINARY));
        }
    }

    Err(AppError::CommandFailed(format!(
        "scrcpy not found for platform '{}' arch '{}'. Checked paths: {:?}",
        platform, arch, candidates
    )))
}

/// 获取 scrcpy-server 文件路径（与 scrcpy 同目录）
pub fn locate_scrcpy_server(scrcpy_path: &Path) -> Option<PathBuf> {
    let dir = scrcpy_path.parent()?;
    let server_path = dir.join("scrcpy-server");
    if server_path.exists() {
        Some(server_path)
    } else {
        None
    }
}
