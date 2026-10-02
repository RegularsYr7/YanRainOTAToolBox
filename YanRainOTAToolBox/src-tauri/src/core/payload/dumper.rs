use anyhow::{bail, Context, Result};
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::reader::ReadAt;
use super::zip_parser::{get_zip_stored_entry_offset, list_zip_entry_names};

/// 生成的 protobuf 模块
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/chromeos_update_engine.rs"));
}

use proto::{DeltaArchiveManifest, InstallOperation};

/// CrAU 文件头魔数
const CRAU_MAGIC: &[u8; 4] = b"CrAU";

/// 分区信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct PayloadPartitionInfo {
    /// 分区名称
    pub partition_name: String,
    /// 分区大小（字节）
    pub size_bytes: u64,
    /// 人类可读大小
    pub size_readable: String,
    /// 操作数量
    pub operation_count: usize,
    /// SHA256 哈希（hex）
    pub hash: String,
}

/// 非 payload 包中的文件条目（线刷包 / 散装 ROM）
#[derive(Debug, Clone, Serialize)]
pub struct NonPayloadEntry {
    /// 文件在 ZIP/文件夹中的相对路径
    pub path: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 人类可读大小
    pub size_readable: String,
    /// 是否为目录
    pub is_dir: bool,
}

/// Payload 初始化结果
#[derive(Debug, Clone, Serialize)]
pub struct PayloadInitResult {
    /// 分区列表（payload 模式下有值）
    pub partitions: Vec<PayloadPartitionInfo>,
    /// 块大小
    pub block_size: u32,
    /// 是否来自 ZIP（OTA 包）
    pub is_zip: bool,
    /// 原始文件/URL
    pub source: String,
    /// 是否为非 payload 包（线刷包 / 散装 ROM）
    pub is_non_payload: bool,
    /// 非 payload 模式下的文件列表
    pub raw_entries: Vec<NonPayloadEntry>,
}

/// 提取进度
#[derive(Debug, Clone, Serialize)]
pub struct PayloadExtractProgress {
    /// 分区名
    pub partition_name: String,
    /// 当前操作索引
    pub current_op: usize,
    /// 总操作数
    pub total_ops: usize,
    /// 进度百分比 (0-100)
    pub percent: f64,
}

/// Payload Dumper 核心
pub struct PayloadDumper {
    /// 数据读取器（本地文件或 HTTP Range）
    reader: Arc<dyn ReadAt>,
    /// payload.bin 在 ZIP 中的起始偏移（非 ZIP 为 0）
    base_offset: u64,
    /// 解析得到的 manifest
    manifest: DeltaArchiveManifest,
    /// 数据区偏移（相对于 payload.bin 起始）
    data_offset: u64,
    /// 块大小
    block_size: u32,
    /// 是否来自 ZIP
    is_zip: bool,
}

impl PayloadDumper {
    /// 初始化 Dumper：解析 payload.bin 或包含它的 ZIP/OTA 包
    pub fn new(reader: Arc<dyn ReadAt>) -> Result<Self> {
        // 尝试作为 ZIP 解析，定位 payload.bin
        let (base_offset, is_zip) =
            match get_zip_stored_entry_offset(reader.as_ref(), "payload.bin") {
                Ok((off, _size)) => {
                    tracing::info!("Found payload.bin in ZIP at offset {}", off);
                    (off, true)
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    if err_msg.contains("not found in ZIP") {
                        // ZIP 解析成功，但找不到 payload.bin
                        // 列出 ZIP 内容帮助诊断
                        let entries = list_zip_entry_names(reader.as_ref()).unwrap_or_default();
                        let sample: Vec<&str> =
                            entries.iter().map(|s| s.as_str()).take(20).collect();
                        tracing::warn!(
                            "ZIP is valid but contains no payload.bin. Entries (first 20): {:?}",
                            sample
                        );
                        bail!(
                            "该 ZIP 包内未找到 payload.bin。\n\
                             这可能不是 OTA 增量/全量包，而是线刷包或其他格式。\n\
                             ZIP 内包含: {}{}",
                            sample.join(", "),
                            if entries.len() > 20 {
                                format!(" ...等共 {} 个文件", entries.len())
                            } else {
                                String::new()
                            }
                        );
                    }
                    // 不是 ZIP 文件，当作原始 payload.bin
                    tracing::info!("Not a ZIP (reason: {}), treating as raw payload.bin", e);
                    (0u64, false)
                }
            };

        // 解析 CrAU 头部
        let head_len = 4 + 8 + 8 + 4; // magic + version + manifest_size + metadata_sig_size
        let header = reader
            .read_at(base_offset, head_len)
            .context("Failed to read CrAU header")?;

        if &header[0..4] != CRAU_MAGIC {
            bail!(
                "Invalid payload magic: expected 'CrAU', got {:?}",
                &header[0..4]
            );
        }

        let version = u64::from_be_bytes(header[4..12].try_into()?);
        if version != 2 {
            bail!("Unsupported payload version: {}, expected 2", version);
        }

        let manifest_size = u64::from_be_bytes(header[12..20].try_into()?);
        let metadata_sig_size = u32::from_be_bytes(header[20..24].try_into()?);

        tracing::info!(
            "Payload v{}: manifest_size={}, metadata_sig_size={}",
            version,
            manifest_size,
            metadata_sig_size
        );

        // 读取并解析 manifest
        let manifest_offset = base_offset + head_len as u64;
        let manifest_data = reader
            .read_at(manifest_offset, manifest_size as usize)
            .context("Failed to read manifest")?;

        let manifest = DeltaArchiveManifest::decode(manifest_data.as_slice())
            .context("Failed to decode protobuf manifest")?;

        let data_offset = head_len as u64 + manifest_size + metadata_sig_size as u64;
        let block_size = manifest.block_size.unwrap_or(4096);

        tracing::info!(
            "Parsed manifest: {} partitions, block_size={}, data_offset={}",
            manifest.partitions.len(),
            block_size,
            data_offset
        );

        Ok(Self {
            reader,
            base_offset,
            manifest,
            data_offset,
            block_size,
            is_zip,
        })
    }

