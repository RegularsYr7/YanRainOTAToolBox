use anyhow::{bail, Context, Result};

use super::reader::ReadAt;

/// ZIP 格式常量
const ZIP_EOCD_MAGIC: &[u8; 4] = b"PK\x05\x06";
const ZIP_EOCD_SIZE: usize = 22;
const ZIP64_EOCD_LOCATOR_MAGIC: &[u8; 4] = b"PK\x06\x07";
const ZIP64_EOCD_LOCATOR_SIZE: usize = 20;
const ZIP64_EOCD_MAGIC: &[u8; 4] = b"PK\x06\x06";
const ZIP64_EOCD_SIZE: usize = 56;
const ZIP_CDFH_MAGIC: &[u8; 4] = b"PK\x01\x02";
const ZIP_CDFH_SIZE: usize = 46;
const ZIP_LFH_MAGIC: &[u8; 4] = b"PK\x03\x04";
const ZIP_LFH_SIZE: usize = 30;
const ZIP_MAX_COMMENT: usize = (1 << 16) - 1;
const ZIP_STORED: u16 = 0;

/// 返回 ZIP 中名为 `name` 的未压缩条目的 (数据偏移, 条目大小)
///
/// 文件名匹配支持：精确匹配和后缀匹配（如 `payload.bin` 会匹配 `firmware-update/payload.bin`）
/// 此函数通过 ReadAt 只读取必要的字节，不需要把整个 ZIP 加载到内存。
/// 参考 `5ec1cff/payload-dumper` 的 `ziputil.py`
pub fn get_zip_stored_entry_offset(file: &dyn ReadAt, name: &str) -> Result<(u64, u64)> {
    let sz = file.size();
    if sz < ZIP_EOCD_SIZE as u64 {
        bail!("File too small to be a ZIP");
    }

    // 查找 EOCD
    let (eocd_data, _eocd_off) = find_eocd(file, sz)?;
    tracing::debug!("Found EOCD at offset {}", _eocd_off);

    // 解析 EOCD
    let cd_num_raw = u16_le(&eocd_data, 8) as u64;
    let cd_sz_raw = u32_le(&eocd_data, 12) as u64;
    let cd_off_raw = u32_le(&eocd_data, 16) as u64;

    let (cd_num, cd_sz, cd_off) =
        if cd_num_raw == 0xFFFF || cd_sz_raw == 0xFFFFFFFF || cd_off_raw == 0xFFFFFFFF {
            // ZIP64: 需要定位 ZIP64 EOCD Locator 和 ZIP64 EOCD
            parse_zip64_eocd(file, _eocd_off)?
        } else {
            (cd_num_raw, cd_sz_raw, cd_off_raw)
        };

    // 读取中央目录
    let cd_data = file.read_at(cd_off, cd_sz as usize)?;
    let name_bytes = name.as_bytes();

    let mut i: u64 = 0;
    let mut p: usize = 0;

    while i < cd_num && p < cd_sz as usize {
        if p + ZIP_CDFH_SIZE > cd_data.len() {
            break;
        }
        if &cd_data[p..p + 4] != ZIP_CDFH_MAGIC {
            bail!(
                "Invalid central directory file header magic at offset {}",
                p
            );
        }

        let compression = u16_le(&cd_data, p + 10);
        let mut file_compressed_size = u32_le(&cd_data, p + 20) as u64;
        let mut file_uncompressed_size = u32_le(&cd_data, p + 24) as u64;
        let file_name_len = u16_le(&cd_data, p + 28) as usize;
        let extra_field_len = u16_le(&cd_data, p + 30) as usize;
        let comment_len = u16_le(&cd_data, p + 32) as usize;
        let mut file_lfh_off = u32_le(&cd_data, p + 42) as u64;

        let cd_ent_sz = ZIP_CDFH_SIZE + file_name_len + extra_field_len + comment_len;

        // 检查文件名：精确匹配或后缀匹配（支持 dir/payload.bin 形式）
        let fn_start = p + ZIP_CDFH_SIZE;
        let fn_end = fn_start + file_name_len;
        let name_matches = if fn_end <= cd_data.len() {
            let entry_name = &cd_data[fn_start..fn_end];
            entry_name == name_bytes
                || (entry_name.len() > name_bytes.len()
                    && entry_name[entry_name.len() - name_bytes.len() - 1] == b'/'
                    && &entry_name[entry_name.len() - name_bytes.len()..] == name_bytes)
        } else {
            false
        };
        if name_matches {
            // 解析 ZIP64 扩展字段
            let extra_start = fn_end;
            let extra_end = extra_start + extra_field_len;
            if extra_end <= cd_data.len() {
                let mut ep = extra_start;
                while extra_end - ep >= 4 {
                    let header_id = u16_le(&cd_data, ep);
                    let field_size = u16_le(&cd_data, ep + 2) as usize;
                    if header_id == 1 {
                        // ZIP64 extended information
                        let mut ext_pos = ep + 4;
                        if file_uncompressed_size == 0xFFFFFFFF {
                            file_uncompressed_size = u64_le(&cd_data, ext_pos);
                            ext_pos += 8;
                        }
                        if file_compressed_size == 0xFFFFFFFF {
                            file_compressed_size = u64_le(&cd_data, ext_pos);
                            ext_pos += 8;
                        }
                        if file_lfh_off == 0xFFFFFFFF {
                            file_lfh_off = u64_le(&cd_data, ext_pos);
                        }
                    }
                    ep += field_size + 4;
                }
            }

            if compression != ZIP_STORED {
                bail!(
                    "Entry '{}' is compressed (method={}), only STORED entries are supported",
                    name,
                    compression
                );
            }

            // 读取 Local File Header 获取实际数据偏移
            let lfh_data = file.read_at(file_lfh_off, ZIP_LFH_SIZE)?;
            if &lfh_data[0..4] != ZIP_LFH_MAGIC {
                bail!("Invalid local file header magic");
            }
            let lfh_fn_len = u16_le(&lfh_data, 26) as u64;
            let lfh_extra_len = u16_le(&lfh_data, 28) as u64;
            let data_offset = file_lfh_off + ZIP_LFH_SIZE as u64 + lfh_fn_len + lfh_extra_len;

            return Ok((data_offset, file_uncompressed_size));
        }

        p += cd_ent_sz;
        i += 1;
    }

    bail!("Entry '{}' not found in ZIP", name)
}

