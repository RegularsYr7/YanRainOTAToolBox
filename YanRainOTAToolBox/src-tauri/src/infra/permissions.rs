use std::path::Path;

/// 确保文件具有可执行权限（仅 Unix 生效，Windows 上为空操作）
///
/// 检查文件是否缺少可执行位，如果缺少则添加 0o755 权限。
/// 用于 Tauri 打包后资源目录下的二进制工具（adb、fastboot 等），
/// 因为打包过程不保留 Unix 可执行权限。
#[cfg(unix)]
pub fn ensure_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use tracing::debug;
    let metadata = std::fs::metadata(path)?;
    let mut perms = metadata.permissions();
    if perms.mode() & 0o111 == 0 {
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms)?;
        debug!("Set executable permission for: {:?}", path);
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn ensure_executable(_path: &Path) -> std::io::Result<()> {
    // Windows 不需要设置可执行权限
    Ok(())
}

/// 递归遍历目录，对所有文件设置可执行权限（仅 Unix）
///
/// 用于应用启动时批量修复 resources 目录下的所有二进制工具权限。
#[cfg(unix)]
pub fn ensure_all_executable_in_dir(dir: &Path) -> std::io::Result<usize> {
    use std::os::unix::fs::PermissionsExt;
    let mut count = 0usize;

    if !dir.exists() || !dir.is_dir() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            count += ensure_all_executable_in_dir(&path)?;
        } else if path.is_file() {
            // 跳过明确不需要可执行权限的文件
            let skip_extensions = [
                "txt",
                "md",
                "json",
                "xml",
                "conf",
                "properties",
                "jar",
                "apk",
                "html",
                "css",
                "js",
                "png",
                "jpg",
                "ico",
                "svg",
                "woff",
                "woff2",
                "ttf",
            ];
            let should_skip = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| skip_extensions.contains(&ext))
                .unwrap_or(false);

            if !should_skip {
                let metadata = std::fs::metadata(&path)?;
                let mut perms = metadata.permissions();
                if perms.mode() & 0o111 == 0 {
                    perms.set_mode(perms.mode() | 0o755);
                    std::fs::set_permissions(&path, perms)?;
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

#[cfg(not(unix))]
pub fn ensure_all_executable_in_dir(_dir: &Path) -> std::io::Result<usize> {
    Ok(0)
}
