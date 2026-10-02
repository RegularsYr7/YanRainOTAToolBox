use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// 日志文件前缀
const LOG_FILE_PREFIX: &str = "YanRainOTAToolBox";

/// 初始化日志系统（控制台 + 按天滚动的日志文件）
///
/// 日志文件保存在 `log_dir` 目录下，文件名格式为 `YanRainOTAToolBox.YYYY-MM-DD`。
/// 同时输出到控制台（带颜色）和文件（无颜色）。
///
/// 返回 `WorkerGuard`，调用方必须持有它直到程序退出，
/// 否则后台写入线程会提前终止，丢失日志。
///
/// 如需深度调试，可设置环境变量 `RUST_LOG` 覆盖默认级别，例如：
///   `RUST_LOG=trace` 或 `RUST_LOG=YanRainToolBox_lib::core::adb::executor=debug`
pub fn init_logging(log_dir: &Path) -> WorkerGuard {
    // 生产环境隐藏模块路径/源码位置信息，避免日志暴露内部实现细节
    let verbose_source = cfg!(debug_assertions);

    // 确保日志目录存在
    let _ = std::fs::create_dir_all(log_dir);

    // 创建按天滚动的日志文件 appender
    let file_appender = tracing_appender::rolling::daily(log_dir, LOG_FILE_PREFIX);
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    // 日志过滤级别
    // 控制台：debug（开发调试），文件：info（生产日志，避免刷屏和敏感信息泄露）
    // 第三方网络库 warn（避免 reqwest/hyper 泄露请求 URL、代理地址等敏感信息）
    let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "debug,reqwest=warn,hyper=warn,hyper_util=warn,rustls=warn,h2=warn,tungstenite=warn,mdns_sd=warn",
        )
    });
    let file_filter = EnvFilter::new(
        "info,reqwest=warn,hyper=warn,hyper_util=warn,rustls=warn,h2=warn,tungstenite=warn,mdns_sd=warn",
    );

    // 控制台输出层（带颜色、带目标模块、debug 级别）
    let console_layer = fmt::layer()
        .with_target(verbose_source)
        .with_thread_ids(true)
        .with_file(verbose_source)
        .with_line_number(verbose_source)
        .with_filter(console_filter);

    // 文件输出层（无颜色、带时间戳、info 级别）
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(verbose_source)
        .with_thread_ids(true)
        .with_file(verbose_source)
        .with_line_number(verbose_source)
        .with_writer(non_blocking_file)
        .with_filter(file_filter);

    // 组合两个输出层（各自有独立的过滤级别）
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    tracing::info!("YanRainOTAToolBox logging initialized");
    tracing::info!("Log directory: {:?}", log_dir);

    guard
}

/// 获取默认日志目录
///
/// - Windows: exe 同级 `logs/`（便携模式友好）
/// - Linux/macOS: `~/.local/share/com.administrator.YanRainOTAToolBox/logs/`
///   避免写入 /usr/bin/logs 等需要 root 权限的目录
pub fn default_log_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // Windows：优先 exe 同级目录（便携模式）
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("logs");
            }
        }
        PathBuf::from("logs")
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Linux/macOS：使用用户数据目录，避免写入系统目录
        if let Some(data_dir) = dirs::data_dir() {
            return data_dir
                .join("com.administrator.YanRainOTAToolBox")
                .join("logs");
        }
        // 回退到 HOME 目录下
        if let Some(home) = dirs::home_dir() {
            return home.join(".YanRainOTAToolBox").join("logs");
        }
        // 最终回退
        PathBuf::from("logs")
    }
}
