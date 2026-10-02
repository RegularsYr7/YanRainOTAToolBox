use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 统一错误码，前后端契约
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    AdbNotFound,
    DeviceOffline,
    DeviceUnauthorized,
    DeviceStateMismatch,
    Timeout,
    PermissionDenied,
    CommandBlocked,
    CommandFailed,
    ParseError,
    NetworkUnavailable,
    NetworkTimeout,
    PayloadParseError,
    PayloadUnsupported,
    PartitionNotFound,
    FlashCancelled,
    FlashFailed,
    FileNotFound,
    DeviceNotRooted,
    Internal,
}

/// 应用统一错误模型
#[derive(Debug, Error)]
pub enum AppError {
    #[error("ADB not found: {0}")]
    AdbNotFound(String),

    #[error("Device offline: {0}")]
    DeviceOffline(String),

    #[error("Device unauthorized: {0}")]
    DeviceUnauthorized(String),

    #[error("Device state mismatch: {0}")]
    DeviceStateMismatch(String),

    #[error("Command timed out after {0}ms")]
    Timeout(u64),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Command blocked by safety policy: {0}")]
    CommandBlocked(String),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Network unavailable: {0}")]
    NetworkUnavailable(String),

    #[error("Network timeout after {timeout_secs}s")]
    NetworkTimeout { timeout_secs: u64 },

    #[error("Payload parse error: {0}")]
    PayloadParseError(String),

    #[error("Unsupported payload operation: {0}")]
    PayloadUnsupported(String),

    #[error("Partition '{partition_name}' not found")]
    PartitionNotFound { partition_name: String },

    #[error("Flash cancelled")]
    FlashCancelled,

    #[error("Flash failed at step '{step}': {message}")]
    FlashFailed { step: String, message: String },

    #[error("File not found at path: {path}")]
    FileNotFound { path: String },

    #[error("Device not rooted: {0}")]
    DeviceNotRooted(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    /// Returns a tuple of (recoverable, hint) for the error.
    /// - recoverable: whether the operation can be retried or recovered from
    /// - hint: an optional suggestion string for the user
    pub fn recoverable(&self) -> (bool, Option<&str>) {
        match self {
            AppError::NetworkUnavailable(_) | AppError::NetworkTimeout { .. } => {
                (true, Some("retry"))
            }
            AppError::Timeout(_) => (true, Some("retry")),
            AppError::DeviceOffline(_) | AppError::DeviceUnauthorized(_) => {
                (true, Some("check_connection"))
            }
            AppError::DeviceNotRooted(_) => (true, Some("request_root")),
            AppError::AdbNotFound(_) => (true, Some("check_adb")),
            AppError::Io(_) => (true, Some("retry")),
            AppError::ParseError(_) => (false, None),
            AppError::PayloadParseError(_) => (false, None),
            AppError::PayloadUnsupported(_) => (false, None),
            AppError::PartitionNotFound { .. } => (false, None),
            AppError::FlashCancelled => (false, None),
            AppError::FlashFailed { .. } => (true, Some("retry")),
            AppError::FileNotFound { .. } => (false, Some("check_path")),
            AppError::DeviceStateMismatch(_) => (true, Some("check_device_state")),
            AppError::PermissionDenied(_) => (false, Some("check_permissions")),
            AppError::CommandBlocked(_) => (false, Some("check_policy")),
            AppError::CommandFailed(_) => (true, Some("retry")),
            AppError::Internal(_) => (false, None),
            AppError::Anyhow(_) => (false, None),
        }
    }

    /// Maps the error to its corresponding ErrorCode
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::AdbNotFound(_) => ErrorCode::AdbNotFound,
            AppError::DeviceOffline(_) => ErrorCode::DeviceOffline,
            AppError::DeviceUnauthorized(_) => ErrorCode::DeviceUnauthorized,
            AppError::DeviceStateMismatch(_) => ErrorCode::DeviceStateMismatch,
            AppError::Timeout(_) => ErrorCode::Timeout,
            AppError::PermissionDenied(_) => ErrorCode::PermissionDenied,
            AppError::CommandBlocked(_) => ErrorCode::CommandBlocked,
            AppError::CommandFailed(_) => ErrorCode::CommandFailed,
            AppError::ParseError(_) => ErrorCode::ParseError,
            AppError::NetworkUnavailable(_) => ErrorCode::NetworkUnavailable,
            AppError::NetworkTimeout { .. } => ErrorCode::NetworkTimeout,
            AppError::PayloadParseError(_) => ErrorCode::PayloadParseError,
            AppError::PayloadUnsupported(_) => ErrorCode::PayloadUnsupported,
            AppError::PartitionNotFound { .. } => ErrorCode::PartitionNotFound,
            AppError::FlashCancelled => ErrorCode::FlashCancelled,
            AppError::FlashFailed { .. } => ErrorCode::FlashFailed,
            AppError::FileNotFound { .. } => ErrorCode::FileNotFound,
            AppError::DeviceNotRooted(_) => ErrorCode::DeviceNotRooted,
            AppError::Internal(_) | AppError::Io(_) | AppError::Anyhow(_) => ErrorCode::Internal,
        }
    }
}

