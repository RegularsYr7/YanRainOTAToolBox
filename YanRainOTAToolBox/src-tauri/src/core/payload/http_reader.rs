use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

use super::reader::ReadAt;

const RETRIES_PER_URL: u32 = 2;
const RETRY_BASE_MS: u64 = 1000;

pub struct HttpRangeReader {
    url: String,
    client: Client,
    file_size: u64,
}

impl HttpRangeReader {
    pub fn new(url: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(20))
            .build()?;

        let file_size = match client.head(url).send() {
            Ok(resp) if resp.status().is_success() => resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .context("Server did not return Content-Length")?,
            Ok(resp) => {
                tracing::warn!("HTTP HEAD returned {}, falling back to GET Range", resp.status());
                get_size_from_range(&client, url)?
            }
            Err(_) => get_size_from_range(&client, url)?,
        };

        tracing::info!("HTTP file size: {} bytes", file_size);
        Ok(Self {
            url: url.to_string(),
            client,
            file_size,
        })
    }
}

fn get_size_from_range(client: &Client, url: &str) -> Result<u64> {
    let resp = client
        .get(url)
        .header("Range", "bytes=0-0")
        .send()
        .context("GET Range bytes=0-0 request failed")?;
    if resp.status().as_u16() != 206 {
        bail!("Expected HTTP 206 for Range request, got {}", resp.status());
    }
    let content_range = resp
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .context("Server did not return Content-Range")?;
    parse_total_size(content_range)
}

fn parse_total_size(content_range: &str) -> Result<u64> {
    let total = content_range
        .rsplit('/')
        .next()
        .context("Invalid Content-Range format")?;
    total
        .trim()
        .parse::<u64>()
        .context("Failed to parse total size from Content-Range")
}

impl ReadAt for HttpRangeReader {
    fn size(&self) -> u64 {
        self.file_size
    }

    fn read_at(&self, offset: u64, size: usize) -> Result<Vec<u8>> {
        if size == 0 {
            return Ok(Vec::new());
        }
        let end = offset + size as u64 - 1;
        let range_header = format!("bytes={}-{}", offset, end);

        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 0..RETRIES_PER_URL {
            match do_read_at(&self.client, &self.url, &range_header, size) {
                Ok(data) => return Ok(data),
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < RETRIES_PER_URL {
                        let delay = Duration::from_millis(RETRY_BASE_MS * (attempt + 1) as u64);
                        tracing::warn!(
                            "HTTP Range read failed (attempt {}/{}): {}. Retrying in {:?}...",
                            attempt + 1,
                            RETRIES_PER_URL,
                            last_err.as_ref().unwrap(),
                            delay
                        );
                        std::thread::sleep(delay);
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("HTTP Range read failed")))
    }
}

fn do_read_at(client: &Client, url: &str, range_header: &str, size: usize) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .header("Range", range_header)
        .send()
        .with_context(|| format!("HTTP Range request failed: {}", range_header))?;
    if resp.status().as_u16() != 206 {
        bail!(
            "Expected HTTP 206, got {} for range {}",
            resp.status(),
            range_header
        );
    }
    let bytes = resp.bytes()?;
    if bytes.len() != size {
        bail!("Expected {} bytes, received {}", size, bytes.len());
    }
    Ok(bytes.to_vec())
}
