use std::sync::OnceLock;
use std::time::Duration;

use crate::infra::errors::AppError;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// 全局共享 HTTP 客户端（连接池复用、统一超时）
pub fn client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to create global HTTP client")
    })
}

/// 判断 reqwest 错误是否为网络不可达类
pub fn is_network_error(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}

/// 将 reqwest::Error 转为不含 URL 的纯类型描述（安全写入日志）
pub fn describe_error(e: &reqwest::Error) -> &'static str {
    if e.is_connect() {
        "connect_error"
    } else if e.is_timeout() {
        "timeout"
    } else if e.is_request() {
        "request_build_error"
    } else if e.is_decode() {
        "decode_error"
    } else if e.is_body() {
        "body_error"
    } else if e.is_status() {
        "status_error"
    } else if e.is_redirect() {
        "redirect_error"
    } else {
        "unknown_error"
    }
}

/// 将 reqwest 错误映射为 AppError
/// 网络类错误 → NetworkUnavailable；其他 → Internal
/// 日志中只记录错误类型，不暴露 URL
pub fn map_reqwest_error(e: reqwest::Error) -> AppError {
    tracing::warn!("HTTP error: {}", describe_error(&e));
    if is_network_error(&e) {
        AppError::NetworkUnavailable("网络连接失败，请检查网络后重试".to_string())
    } else if e.is_decode() {
        AppError::Internal("服务器返回了无法解析的数据".to_string())
    } else if e.is_status() {
        AppError::Internal("服务器返回了异常状态".to_string())
    } else {
        AppError::Internal("请求处理失败，请稍后重试".to_string())
    }
}

/// 将 reqwest 错误映射为 String（用于返回 Result<T, String> 的命令）
/// 日志中只记录错误类型，不暴露 URL
pub fn map_reqwest_error_string(e: reqwest::Error) -> String {
    tracing::warn!("HTTP error: {}", describe_error(&e));
    if is_network_error(&e) {
        "网络连接失败，请检查网络后重试".to_string()
    } else if e.is_decode() {
        "服务器返回了无法解析的数据".to_string()
    } else if e.is_status() {
        "服务器返回了异常状态".to_string()
    } else {
        "请求处理失败，请稍后重试".to_string()
    }
}
