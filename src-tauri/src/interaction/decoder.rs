//! 解码功能的前端交互调度。
//!
//! 这个模块读取配置中的扫描目录、输出目录和处理格式，调用底层 `decoder`
//! 功能域执行实际解码，并整理成前端弹窗需要展示的统计信息。

use super::models::*;
use crate::decoder::*;
use crate::logger::{self, LogKind};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, Serialize)]
/// 解码任务执行摘要。
pub(crate) struct DecoderRunSummary {
    /// 是否真正执行了解码扫描。
    pub(crate) executed: bool,
    /// 扫描到的文件数。
    pub(crate) scanned: usize,
    /// 成功解码的文件数。
    pub(crate) processed: usize,
    /// 跳过的文件数。
    pub(crate) skipped: usize,
    /// 失败的文件数。
    pub(crate) failed: usize,
    /// 解码输出目录。
    pub(crate) output_dir: String,
    /// 有效扫描目录数量。
    pub(crate) scan_directory_count: usize,
    /// 给前端弹窗展示的摘要消息。
    pub(crate) message: String,
}

/// 按当前配置执行一次解码任务，并返回前端可直接展示的摘要。
///
/// 注意：调用方应放到阻塞任务中执行，避免扫描和文件写入卡住 Tauri 主线程。
pub(crate) fn run_decoder(config: &AppConfig) -> DecoderRunSummary {
    let output_dir = config.decoder.output_dir.trim();
    if output_dir.is_empty() {
        write_decoder_operation_log(config, "跳过解码", "输出目录为空");
        return DecoderRunSummary {
            message: "输出目录为空，未执行解码".to_string(),
            ..DecoderRunSummary::default()
        };
    }

    let invalid_scan_dirs: Vec<String> = config
        .decoder
        .scan_directory
        .iter()
        .map(|directory| directory.trim())
        .filter(|directory| !directory.is_empty() && !Path::new(directory).is_dir())
        .map(str::to_string)
        .collect();
    for directory in &invalid_scan_dirs {
        write_decoder_operation_log(
            config,
            "跳过扫描目录",
            &format!("目录不存在或不是文件夹: {directory}"),
        );
    }

    let scan_dirs: Vec<PathBuf> = config
        .decoder
        .scan_directory
        .iter()
        .map(|directory| PathBuf::from(directory.trim()))
        .filter(|directory| directory.is_dir())
        .collect();
    if scan_dirs.is_empty() {
        write_decoder_operation_log(config, "跳过解码", "没有有效扫描目录");
        return DecoderRunSummary {
            output_dir: output_dir.to_string(),
            message: "没有有效扫描目录，未执行解码".to_string(),
            ..DecoderRunSummary::default()
        };
    }

    let output_dir = PathBuf::from(output_dir);
    let formats = process_formats(&config.decoder.process_formats);
    let scan_directory_count = scan_dirs.len();
    write_decoder_operation_log(
        config,
        "开始解码扫描",
        &format!(
            "输出目录=\"{}\" | 扫描目录=\"{}\" | 处理格式={}",
            output_dir.to_string_lossy(),
            join_paths(&scan_dirs),
            join_formats(&formats),
        ),
    );
    let scanner = Scanner::new(scan_dirs, &output_dir).with_process_formats(formats);

    match scanner.scan_with_progress(|event| match event {
        ScanEvent::Processed(result) => {
            write_decoder_info_log(
                config,
                &result.source,
                &result.output,
                &format!("{:?}", result.kind),
            );
        }
        ScanEvent::Skipped(skipped) => {
            write_decoder_operation_log(
                config,
                "跳过文件",
                &format!(
                    "源文件=\"{}\" | 原因=\"{}\"",
                    skipped.path.to_string_lossy(),
                    skipped.reason,
                ),
            );
        }
        ScanEvent::Failed(error) => {
            write_decoder_error_log(
                config,
                Some(&error.path),
                "process_file",
                &error.error.to_string(),
            );
        }
    }) {
        Ok(report) => {
            let message = format!(
                "已扫描 {} 个文件，解码 {} 个，失败 {} 个，跳过 {} 个",
                report.scanned, report.processed, report.failed, report.skipped,
            );
            write_decoder_operation_log(
                config,
                "解码扫描完成",
                &format!(
                    "已扫描={} | 已解码={} | 失败={} | 已跳过={}",
                    report.scanned, report.processed, report.failed, report.skipped,
                ),
            );
            DecoderRunSummary {
                executed: true,
                scanned: report.scanned,
                processed: report.processed,
                skipped: report.skipped,
                failed: report.failed,
                output_dir: output_dir.to_string_lossy().to_string(),
                scan_directory_count,
                message,
            }
        }
        Err(err) => {
            write_decoder_error_log(config, None, "scan", &err.to_string());
            DecoderRunSummary {
                executed: true,
                failed: 1,
                output_dir: output_dir.to_string_lossy().to_string(),
                scan_directory_count,
                message: format!("解码扫描失败: {err}"),
                ..DecoderRunSummary::default()
            }
        }
    }
}

/// 解析配置中的处理格式，例如 `mp3,flac,kgm`。
fn process_formats(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(|format| format.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|format| !format.is_empty())
        .collect()
}

/// 将扫描目录列表拼成日志友好的字符串。
fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(";")
}

/// 将处理格式集合拼成日志友好的字符串。
fn join_formats(formats: &BTreeSet<String>) -> String {
    if formats.is_empty() {
        "默认".to_string()
    } else {
        formats.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

/// 记录解码流程级日志。
fn write_decoder_operation_log(_config: &AppConfig, action: &str, detail: &str) {
    logger::info(
        LogKind::Decoder,
        format!("解码操作 | 操作={} | 详情=\"{}\"", action, detail,),
    );
}

/// 记录单个文件解码成功日志。
fn write_decoder_info_log(_config: &AppConfig, source: &Path, output: &Path, kind: &str) {
    logger::info(
        LogKind::Decoder,
        format!(
            "解码成功 | 类型={} | 源文件=\"{}\" | 输出=\"{}\" | 源文件大小已重置为0",
            kind,
            &source.to_string_lossy(),
            &output.to_string_lossy(),
        ),
    );
}

/// 记录解码失败日志。
fn write_decoder_error_log(_config: &AppConfig, path: Option<&Path>, stage: &str, reason: &str) {
    let song = path
        .map(|path| path.to_string_lossy())
        .unwrap_or_else(|| "无".into());
    logger::error(
        LogKind::Decoder,
        format!(
            "解码失败 | 文件=\"{}\" | 阶段={} | 原因=\"{}\"",
            song, stage, reason,
        ),
    );
}
