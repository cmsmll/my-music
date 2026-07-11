//! 解码扫描器。
//!
//! 根据配置扫描解码目录，识别 KGM/KGMA、NCM 和普通 MP3/FLAC，并把处理后的文件写入输出目录。
//! 成功处理后会把源文件截断为 0 字节，用于标记已经处理过。

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use super::kgm::*;
use super::ncm::*;

/// 扫描时预读的字节数。
///
/// 这段数据只用来判断文件格式：KGMA/KGM 看魔数，普通音频交给 `infer` 判断。
const PROBE_LEN: usize = 8192;

#[derive(Debug, Clone)]
/// 解码文件扫描器。
///
/// 注意：只扫描配置目录的本层文件，不递归处理子目录。
pub struct Scanner {
    /// 需要扫描的目录列表。每个目录只扫描本层文件。
    pub scan_dirs: Vec<PathBuf>,
    /// 所有处理结果统一输出到这个目录。
    pub output_dir: PathBuf,
    process_formats: BTreeSet<String>,
}

impl Scanner {
    /// 创建扫描器，默认处理 kgm、kgma、ncm、mp3、flac。
    pub fn new(scan_dirs: Vec<PathBuf>, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            scan_dirs,
            output_dir: output_dir.into(),
            process_formats: default_process_formats(),
        }
    }

    /// 覆盖默认处理格式。
    pub fn with_process_formats(mut self, process_formats: BTreeSet<String>) -> Self {
        if !process_formats.is_empty() {
            self.process_formats = process_formats;
        }
        self
    }

    /// 扫描所有目录并处理可识别文件。
    ///
    /// - KGMA/KGM：调用 `KgmDecoder` 解码后输出。
    /// - MP3/FLAC：不做转码或改写，直接复制到输出目录。
    /// - 其他文件：跳过。
    /// - 已成功处理的源文件：保留文件名和 inode，但截断为 0 字节。
    pub fn scan_with_progress(
        &self,
        mut on_event: impl FnMut(ScanEvent),
    ) -> Result<ScanReport, ScannerError> {
        fs::create_dir_all(&self.output_dir)?;

        let mut report = ScanReport::default();

        for dir in &self.scan_dirs {
            self.scan_dir(dir, &mut report, &mut on_event)?;
        }

        Ok(report)
    }

    /// 扫描单个目录的本层文件。
    fn scan_dir(
        &self,
        dir: &Path,
        report: &mut ScanReport,
        on_event: &mut impl FnMut(ScanEvent),
    ) -> Result<(), ScannerError> {
        let meta = fs::metadata(dir)?;
        if !meta.is_dir() {
            return Err(ScannerError::NotDirectory(dir.to_path_buf()));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let meta = entry.metadata()?;

            if meta.is_dir() {
                continue;
            }

            if !meta.is_file() {
                continue;
            }

            if !self.is_supported_ext(&path) {
                continue;
            }

            report.scanned += 1;

            if meta.len() == 0 {
                report.skipped += 1;
                on_event(ScanEvent::Skipped(ScanSkipped {
                    path,
                    reason: "源文件大小为0，跳过处理".to_string(),
                }));
                continue;
            }

            match self.process_file(&path) {
                Ok(Some(result)) => {
                    report.processed += 1;
                    report.results.push(result.clone());
                    on_event(ScanEvent::Processed(result));
                }
                Ok(None) => {
                    report.skipped += 1;
                    on_event(ScanEvent::Skipped(ScanSkipped {
                        path,
                        reason: "无法识别为可处理音频，跳过处理".to_string(),
                    }));
                }
                Err(err) => {
                    report.failed += 1;
                    let file_error = FileError { path, error: err };
                    report.errors.push(file_error.clone());
                    on_event(ScanEvent::Failed(file_error));
                }
            }
        }

        Ok(())
    }

    /// 处理单个文件并返回输出结果。
    fn process_file(&self, path: &Path) -> Result<Option<ScanResult>, ScannerError> {
        // 1. 打开源文件并读取一小段头部数据，用来识别文件结构。
        let mut input = BufReader::new(File::open(path)?);
        let mut probe = vec![0_u8; PROBE_LEN];
        let len = input.read(&mut probe)?;
        probe.truncate(len);

        let Some(kind) = FileKind::detect(path, &probe) else {
            return Ok(None);
        };

        match kind {
            FileKind::KugouEncrypted => {
                // 3a. KGMA/KGM 需要重新打开文件，让解码器从文件开头读取并消费加密头。
                let input = BufReader::new(File::open(path)?);
                let mut decoder = KgmDecoder::new(input)?;

                // 先解出一小段明文头，判断真实音频格式后再决定输出扩展名。
                let mut decoded_probe = vec![0_u8; PROBE_LEN];
                let len = decoder.read(&mut decoded_probe)?;
                decoded_probe.truncate(len);

                let output_path =
                    self.available_output_path(path, infer_audio_extension(&decoded_probe));
                let mut output = BufWriter::new(File::create(&output_path)?);
                output.write_all(&decoded_probe)?;
                io::copy(&mut decoder, &mut output)?;
                output.flush()?;

                // 4. 处理成功后不删除源文件，只把长度截断为 0。
                truncate_to_empty(path)?;

                Ok(Some(ScanResult {
                    source: path.to_path_buf(),
                    output: output_path,
                    kind,
                }))
            }
            FileKind::NeteaseEncrypted => {
                // 3b. NCM 同样是容器加密格式，需要先解析容器头再还原音频流。
                let input = BufReader::new(File::open(path)?);
                let mut decoder = NcmDecoder::new(input)?;

                let metadata = decoder.metadata().clone();
                let cover = decoder.cover().to_vec();
                let mut audio = Vec::new();
                decoder.read_to_end(&mut audio)?;

                let ext = metadata
                    .format
                    .as_deref()
                    .filter(|value| *value == "mp3" || *value == "flac")
                    .unwrap_or_else(|| infer_audio_extension(&audio));
                let output_path = self.available_output_path(path, ext);
                let mut output = BufWriter::new(File::create(&output_path)?);
                write_tagged_audio(&mut output, &audio, ext, &metadata, &cover)?;
                output.flush()?;

                truncate_to_empty(path)?;

                Ok(Some(ScanResult {
                    source: path.to_path_buf(),
                    output: output_path,
                    kind,
                }))
            }
            FileKind::Mp3 | FileKind::Flac => {
                // 3b. MP3/FLAC 已经是普通音频，不做特殊处理，直接复制原始字节。
                let output_path = self.available_output_path(path, kind.extension());
                let mut output = BufWriter::new(File::create(&output_path)?);
                output.write_all(&probe)?;
                io::copy(&mut input, &mut output)?;
                output.flush()?;

                // 4. 处理成功后不删除源文件，只把长度截断为 0。
                truncate_to_empty(path)?;

                Ok(Some(ScanResult {
                    source: path.to_path_buf(),
                    output: output_path,
                    kind,
                }))
            }
        }
    }

    /// 根据源文件名和真实音频格式生成输出路径。
    fn available_output_path(&self, source: &Path, ext: &str) -> PathBuf {
        let stem = source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");

        self.output_dir.join(format!("{stem}.{ext}"))
    }

    /// 判断文件扩展名是否在当前处理格式列表内。
    fn is_supported_ext(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|value| value.to_str())
            .map(|ext| self.process_formats.contains(&ext.to_ascii_lowercase()))
            .unwrap_or(false)
    }
}

