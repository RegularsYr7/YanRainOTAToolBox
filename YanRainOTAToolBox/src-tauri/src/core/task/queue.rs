use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::core::adb::executor::{self, OutputLine};
use crate::core::adb::safety::SafetyPolicy;
use crate::core::device::service::DeviceService;
use crate::core::events::emitter::{self, TaskOutputPayload, TaskStatusPayload};
use crate::core::task::history::TaskHistory;
use crate::domain::command::{CommandRequest, RiskLevel};
use crate::domain::task::{TaskInfo, TaskStatus};
use crate::infra::errors::AppError;

/// 任务队列 - 管理命令的排队和执行
pub struct TaskQueue {
    device_service: Arc<DeviceService>,
    safety: SafetyPolicy,
    history: Arc<TaskHistory>,
}

impl TaskQueue {
    pub fn new(device_service: Arc<DeviceService>, history: Arc<TaskHistory>) -> Self {
        Self {
            device_service,
            safety: SafetyPolicy::new(),
            history,
        }
    }

    /// 提交命令（非流式，等待结果）
    pub async fn submit_command(&self, request: CommandRequest) -> Result<TaskInfo, AppError> {
        // 1. 安全检查
        let (adb_args, risk_level) = self
            .safety
            .check_and_build_args(&request.command, &request.args)?;

        if risk_level == RiskLevel::Dangerous {
            info!("Dangerous command requested: {}", request.command);
            // 在实际应用中，前端应该已经进行了二次确认
            // 这里我们信任前端发送的确认
        }

        // 2. 创建任务
        let mut task = TaskInfo::new(
            request.target_device.clone(),
            request.command.clone(),
            request.args.clone(),
        );
        task.status = TaskStatus::Running;
        task.started_at = Some(chrono::Utc::now());
        self.history.add(task.clone());

        // 3. 执行命令
        let result = self
            .device_service
            .execute_adb_command(
                request.target_device.as_deref(),
                &adb_args,
                request.timeout_ms,
            )
            .await;

        // 4. 更新任务状态
        match &result {
            Ok(cmd_result) => {
                self.history.update_status(
                    &task.id,
                    if cmd_result.exit_code == 0 {
                        TaskStatus::Success
                    } else {
                        TaskStatus::Failed
                    },
                    Some(cmd_result.exit_code),
                    if cmd_result.exit_code != 0 {
                        Some(cmd_result.stderr.clone())
                    } else {
                        None
                    },
                );
                task.status = if cmd_result.exit_code == 0 {
                    TaskStatus::Success
                } else {
                    TaskStatus::Failed
                };
                task.exit_code = Some(cmd_result.exit_code);
            }
            Err(e) => {
                error!("Command execution failed: {}", e);
                self.history
                    .update_status(&task.id, TaskStatus::Failed, None, Some(e.to_string()));
                task.status = TaskStatus::Failed;
                task.error_message = Some(e.to_string());
            }
        }

        task.finished_at = Some(chrono::Utc::now());
        Ok(task)
    }

    /// 提交命令（流式，通过事件推送输出）
    pub async fn submit_command_streaming(
        &self,
        request: CommandRequest,
        app: tauri::AppHandle,
    ) -> Result<String, AppError> {
        // 1. 安全检查
        let (adb_args, risk_level) = self
            .safety
            .check_and_build_args(&request.command, &request.args)?;

        if risk_level == RiskLevel::Dangerous {
            info!("Dangerous streaming command requested: {}", request.command);
        }

        // 2. 创建任务
        let task = TaskInfo::new(
            request.target_device.clone(),
            request.command.clone(),
            request.args.clone(),
        );
        let task_id = task.id.clone();
        self.history.add(task);
        self.history
            .update_status(&task_id, TaskStatus::Running, None, None);

        // 通知前端任务开始
        emitter::emit_task_status(
            &app,
            TaskStatusPayload {
                task_id: task_id.clone(),
                status: "running".to_string(),
                exit_code: None,
                error_message: None,
            },
        );

        // 3. 准备执行参数
        let adb_path = self.device_service.get_adb_path()?;
        let mut full_args: Vec<String> = Vec::new();
        // 多设备时添加 -s 参数指向目标设备，单设备时省略
        if let Some(ref device_id) = request.target_device {
            let device_count = self.device_service.get_cached_devices().len();
            if device_count > 1 {
                full_args.push("-s".to_string());
                full_args.push(device_id.clone());
            }
        }
        full_args.extend(adb_args);

        let (tx, mut rx) = mpsc::unbounded_channel::<(String, OutputLine)>();

        let _task_id_clone = task_id.clone();
        let history = self.history.clone();
        let app_clone = app.clone();

        // 4. 启动输出转发
        tokio::spawn(async move {
            while let Some((tid, line)) = rx.recv().await {
                let (stream, text) = match line {
                    OutputLine::Stdout(s) => ("stdout", s),
                    OutputLine::Stderr(s) => ("stderr", s),
                };
                emitter::emit_task_output(
                    &app_clone,
                    TaskOutputPayload {
                        task_id: tid,
                        stream: stream.to_string(),
                        line: text,
                    },
                );
            }
        });

        // 5. 执行命令
        let task_id_for_exec = task_id.clone();
        let history_clone = history.clone();
        let app_for_status = app.clone();

        tokio::spawn(async move {
            match executor::execute_command_streaming(
                &adb_path,
                &full_args,
                task_id_for_exec.clone(),
                tx,
            )
            .await
            {
                Ok(exit_code) => {
                    let status = if exit_code == 0 {
                        TaskStatus::Success
                    } else {
                        TaskStatus::Failed
                    };
                    history_clone.update_status(
                        &task_id_for_exec,
                        status.clone(),
                        Some(exit_code),
                        None,
                    );
                    emitter::emit_task_status(
                        &app_for_status,
                        TaskStatusPayload {
                            task_id: task_id_for_exec,
                            status: format!("{:?}", status).to_lowercase(),
                            exit_code: Some(exit_code),
                            error_message: None,
                        },
                    );
                }
                Err(e) => {
                    error!("Streaming command failed: {}", e);
                    history_clone.update_status(
                        &task_id_for_exec,
                        TaskStatus::Failed,
                        None,
                        Some(e.to_string()),
                    );
                    emitter::emit_task_status(
                        &app_for_status,
                        TaskStatusPayload {
                            task_id: task_id_for_exec,
                            status: "failed".to_string(),
                            exit_code: None,
                            error_message: Some(e.to_string()),
                        },
                    );
                }
            }
        });

        Ok(task_id)
    }

    /// 获取已注册的命令列表
    pub fn list_available_commands(&self) -> Vec<crate::core::adb::safety::CommandInfo> {
        self.safety.list_commands()
    }
}
