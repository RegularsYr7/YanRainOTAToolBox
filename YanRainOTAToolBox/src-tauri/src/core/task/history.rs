use parking_lot::RwLock;
use std::collections::VecDeque;
use tracing::info;

use crate::domain::task::{TaskInfo, TaskStatus};

/// 任务历史管理
pub struct TaskHistory {
    /// 历史任务记录（最多保留 N 条）
    tasks: RwLock<VecDeque<TaskInfo>>,
    /// 最大保留数量
    max_size: usize,
}

impl TaskHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            tasks: RwLock::new(VecDeque::new()),
            max_size,
        }
    }

    /// 添加任务到历史
    pub fn add(&self, task: TaskInfo) {
        let mut tasks = self.tasks.write();
        if tasks.len() >= self.max_size {
            tasks.pop_front();
        }
        info!("Task added to history: {} - {}", task.id, task.command_name);
        tasks.push_back(task);
    }

    /// 更新任务状态
    pub fn update_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        exit_code: Option<i32>,
        error: Option<String>,
    ) {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
            task.exit_code = exit_code;
            task.error_message = error;
            if task.started_at.is_none() && task.status == TaskStatus::Running {
                task.started_at = Some(chrono::Utc::now());
            }
            if matches!(
                task.status,
                TaskStatus::Success | TaskStatus::Failed | TaskStatus::Canceled
            ) {
                task.finished_at = Some(chrono::Utc::now());
            }
        }
    }

    /// 获取所有历史任务
    pub fn get_all(&self) -> Vec<TaskInfo> {
        self.tasks.read().iter().cloned().collect()
    }

    /// 获取指定任务
    pub fn get(&self, task_id: &str) -> Option<TaskInfo> {
        self.tasks.read().iter().find(|t| t.id == task_id).cloned()
    }

    /// 清空历史
    pub fn clear(&self) {
        self.tasks.write().clear();
    }
}

impl Default for TaskHistory {
    fn default() -> Self {
        Self::new(500)
    }
}
