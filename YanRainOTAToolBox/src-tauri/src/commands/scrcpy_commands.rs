use std::sync::Arc;
use tauri::State;

use crate::core::scrcpy::manager::{ScrcpyConfig, ScrcpyManager, ScrcpyStatus};
use crate::infra::errors::AppError;

/// Android 常用键码常量
mod keycodes {
    pub const KEYCODE_BACK: i32 = 4;
    pub const KEYCODE_HOME: i32 = 3;
    pub const KEYCODE_APP_SWITCH: i32 = 187; // 多任务/最近任务
    pub const KEYCODE_POWER: i32 = 26;
    pub const KEYCODE_VOLUME_UP: i32 = 24;
    pub const KEYCODE_VOLUME_DOWN: i32 = 25;
    pub const KEYCODE_VOLUME_MUTE: i32 = 164;
}

// ===== 投屏生命周期 =====

/// 启动 scrcpy 投屏
#[tauri::command]
pub async fn start_scrcpy(
    config: ScrcpyConfig,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<String, AppError> {
    scrcpy_manager.start(config).await
}

/// 停止 scrcpy 投屏
#[tauri::command]
pub async fn stop_scrcpy(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<String, AppError> {
    scrcpy_manager.stop(&device_id).await
}

/// 获取 scrcpy 状态
#[tauri::command]
pub async fn get_scrcpy_status(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<ScrcpyStatus, AppError> {
    Ok(scrcpy_manager.get_status(&device_id))
}

// ===== 按键模拟 =====

/// 发送返回键
#[tauri::command]
pub async fn scrcpy_key_back(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_BACK)
        .await
}

/// 发送主页键
#[tauri::command]
pub async fn scrcpy_key_home(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_HOME)
        .await
}

/// 发送多任务键
#[tauri::command]
pub async fn scrcpy_key_app_switch(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_APP_SWITCH)
        .await
}

/// 发送锁屏键
#[tauri::command]
pub async fn scrcpy_key_power(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_POWER)
        .await
}

/// 音量加
#[tauri::command]
pub async fn scrcpy_volume_up(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_VOLUME_UP)
        .await
}

/// 音量减
#[tauri::command]
pub async fn scrcpy_volume_down(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_VOLUME_DOWN)
        .await
}

/// 静音
#[tauri::command]
pub async fn scrcpy_volume_mute(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager
        .send_key_event(&device_id, keycodes::KEYCODE_VOLUME_MUTE)
        .await
}

// ===== 屏幕操作 =====

/// 设置屏幕旋转
/// orientation: 0=自动旋转, 1=强制竖屏, 2=强制横屏
#[tauri::command]
pub async fn scrcpy_set_rotation(
    device_id: String,
    orientation: u8,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<(), AppError> {
    scrcpy_manager.set_rotation(&device_id, orientation).await
}

/// 截屏
#[tauri::command]
pub async fn scrcpy_screenshot(
    device_id: String,
    scrcpy_manager: State<'_, Arc<ScrcpyManager>>,
) -> Result<String, AppError> {
    scrcpy_manager.take_screenshot(&device_id).await
}
