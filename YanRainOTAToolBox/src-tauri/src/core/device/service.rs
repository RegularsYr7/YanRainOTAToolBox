use async_trait::async_trait;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, trace, warn};

use crate::core::adb::traits::{CommandExecutor, DeviceDetector};
use crate::core::adb::{binary_locator, executor, parser};
use crate::core::device::models::{AdbStatus, DeviceListResponse};
use crate::domain::command::CommandResult;
use crate::domain::device::{DeviceInfo, DeviceState};
use crate::infra::config::AppConfig;
use crate::infra::errors::AppError;

/// 设备服务 - 管理设备查询和状态缓存
pub struct DeviceService {
    config: Arc<RwLock<AppConfig>>,
    /// 缓存的设备列表
    cached_devices: Arc<RwLock<Vec<DeviceInfo>>>,
    /// 应用根目录（用于定位内置 platform-tools）
    app_dir: PathBuf,
    /// ADB daemon 预热完成信号（watch channel，false=未就绪，true=已就绪）
    adb_ready_rx: tokio::sync::watch::Receiver<bool>,
    adb_ready_tx: Arc<tokio::sync::watch::Sender<bool>>,
    /// Fastboot 操作锁标志：为 true 时轮询跳过 fastboot devices 扫描，避免 USB 独占冲突
    fastboot_busy: AtomicBool,
}

impl DeviceService {
    pub fn new(config: Arc<RwLock<AppConfig>>, app_dir: PathBuf) -> Self {
        let (tx, rx) = tokio::sync::watch::channel(false);
        Self {
            config,
            cached_devices: Arc::new(RwLock::new(Vec::new())),
            app_dir,
            adb_ready_rx: rx,
            adb_ready_tx: Arc::new(tx),
            fastboot_busy: AtomicBool::new(false),
        }
    }

    /// 获取 ADB 路径
    pub fn get_adb_path(&self) -> Result<PathBuf, AppError> {
        let config = self.config.read();
        binary_locator::auto_locate_adb(&config, &self.app_dir)
    }

    /// 获取应用资源目录
    pub fn get_app_dir(&self) -> &PathBuf {
        &self.app_dir
    }

    /// 获取 Fastboot 路径
    pub fn get_fastboot_path(&self) -> Result<PathBuf, AppError> {
        let config = self.config.read();
        binary_locator::auto_locate_fastboot(&config, &self.app_dir)
    }

