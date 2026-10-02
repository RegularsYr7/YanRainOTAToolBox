use chrono::Utc;
use tracing::trace;

use crate::domain::device::{ConnectionType, DeviceInfo, DeviceState};
use crate::infra::errors::AppError;

/// 解析 `adb devices -l` 的输出
///
/// 示例输出：
/// ```text
/// List of devices attached
/// ABCDEF1234     device usb:1-2 product:walleye model:Pixel_2 device:walleye transport_id:1
/// 192.168.1.100:9527 device product:raven model:Pixel_6_Pro device:raven transport_id:3
/// GHIJKL5678     unauthorized usb:1-3 transport_id:2
/// ```
pub fn parse_device_list(output: &str) -> Result<Vec<DeviceInfo>, AppError> {
    let mut devices = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // 跳过空行和标题行
        if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
            continue;
        }

        if let Some(device) = parse_device_line(line) {
            devices.push(device);
        }
    }

    // 去重：同一物理设备可能同时以 IP:port 和 mDNS 服务名出现
    // 当两个无线设备 model 相同时，优先保留 IP:port 条目（更直观可控）
    dedup_wireless_devices(&mut devices);

    trace!("Parsed {} devices", devices.len());
    Ok(devices)
}

/// 判断设备 ID 是否为 mDNS 自动发现的服务名（如 `adb-xxx._adb-tls-connect._tcp`）
fn is_mdns_id(id: &str) -> bool {
    id.contains("_adb-tls-")
}

/// 去重：同一设备通过多种方式同时出现时，只保留最优条目
/// 场景 1：mDNS 服务名（adb-xxx._adb-tls-connect._tcp）+ IP:port → 保留 IP:port
/// 场景 2：序列号（如 25098PN5AC）+ IP:port（如 10.11.23.216:34075）→ 保留 IP:port
fn dedup_wireless_devices(devices: &mut Vec<DeviceInfo>) {
    // 收集所有 IP:port 无线设备的 model
    let ip_models: std::collections::HashSet<String> = devices
        .iter()
        .filter(|d| {
            d.connection_type == ConnectionType::Tcpip && !is_mdns_id(&d.id) && d.model.is_some()
        })
        .filter_map(|d| d.model.clone())
        .collect();

    if ip_models.is_empty() {
        return;
    }

    let before = devices.len();
    devices.retain(|d| {
        // IP:port 设备始终保留
        if d.connection_type == ConnectionType::Tcpip && !is_mdns_id(&d.id) {
            return true;
        }
        // mDNS 设备或序列号设备：如果已有同 model 的 IP:port 设备，则移除
        match &d.model {
            Some(m) => !ip_models.contains(m),
            None => true,
        }
    });

    let removed = before - devices.len();
    if removed > 0 {
        trace!(
            "Deduped {} duplicate device(s) with existing IP:port connection",
            removed
        );
    }
}

