use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::core::device::service::DeviceService;
use crate::core::scrcpy::locator;
use crate::infra::errors::AppError;

/// 清理 Windows 扩展路径前缀 `\\?\`
///
/// scrcpy 等外部程序无法识别 `\\?\C:\...` 格式的路径，
/// 需要转换为标准格式 `C:\...`。
fn normalize_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
    } else {
        path.to_path_buf()
    }
}

/// scrcpy 启动配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrcpyConfig {
    /// 目标设备 ID
    pub device_id: String,
    /// 最大分辨率（宽度像素），0 为不限制
    pub max_size: u16,
    /// 帧率限制，0 为不限制
    pub max_fps: u16,
    /// 视频码率（bps），默认 8M
    pub video_bitrate: u32,
    /// 屏幕旋转角度：0/90/180/270
    pub rotation: u16,
    /// 是否全屏启动
    pub fullscreen: bool,
    /// 是否窗口置顶
    pub always_on_top: bool,
    /// 是否保持屏幕常亮
    pub stay_awake: bool,
    /// 是否使用设备原始角度
    pub no_rotation: bool,
    /// 是否同步剪贴板
    pub clipboard_sync: bool,
    /// 窗口高度（像素），不传=由 scrcpy 自行决定
    pub window_height: Option<u32>,
    /// 窗口宽度（像素），不传=由 scrcpy 自行决定
    pub window_width: Option<u32>,
}

impl Default for ScrcpyConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            max_size: 0,
            max_fps: 60,
            video_bitrate: 8_000_000,
            rotation: 0,
            fullscreen: false,
            always_on_top: false,
            stay_awake: false,
            no_rotation: false,
            clipboard_sync: true,
            window_height: None,
            window_width: None,
        }
    }
}

impl ScrcpyConfig {
    /// 将配置转换为 scrcpy 命令行参数
    pub fn to_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // 指定设备
        if !self.device_id.is_empty() {
            args.push("--serial".into());
            args.push(self.device_id.clone());
        }

        // 分辨率
        if self.max_size > 0 {
            args.push("--max-size".into());
            args.push(self.max_size.to_string());
        }

        // 帧率
        if self.max_fps > 0 {
            args.push("--max-fps".into());
            args.push(self.max_fps.to_string());
        }

        // 码率
        args.push("--video-bit-rate".into());
        args.push(format!("{}M", self.video_bitrate / 1_000_000));

        // 旋转（scrcpy 3.x 使用 --capture-orientation）
        if self.rotation > 0 {
            args.push("--capture-orientation".into());
            args.push(format!("@{}", self.rotation));
        }

        // 全屏
        if self.fullscreen {
            args.push("--fullscreen".into());
        }

        // 窗口置顶
        if self.always_on_top {
            args.push("--always-on-top".into());
        }

        // 屏幕常亮
        if self.stay_awake {
            args.push("--stay-awake".into());
        }

        // 禁用旋转（保持原始角度）
        if self.no_rotation {
            args.push("--capture-orientation".into());
            args.push("@".into());
        }

        // 剪贴板同步
        if !self.clipboard_sync {
            args.push("--no-clipboard-autosync".into());
        }

        // 窗口尺寸（仅设置高度时宽度由 scrcpy 按手机宽高比自动计算）
        if let Some(h) = self.window_height {
            args.push("--window-height".into());
            args.push(h.to_string());
        }
        if let Some(w) = self.window_width {
            args.push("--window-width".into());
            args.push(w.to_string());
        }

        args
    }
}

/// scrcpy 进程状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrcpyStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// scrcpy 进程管理器
///
/// 管理所有设备的 scrcpy 进程生命周期
pub struct ScrcpyManager {
    /// 运行中的 scrcpy 进程 (device_id -> child process)
    processes: Arc<RwLock<HashMap<String, u32>>>,
    /// 进程状态
    statuses: Arc<RwLock<HashMap<String, ScrcpyStatus>>>,
    /// 设备服务（用于获取 ADB 路径）
    device_service: Arc<DeviceService>,
    /// 应用资源目录
    app_dir: std::path::PathBuf,
}

