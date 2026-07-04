use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::kgm::{KgmDecoder, KgmError};
use crate::ncm::{write_tagged_audio, NcmDecoder, NcmError};

/// 扫描时预读的字节数。
///
/// 这段数据只用来判断文件格式：KGMA/KGM 看魔数，普通音频交给 `infer` 判断。
const PROBE_LEN: usize = 8192;

#[derive(Debug, Clone)]
pub struct Scanner {
    /// 需要扫描的目录列表。每个目录只扫描本层文件。
    pub scan_dirs: Vec<PathBuf>,
    /// 所有处理结果统一输出到这个目录。
    pub output_dir: PathBuf,
    process_formats: BTreeSet<String>,
}

impl Scanner {
    pub fn new(scan_dirs: Vec<PathBuf>, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            scan_dirs,
            output_dir: output_dir.into(),
            process_formats: default_process_formats(),
        }
    }

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

            if meta.len() == 0 {
                report.skipped += 1;
                on_event(ScanEvent::Skipped(ScanSkipped {
                    path,
                    reason: "源文件大小为0，跳过处理".to_string(),
                }));
                continue;
            }

            report.scanned += 1;
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

    fn available_output_path(&self, source: &Path, ext: &str) -> PathBuf {
        let stem = source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");

        self.output_dir.join(format!("{stem}.{ext}"))
    }

    fn is_supported_ext(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|value| value.to_str())
            .map(|ext| self.process_formats.contains(&ext.to_ascii_lowercase()))
            .unwrap_or(false)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScanReport {
    pub scanned: usize,
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub results: Vec<ScanResult>,
    pub errors: Vec<FileError>,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub source: PathBuf,
    pub output: PathBuf,
    pub kind: FileKind,
}

#[derive(Debug, Clone)]
pub struct FileError {
    pub path: PathBuf,
    pub error: ScannerError,
}

#[derive(Debug, Clone)]
pub struct ScanSkipped {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Processed(ScanResult),
    Skipped(ScanSkipped),
    Failed(FileError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    KugouEncrypted,
    NeteaseEncrypted,
    Mp3,
    Flac,
}

impl FileKind {
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
pub enum ScannerError {
    Io(String),
    Kgm(String),
    Ncm(String),
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

fn is_kugou_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("kgm") || ext.eq_ignore_ascii_case("kgma"))
        .unwrap_or(false)
}

fn is_ncm_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("ncm"))
        .unwrap_or(false)
}

fn default_process_formats() -> BTreeSet<String> {
    ["kgm", "kgma", "ncm", "mp3", "flac"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn is_kugou_header(probe: &[u8]) -> bool {
    probe.starts_with(&crate::kgm::MAGIC_HEADER)
}

fn is_ncm_header(probe: &[u8]) -> bool {
    probe.starts_with(b"CTENFDAM")
}

fn infer_audio_extension(probe: &[u8]) -> &'static str {
    match infer::get(probe).map(|kind| kind.extension()) {
        Some("mp3") => "mp3",
        Some("flac") => "flac",
        _ => "mp3",
    }
}

fn truncate_to_empty(path: &Path) -> Result<(), ScannerError> {
    OpenOptions::new().write(true).truncate(true).open(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{FileKind, Scanner};

    const FLAC_BYTES: &[u8] = b"fLaC\x00\x00\x00\x22tiny flac-like fixture bytes";

    #[test]
    fn scanner_copies_flac_and_truncates_source() {
        let root =
            std::env::temp_dir().join(format!("unlock-music-scanner-{}", std::process::id()));
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&input_dir).unwrap();

        let source = input_dir.join("fixture.flac");
        fs::write(&source, FLAC_BYTES).unwrap();

        let scanner = Scanner::new(vec![input_dir], &output_dir);
        let report = scanner.scan().unwrap();

        assert_eq!(report.scanned, 1);
        assert_eq!(report.processed, 1);
        assert_eq!(report.results[0].kind, FileKind::Flac);
        assert_eq!(fs::metadata(&source).unwrap().len(), 0);
        assert_eq!(fs::read(&report.results[0].output).unwrap(), FLAC_BYTES);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scanner_loads_toml_config() {
        let root = std::env::temp_dir().join(format!("unlock-music-config-{}", std::process::id()));
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        let config_path = root.join("config.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&input_dir).unwrap();

        fs::write(
            &config_path,
            format!(
                "scan_dirs = [\"{}\"]\noutput_dir = \"{}\"\n",
                toml_escape_path(&input_dir),
                toml_escape_path(&output_dir)
            ),
        )
        .unwrap();

        let scanner = Scanner::load(&config_path).unwrap();
        assert_eq!(scanner.scan_dirs, vec![input_dir]);
        assert_eq!(scanner.output_dir, output_dir);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scanner_counts_supported_empty_files_as_skipped() {
        let root = std::env::temp_dir().join(format!("unlock-music-empty-{}", std::process::id()));
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&input_dir).unwrap();
        fs::write(input_dir.join("empty.kgma"), []).unwrap();
        fs::write(input_dir.join("empty.ncm"), []).unwrap();
        fs::write(input_dir.join("empty.txt"), []).unwrap();

        let scanner = Scanner::new(vec![input_dir], output_dir);
        let report = scanner.scan().unwrap();

        assert_eq!(report.scanned, 0);
        assert_eq!(report.processed, 0);
        assert_eq!(report.skipped, 2);
        assert_eq!(report.failed, 0);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scanner_overwrites_existing_output() {
        let root =
            std::env::temp_dir().join(format!("unlock-music-overwrite-{}", std::process::id()));
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&input_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();

        let source = input_dir.join("fixture.flac");
        let output = output_dir.join("fixture.flac");
        fs::write(&source, FLAC_BYTES).unwrap();
        fs::write(&output, b"old output").unwrap();

        let scanner = Scanner::new(vec![input_dir], output_dir);
        let report = scanner.scan().unwrap();

        assert_eq!(report.processed, 1);
        assert_eq!(report.results[0].output, output);
        assert_eq!(fs::read(&report.results[0].output).unwrap(), FLAC_BYTES);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scanner_does_not_scan_nested_directories() {
        let root =
            std::env::temp_dir().join(format!("unlock-music-non-recursive-{}", std::process::id()));
        let input_dir = root.join("input");
        let nested_dir = input_dir.join("nested");
        let output_dir = root.join("output");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&nested_dir).unwrap();

        let nested_source = nested_dir.join("fixture.flac");
        fs::write(&nested_source, FLAC_BYTES).unwrap();

        let scanner = Scanner::new(vec![input_dir], output_dir);
        let report = scanner.scan().unwrap();

        assert_eq!(report.scanned, 0);
        assert_eq!(report.processed, 0);
        assert_eq!(
            fs::metadata(&nested_source).unwrap().len(),
            FLAC_BYTES.len() as u64
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn toml_escape_path(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('\\', "\\\\")
    }
}
