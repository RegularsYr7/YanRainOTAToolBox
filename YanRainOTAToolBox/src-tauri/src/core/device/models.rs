// 设备相关的额外 DTO 模型（补充 domain::device）
use crate::domain::device::DeviceInfo;
use serde::{Deserialize, Serialize};

/// 设备列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
    pub total: usize,
}

/// ADB 状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdbStatus {
    /// ADB 是否可用
    pub available: bool,
    /// ADB 版本
    pub version: Option<String>,
    /// ADB 路径
    pub path: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

/// 设备详细信息（仪表盘展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDetail {
    /// 设备名称（市场名）
    pub device_name: String,
    /// 设备代号
    pub device_codename: String,
    /// 安卓版本
    pub android_version: String,
    /// 解锁状态
    pub unlock_status: String,
    /// A/B 分区
    pub ab_partition: String,
    /// 内核版本
    pub kernel_version: String,
    /// 构建日期
    pub build_date: String,
    /// 版本信息 (build display id)
    pub build_display: String,
    /// CPU 厂家
    pub cpu_vendor: String,
    /// CPU 代号
    pub cpu_codename: String,
    /// CPU 名称
    pub cpu_name: String,
    /// 操作系统
    pub os_version: String,
    /// 主板 ID（SoC 序列号）
    pub board_id: String,
    /// CPU 架构 (如 arm64-v8a)
    pub cpu_abi: String,
    /// 屏幕分辨率 (如 "1220x2656")
    pub screen_resolution: String,
    /// 品牌
    pub brand: String,
    /// VNDK 版本
    pub vndk_version: String,
    /// 开机时长（秒）
    pub power_on_time: Option<u64>,
    /// 显示密度 (DPI)
    pub density: String,
    /// 硬件平台 (从 /proc/cpuinfo Hardware 行)
    pub platform: String,
    /// 编译版本号 (incremental)
    pub compile_version: String,
}

/// CPU 频率信息（每个核心的详细频率信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuFreqInfo {
    /// 核心编号 (cpu0, cpu1, ...)
    pub core: String,
    /// 当前频率 (MHz)
    pub freq_mhz: u64,
    /// 最小频率 (MHz)
    pub min_freq_mhz: Option<u64>,
    /// 最大频率 (MHz)
    pub max_freq_mhz: Option<u64>,
    /// 可用频率列表 (MHz)
    pub available_frequencies: Vec<u64>,
    /// 当前调度器
    pub governor: Option<String>,
}

/// 存储信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// 总字节数
    pub total_bytes: u64,
    /// 已用字节数
    pub used_bytes: u64,
}

/// 内存信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// 总字节数
    pub total_bytes: u64,
    /// 已用字节数
    pub used_bytes: u64,
}

/// 电池信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    /// 电量百分比 (0-100, -1 表示未知)
    pub level: i32,
    /// 温度（摄氏度）
    pub temperature: f64,
}
