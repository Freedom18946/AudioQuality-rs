use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::Builder;

/// 原子写入文件，避免符号链接跟随导致的外部文件覆盖风险。
pub fn atomic_write_bytes(path: &Path, data: &[u8], safe_mode: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("输出路径缺少父目录: {}", path.display()))?;

    if safe_mode {
        reject_symlink(path)?;
    }

    let mut tmp = Builder::new()
        .prefix(".audio_quality_tmp_")
        .tempfile_in(parent)
        .with_context(|| format!("无法在目录中创建临时文件: {}", parent.display()))?;

    tmp.write_all(data)
        .with_context(|| format!("写入临时文件失败: {}", path.display()))?;
    tmp.as_file()
        .sync_all()
        .with_context(|| format!("同步临时文件失败: {}", path.display()))?;

    if safe_mode {
        reject_symlink(path)?;
    }

    tmp.persist(path)
        .map_err(|e| anyhow!(e.error))
        .with_context(|| format!("原子写入失败: {}", path.display()))?;

    Ok(())
}

/// 原子写入字符串。
pub fn atomic_write_string(path: &Path, content: &str, safe_mode: bool) -> Result<()> {
    atomic_write_bytes(path, content.as_bytes(), safe_mode)
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(anyhow!(
            "检测到符号链接输出路径，已拒绝写入: {}",
            path.display()
        )),
        Ok(_) | Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write_string_basic() {
        let dir = TempDir::new().expect("tempdir");
        let output = dir.path().join("out.txt");
        atomic_write_string(&output, "hello", true).expect("write failed");
        let content = std::fs::read_to_string(&output).expect("read failed");
        assert_eq!(content, "hello");
    }

    #[cfg(unix)]
    #[test]
    fn test_atomic_write_reject_symlink() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "old").expect("write old");

        let link = dir.path().join("out.txt");
        symlink(&target, &link).expect("symlink");

        let err = atomic_write_string(&link, "new", true).expect_err("should reject symlink");
        assert!(err.to_string().contains("符号链接"));
        let content = std::fs::read_to_string(&target).expect("read target");
        assert_eq!(content, "old");
    }
}
