use tauri::State;

use crate::core::adb::safety::CommandInfo;
use crate::core::task::queue::TaskQueue;
use crate::domain::command::CommandRequest;
use crate::domain::task::TaskInfo;
use crate::infra::errors::AppError;

/// 执行 ADB 命令（同步等待结果）
#[tauri::command]
pub async fn run_adb_command(
    request: CommandRequest,
    task_queue: State<'_, std::sync::Arc<TaskQueue>>,
) -> Result<TaskInfo, AppError> {
    task_queue.submit_command(request).await
}

/// 执行 ADB 命令（流式输出，通过事件推送）
#[tauri::command]
pub async fn run_adb_command_streaming(
    request: CommandRequest,
    app: tauri::AppHandle,
    task_queue: State<'_, std::sync::Arc<TaskQueue>>,
) -> Result<String, AppError> {
    task_queue.submit_command_streaming(request, app).await
}

/// 获取可用命令列表
#[tauri::command]
pub async fn list_available_commands(
    task_queue: State<'_, std::sync::Arc<TaskQueue>>,
) -> Result<Vec<CommandInfo>, AppError> {
    Ok(task_queue.list_available_commands())
}