    /// 列出所有分区信息
    pub fn list_partitions(&self) -> Vec<PayloadPartitionInfo> {
        self.manifest
            .partitions
            .iter()
            .map(|part| {
                let size_bytes: u64 = part
                    .operations
                    .iter()
                    .flat_map(|op| op.dst_extents.iter())
                    .map(|ext| ext.num_blocks.unwrap_or(0) * self.block_size as u64)
                    .sum();

                let size_readable = format_size(size_bytes);

                let hash = part
                    .new_partition_info
                    .as_ref()
                    .map(|info| hex::encode(info.hash.as_deref().unwrap_or(&[])))
                    .unwrap_or_default();

                PayloadPartitionInfo {
                    partition_name: part.partition_name.clone(),
                    size_bytes,
                    size_readable,
                    operation_count: part.operations.len(),
                    hash,
                }
            })
            .collect()
    }

    /// 提取指定分区到输出目录
    pub fn extract_partition<F>(
        &self,
        partition_name: &str,
        output_dir: &str,
        cancelled: &AtomicBool,
        progress_cb: F,
    ) -> Result<String>
    where
        F: Fn(PayloadExtractProgress) + Send,
    {
        let partition = self
            .manifest
            .partitions
            .iter()
            .find(|p| p.partition_name == partition_name)
            .with_context(|| format!("Partition '{}' not found", partition_name))?;

        let total_ops = partition.operations.len();
        let output_path = Path::new(output_dir).join(format!("{}.img", partition_name));

        // 计算输出文件大小
        let total_size: u64 = partition
            .operations
            .iter()
            .flat_map(|op| op.dst_extents.iter())
            .map(|ext| ext.num_blocks.unwrap_or(0) * self.block_size as u64)
            .sum();

        // 创建输出文件
        let mut out_file = std::fs::File::create(&output_path)
            .with_context(|| format!("Failed to create output file: {:?}", output_path))?;

        // 预分配文件大小
        out_file.set_len(total_size)?;

        tracing::info!(
            "Extracting partition '{}': {} ops, {} total size -> {:?}",
            partition_name,
            total_ops,
            format_size(total_size),
            output_path
        );

        for (idx, operation) in partition.operations.iter().enumerate() {
            // 检查取消标志
            if cancelled.load(Ordering::Relaxed) {
                // 清理未完成的输出文件
                drop(out_file);
                let _ = std::fs::remove_file(&output_path);
                bail!("用户取消了提取任务");
            }

            self.execute_operation(operation, &mut out_file)?;

            progress_cb(PayloadExtractProgress {
                partition_name: partition_name.to_string(),
                current_op: idx + 1,
                total_ops,
                percent: ((idx + 1) as f64 / total_ops as f64) * 100.0,
            });
        }

        out_file.flush()?;

        Ok(output_path.to_string_lossy().to_string())
    }

    /// 批量提取多个分区
    pub fn extract_partitions<F>(
        &self,
        partition_names: &[String],
        output_dir: &str,
        cancelled: &AtomicBool,
        progress_cb: F,
    ) -> Result<Vec<String>>
    where
        F: Fn(PayloadExtractProgress) + Send + Sync,
    {
        std::fs::create_dir_all(output_dir)?;
        let mut results = Vec::new();

        for name in partition_names {
            // 检查取消标志
            if cancelled.load(Ordering::Relaxed) {
                bail!("用户取消了提取任务");
            }
            let path = self.extract_partition(name, output_dir, cancelled, &progress_cb)?;
            results.push(path);
        }

        Ok(results)
    }

