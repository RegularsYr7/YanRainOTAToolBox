use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

/// 统一的随机读取接口：支持本地文件和 HTTP Range 两种数据源
pub trait ReadAt: Send + Sync {
    /// 获取数据总大小
    fn size(&self) -> u64;

    /// 从 offset 读取 size 字节
    fn read_at(&self, offset: u64, size: usize) -> Result<Vec<u8>>;
}

// ============================
// 本地文件实现
// ============================

pub struct LocalFileReader {
    path: String,
    file_size: u64,
}

impl LocalFileReader {
    pub fn new(path: &str) -> Result<Self> {
        let meta = std::fs::metadata(path)?;
        Ok(Self {
            path: path.to_string(),
            file_size: meta.len(),
        })
    }
}

impl ReadAt for LocalFileReader {
    fn size(&self) -> u64 {
        self.file_size
    }

    fn read_at(&self, offset: u64, size: usize) -> Result<Vec<u8>> {
        let mut file = std::fs::File::open(&self.path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; size];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }
}
