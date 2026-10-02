use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 设备连接类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Tcpip,
}

/// 设备状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    Recovery,
    Bootloader,
    Sideload,
    Unknown(String),
}

impl From<&str> for DeviceState {
    fn from(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "device" => DeviceState::Device,
            "offline" => DeviceState::Offline,
            "unauthorized" => DeviceState::Unauthorized,
            "recovery" => DeviceState::Recovery,
            "bootloader" => DeviceState::Bootloader,
            "sideload" => DeviceState::Sideload,
            other => DeviceState::Unknown(other.to_string()),
        }
    }
}

/// 设备信息模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// 序列号
    pub id: String,
    /// 设备状态
    pub state: DeviceState,
    /// 设备型号
    pub model: Option<String>,
    /// 品牌
    pub brand: Option<String>,
    /// 产品名
    pub product: Option<String>,
    /// transport_id
    pub transport_id: Option<String>,
    /// 连接类型
    pub connection_type: ConnectionType,
    /// 最后发现时间
    pub last_seen_at: DateTime<Utc>,
}
