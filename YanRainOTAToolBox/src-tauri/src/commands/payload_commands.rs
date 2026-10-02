use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tracing::info;

use crate::core::payload::dumper::{PayloadDumper, PayloadInitResult, PayloadPartitionInfo};
use crate::core::payload::http_reader::HttpRangeReader;
use crate::core::payload::non_payload;
use crate::core::payload::reader::{LocalFileReader, ReadAt};
use crate::infra::errors::AppError;

/// 全局共享的 Dumper 实例
pub struct PayloadState {
    pub dumper: Arc<RwLock<Option<PayloadDumper>>>,
    /// 取消标志：前端调用 payload_cancel 时设为 true
    pub cancelled: Arc<AtomicBool>,
}

impl Default for PayloadState {
    fn default() -> Self {
        Self {
            dumper: Arc::new(RwLock::new(None)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 初始化 payload：解析本地文件或远程 URL
/// 返回分区列表等元信息
#[tauri::command]
pub async fn payload_init(
    source: String,
    payload_state: State<'_, Arc<PayloadState>>,
) -> Result<PayloadInitResult, AppError> {
    info!("Initializing payload from: {}", source);

    // 重置取消标志
    payload_state.cancelled.store(false, Ordering::Relaxed);

    // 在线程中执行阻塞 IO
    let source_for_blocking = source.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<(Option<PayloadDumper>, PayloadInitResult), AppError> {
            let is_url = source_for_blocking.starts_with("http://")
                || source_for_blocking.starts_with("https://");
            let is_dir = !is_url && std::path::Path::new(&source_for_blocking).is_dir();

            // 本地文件夹：直接扫描，无需 reader
            if is_dir {
                let result = non_payload::scan_local_folder(&source_for_blocking)
                    .map_err(|e| AppError::CommandFailed(format!("文件夹扫描失败: {}", e)))?;
                return Ok((None, result));
            }

            // 文件或 URL：先创建 reader
            let reader: Arc<dyn ReadAt> = if is_url {
                Arc::new(
                    HttpRangeReader::new(&source_for_blocking)
                        .map_err(|e| AppError::CommandFailed(format!("HTTP 连接失败: {}", e)))?,
                )
            } else {
                Arc::new(
                    LocalFileReader::new(&source_for_blocking)
                        .map_err(|e| AppError::CommandFailed(format!("文件读取失败: {}", e)))?,
                )
            };

            // 先尝试 payload 解析
            match PayloadDumper::new(reader.clone()) {
                Ok(dumper) => {
                    let result = dumper.init_result(&source_for_blocking);
                    Ok((Some(dumper), result))
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    // 如果是 ZIP 但缺少 payload.bin，走非 payload 路径
                    if err_msg.contains("未找到 payload.bin") {
                        info!("ZIP without payload.bin, scanning as non-payload package");
                        let result =
                            non_payload::scan_zip_entries(reader.as_ref(), &source_for_blocking)
                                .map_err(|e| {
                                    AppError::CommandFailed(format!("ZIP 扫描失败: {}", e))
                                })?;
                        Ok((None, result))
                    } else {
                        Err(AppError::CommandFailed(format!(
                            "Payload 解析失败: {}",
                            err_msg
                        )))
                    }
                }
            }
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    // 保存 dumper 实例（非 payload 模式为 None）
    let (dumper, init_result) = result;
    *payload_state.dumper.write() = dumper;

    info!(
        "Payload initialized: is_non_payload={}, partitions={}, raw_entries={}, is_zip={}",
        init_result.is_non_payload,
        init_result.partitions.len(),
        init_result.raw_entries.len(),
        init_result.is_zip
    );

    Ok(init_result)
}

/// 获取分区列表（需先调用 payload_init）
#[tauri::command]
pub async fn payload_list_partitions(
    payload_state: State<'_, Arc<PayloadState>>,
) -> Result<Vec<PayloadPartitionInfo>, AppError> {
    let guard = payload_state.dumper.read();
    let dumper = guard
        .as_ref()
        .ok_or_else(|| AppError::CommandFailed("Payload 未初始化，请先加载文件".into()))?;

    Ok(dumper.list_partitions())
}

/// 提取选中的分区到输出目录（异步，可取消）
#[tauri::command]
pub async fn payload_extract_partitions(
    partition_names: Vec<String>,
    output_dir: String,
    app: AppHandle,
    payload_state: State<'_, Arc<PayloadState>>,
) -> Result<Vec<String>, AppError> {
    if partition_names.is_empty() {
        return Err(AppError::CommandFailed("未选择任何分区".into()));
    }

    info!(
        "Extracting {} partitions to {}",
        partition_names.len(),
        output_dir
    );

    // 重置取消标志
    payload_state.cancelled.store(false, Ordering::Relaxed);

    // 克隆需要的 Arc，以便 move 到 spawn_blocking 中
    let dumper_arc = payload_state.dumper.clone();
    let cancelled = payload_state.cancelled.clone();
    let app_handle = app.clone();

    let results = tokio::task::spawn_blocking(move || -> Result<Vec<String>, AppError> {
        let guard = dumper_arc.read();
        let dumper = guard
            .as_ref()
            .ok_or_else(|| AppError::CommandFailed("Payload 未初始化，请先加载文件".into()))?;

        dumper
            .extract_partitions(&partition_names, &output_dir, &cancelled, move |progress| {
                let _ = app_handle.emit("payload-extract-progress", &progress);
            })
            .map_err(|e| AppError::CommandFailed(format!("{}", e)))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    info!("Extraction complete: {} files", results.len());
    Ok(results)
}

/// 取消正在进行的提取任务
#[tauri::command]
pub async fn payload_cancel(payload_state: State<'_, Arc<PayloadState>>) -> Result<(), AppError> {
    info!("Payload extraction cancelled by user");
    payload_state.cancelled.store(true, Ordering::Relaxed);
    Ok(())
}

/// 释放 payload 资源
#[tauri::command]
pub async fn payload_close(payload_state: State<'_, Arc<PayloadState>>) -> Result<(), AppError> {
    // 先取消任何进行中的任务
    payload_state.cancelled.store(true, Ordering::Relaxed);
    *payload_state.dumper.write() = None;
    info!("Payload dumper closed");
    Ok(())
}

/// 非 payload 包提取：从本地文件夹复制选中的文件到输出目录
#[tauri::command]
pub async fn non_payload_extract_folder(
    source: String,
    file_paths: Vec<String>,
    output_dir: String,
    app: AppHandle,
    payload_state: State<'_, Arc<PayloadState>>,
) -> Result<Vec<String>, AppError> {
    if file_paths.is_empty() {
        return Err(AppError::CommandFailed("未选择任何文件".into()));
    }
    info!(
        "Non-payload folder extract: {} files to {}",
        file_paths.len(),
        output_dir
    );
    payload_state.cancelled.store(false, Ordering::Relaxed);
    let cancelled = payload_state.cancelled.clone();
    let app_handle = app.clone();

    let results = tokio::task::spawn_blocking(move || -> Result<Vec<String>, AppError> {
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| AppError::CommandFailed(format!("创建输出目录失败: {}", e)))?;

        let total = file_paths.len();
        let mut out_paths = Vec::new();

        for (idx, rel_path) in file_paths.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(AppError::CommandFailed("用户取消了提取任务".into()));
            }
            let src = std::path::Path::new(&source).join(rel_path);
            // 保留子目录结构
            let dst = std::path::Path::new(&output_dir).join(rel_path);
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::copy(&src, &dst)
                .map_err(|e| AppError::CommandFailed(format!("复制失败 {}: {}", rel_path, e)))?;
            out_paths.push(dst.to_string_lossy().to_string());

            // 发送进度事件
            let _ = app_handle.emit(
                "payload-extract-progress",
                &crate::core::payload::dumper::PayloadExtractProgress {
                    partition_name: rel_path.clone(),
                    current_op: idx + 1,
                    total_ops: total,
                    percent: ((idx + 1) as f64 / total as f64) * 100.0,
                },
            );
        }

        Ok(out_paths)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    info!(
        "Non-payload folder extraction complete: {} files",
        results.len()
    );
    Ok(results)
}

/// 非 payload 包提取：从 ZIP 解压选中的文件到输出目录
/// 自动区分本地 ZIP 和在线 URL ZIP
#[tauri::command]
pub async fn non_payload_extract_zip(
    source: String,
    file_paths: Vec<String>,
    output_dir: String,
    app: AppHandle,
    payload_state: State<'_, Arc<PayloadState>>,
) -> Result<Vec<String>, AppError> {
    if file_paths.is_empty() {
        return Err(AppError::CommandFailed("未选择任何文件".into()));
    }
    info!(
        "Non-payload ZIP extract: {} files to {}",
        file_paths.len(),
        output_dir
    );
    payload_state.cancelled.store(false, Ordering::Relaxed);
    let cancelled = payload_state.cancelled.clone();
    let app_handle = app.clone();

    let is_url = source.starts_with("http://") || source.starts_with("https://");

    let results = tokio::task::spawn_blocking(move || -> Result<Vec<String>, AppError> {
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| AppError::CommandFailed(format!("创建输出目录失败: {}", e)))?;

        if is_url {
            // ===== 在线 URL：通过 HTTP Range + 自定义 ZIP 解析器提取 =====
            extract_zip_from_url(&source, &file_paths, &output_dir, &cancelled, &app_handle)
        } else {
            // ===== 本地文件：使用 zip crate 解压 =====
            extract_zip_from_local(&source, &file_paths, &output_dir, &cancelled, &app_handle)
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))??;

    info!(
        "Non-payload ZIP extraction complete: {} files",
        results.len()
    );
    Ok(results)
}

/// 从在线 URL ZIP 提取文件（通过 HTTP Range）
fn extract_zip_from_url(
    source: &str,
    file_paths: &[String],
    output_dir: &str,
    cancelled: &AtomicBool,
    app_handle: &AppHandle,
) -> Result<Vec<String>, AppError> {
    use crate::core::payload::zip_parser::{extract_zip_entry_to_file, list_zip_entries_with_size};

    let reader: Arc<dyn ReadAt> = Arc::new(
        HttpRangeReader::new(source)
            .map_err(|e| AppError::CommandFailed(format!("HTTP 连接失败: {}", e)))?,
    );

    // 获取 ZIP 条目的完整信息（含 lfh_offset、compression 等）
    let all_entries = list_zip_entries_with_size(reader.as_ref())
        .map_err(|e| AppError::CommandFailed(format!("ZIP 解析失败: {}", e)))?;

    let paths_set: std::collections::HashSet<&str> =
        file_paths.iter().map(|s| s.as_str()).collect();
    let total = file_paths.len();
    let mut out_paths = Vec::new();
    let mut done = 0usize;

    for entry in &all_entries {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::CommandFailed("用户取消了提取任务".into()));
        }
        if !paths_set.contains(entry.name.as_str()) {
            continue;
        }

        let dst = std::path::Path::new(output_dir).join(&entry.name);
        extract_zip_entry_to_file(reader.as_ref(), entry, &dst)
            .map_err(|e| AppError::CommandFailed(format!("提取失败 {}: {}", entry.name, e)))?;

        out_paths.push(dst.to_string_lossy().to_string());
        done += 1;

        let _ = app_handle.emit(
            "payload-extract-progress",
            &crate::core::payload::dumper::PayloadExtractProgress {
                partition_name: entry.name.clone(),
                current_op: done,
                total_ops: total,
                percent: (done as f64 / total as f64) * 100.0,
            },
        );

        if done >= total {
            break;
        }
    }

    Ok(out_paths)
}

/// 从本地 ZIP 文件解压（使用 zip crate）
fn extract_zip_from_local(
    source: &str,
    file_paths: &[String],
    output_dir: &str,
    cancelled: &AtomicBool,
    app_handle: &AppHandle,
) -> Result<Vec<String>, AppError> {
    let file = std::fs::File::open(source)
        .map_err(|e| AppError::CommandFailed(format!("打开 ZIP 失败: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::CommandFailed(format!("解析 ZIP 失败: {}", e)))?;

    let paths_set: std::collections::HashSet<String> = file_paths.iter().cloned().collect();
    let total = file_paths.len();
    let mut out_paths = Vec::new();
    let mut done = 0usize;

    for i in 0..archive.len() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::CommandFailed("用户取消了提取任务".into()));
        }
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::CommandFailed(format!("读取 ZIP 条目失败: {}", e)))?;
        let name = entry.name().to_string();
        if !paths_set.contains(&name) {
            continue;
        }

        let dst = std::path::Path::new(output_dir).join(&name);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut out_file = std::fs::File::create(&dst)
            .map_err(|e| AppError::CommandFailed(format!("创建文件失败 {}: {}", name, e)))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| AppError::CommandFailed(format!("解压失败 {}: {}", name, e)))?;

        out_paths.push(dst.to_string_lossy().to_string());
        done += 1;

        let _ = app_handle.emit(
            "payload-extract-progress",
            &crate::core::payload::dumper::PayloadExtractProgress {
                partition_name: name,
                current_op: done,
                total_ops: total,
                percent: (done as f64 / total as f64) * 100.0,
            },
        );

        if done >= total {
            break;
        }
    }

    Ok(out_paths)
}
