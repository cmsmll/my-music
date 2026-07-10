//! 应用日志工具。
//!
//! 按日志种类维护独立文件句柄，配置中的日志目录变化时会重新打开所有日志文件。

use crate::utils::current_app_dir;
use chrono::Local;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

static LOG_DIR: LazyLock<Mutex<PathBuf>> =
    LazyLock::new(|| Mutex::new(current_app_dir().join("logs")));
static APP_LOG: LazyLock<Mutex<File>> =
    LazyLock::new(|| Mutex::new(open_log_file_or_fallback(LogKind::App)));
static AUDIO_LOG: LazyLock<Mutex<File>> =
    LazyLock::new(|| Mutex::new(open_log_file_or_fallback(LogKind::Audio)));
static DECODER_LOG: LazyLock<Mutex<File>> =
    LazyLock::new(|| Mutex::new(open_log_file_or_fallback(LogKind::Decoder)));
static LIBRARY_LOG: LazyLock<Mutex<File>> =
    LazyLock::new(|| Mutex::new(open_log_file_or_fallback(LogKind::Library)));
static LYRICS_LOG: LazyLock<Mutex<File>> =
    LazyLock::new(|| Mutex::new(open_log_file_or_fallback(LogKind::Lyrics)));

#[derive(Clone, Copy)]
/// 日志文件分类。
pub(crate) enum LogKind {
    /// 应用通用日志。
    App,
    /// 前端音频播放错误日志。
    Audio,
    /// 解码器操作日志。
    Decoder,
    /// 曲库扫描和缓存日志。
    Library,
    /// 歌词搜索和缓存日志。
    Lyrics,
}

impl LogKind {
    /// 返回日志种类对应的文件名。
    fn file_name(self) -> &'static str {
        match self {
            Self::App => "app.log",
            Self::Audio => "audio.log",
            Self::Decoder => "decoder.log",
            Self::Library => "library.log",
            Self::Lyrics => "lyrics.log",
        }
    }

    /// 返回日志种类对应的全局文件句柄。
    fn file(self) -> &'static Mutex<File> {
        match self {
            Self::App => &APP_LOG,
            Self::Audio => &AUDIO_LOG,
            Self::Decoder => &DECODER_LOG,
            Self::Library => &LIBRARY_LOG,
            Self::Lyrics => &LYRICS_LOG,
        }
    }
}

#[derive(Clone, Copy)]
/// 日志级别。
enum LogLevel {
    /// 普通信息。
    Info,
    /// 警告信息。
    Warn,
    /// 错误信息。
    Error,
}

impl LogLevel {
    /// 返回写入文件时显示的中文日志级别。
    const fn label(self) -> &'static str {
        match self {
            Self::Info => "信息",
            Self::Warn => "警告",
            Self::Error => "错误",
        }
    }
}

/// 更新日志目录，并重新打开所有日志文件。
pub(crate) fn set_log_dir(log_dir: impl Into<PathBuf>) {
    let log_dir = log_dir.into();
    if let Ok(mut current_log_dir) = LOG_DIR.lock() {
        if *current_log_dir == log_dir {
            return;
        }
        *current_log_dir = log_dir;
    }

    reopen_log_file(LogKind::App);
    reopen_log_file(LogKind::Audio);
    reopen_log_file(LogKind::Decoder);
    reopen_log_file(LogKind::Library);
    reopen_log_file(LogKind::Lyrics);
}

/// 写入信息日志。
pub(crate) fn info(kind: LogKind, message: impl Into<String>) {
    write(kind, LogLevel::Info, message);
}

/// 写入警告日志。
pub(crate) fn warn(kind: LogKind, message: impl Into<String>) {
    write(kind, LogLevel::Warn, message);
}

/// 写入错误日志。
pub(crate) fn error(kind: LogKind, message: impl Into<String>) {
    write(kind, LogLevel::Error, message);
}

/// 按统一格式写入一行日志。
fn write(kind: LogKind, level: LogLevel, message: impl Into<String>) {
    let Ok(mut file) = kind.file().lock() else {
        return;
    };

    let _ = writeln!(
        file,
        "[{}] [{}] {}",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        level.label(),
        message.into(),
    );
}

/// 重新打开指定种类的日志文件。
fn reopen_log_file(kind: LogKind) {
    let next_file = open_log_file_or_fallback(kind);
    if let Ok(mut file) = kind.file().lock() {
        *file = next_file;
    }
}

/// 打开日志文件；失败时回退到系统临时目录。
fn open_log_file_or_fallback(kind: LogKind) -> File {
    open_log_file(kind).unwrap_or_else(|_| {
        let fallback_dir = std::env::temp_dir().join("my-music-log-cache");
        let _ = fs::create_dir_all(&fallback_dir);
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(fallback_dir.join(kind.file_name()))
            .unwrap_or_else(|err| panic!("无法打开日志文件: {err}"))
    })
}

/// 在当前日志目录中打开指定日志文件。
fn open_log_file(kind: LogKind) -> Result<File, String> {
    let log_dir = LOG_DIR
        .lock()
        .map_err(|_| "日志目录状态不可用".to_string())?
        .clone();
    fs::create_dir_all(&log_dir).map_err(|err| format!("无法创建日志目录: {err}"))?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(kind.file_name()))
        .map_err(|err| format!("无法打开日志文件: {err}"))
}