    /// 检查 ADB 状态
    pub async fn check_adb_status(&self) -> AdbStatus {
        // 等待 ADB daemon 预热完成
        self.wait_adb_ready().await;

        match self.get_adb_path() {
            Ok(adb_path) => match binary_locator::verify_adb(&adb_path).await {
                Ok(version) => AdbStatus {
                    available: true,
                    version: Some(version),
                    path: Some(adb_path.to_string_lossy().to_string()),
                    error: None,
                },
                Err(e) => AdbStatus {
                    available: false,
                    version: None,
                    path: Some(adb_path.to_string_lossy().to_string()),
                    error: Some(e.to_string()),
                },
            },
            Err(e) => AdbStatus {
                available: false,
                version: None,
                path: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// 预热 ADB daemon（后台执行 `adb start-server`）
    ///
    /// ADB daemon 首次启动需要 2-3 秒，在 setup() 中尽早调用此方法，
    /// 等前端真正需要 `adb devices` 时 daemon 已就绪，消除冷启动延迟。
    /// 其他调用方（polling、前端命令）会通过 `wait_adb_ready()` 等待此方法完成。
    pub fn warm_adb_daemon(self: &Arc<Self>) {
        let adb_path = match self.get_adb_path() {
            Ok(p) => p,
            Err(e) => {
                warn!("Cannot warm ADB daemon: {}", e);
                // 即使失败也标记为就绪，让后续命令正常尝试
                let _ = self.adb_ready_tx.send(true);
                return;
            }
        };

        let tx = self.adb_ready_tx.clone();
        tauri::async_runtime::spawn(async move {
            tracing::info!("🔥 Pre-warming ADB daemon...");
            let mut cmd = tokio::process::Command::new(&adb_path);
            cmd.arg("start-server");
            crate::infra::process::hide_console_window(&mut cmd);

            // 注入库搜索路径（Linux/macOS）
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

            match cmd.output().await {
                Ok(output) if output.status.success() => {
                    tracing::info!("✅ ADB daemon ready");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("ADB start-server exited with error: {}", stderr);
                }
                Err(e) => {
                    warn!("Failed to start ADB daemon: {}", e);
                }
            }

            let _ = tx.send(true);
        });
    }

    /// 等待 ADB daemon 预热完成
    ///
    /// 如果 daemon 已就绪则立即返回，否则最多等待 10 秒。
    /// 超时后标记为就绪（让后续命令自行尝试），避免无限阻塞。
    async fn wait_adb_ready(&self) {
        let mut rx = self.adb_ready_rx.clone();
        // 如果当前值已经是 true，直接返回
        if *rx.borrow() {
            return;
        }
        // 最多等待 10 秒，超时后强制放行
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx.wait_for(|ready| *ready),
        )
        .await
        {
            Ok(_) => {}
            Err(_) => {
                warn!("wait_adb_ready timed out after 10s, proceeding anyway");
                // 标记为就绪，避免后续调用再次等待
                let _ = self.adb_ready_tx.send(true);
            }
        };
    }

    /// 公开版本的等待 ADB daemon 预热完成（供 polling 模块使用）
    pub async fn wait_adb_ready_public(&self) {
        self.wait_adb_ready().await;
    }

    /// 获取设备列表（ADB + Fastboot 合并）
    pub async fn list_devices(&self) -> Result<DeviceListResponse, AppError> {
        // 等待 ADB daemon 预热完成，避免与 adb start-server 并发冲突
        self.wait_adb_ready().await;

        let timeout = {
            let config = self.config.read();
            config.default_timeout_ms
        };

        // 1. ADB 设备
        let adb_path = self.get_adb_path()?;
        let adb_args = vec!["devices".to_string(), "-l".to_string()];
        let adb_result = executor::execute_command(&adb_path, &adb_args, timeout).await?;
        let mut devices = parser::parse_device_list(&adb_result.stdout)?;

        // 2. Fastboot 设备（尽力而为，失败不影响 ADB 结果）
        // 如果 Fastboot 操作正在进行，跳过扫描，保留缓存中的 Fastboot 设备
        if self.fastboot_busy.load(Ordering::Relaxed) {
            trace!("Fastboot busy, skipping fastboot devices scan, preserving cached entries");
            let cache = self.cached_devices.read();
            for cached_dev in cache.iter() {
                if cached_dev.state == DeviceState::Bootloader
                    && !devices.iter().any(|d| d.id == cached_dev.id)
                {
                    devices.push(cached_dev.clone());
                }
            }
        } else {
            match self.get_fastboot_path() {
                Ok(fb_path) => {
                    let fb_args = vec!["devices".to_string(), "-l".to_string()];
                    match executor::execute_command(&fb_path, &fb_args, timeout).await {
                        Ok(fb_result) => {
                            let fb_devices = parser::parse_fastboot_device_list(&fb_result.stdout);
                            trace!("Found {} fastboot devices", fb_devices.len());
                            // 合并：去重（同一序列号不重复添加）
                            for fb_dev in fb_devices {
                                if !devices.iter().any(|d| d.id == fb_dev.id) {
                                    devices.push(fb_dev);
                                }
                            }
                        }
                        Err(e) => {
                            trace!("Fastboot devices query failed (non-critical): {}", e);
                        }
                    }
                }
                Err(e) => {
                    trace!("Fastboot not available (non-critical): {}", e);
                }
            }
        }

        let total = devices.len();

        // 更新缓存
        {
            let mut cache = self.cached_devices.write();
            *cache = devices.clone();
        }

        trace!("Found {} devices total (adb + fastboot)", total);
        Ok(DeviceListResponse { devices, total })
    }

    /// 获取缓存的设备列表
    pub fn get_cached_devices(&self) -> Vec<DeviceInfo> {
        self.cached_devices.read().clone()
    }

    /// 外部设置 fastboot_busy 标志（用于绕过 execute_fastboot_command 的场景）
    pub fn set_fastboot_busy(&self, busy: bool) {
        self.fastboot_busy.store(busy, Ordering::Relaxed);
    }

    /// 根据设备 ID 从缓存中查找设备信息
    fn find_cached_device(&self, device_id: &str) -> Option<DeviceInfo> {
        self.cached_devices
            .read()
            .iter()
            .find(|d| d.id == device_id)
            .cloned()
    }

    /// 校验设备是否适合执行 ADB 命令
    /// ADB 命令要求设备处于 Device / Recovery / Sideload 状态
    fn validate_device_for_adb(&self, device_id: &str) -> Result<(), AppError> {
        if let Some(device) = self.find_cached_device(device_id) {
            match &device.state {
                DeviceState::Bootloader => {
                    return Err(AppError::DeviceStateMismatch(
                        format!(
                            "设备 {} 当前处于 Fastboot 模式，无法执行 ADB 命令。请长按电源键重启到系统后再试。",
                            device_id
                        )
                    ));
                }
                DeviceState::Offline => {
                    return Err(AppError::DeviceOffline(format!(
                        "设备 {} 已离线，请检查连接后重试。",
                        device_id
                    )));
                }
                DeviceState::Unauthorized => {
                    return Err(AppError::DeviceUnauthorized(format!(
                        "设备 {} 未授权调试，请在设备上确认「允许 USB 调试」弹窗。",
                        device_id
                    )));
                }
                DeviceState::Unknown(s) => {
                    warn!(
                        "Device {} in unknown state '{}', proceeding with ADB",
                        device_id, s
                    );
                }
                // Device / Recovery / Sideload 均可执行 ADB 命令
                _ => {}
            }
        }
        // 缓存中没有该设备时不阻止（可能是新插入的设备）
        Ok(())
    }

    /// 校验设备是否适合执行 Fastboot 命令
    /// Fastboot 命令要求设备处于 Bootloader 状态
    fn validate_device_for_fastboot(&self, device_id: &str) -> Result<(), AppError> {
        if let Some(device) = self.find_cached_device(device_id) {
            match &device.state {
                DeviceState::Device => {
                    return Err(AppError::DeviceStateMismatch(
                        format!(
                            "设备 {} 当前在系统中运行，无法执行 Fastboot 命令。请先执行「重启到 Bootloader」或手动进入 Fastboot 模式。",
                            device_id
                        )
                    ));
                }
                DeviceState::Recovery => {
                    return Err(AppError::DeviceStateMismatch(
                        format!(
                            "设备 {} 当前处于 Recovery 模式，无法执行 Fastboot 命令。请先重启到 Bootloader / Fastboot 模式。",
                            device_id
                        )
                    ));
                }
                DeviceState::Sideload => {
                    return Err(AppError::DeviceStateMismatch(
                        format!(
                            "设备 {} 当前处于 Sideload 模式，无法执行 Fastboot 命令。请先重启到 Bootloader / Fastboot 模式。",
                            device_id
                        )
                    ));
                }
                DeviceState::Offline => {
                    return Err(AppError::DeviceOffline(format!(
                        "设备 {} 已离线，请检查连接后重试。",
                        device_id
                    )));
                }
                DeviceState::Unauthorized => {
                    return Err(AppError::DeviceUnauthorized(format!(
                        "设备 {} 未授权调试，请在设备上确认「允许 USB 调试」弹窗后重试。",
                        device_id
                    )));
                }
                DeviceState::Unknown(s) => {
                    warn!(
                        "Device {} in unknown state '{}', proceeding with Fastboot",
                        device_id, s
                    );
                }
                // Bootloader 状态正常
                DeviceState::Bootloader => {}
            }
        }
        Ok(())
    }

    /// 执行指定设备的 ADB 命令（自动校验设备状态 + 强制 -s 指向设备）
    pub async fn execute_adb_command(
        &self,
        device_id: Option<&str>,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<crate::domain::command::CommandResult, AppError> {
        // 等待 ADB daemon 预热完成
        self.wait_adb_ready().await;

        // 校验设备状态（disconnect 不需要设备在线）
        let skip_validation = args.first().map(|a| a.as_str()) == Some("disconnect");
        if let Some(serial) = device_id {
            if !skip_validation {
                self.validate_device_for_adb(serial)?;
            }
        }

        let adb_path = self.get_adb_path()?;
        let timeout = {
            let config = self.config.read();
            timeout_ms.unwrap_or(config.default_timeout_ms)
        };

        let mut full_args: Vec<String> = Vec::new();

        // 指定了设备时始终添加 -s 参数，避免多设备时 "more than one device/emulator" 错误
        if let Some(serial) = device_id {
            full_args.push("-s".to_string());
            full_args.push(serial.to_string());
        }
        full_args.extend(args.iter().cloned());

        let mut result = executor::execute_command(&adb_path, &full_args, timeout).await?;
        result.task_id = uuid::Uuid::new_v4().to_string();

        if result.exit_code != 0 && !result.stderr.is_empty() {
            error!("ADB command failed: {}", result.stderr);
        }

        Ok(result)
    }

    /// 执行指定设备的 Fastboot 命令（自动校验设备状态 + 强制 -s 指向设备）
    pub async fn execute_fastboot_command(
        &self,
        device_id: &str,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<crate::domain::command::CommandResult, AppError> {
        // 提前标记 Fastboot 忙碌，阻止轮询扫描 fastboot devices 导致 USB 冲突
        self.fastboot_busy.store(true, Ordering::Relaxed);

        // 校验设备状态（失败时重置忙碌标记）
        if let Err(e) = self.validate_device_for_fastboot(device_id) {
            self.fastboot_busy.store(false, Ordering::Relaxed);
            return Err(e);
        }

        let fastboot_path = match self.get_fastboot_path() {
            Ok(p) => p,
            Err(e) => {
                self.fastboot_busy.store(false, Ordering::Relaxed);
                return Err(e);
            }
        };
        let timeout = {
            let config = self.config.read();
            timeout_ms.unwrap_or(config.default_timeout_ms)
        };

        // 多 Fastboot 设备时添加 -s 参数指向目标设备，单设备时省略（部分设备在 Fastboot 下读不出序列号）
        let mut full_args: Vec<String> = Vec::new();
        let fastboot_count = self
            .cached_devices
            .read()
            .iter()
            .filter(|d| matches!(d.state, crate::domain::device::DeviceState::Bootloader))
            .count();
        if fastboot_count > 1 {
            full_args.push("-s".to_string());
            full_args.push(device_id.to_string());
        }
        full_args.extend(args.iter().cloned());

        let result = executor::execute_command(&fastboot_path, &full_args, timeout).await;
        self.fastboot_busy.store(false, Ordering::Relaxed);

        let mut result = result?;
        result.task_id = uuid::Uuid::new_v4().to_string();

        if result.exit_code != 0 && !result.stderr.is_empty() {
            error!("Fastboot command failed: {}", result.stderr);
        }

        Ok(result)
    }
}

#[async_trait]
impl DeviceDetector for DeviceService {
    fn get_adb_path(&self) -> Result<PathBuf, AppError> {
        self.get_adb_path()
    }

    fn get_fastboot_path(&self) -> Result<PathBuf, AppError> {
        self.get_fastboot_path()
    }

    fn get_app_dir(&self) -> &PathBuf {
        self.get_app_dir()
    }

    async fn check_adb_status(&self) -> AdbStatus {
        self.check_adb_status().await
    }

    async fn list_devices(&self) -> Result<DeviceListResponse, AppError> {
        self.list_devices().await
    }

    fn get_cached_devices(&self) -> Vec<DeviceInfo> {
        self.get_cached_devices()
    }

    fn warm_adb_daemon(&self) {
        let adb_path = match self.get_adb_path() {
            Ok(p) => p,
            Err(e) => {
                warn!("Cannot warm ADB daemon: {}", e);
                let _ = self.adb_ready_tx.send(true);
                return;
            }
        };

        let tx = self.adb_ready_tx.clone();
        tauri::async_runtime::spawn(async move {
            tracing::info!("🔥 Pre-warming ADB daemon...");
            let mut cmd = tokio::process::Command::new(&adb_path);
            cmd.arg("start-server");
            crate::infra::process::hide_console_window(&mut cmd);

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

            match cmd.output().await {
                Ok(output) if output.status.success() => {
                    tracing::info!("✅ ADB daemon ready");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("ADB start-server exited with error: {}", stderr);
                }
                Err(e) => {
                    warn!("Failed to start ADB daemon: {}", e);
                }
            }

            let _ = tx.send(true);
        });
    }
}

#[async_trait]
impl CommandExecutor for DeviceService {
    async fn execute_adb_command(
        &self,
        device_id: Option<&str>,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<CommandResult, AppError> {
        self.execute_adb_command(device_id, args, timeout_ms).await
    }

    async fn execute_fastboot_command(
        &self,
        device_id: &str,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<CommandResult, AppError> {
        self.execute_fastboot_command(device_id, args, timeout_ms)
            .await
    }

    fn set_fastboot_busy(&self, busy: bool) {
        self.set_fastboot_busy(busy);
    }
}
