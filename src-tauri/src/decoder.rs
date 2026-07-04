use crate::models::AppConfig;
use crate::scanner::{ScanEvent, Scanner};
use crate::utils::unix_timestamp;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct DecoderRunSummary {
    pub(crate) executed: bool,
    pub(crate) scanned: usize,
    pub(crate) processed: usize,
    pub(crate) skipped: usize,
    pub(crate) failed: usize,
    pub(crate) output_dir: String,
    pub(crate) scan_directory_count: usize,
    pub(crate) message: String,
}

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
                "已扫描 {} 个文件，处理 {} 个，跳过 {} 个，失败 {} 个",
                report.scanned, report.processed, report.skipped, report.failed,
            );
            write_decoder_operation_log(
                config,
                "解码扫描完成",
                &format!(
                    "已扫描={} | 已处理={} | 已跳过={} | 失败={}",
                    report.scanned, report.processed, report.skipped, report.failed,
                ),
            );
            return DecoderRunSummary {
                executed: true,
                scanned: report.scanned,
                processed: report.processed,
                skipped: report.skipped,
                failed: report.failed,
                output_dir: output_dir.to_string_lossy().to_string(),
                scan_directory_count,
                message,
            };
        }
        Err(err) => {
            write_decoder_error_log(config, None, "scan", &err.to_string());
            return DecoderRunSummary {
                executed: true,
                failed: 1,
                output_dir: output_dir.to_string_lossy().to_string(),
                scan_directory_count,
                message: format!("解码扫描失败: {err}"),
                ..DecoderRunSummary::default()
            };
        }
    }
}

fn process_formats(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(|format| format.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|format| !format.is_empty())
        .collect()
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(";")
}

fn join_formats(formats: &BTreeSet<String>) -> String {
    if formats.is_empty() {
        "默认".to_string()
    } else {
        formats.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

fn write_decoder_operation_log(config: &AppConfig, action: &str, detail: &str) {
    let log_dir = PathBuf::from(&config.cache.log_dir);
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("decoder.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let _ = writeln!(
        file,
        "[{}] 解码操作 | 操作={} | 详情=\"{}\"",
        unix_timestamp(),
        action,
        log_value(detail),
    );
}

fn write_decoder_info_log(config: &AppConfig, source: &Path, output: &Path, kind: &str) {
    let log_dir = PathBuf::from(&config.cache.log_dir);
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("decoder.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let _ = writeln!(
        file,
        "[{}] 解码成功 | 类型={} | 源文件=\"{}\" | 输出=\"{}\" | 源文件大小已重置为0",
        unix_timestamp(),
        kind,
        log_value(&source.to_string_lossy()),
        log_value(&output.to_string_lossy()),
    );
}

fn write_decoder_error_log(config: &AppConfig, path: Option<&Path>, stage: &str, reason: &str) {
    let log_dir = PathBuf::from(&config.cache.log_dir);
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("decoder.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let song = path
        .map(|path| log_value(&path.to_string_lossy()))
        .unwrap_or_else(|| "无".to_string());
    let _ = writeln!(
        file,
        "[{}] 解码失败 | 文件=\"{}\" | 阶段={} | 原因=\"{}\"",
        unix_timestamp(),
        song,
        stage,
        log_value(reason),
    );
}

fn log_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
