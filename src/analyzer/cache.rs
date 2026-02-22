use crate::analyzer::metrics::FileMetrics;
use crate::analyzer::safe_io;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub mtime_unix_secs: u64,
    pub file_size_bytes: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    fingerprint: FileFingerprint,
    metrics: FileMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisCache {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

impl Default for AnalysisCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

impl AnalysisCache {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取缓存文件失败: {}", path.display()))?;
        let cache: AnalysisCache = serde_json::from_str(&content)
            .with_context(|| format!("解析缓存文件失败: {}", path.display()))?;

        if cache.version != CACHE_VERSION {
            return Ok(Self::default());
        }

        Ok(cache)
    }

    pub fn save(&self, path: &Path, safe_mode: bool) -> Result<()> {
        let content = serde_json::to_string_pretty(self).context("序列化缓存失败")?;
        safe_io::atomic_write_string(path, &content, safe_mode)
    }

    pub fn lookup(&self, file_path: &Path, fingerprint: &FileFingerprint) -> Option<FileMetrics> {
        let key = normalize_cache_key(file_path);
        let entry = self.entries.get(&key)?;

        if entry.fingerprint.mtime_unix_secs == fingerprint.mtime_unix_secs
            && entry.fingerprint.file_size_bytes == fingerprint.file_size_bytes
            && entry.fingerprint.content_sha256 == fingerprint.content_sha256
        {
            let mut metrics = entry.metrics.clone();
            metrics.cache_hit = true;
            return Some(metrics);
        }

        None
    }

    pub fn upsert(&mut self, file_path: &Path, fingerprint: FileFingerprint, metrics: FileMetrics) {
        let key = normalize_cache_key(file_path);
        self.entries.insert(
            key,
            CacheEntry {
                fingerprint,
                metrics,
            },
        );
    }
}

pub fn fingerprint_file(path: &Path) -> Result<FileFingerprint> {
    let metadata = path
        .metadata()
        .with_context(|| format!("读取文件元数据失败: {}", path.display()))?;

    let mtime_unix_secs = metadata
        .modified()
        .ok()
        .and_then(system_time_to_unix_secs)
        .unwrap_or(0);

    let file_size_bytes = metadata.len();
    let content_sha256 = sha256_file(path)?;

    Ok(FileFingerprint {
        mtime_unix_secs,
        file_size_bytes,
        content_sha256,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let file =
        File::open(path).with_context(|| format!("无法打开文件用于哈希: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_cache_key(path: &Path) -> String {
    let canonical: PathBuf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical.to_string_lossy().into_owned()
}

fn system_time_to_unix_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metrics() -> FileMetrics {
        FileMetrics {
            file_path: "/tmp/a.flac".to_string(),
            file_size_bytes: 1,
            lra: None,
            peak_amplitude_db: None,
            overall_rms_db: None,
            rms_db_above_16k: None,
            rms_db_above_18k: None,
            rms_db_above_20k: None,
            integrated_loudness_lufs: None,
            true_peak_dbtp: None,
            processing_time_ms: 1,
            sample_rate_hz: None,
            bitrate_kbps: None,
            channels: None,
            codec_name: None,
            container_format: None,
            duration_seconds: None,
            cache_hit: false,
            content_sha256: Some("abc".to_string()),
            error_codes: vec![],
        }
    }

    #[test]
    fn test_cache_lookup_hit() {
        let mut cache = AnalysisCache::default();
        let path = Path::new("/tmp/a.flac");
        let fp = FileFingerprint {
            mtime_unix_secs: 1,
            file_size_bytes: 1,
            content_sha256: "abc".to_string(),
        };
        cache.upsert(path, fp.clone(), sample_metrics());

        let hit = cache.lookup(path, &fp).expect("expected cache hit");
        assert!(hit.cache_hit);
    }
}