#[derive(Debug, Default, Clone)]
/// 单次扫描的统计报告。
pub struct ScanReport {
    /// 扫描到的可处理扩展名文件数，包含已处理、跳过和失败的文件。
    pub scanned: usize,
    /// 成功解码文件数。
    pub processed: usize,
    /// 跳过文件数。
    pub skipped: usize,
    /// 失败文件数。
    pub failed: usize,
    /// 成功处理结果列表。
    pub results: Vec<ScanResult>,
    /// 失败文件列表。
    pub errors: Vec<FileError>,
}

#[derive(Debug, Clone)]
/// 单个文件处理成功结果。
pub struct ScanResult {
    /// 源文件路径。
    pub source: PathBuf,
    /// 输出文件路径。
    pub output: PathBuf,
    /// 识别到的文件类型。
    pub kind: FileKind,
}

#[derive(Debug, Clone)]
/// 单个文件处理失败结果。
pub struct FileError {
    /// 失败文件路径。
    pub path: PathBuf,
    /// 失败原因。
    pub error: ScannerError,
}

#[derive(Debug, Clone)]
/// 单个文件跳过结果。
pub struct ScanSkipped {
    /// 被跳过的文件路径。
    pub path: PathBuf,
    /// 跳过原因。
    pub reason: String,
}

