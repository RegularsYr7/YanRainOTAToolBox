/// Windows 进程辅助工具
///
/// 在 Windows 上调用外部命令时，默认会弹出控制台窗口。
/// 此模块提供工具函数来隐藏这些窗口。

/// Windows CREATE_NO_WINDOW 标志值
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 为 tokio::process::Command 设置 Windows 隐藏窗口标志
///
/// 在 Windows 上调用 `creation_flags(CREATE_NO_WINDOW)` 隐藏控制台窗口；
/// 在其他平台上不做任何操作。
#[cfg(target_os = "windows")]
pub fn hide_console_window(cmd: &mut tokio::process::Command) {
    #[allow(unused_imports)]
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
pub fn hide_console_window(_cmd: &mut tokio::process::Command) {
    // non-Windows: no-op
}

/// 为 std::process::Command 设置 Windows 隐藏窗口标志
#[cfg(target_os = "windows")]
pub fn hide_console_window_std(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
pub fn hide_console_window_std(_cmd: &mut std::process::Command) {
    // non-Windows: no-op
}
