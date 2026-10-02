use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::debug;

/// 事件名称常量
pub const EVENT_TASK_OUTPUT: &str = "task-output";
pub const EVENT_TASK_STATUS: &str = "task-status";
pub const EVENT_DEVICE_CHANGE: &str = "device-change";

/// 任务输出事件载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputPayload {
    pub task_id: String,
    pub stream: String, // "stdout" or "stderr"
    pub line: String,
}

/// 任务状态变化事件载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusPayload {
    pub task_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
}

/// 设备变化事件载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceChangePayload {
    pub action: String, // "refresh"
    pub device_count: usize,
}

/// 向前端发送任务输出事件
pub fn emit_task_output(app: &AppHandle, payload: TaskOutputPayload) {
    debug!(
        "Emitting task output: {} - {}",
        payload.task_id, payload.stream
    );
    let _ = app.emit(EVENT_TASK_OUTPUT, payload);
}

/// 向前端发送任务状态变化事件
pub fn emit_task_status(app: &AppHandle, payload: TaskStatusPayload) {
    debug!(
        "Emitting task status: {} - {}",
        payload.task_id, payload.status
    );
    let _ = app.emit(EVENT_TASK_STATUS, payload);
}

/// 向前端发送设备变化事件
pub fn emit_device_change(app: &AppHandle, payload: DeviceChangePayload) {
    debug!("Emitting device change: {}", payload.action);
    let _ = app.emit(EVENT_DEVICE_CHANGE, payload);
}