/// ZIP 条目信息（名称 + 大小 + 压缩信息）
pub struct ZipEntryInfo {
    pub name: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub compression: u16,
    pub lfh_offset: u64,
}

/// 列出 ZIP 中所有条目的文件名和未压缩大小
pub fn list_zip_entries_with_size(file: &dyn ReadAt) -> Result<Vec<ZipEntryInfo>> {
    let sz = file.size();
    if sz < ZIP_EOCD_SIZE as u64 {
        bail!("File too small to be a ZIP");
    }

    let (eocd_data, eocd_off) = find_eocd(file, sz)?;
    let cd_num_raw = u16_le(&eocd_data, 8) as u64;
    let cd_sz_raw = u32_le(&eocd_data, 12) as u64;
    let cd_off_raw = u32_le(&eocd_data, 16) as u64;

    let (cd_num, cd_sz, cd_off) =
        if cd_num_raw == 0xFFFF || cd_sz_raw == 0xFFFFFFFF || cd_off_raw == 0xFFFFFFFF {
            parse_zip64_eocd(file, eocd_off)?
        } else {
            (cd_num_raw, cd_sz_raw, cd_off_raw)
        };

    let cd_data = file.read_at(cd_off, cd_sz as usize)?;
    let mut entries = Vec::new();
    let mut i: u64 = 0;
    let mut p: usize = 0;

    while i < cd_num && p < cd_sz as usize {
        if p + ZIP_CDFH_SIZE > cd_data.len() {
            break;
        }
        if &cd_data[p..p + 4] != ZIP_CDFH_MAGIC {
            break;
        }

        let compression = u16_le(&cd_data, p + 10);
        let mut comp_size = u32_le(&cd_data, p + 20) as u64;
        let mut uncomp_size = u32_le(&cd_data, p + 24) as u64;
        let file_name_len = u16_le(&cd_data, p + 28) as usize;
        let extra_field_len = u16_le(&cd_data, p + 30) as usize;
        let comment_len = u16_le(&cd_data, p + 32) as usize;
        let mut lfh_offset = u32_le(&cd_data, p + 42) as u64;

        // 解析 ZIP64 扩展字段
        let extra_start = p + ZIP_CDFH_SIZE + file_name_len;
        let extra_end = extra_start + extra_field_len;
        if extra_end <= cd_data.len() {
            let mut ep = extra_start;
            while extra_end - ep >= 4 {
                let header_id = u16_le(&cd_data, ep);
                let field_size = u16_le(&cd_data, ep + 2) as usize;
                if header_id == 1 {
                    let mut ext_pos = ep + 4;
                    if uncomp_size == 0xFFFFFFFF && ext_pos + 8 <= extra_end {
                        uncomp_size = u64_le(&cd_data, ext_pos);
                        ext_pos += 8;
                    }
                    if comp_size == 0xFFFFFFFF && ext_pos + 8 <= extra_end {
                        comp_size = u64_le(&cd_data, ext_pos);
                        ext_pos += 8;
                    }
                    if lfh_offset == 0xFFFFFFFF && ext_pos + 8 <= extra_end {
                        lfh_offset = u64_le(&cd_data, ext_pos);
                    }
                }
                ep += field_size + 4;
            }
        }

        let fn_start = p + ZIP_CDFH_SIZE;
        let fn_end = fn_start + file_name_len;
        if fn_end <= cd_data.len() {
            if let Ok(name) = std::str::from_utf8(&cd_data[fn_start..fn_end]) {
                entries.push(ZipEntryInfo {
                    name: name.to_string(),
                    uncompressed_size: uncomp_size,
                    compressed_size: comp_size,
                    compression,
                    lfh_offset,
                });
            }
        }

        p += ZIP_CDFH_SIZE + file_name_len + extra_field_len + comment_len;
        i += 1;
    }

    Ok(entries)
}

