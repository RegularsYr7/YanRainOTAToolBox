use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::debug;

use crate::domain::command::CommandResult;
use crate::infra::errors::AppError;
use crate::infra::process::hide_console_window;

/// 为命令注入平台相关的库搜索路径
///
/// Linux 下 adb/fastboot 依赖 lib64/ 中的共享库，
/// 需要设置 LD_LIBRARY_PATH 指向二进制同级的 lib64 目录。
/// macOS 下对应 DYLD_LIBRARY_PATH。
fn inject_library_path(cmd: &mut Command, binary_path: &Path) {
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = binary_path.parent() {
            let lib64 = parent.join("lib64");
            if lib64.exists() {
                let existing = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
                let new_val = if existing.is_empty() {
                    lib64.to_string_lossy().to_string()
                } else {
                    format!("{}:{}", lib64.display(), existing)
                };
                cmd.env("LD_LIBRARY_PATH", &new_val);
                debug!("Set LD_LIBRARY_PATH={}", new_val);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(parent) = binary_path.parent() {
            let lib64 = parent.join("lib64");
            if lib64.exists() {
                let existing = std::env::var("DYLD_LIBRARY_PATH").unwrap_or_default();
                let new_val = if existing.is_empty() {
                    lib64.to_string_lossy().to_string()
                } else {
                    format!("{}:{}", lib64.display(), existing)
                };
                cmd.env("DYLD_LIBRARY_PATH", &new_val);
                debug!("Set DYLD_LIBRARY_PATH={}", new_val);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (cmd, binary_path); // suppress unused warnings
    }
}

/// 输出行类型
#[derive(Debug, Clone)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

/// 执行命令并同步等待结果
pub async fn execute_command(
    binary_path: &Path,
    args: &[String],
    timeout_ms: u64,
) -> Result<CommandResult, AppError> {
    // debug!(
    //     "Executing: {:?} {:?} (timeout: {}ms)",
    //     binary_path, args, timeout_ms
    // );

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        run_process(binary_path, args),
    )
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(AppError::Timeout(timeout_ms)),
    }
}

/// 执行命令并流式返回输出（通过 channel）
pub async fn execute_command_streaming(
    binary_path: &Path,
    args: &[String],
    task_id: String,
    tx: mpsc::UnboundedSender<(String, OutputLine)>,
) -> Result<i32, AppError> {
    debug!("Streaming execute: {:?} {:?}", binary_path, args);

    let mut cmd = Command::new(binary_path);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    // 设置工作目录为可执行文件所在目录
    if let Some(parent) = binary_path.parent() {
        cmd.current_dir(parent);
    }
    inject_library_path(&mut cmd, binary_path);
    hide_console_window(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::CommandFailed(format!("Failed to spawn process: {}", e)))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let task_id_clone = task_id.clone();
    let tx_clone = tx.clone();

    // 读取 stdout
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[{}] stdout: {}", task_id_clone, line);
            let _ = tx_clone.send((task_id_clone.clone(), OutputLine::Stdout(line)));
        }
    });

    let task_id_clone2 = task_id.clone();
    let tx_clone2 = tx.clone();

    // 读取 stderr
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!("[{}] stderr: {}", task_id_clone2, line);
            let _ = tx_clone2.send((task_id_clone2.clone(), OutputLine::Stderr(line)));
        }
    });

    // 等待子进程结束
    let status = child
        .wait()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Failed to wait for process: {}", e)))?;

    // 等待输出读取完成
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let code = status.code().unwrap_or(-1);
    debug!("Process exited with code: {}", code);
    Ok(code)
}

async fn run_process(binary_path: &Path, args: &[String]) -> Result<CommandResult, AppError> {
    let mut cmd = Command::new(binary_path);
    cmd.args(args);
    // 设置工作目录为可执行文件所在目录（QSaharaServer 等高通工具需要）
    if let Some(parent) = binary_path.parent() {
        cmd.current_dir(parent);
    }
    inject_library_path(&mut cmd, binary_path);
    hide_console_window(&mut cmd);
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Failed to execute command: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    if !stderr.is_empty() {
        debug!("stderr: {}", stderr);
    }

    Ok(CommandResult {
        task_id: String::new(), // 调用方填充
        exit_code,
        stdout,
        stderr,
    })
}

/// 取消正在运行的进程
pub async fn cancel_process(child: &mut tokio::process::Child) -> Result<(), AppError> {
    child
        .kill()
        .await
        .map_err(|e| AppError::CommandFailed(format!("Failed to kill process: {}", e)))?;
    debug!("Process killed");
    Ok(())
}