    /// 获取初始化结果
    pub fn init_result(&self, source: &str) -> PayloadInitResult {
        PayloadInitResult {
            partitions: self.list_partitions(),
            block_size: self.block_size,
            is_zip: self.is_zip,
            source: source.to_string(),
            is_non_payload: false,
            raw_entries: Vec::new(),
        }
    }

    // ===== 内部方法 =====

    /// 执行单个 InstallOperation
    fn execute_operation(&self, op: &InstallOperation, out_file: &mut std::fs::File) -> Result<()> {
        use std::io::Seek;

        let op_type = op.r#type();
        let data_offset = op.data_offset.unwrap_or(0);
        let data_length = op.data_length.unwrap_or(0);

        // 读取操作数据
        let data = if data_length > 0 {
            self.reader.read_at(
                self.base_offset + self.data_offset + data_offset,
                data_length as usize,
            )?
        } else {
            Vec::new()
        };

        // 校验数据哈希
        if let Some(ref expected_hash) = op.data_sha256_hash {
            let actual_hash = Sha256::digest(&data);
            if actual_hash.as_slice() != expected_hash.as_slice() {
                bail!("Data hash mismatch for operation at offset {}", data_offset);
            }
        }

        // 根据操作类型执行
        match op_type {
            proto::install_operation::Type::ReplaceXz => {
                let decompressed = decompress_xz(&data)?;
                self.write_to_extents(out_file, &op.dst_extents, &decompressed)?;
            }
            proto::install_operation::Type::ReplaceBz => {
                let decompressed = decompress_bz2(&data)?;
                self.write_to_extents(out_file, &op.dst_extents, &decompressed)?;
            }
            proto::install_operation::Type::Replace => {
                self.write_to_extents(out_file, &op.dst_extents, &data)?;
            }
            proto::install_operation::Type::Zero | proto::install_operation::Type::Discard => {
                for ext in &op.dst_extents {
                    let start = ext.start_block.unwrap_or(0) * self.block_size as u64;
                    let size = ext.num_blocks.unwrap_or(0) * self.block_size as u64;
                    out_file.seek(std::io::SeekFrom::Start(start))?;
                    // 由于文件已预分配为零，可以跳过大量零写入
                    // 但为确保正确性，仍写入零
                    let zeros = vec![0u8; size as usize];
                    out_file.write_all(&zeros)?;
                }
            }
            proto::install_operation::Type::Zstd => {
                let decompressed = decompress_zstd(&data)?;
                self.write_to_extents(out_file, &op.dst_extents, &decompressed)?;
            }
            proto::install_operation::Type::SourceCopy => {
                // SOURCE_COPY 需要旧分区，非差分 OTA 不应出现
                bail!("SOURCE_COPY operation requires differential OTA (old partition). Not supported in this mode.");
            }
            proto::install_operation::Type::SourceBsdiff
            | proto::install_operation::Type::BrotliBsdiff => {
                bail!(
                    "BSDIFF operation type ({:?}) requires differential OTA. Not supported.",
                    op_type
                );
            }
            proto::install_operation::Type::Puffdiff
            | proto::install_operation::Type::Zucchini
            | proto::install_operation::Type::Lz4diffBsdiff
            | proto::install_operation::Type::Lz4diffPuffdiff => {
                bail!("Operation type {:?} is not supported yet", op_type);
            }
            _ => {
                bail!("Unknown operation type: {:?}", op_type);
            }
        }

        Ok(())
    }

    /// 将数据写入目标 extents
    fn write_to_extents(
        &self,
        out_file: &mut std::fs::File,
        extents: &[proto::Extent],
        data: &[u8],
    ) -> Result<()> {
        use std::io::Seek;

        let mut data_pos = 0usize;
        for ext in extents {
            let start = ext.start_block.unwrap_or(0) * self.block_size as u64;
            let size = (ext.num_blocks.unwrap_or(0) * self.block_size as u64) as usize;

            let end = std::cmp::min(data_pos + size, data.len());
            if data_pos >= data.len() {
                break;
            }

            out_file.seek(std::io::SeekFrom::Start(start))?;
            out_file.write_all(&data[data_pos..end])?;
            data_pos = end;
        }

        Ok(())
    }
}

// ===== 解压函数 =====

fn decompress_xz(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = xz2::read::XzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .context("XZ decompression failed")?;
    Ok(out)
}

fn decompress_bz2(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = bzip2::read::BzDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .context("BZ2 decompression failed")?;
    Ok(out)
}

fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = zstd::stream::read::Decoder::new(data)?;
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .context("ZSTD decompression failed")?;
    Ok(out)
}

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

/// hex 编码辅助
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