const ZIP_DEFLATE: u16 = 8;

/// 通过 ReadAt 从 ZIP 中提取单个文件到本地路径
/// 支持 STORED(0) 和 DEFLATE(8) 两种压缩方式
pub fn extract_zip_entry_to_file(
    reader: &dyn ReadAt,
    entry: &ZipEntryInfo,
    output_path: &std::path::Path,
) -> Result<()> {
    use std::io::{Read, Write};

    // 读取 Local File Header 获取数据实际偏移
    let lfh_data = reader.read_at(entry.lfh_offset, ZIP_LFH_SIZE)?;
    if &lfh_data[0..4] != ZIP_LFH_MAGIC {
        bail!("Invalid local file header magic for '{}'", entry.name);
    }
    let lfh_fn_len = u16_le(&lfh_data, 26) as u64;
    let lfh_extra_len = u16_le(&lfh_data, 28) as u64;
    let data_offset = entry.lfh_offset + ZIP_LFH_SIZE as u64 + lfh_fn_len + lfh_extra_len;

    // 读取压缩数据
    let compressed_data = reader.read_at(data_offset, entry.compressed_size as usize)?;

    // 创建输出文件
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out_file = std::fs::File::create(output_path)
        .with_context(|| format!("创建文件失败: {:?}", output_path))?;

    match entry.compression {
        ZIP_STORED => {
            // 直接写入
            out_file.write_all(&compressed_data)?;
        }
        ZIP_DEFLATE => {
            // DEFLATE 解压（raw deflate，不含 zlib header）
            let mut decoder = flate2::read::DeflateDecoder::new(&compressed_data[..]);
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = decoder
                    .read(&mut buf)
                    .with_context(|| format!("解压失败: {}", entry.name))?;
                if n == 0 {
                    break;
                }
                out_file.write_all(&buf[..n])?;
            }
        }
        other => {
            bail!(
                "不支持的压缩方式 {} (文件: {})，仅支持 STORED 和 DEFLATE",
                other,
                entry.name
            );
        }
    }

    out_file.flush()?;
    Ok(())
}