/// 可序列化的错误响应，用于前端
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
}

impl From<&AppError> for ErrorResponse {
    fn from(err: &AppError) -> Self {
        let (code, message) = match err {
            AppError::AdbNotFound(msg) => (ErrorCode::AdbNotFound, msg.clone()),
            AppError::DeviceOffline(msg) => (ErrorCode::DeviceOffline, msg.clone()),
            AppError::DeviceUnauthorized(msg) => (ErrorCode::DeviceUnauthorized, msg.clone()),
            AppError::DeviceStateMismatch(msg) => (ErrorCode::DeviceStateMismatch, msg.clone()),
            AppError::Timeout(ms) => (ErrorCode::Timeout, format!("Timed out after {}ms", ms)),
            AppError::PermissionDenied(msg) => (ErrorCode::PermissionDenied, msg.clone()),
            AppError::CommandBlocked(msg) => (ErrorCode::CommandBlocked, msg.clone()),
            AppError::CommandFailed(msg) => (ErrorCode::CommandFailed, msg.clone()),
            AppError::ParseError(msg) => (ErrorCode::ParseError, msg.clone()),
            AppError::NetworkUnavailable(msg) => (ErrorCode::NetworkUnavailable, msg.clone()),
            AppError::NetworkTimeout { timeout_secs } => (
                ErrorCode::NetworkTimeout,
                format!("Network timeout after {}s", timeout_secs),
            ),
            AppError::PayloadParseError(msg) => (ErrorCode::PayloadParseError, msg.clone()),
            AppError::PayloadUnsupported(msg) => (ErrorCode::PayloadUnsupported, msg.clone()),
            AppError::PartitionNotFound { partition_name } => (
                ErrorCode::PartitionNotFound,
                format!("Partition '{}' not found", partition_name),
            ),
            AppError::FlashCancelled => (ErrorCode::FlashCancelled, "Flash cancelled".to_string()),
            AppError::FlashFailed { step, message } => (
                ErrorCode::FlashFailed,
                format!("Flash failed at step '{}': {}", step, message),
            ),
            AppError::FileNotFound { path } => {
                (ErrorCode::FileNotFound, format!("File not found: {}", path))
            }
            AppError::DeviceNotRooted(msg) => (ErrorCode::DeviceNotRooted, msg.clone()),
            AppError::Internal(msg) => (ErrorCode::Internal, msg.clone()),
            AppError::Io(e) => (ErrorCode::Internal, e.to_string()),
            AppError::Anyhow(e) => (ErrorCode::Internal, e.to_string()),
        };
        ErrorResponse {
            code,
            message,
            detail: None,
        }
    }
}

// 让 AppError 可以作为 Tauri 命令的返回错误
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let response = ErrorResponse::from(self);
        response.serialize(serializer)
    }
}
