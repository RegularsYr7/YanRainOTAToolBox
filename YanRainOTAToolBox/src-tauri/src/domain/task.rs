use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Success,
    Failed,
    Canceled,
}

/// 任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    /// 任务 ID
    pub id: String,
    /// 目标设备序列号（可选）
    pub device_id: Option<String>,
    /// 命令名称
    pub command_name: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 任务状态
    pub status: TaskStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    pub finished_at: Option<DateTime<Utc>>,
    /// 退出码
    pub exit_code: Option<i32>,
    /// 错误信息
    pub error_message: Option<String>,
}

impl TaskInfo {
    pub fn new(device_id: Option<String>, command_name: String, args: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            device_id,
            command_name,
            args,
            status: TaskStatus::Queued,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            error_message: None,
        }
    }
}
