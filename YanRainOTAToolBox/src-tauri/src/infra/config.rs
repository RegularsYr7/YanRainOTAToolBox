use serde::{Deserialize, Serialize};

/// 应用全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// ADB 路径来源
    pub adb_path_source: AdbPathSource,
    /// 自定义 ADB 路径（仅在 adb_path_source 为 Custom 时使用）
    pub custom_adb_path: Option<String>,
    /// 命令默认超时时间（毫秒）
    pub default_timeout_ms: u64,
    /// 同设备最大并发任务数
    pub max_concurrent_per_device: usize,
    /// 不同设备最大并发任务数
    pub max_concurrent_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdbPathSource {
    /// 使用应用内置的 platform-tools
    Builtin,
    /// 使用用户自定义路径
    Custom,
    /// 使用系统 PATH
    SystemPath,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            adb_path_source: AdbPathSource::Builtin,
            custom_adb_path: None,
            default_timeout_ms: 30_000,
            max_concurrent_per_device: 1,
            max_concurrent_total: 4,
        }
    }
}