/// 列出 ZIP 中所有条目的文件名（用于诊断）
pub fn list_zip_entry_names(file: &dyn ReadAt) -> Result<Vec<String>> {
    let sz = file.size();
    if sz < ZIP_EOCD_SIZE as u64 {
        bail!("File too small to be a ZIP");
    }

    let (eocd_data, eocd_off) = find_eocd(file, sz)?;

    let cd_num_raw = u16_le(&eocd_data, 8) as u64;
    let cd_sz_raw = u32_le(&eocd_data, 12) as u64;
    let cd_off_raw = u32_le(&eocd_data, 16) as u64;

    let (cd_num, cd_sz, cd_off) =
        if cd_num_raw == 0xFFFF || cd_sz_raw == 0xFFFFFFFF || cd_off_raw == 0xFFFFFFFF {
            parse_zip64_eocd(file, eocd_off)?
        } else {
            (cd_num_raw, cd_sz_raw, cd_off_raw)
        };

    let cd_data = file.read_at(cd_off, cd_sz as usize)?;
    let mut names = Vec::new();
    let mut i: u64 = 0;
    let mut p: usize = 0;

    while i < cd_num && p < cd_sz as usize {
        if p + ZIP_CDFH_SIZE > cd_data.len() {
            break;
        }
        if &cd_data[p..p + 4] != ZIP_CDFH_MAGIC {
            break;
        }
        let file_name_len = u16_le(&cd_data, p + 28) as usize;
        let extra_field_len = u16_le(&cd_data, p + 30) as usize;
        let comment_len = u16_le(&cd_data, p + 32) as usize;

        let fn_start = p + ZIP_CDFH_SIZE;
        let fn_end = fn_start + file_name_len;
        if fn_end <= cd_data.len() {
            if let Ok(name) = std::str::from_utf8(&cd_data[fn_start..fn_end]) {
                names.push(name.to_string());
            }
        }

        p += ZIP_CDFH_SIZE + file_name_len + extra_field_len + comment_len;
        i += 1;
    }

    Ok(names)
}

// ===== 内部辅助函数 =====

fn find_eocd(file: &dyn ReadAt, sz: u64) -> Result<(Vec<u8>, u64)> {
    // 先尝试无注释的 EOCD（文件末尾 22 字节）
    let data = file.read_at(sz - ZIP_EOCD_SIZE as u64, ZIP_EOCD_SIZE)?;
    if data.len() == ZIP_EOCD_SIZE
        && &data[0..4] == ZIP_EOCD_MAGIC
        && &data[ZIP_EOCD_SIZE - 2..] == &[0, 0]
    {
        return Ok((data, sz - ZIP_EOCD_SIZE as u64));
    }

    // 有注释 / 签名块的情况下，需要回溯搜索 EOCD 魔数
    let try_sz = std::cmp::min(ZIP_MAX_COMMENT + ZIP_EOCD_SIZE, sz as usize);
    let start = sz - try_sz as u64;
    let data = file.read_at(start, try_sz)?;

    // 从后往前扫描 PK\x05\x06 魔数
    // 搜索起始位置：data 末尾减去 ZIP_EOCD_SIZE（EOCD 至少 22 字节）
    let scan_end = if try_sz >= ZIP_EOCD_SIZE {
        try_sz - ZIP_EOCD_SIZE
    } else {
        0
    };
    for pos in (0..=scan_end).rev() {
        if pos + 4 > data.len() {
            continue;
        }
        if &data[pos..pos + 4] != ZIP_EOCD_MAGIC {
            continue;
        }
        // 确保能读取完整 EOCD
        if pos + ZIP_EOCD_SIZE > data.len() {
            continue;
        }
        let comment_len = u16_le(&data, pos + 20) as u64;
        let eocd_abs_off = start + pos as u64;
        // 验证: EOCD 起始 + 22 + comment_len == 文件大小
        if eocd_abs_off + ZIP_EOCD_SIZE as u64 + comment_len == sz {
            return Ok((data[pos..pos + ZIP_EOCD_SIZE].to_vec(), eocd_abs_off));
        }
    }

    bail!("Not a valid ZIP file: EOCD not found")
}

fn parse_zip64_eocd(file: &dyn ReadAt, eocd_off: u64) -> Result<(u64, u64, u64)> {
    let locator_off = eocd_off
        .checked_sub(ZIP64_EOCD_LOCATOR_SIZE as u64)
        .context("Not enough space for ZIP64 EOCD Locator")?;

    let loc_data = file.read_at(locator_off, ZIP64_EOCD_LOCATOR_SIZE)?;
    if &loc_data[0..4] != ZIP64_EOCD_LOCATOR_MAGIC {
        bail!("ZIP64 EOCD Locator magic mismatch");
    }

    let eocd64_off = u64_le(&loc_data, 8);
    let eocd64_data = file.read_at(eocd64_off, ZIP64_EOCD_SIZE)?;
    if &eocd64_data[0..4] != ZIP64_EOCD_MAGIC {
        bail!("ZIP64 EOCD magic mismatch");
    }

    let cd_num = u64_le(&eocd64_data, 32);
    let cd_sz = u64_le(&eocd64_data, 40);
    let cd_off = u64_le(&eocd64_data, 48);

    Ok((cd_num, cd_sz, cd_off))
}

#[inline]
fn u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[inline]
fn u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}
