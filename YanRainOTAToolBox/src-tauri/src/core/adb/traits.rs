use crate::core::device::models::{AdbStatus, DeviceListResponse};
use crate::domain::command::CommandResult;
use crate::domain::device::DeviceInfo;
use crate::infra::errors::AppError;
use async_trait::async_trait;
use std::path::PathBuf;

/// 设备检测能力抽象
#[async_trait]
pub trait DeviceDetector: Send + Sync {
    /// 获取 ADB 二进制路径
    fn get_adb_path(&self) -> Result<PathBuf, AppError>;
    /// 获取 Fastboot 二进制路径
    fn get_fastboot_path(&self) -> Result<PathBuf, AppError>;
    /// 获取应用资源目录
    fn get_app_dir(&self) -> &PathBuf;
    /// 检查 ADB 可用状态
    async fn check_adb_status(&self) -> AdbStatus;
    /// 获取设备列表（ADB + Fastboot 合并）
    async fn list_devices(&self) -> Result<DeviceListResponse, AppError>;
    /// 获取缓存的设备列表
    fn get_cached_devices(&self) -> Vec<DeviceInfo>;
    /// 预热 ADB daemon
    fn warm_adb_daemon(&self);
}

/// ADB/Fastboot 命令执行能力抽象
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// 执行 ADB 命令（面向指定设备）
    async fn execute_adb_command(
        &self,
        device_id: Option<&str>,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<CommandResult, AppError>;

    /// 执行 Fastboot 命令（面向指定设备）
    async fn execute_fastboot_command(
        &self,
        device_id: &str,
        args: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<CommandResult, AppError>;

    /// 设置 Fastboot 忙碌标志（防止 USB 冲突）
    fn set_fastboot_busy(&self, busy: bool);
}