fn parse_device_line(line: &str) -> Option<DeviceInfo> {
    let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 {
        return None;
    }

    let id = parts[0].to_string();
    let rest = parts[1].trim();

    // 获取状态（rest 第一个单词）
    let state_and_props: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
    let state = DeviceState::from(state_and_props[0]);

    // 判断连接类型：IP:port 或 mDNS 服务名（adb-xxx._adb-tls-*）均为无线
    let connection_type = if id.contains(':') || id.contains("_adb-tls-") {
        ConnectionType::Tcpip
    } else {
        ConnectionType::Usb
    };

    // 解析属性
    let props_str = if state_and_props.len() > 1 {
        state_and_props[1]
    } else {
        ""
    };

    let mut model = None;
    let mut brand = None;
    let mut product = None;
    let mut transport_id = None;

    for token in props_str.split_whitespace() {
        if let Some((key, value)) = token.split_once(':') {
            match key {
                "model" => model = Some(value.replace('_', " ")),
                "brand" => brand = Some(value.to_string()),
                "product" => product = Some(value.to_string()),
                "transport_id" => transport_id = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Some(DeviceInfo {
        id,
        state,
        model,
        brand,
        product,
        transport_id,
        connection_type,
        last_seen_at: Utc::now(),
    })
}

/// 解析 `fastboot devices -l` 的输出
///
/// 示例输出：
/// ```text
/// ABCDEF1234     fastboot usb:1-2
/// GHIJKL5678     fastboot
/// ```
/// 或带 -l 参数时：
/// ```text
/// ABCDEF1234     fastboot usb:3-1.2
/// ```
pub fn parse_fastboot_device_list(output: &str) -> Vec<DeviceInfo> {
    let mut devices = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let id = parts[0].to_string();

        // 判断连接类型：IP:port 或 mDNS 服务名均为无线
        let connection_type = if id.contains(':') || id.contains("_adb-tls-") {
            ConnectionType::Tcpip
        } else {
            ConnectionType::Usb
        };

        devices.push(DeviceInfo {
            id,
            state: DeviceState::Bootloader,
            model: None,
            brand: None,
            product: None,
            transport_id: None,
            connection_type,
            last_seen_at: Utc::now(),
        });
    }

    trace!("Parsed {} fastboot devices", devices.len());
    devices
}

/// 解析 `adb version` 输出，提取版本号
pub fn parse_adb_version(output: &str) -> Option<String> {
    // 示例: "Android Debug Bridge version 1.0.41"
    for line in output.lines() {
        if line.contains("Android Debug Bridge version") {
            return Some(line.trim().to_string());
        }
    }
    // 如果找不到特定格式，返回第一行
    output.lines().next().map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_device_list_basic() {
        let output = r#"List of devices attached
ABCDEF1234     device usb:1-2 product:walleye model:Pixel_2 device:walleye transport_id:1
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "ABCDEF1234");
        assert_eq!(devices[0].state, DeviceState::Device);
        assert_eq!(devices[0].model, Some("Pixel 2".to_string()));
        assert_eq!(devices[0].connection_type, ConnectionType::Usb);
    }

    #[test]
    fn test_parse_device_list_tcpip() {
        let output = r#"List of devices attached
192.168.1.100:9527 device product:raven model:Pixel_6_Pro device:raven transport_id:3
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "192.168.1.100:9527");
        assert_eq!(devices[0].connection_type, ConnectionType::Tcpip);
    }

    #[test]
    fn test_parse_device_list_unauthorized() {
        let output = r#"List of devices attached
GHIJKL5678     unauthorized usb:1-3 transport_id:2
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].state, DeviceState::Unauthorized);
    }

    #[test]
    fn test_parse_empty_output() {
        let output = "List of devices attached\n\n";
        let devices = parse_device_list(output).unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_parse_adb_version() {
        let output = "Android Debug Bridge version 1.0.41\nVersion 34.0.5-10900879\nInstalled as /usr/bin/adb";
        let version = parse_adb_version(output);
        assert_eq!(
            version,
            Some("Android Debug Bridge version 1.0.41".to_string())
        );
    }

    #[test]
    fn test_parse_mdns_service_name_as_tcpip() {
        let output = r#"List of devices attached
adb-5f75c692-wa80xF._adb-tls-connect._tcp. device product:pandora model:25098PN5AC transport_id:5
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].connection_type, ConnectionType::Tcpip);
    }

    #[test]
    fn test_dedup_wireless_mdns_and_ip() {
        // 同一设备同时以 IP:port 和 mDNS 服务名出现
        let output = r#"List of devices attached
10.11.23.216:46235 device product:pandora model:25098PN5AC transport_id:3
adb-5f75c692-wa80xF._adb-tls-connect._tcp device product:pandora model:25098PN5AC transport_id:5
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "10.11.23.216:46235");
    }

    #[test]
    fn test_no_dedup_different_models() {
        // 不同型号的设备不应被去重
        let output = r#"List of devices attached
10.11.23.216:46235 device product:pandora model:25098PN5AC transport_id:3
adb-abcd1234-xyz._adb-tls-connect._tcp device product:raven model:Pixel_6_Pro transport_id:5
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn test_no_dedup_mdns_only() {
        // 只有 mDNS 没有 IP:port 时不应去重
        let output = r#"List of devices attached
adb-5f75c692-wa80xF._adb-tls-connect._tcp device product:pandora model:25098PN5AC transport_id:5
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
    }

    #[test]
    fn test_dedup_serial_and_ip_same_device() {
        // 同一设备同时以序列号和 IP:port 出现（无线连接时常见）
        let output = r#"List of devices attached
25098PN5AC device product:pandora model:25098PN5AC transport_id:1
10.11.23.216:34075 device product:pandora model:25098PN5AC transport_id:3
"#;
        let devices = parse_device_list(output).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "10.11.23.216:34075");
    }
}