#[derive(Debug, Clone)]
/// 扫描过程事件。
pub enum ScanEvent {
    /// 文件处理成功。
    Processed(ScanResult),
    /// 文件被跳过。
    Skipped(ScanSkipped),
    /// 文件处理失败。
    Failed(FileError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 解码扫描器识别出的文件类型。
pub enum FileKind {
    /// 酷狗 KGM/KGMA 加密音频。
    KugouEncrypted,
    /// 网易云 NCM 加密音频。
    NeteaseEncrypted,
    /// 普通 MP3 音频。
    Mp3,
    /// 普通 FLAC 音频。
    Flac,
}

impl FileKind {
    /// 根据扩展名和文件头识别待处理文件类型。
    fn detect(path: &Path, probe: &[u8]) -> Option<Self> {
        if is_kugou_ext(path) || is_kugou_header(probe) {
            return Some(Self::KugouEncrypted);
        }
        if is_ncm_ext(path) || is_ncm_header(probe) {
            return Some(Self::NeteaseEncrypted);
        }

        match infer::get(probe).map(|kind| kind.extension()) {
            Some("mp3") => Some(Self::Mp3),
            Some("flac") => Some(Self::Flac),
            _ => None,
        }
    }

    /// 返回该类型默认输出扩展名。
    fn extension(self) -> &'static str {
        match self {
            Self::KugouEncrypted => "flac",
            Self::NeteaseEncrypted => "mp3",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
        }
    }
}

#[derive(Debug, Clone)]
/// 解码扫描器错误。
pub enum ScannerError {
    /// 文件系统错误。
    Io(String),
    /// KGM/KGMA 解码错误。
    Kgm(String),
    /// NCM 解码错误。
    Ncm(String),
    /// 配置的扫描路径不是目录。
    NotDirectory(PathBuf),
}

impl fmt::Display for ScannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScannerError::Io(err) => write!(f, "文件系统错误: {err}"),
            ScannerError::Kgm(err) => write!(f, "KGMA/KGM 解码失败: {err}"),
            ScannerError::Ncm(err) => write!(f, "NCM 解码失败: {err}"),
            ScannerError::NotDirectory(path) => write!(f, "扫描路径不是目录: {}", path.display()),
        }
    }
}

impl std::error::Error for ScannerError {}

impl From<io::Error> for ScannerError {
    fn from(value: io::Error) -> Self {
        ScannerError::Io(value.to_string())
    }
}

impl From<KgmError> for ScannerError {
    fn from(value: KgmError) -> Self {
        ScannerError::Kgm(value.to_string())
    }
}

impl From<NcmError> for ScannerError {
    fn from(value: NcmError) -> Self {
        ScannerError::Ncm(value.to_string())
    }
}

/// 判断扩展名是否是酷狗加密格式。
fn is_kugou_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("kgm") || ext.eq_ignore_ascii_case("kgma"))
        .unwrap_or(false)
}

/// 判断扩展名是否是网易云 NCM 格式。
fn is_ncm_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("ncm"))
        .unwrap_or(false)
}

/// 默认解码器处理格式集合。
fn default_process_formats() -> BTreeSet<String> {
    ["kgm", "kgma", "ncm", "mp3", "flac"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// 判断文件头是否匹配 KGM/KGMA 魔数。
fn is_kugou_header(probe: &[u8]) -> bool {
    probe.starts_with(&KGM_MAGIC_HEADER)
}

/// 判断文件头是否匹配 NCM 魔数。
fn is_ncm_header(probe: &[u8]) -> bool {
    probe.starts_with(NCM_MAGIC_HEADER)
}

/// 根据音频明文字节推断真实输出格式。
fn infer_audio_extension(probe: &[u8]) -> &'static str {
    match infer::get(probe).map(|kind| kind.extension()) {
        Some("mp3") => "mp3",
        Some("flac") => "flac",
        _ => "mp3",
    }
}

/// 将源文件大小重置为 0，保留原路径占位。
fn truncate_to_empty(path: &Path) -> Result<(), ScannerError> {
    OpenOptions::new().write(true).truncate(true).open(path)?;
    Ok(())
}
