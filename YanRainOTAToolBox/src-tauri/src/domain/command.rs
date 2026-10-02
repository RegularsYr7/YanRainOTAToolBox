use serde::{Deserialize, Serialize};

/// 命令执行请求（前端 -> 后端的 DTO）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// 目标设备序列号（null 表示不指定设备）
    pub target_device: Option<String>,
    /// 逻辑命令名（不是原始 shell 命令）
    pub command: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 超时时间（毫秒，可选）
    pub timeout_ms: Option<u64>,
}

/// 命令执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// 任务 ID
    pub task_id: String,
    /// 退出码
    pub exit_code: i32,
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
}

/// 命令风险等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    /// 安全命令，无需确认
    Safe,
    /// 普通命令，需要基本校验
    Normal,
    /// 危险命令，需要二次确认
    Dangerous,
    /// 禁止执行
    Blocked,
}
