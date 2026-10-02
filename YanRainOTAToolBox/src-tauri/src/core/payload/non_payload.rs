use anyhow::Result;
use std::path::Path;

use super::dumper::{NonPayloadEntry, PayloadInitResult};
use super::reader::ReadAt;
use super::zip_parser::list_zip_entries_with_size;

/// 人类可读的文件大小
fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// 从 ZIP 的 reader 中扫描文件列表，构建 PayloadInitResult（非 payload 模式）
pub fn scan_zip_entries(reader: &dyn ReadAt, source: &str) -> Result<PayloadInitResult> {
    let entries = list_zip_entries_with_size(reader)?;

    let raw_entries: Vec<NonPayloadEntry> = entries
        .into_iter()
        .map(|e| {
            let is_dir = e.name.ends_with('/');
            NonPayloadEntry {
                size_readable: format_size(e.uncompressed_size),
                path: e.name,
                size_bytes: e.uncompressed_size,
                is_dir,
            }
        })
        .filter(|e| !e.is_dir) // 只保留文件，不保留目录
        .collect();

    tracing::info!(
        "Non-payload ZIP scanned: {} files from source: {}",
        raw_entries.len(),
        source,
    );

    Ok(PayloadInitResult {
        partitions: Vec::new(),
        block_size: 0,
        is_zip: true,
        source: source.to_string(),
        is_non_payload: true,
        raw_entries,
    })
}

/// 扫描本地文件夹，递归列出所有文件，构建 PayloadInitResult（非 payload 模式）
pub fn scan_local_folder(folder: &str) -> Result<PayloadInitResult> {
    let root = Path::new(folder);
    let mut raw_entries = Vec::new();

    fn walk(dir: &Path, root: &Path, out: &mut Vec<NonPayloadEntry>) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if let Ok(meta) = path.metadata() {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(NonPayloadEntry {
                    size_readable: format_size(meta.len()),
                    path: rel,
                    size_bytes: meta.len(),
                    is_dir: false,
                });
            }
        }
    }

    walk(root, root, &mut raw_entries);

    // 按路径排序
    raw_entries.sort_by(|a, b| a.path.cmp(&b.path));

    tracing::info!(
        "Non-payload folder scanned: {} files from: {}",
        raw_entries.len(),
        folder,
    );

    Ok(PayloadInitResult {
        partitions: Vec::new(),
        block_size: 0,
        is_zip: false,
        source: folder.to_string(),
        is_non_payload: true,
        raw_entries,
    })
}
