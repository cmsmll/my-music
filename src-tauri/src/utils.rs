//! 通用工具函数。
//!
//! 这里放跨模块复用的小工具，例如应用目录定位、安全文件名、短哈希和 JSON 缓存写入。

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

/// 获取当前应用所在目录。
pub(crate) fn current_app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 将任意名称转换为 Windows 可保存的文件名。
pub(crate) fn safe_file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let safe = safe.trim().trim_matches('.').trim().to_string();

    if safe.trim_matches('_').is_empty() {
        "music-library".to_string()
    } else if is_windows_reserved_name(&safe) {
        format!("{safe}_")
    } else {
        safe
    }
}

/// 判断文件名是否命中 Windows 保留设备名。
fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// 将名称转换为更适合缓存文件名的 ASCII 名称。
pub(crate) fn safe_cache_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    let safe = safe.trim_matches('_');
    if safe.is_empty() {
        "playlist".to_string()
    } else {
        safe.to_string()
    }
}

/// 生成稳定的 8 字节短哈希。
pub(crate) fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 返回当前 Unix 秒级时间戳。
pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 在目标文件所在目录写入临时文件，完整落盘后原子替换正式文件。
pub(crate) fn atomic_write(path: &Path, content: &[u8], label: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| format!("无法创建{label}目录: {err}"))?;

    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|err| format!("无法创建{label}临时文件: {err}"))?;
    temporary
        .write_all(content)
        .map_err(|err| format!("无法写入{label}临时文件: {err}"))?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|err| format!("无法同步{label}临时文件: {err}"))?;
    temporary
        .persist(path)
        .map_err(|err| format!("无法替换{label}: {}", err.error))?;
    Ok(())
}

/// 写入格式化 JSON 缓存文件。
pub(crate) fn write_json_cache<T: Serialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(value).map_err(|err| format!("无法序列化{label}: {err}"))?;
    atomic_write(path, content.as_bytes(), label)
}
#[cfg(test)]
mod tests {
    use super::atomic_write;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_replaces_existing_content_without_leaving_temp_files() {
        let directory = tempdir().expect("create temp directory");
        let path = directory.path().join("cache.json");
        atomic_write(&path, b"old", "test cache").expect("write initial content");
        atomic_write(&path, b"new content", "test cache").expect("replace content");

        assert_eq!(std::fs::read(&path).expect("read cache"), b"new content");
        let entries = std::fs::read_dir(directory.path())
            .expect("read directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect entries");
        assert_eq!(entries.len(), 1);
    }
}