impl ScrcpyManager {
    pub fn new(device_service: Arc<DeviceService>, app_dir: std::path::PathBuf) -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            statuses: Arc::new(RwLock::new(HashMap::new())),
            device_service,
            app_dir,
        }
    }

    /// 启动 scrcpy 投屏
    pub async fn start(&self, config: ScrcpyConfig) -> Result<String, AppError> {
        let device_id = config.device_id.clone();

        // 检查是否已在运行
        {
            let statuses = self.statuses.read();
            if let Some(ScrcpyStatus::Running) = statuses.get(&device_id) {
                return Err(AppError::CommandFailed(format!(
                    "scrcpy is already running for device {}",
                    device_id
                )));
            }
        }

        // 设置状态为启动中
        {
            let mut statuses = self.statuses.write();
            statuses.insert(device_id.clone(), ScrcpyStatus::Starting);
        }

        // 定位 scrcpy
        let scrcpy_path_raw = locator::locate_scrcpy(&self.app_dir)?;
        let scrcpy_path = normalize_path(&scrcpy_path_raw);
        let scrcpy_dir = normalize_path(scrcpy_path.parent().unwrap_or(Path::new(".")));

        // 获取 ADB 路径，让 scrcpy 使用我们的 ADB
        let adb_path_raw = self.device_service.get_adb_path()?;
        let adb_path = normalize_path(&adb_path_raw);

        // 定位 scrcpy-server
        let server_path =
            locator::locate_scrcpy_server(&scrcpy_path_raw).map(|p| normalize_path(&p));

        // 构建参数
        let args = config.to_args();

        info!("Starting scrcpy: {:?} {:?}", scrcpy_path, args);

        // 启动进程
        let mut cmd = Command::new(&scrcpy_path);
        cmd.args(&args)
            .current_dir(&scrcpy_dir)
            .env("ADB", adb_path.to_string_lossy().to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // 设置 scrcpy 窗口图标为应用自有图标（公共 icon.png 位于 scrcpy 根目录）
        // scrcpy_dir = resources/scrcpy/<platform>/<arch>/，icon 在 resources/scrcpy/icon.png
        let icon_path = scrcpy_dir.join("..").join("..").join("icon.png");
        if icon_path.exists() {
            let icon_path = normalize_path(&icon_path.canonicalize().unwrap_or(icon_path));
            info!("Setting SCRCPY_ICON_PATH={:?}", icon_path);
            cmd.env("SCRCPY_ICON_PATH", icon_path.to_string_lossy().to_string());
        }

        // 设置 scrcpy-server 路径（Linux/macOS 自带的 scrcpy 需要此环境变量）
        if let Some(ref sp) = server_path {
            info!("Setting SCRCPY_SERVER_PATH={:?}", sp);
            cmd.env("SCRCPY_SERVER_PATH", sp.to_string_lossy().to_string());
        }

        // Windows 下隐藏控制台窗口
        crate::infra::process::hide_console_window(&mut cmd);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id().unwrap_or(0);
                info!("scrcpy started with PID: {}", pid);

                {
                    let mut processes = self.processes.write();
                    processes.insert(device_id.clone(), pid);
                }
                {
                    let mut statuses = self.statuses.write();
                    statuses.insert(device_id.clone(), ScrcpyStatus::Running);
                }

                // 在后台监控进程退出，并捕获 stdout/stderr 用于诊断
                let processes = self.processes.clone();
                let statuses = self.statuses.clone();
                let dev_id = device_id.clone();
                tokio::spawn(async move {
                    let mut child_process = child;

                    // 先取出 stderr/stdout 句柄，避免进程退出后无法读取
                    let stderr_handle = child_process.stderr.take();
                    let stdout_handle = child_process.stdout.take();

                    let status = child_process.wait().await;
                    info!("scrcpy process for {} exited: {:?}", dev_id, status);

                    // 进程非正常退出时，尝试读取 stderr/stdout 帮助诊断
                    let exited_abnormally = match &status {
                        Ok(s) => !s.success(),
                        Err(_) => true,
                    };
                    if exited_abnormally {
                        if let Some(mut stderr) = stderr_handle {
                            let mut buf = String::new();
                            if let Ok(_) =
                                tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut buf).await
                            {
                                if !buf.trim().is_empty() {
                                    error!("scrcpy stderr for {}: {}", dev_id, buf.trim());
                                }
                            }
                        }
                        if let Some(mut stdout) = stdout_handle {
                            let mut buf = String::new();
                            if let Ok(_) =
                                tokio::io::AsyncReadExt::read_to_string(&mut stdout, &mut buf).await
                            {
                                if !buf.trim().is_empty() {
                                    warn!("scrcpy stdout for {}: {}", dev_id, buf.trim());
                                }
                            }
                        }
                    }

                    {
                        let mut processes = processes.write();
                        processes.remove(&dev_id);
                    }
                    {
                        let mut statuses = statuses.write();
                        statuses.insert(dev_id, ScrcpyStatus::Stopped);
                    }
                });

                Ok(format!("scrcpy started (PID: {})", pid))
            }
            Err(e) => {
                error!("Failed to start scrcpy: {}", e);
                {
                    let mut statuses = self.statuses.write();
                    statuses.insert(device_id, ScrcpyStatus::Error(e.to_string()));
                }
                Err(AppError::CommandFailed(format!(
                    "Failed to start scrcpy: {}",
                    e
                )))
            }
        }
    }

    /// 停止指定设备的 scrcpy
    pub async fn stop(&self, device_id: &str) -> Result<String, AppError> {
        let pid = {
            let processes = self.processes.read();
            processes.get(device_id).copied()
        };

        if let Some(pid) = pid {
            info!("Stopping scrcpy for device {} (PID: {})", device_id, pid);

            // 跨平台杀进程
            #[cfg(target_os = "windows")]
            {
                let mut cmd = std::process::Command::new("taskkill");
                cmd.args(["/F", "/PID", &pid.to_string()]);
                crate::infra::process::hide_console_window_std(&mut cmd);
                let _ = cmd.output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }

            {
                let mut processes = self.processes.write();
                processes.remove(device_id);
            }
            {
                let mut statuses = self.statuses.write();
                statuses.insert(device_id.to_string(), ScrcpyStatus::Stopped);
            }

            Ok("scrcpy stopped".to_string())
        } else {
            warn!("No scrcpy process found for device {}", device_id);
            Ok("scrcpy is not running".to_string())
        }
    }

    /// 停止所有 scrcpy 进程
    pub async fn stop_all(&self) -> Result<String, AppError> {
        let device_ids: Vec<String> = {
            let processes = self.processes.read();
            processes.keys().cloned().collect()
        };

        for device_id in &device_ids {
            let _ = self.stop(device_id).await;
        }

        Ok(format!("Stopped {} scrcpy processes", device_ids.len()))
    }

    /// 获取指定设备的 scrcpy 状态
    pub fn get_status(&self, device_id: &str) -> ScrcpyStatus {
        let statuses = self.statuses.read();
        statuses
            .get(device_id)
            .cloned()
            .unwrap_or(ScrcpyStatus::Stopped)
    }

    /// 获取所有正在运行 scrcpy 的设备 ID 列表
    pub fn get_running_device_ids(&self) -> Vec<String> {
        let statuses = self.statuses.read();
        statuses
            .iter()
            .filter(|(_, s)| matches!(s, ScrcpyStatus::Running | ScrcpyStatus::Starting))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 通过 ADB 发送按键事件
    pub async fn send_key_event(&self, device_id: &str, keycode: i32) -> Result<(), AppError> {
        self.device_service
            .execute_adb_command(
                Some(device_id),
                &[
                    "shell".into(),
                    "input".into(),
                    "keyevent".into(),
                    keycode.to_string(),
                ],
                None,
            )
            .await?;
        Ok(())
    }

    /// 设置屏幕旋转
    ///
    /// orientation: 0=自动, 1=竖屏, 2=横屏
    pub async fn set_rotation(&self, device_id: &str, orientation: u8) -> Result<(), AppError> {
        match orientation {
            0 => {
                // 自动旋转：打开加速计旋转
                self.device_service
                    .execute_adb_command(
                        Some(device_id),
                        &[
                            "shell".into(),
                            "settings".into(),
                            "put".into(),
                            "system".into(),
                            "accelerometer_rotation".into(),
                            "1".into(),
                        ],
                        None,
                    )
                    .await?;
            }
            _ => {
                // 关闭自动旋转
                self.device_service
                    .execute_adb_command(
                        Some(device_id),
                        &[
                            "shell".into(),
                            "settings".into(),
                            "put".into(),
                            "system".into(),
                            "accelerometer_rotation".into(),
                            "0".into(),
                        ],
                        None,
                    )
                    .await?;
                // 设置固定方向: 0=竖屏, 1=横屏, 2=反向竖屏, 3=反向横屏
                let rotation_value = match orientation {
                    1 => "0", // 竖屏
                    2 => "1", // 横屏
                    _ => "0",
                };
                self.device_service
                    .execute_adb_command(
                        Some(device_id),
                        &[
                            "shell".into(),
                            "settings".into(),
                            "put".into(),
                            "system".into(),
                            "user_rotation".into(),
                            rotation_value.into(),
                        ],
                        None,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// 截屏并保存到设备
    pub async fn take_screenshot(&self, device_id: &str) -> Result<String, AppError> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let remote_path = format!("/sdcard/Screenshots/screenshot_{}.png", timestamp);

        self.device_service
            .execute_adb_command(
                Some(device_id),
                &[
                    "shell".into(),
                    "screencap".into(),
                    "-p".into(),
                    remote_path.clone(),
                ],
                None,
            )
            .await?;

        Ok(remote_path)
    }
}
