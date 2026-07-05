use crate::models::{LyricsSearchResponse, LyricsSearchResult, LyricsUseResult};
use lyrix::{Lyrix, MusicPlayer};
use moka::future::Cache;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const LYRICS_CACHE_CAPACITY: u64 = 10;
const PROVIDER_TIMEOUT_SECONDS: u64 = 12;

#[derive(Clone)]
pub(crate) struct LyricsSearchService {
    lyrix: Arc<Lyrix>,
    cache: Cache<String, Vec<LyricsSearchResult>>,
}

impl LyricsSearchService {
    pub(crate) fn new() -> Self {
        lyrix::logger::set_console_output(false);
        Self {
            lyrix: Arc::new(Lyrix::new(None)),
            cache: Cache::builder()
                .max_capacity(LYRICS_CACHE_CAPACITY)
                .time_to_live(Duration::from_secs(30 * 60))
                .build(),
        }
    }

    pub(crate) async fn search(
        &self,
        title: String,
        artist: String,
        album: String,
        duration: Option<u64>,
        lyrics_cache_path: String,
        lyrics_cache_hash: Option<String>,
    ) -> Result<LyricsSearchResponse, String> {
        let title = title.trim().to_string();
        if title.is_empty() || title == "未知歌曲" {
            return Err("歌曲名称为空，无法搜索歌词".to_string());
        }

        let artist = known_value(&artist, "未知歌手");
        let album = known_value(&album, "未知专辑");
        let cache_key = cache_key(&title, artist.as_deref(), album.as_deref(), duration);
        let lyrix = self.lyrix.clone();
        let current_hash = match lyrics_cache_hash
            .as_deref()
            .map(str::trim)
            .filter(|hash| !hash.is_empty())
        {
            Some(hash) => Some(hash.to_string()),
            None => current_lyrics_hash(&lyrics_cache_path)?,
        };

        let results = self
            .cache
            .try_get_with(cache_key, async move {
                search_from_providers(lyrix, title, artist, album, duration).await
            })
            .await
            .map_err(|err| err.to_string())?;

        Ok(LyricsSearchResponse {
            current_lyrics_hash: current_hash,
            results,
        })
    }

    pub(crate) fn use_lyrics(
        &self,
        lyrics_cache_path: String,
        lyrics: String,
    ) -> Result<LyricsUseResult, String> {
        let lyrics = lyrics.trim().to_string();
        if lyrics.is_empty() {
            return Err("歌词内容为空，无法使用".to_string());
        }

        let path = PathBuf::from(lyrics_cache_path.trim());
        if path.as_os_str().is_empty() {
            return Err("歌词缓存路径为空".to_string());
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
        }

        fs::write(&path, &lyrics).map_err(|err| format!("无法写入歌词缓存: {err}"))?;
        let lyrics_hash = lyrics_hash(&lyrics);
        write_hash_sidecar(&path, &lyrics_hash)?;

        Ok(LyricsUseResult {
            lyrics_cache_path: path.to_string_lossy().to_string(),
            lyrics_hash,
            lyrics,
            track: None,
        })
    }
}

async fn search_from_providers(
    lyrix: Arc<Lyrix>,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<u64>,
) -> Result<Vec<LyricsSearchResult>, String> {
    let mut results = Vec::new();
    let duration_ms = duration
        .and_then(|seconds| seconds.checked_mul(1000))
        .and_then(|milliseconds| u32::try_from(milliseconds).ok())
        .unwrap_or(0);

    for player in [
        MusicPlayer::Netease,
        MusicPlayer::QQMusic,
        MusicPlayer::Kugou,
        MusicPlayer::SodaMusic,
    ] {
        let task = lyrix.get_lyrics_with_player(
            &player,
            &title,
            artist.as_deref(),
            album.as_deref(),
            None,
            duration_ms,
        );

        match tokio::time::timeout(Duration::from_secs(PROVIDER_TIMEOUT_SECONDS), task).await {
            Ok(Ok(data)) => {
                let data = if data
                    .track_metadata
                    .as_ref()
                    .map(|metadata| metadata.is_trial)
                    .unwrap_or(false)
                {
                    lyrix.get_trial_part(data.clone()).unwrap_or(data)
                } else {
                    data
                };
                if let Some(result) = map_lyrics_result(player, data, duration) {
                    results.push(result);
                }
            }
            Ok(Err(err)) => {
                eprintln!("歌词搜索失败:{}: {err}", player.display_name());
            }
            Err(_) => {
                eprintln!("歌词搜索超时:{}", player.display_name());
            }
        }
    }

    Ok(results)
}

fn map_lyrics_result(
    player: MusicPlayer,
    data: lyrix::models::LyricsData,
    fallback_duration: Option<u64>,
) -> Option<LyricsSearchResult> {
    if data.lines.is_empty() {
        return None;
    }

    let metadata = data.track_metadata;
    let track_name = metadata
        .as_ref()
        .and_then(|metadata| metadata.title.clone())
        .unwrap_or_else(|| "未知歌曲".to_string());
    let artist_name = metadata
        .as_ref()
        .and_then(|metadata| metadata.artist.clone())
        .unwrap_or_else(|| "未知歌手".to_string());
    let album_name = metadata
        .as_ref()
        .and_then(|metadata| metadata.album.clone())
        .unwrap_or_default();
    let duration = metadata
        .as_ref()
        .and_then(|metadata| metadata.duration_ms)
        .map(|milliseconds| (milliseconds as u64 + 500) / 1000)
        .or(fallback_duration);
    let synced_lyrics = format_lrc(&data.lines);
    let plain_lyrics = format_plain_lyrics(&data.lines);
    let lyrics_for_hash = if !synced_lyrics.is_empty() {
        synced_lyrics.as_str()
    } else {
        plain_lyrics.as_str()
    };
    let lyrics_hash = lyrics_hash(lyrics_for_hash);
    let id = format!(
        "{}:{}:{}:{}",
        player.display_name(),
        track_name,
        artist_name,
        duration.unwrap_or_default()
    );

    Some(LyricsSearchResult {
        source: player.display_name().to_string(),
        id,
        track_name,
        artist_name,
        album_name,
        duration,
        lyrics_hash,
        synced_lyrics: (!synced_lyrics.is_empty()).then_some(synced_lyrics),
        plain_lyrics: (!plain_lyrics.is_empty()).then_some(plain_lyrics),
    })
}

fn format_lrc(lines: &[lyrix::models::LineInfo]) -> String {
    lines
        .iter()
        .filter_map(|line| {
            let text = line_text(line);
            (!text.is_empty()).then(|| format!("[{}] {}", format_lrc_time(line.start_time), text))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_plain_lyrics(lines: &[lyrix::models::LineInfo]) -> String {
    lines
        .iter()
        .map(line_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_text(line: &lyrix::models::LineInfo) -> String {
    if !line.text.trim().is_empty() {
        return line.text.trim().to_string();
    }

    line.syllables
        .iter()
        .map(|syllable| syllable.text.as_str())
        .collect::<String>()
        .trim()
        .to_string()
}

fn format_lrc_time(milliseconds: u32) -> String {
    let total_centiseconds = milliseconds / 10;
    let minutes = total_centiseconds / 6000;
    let seconds = (total_centiseconds / 100) % 60;
    let centiseconds = total_centiseconds % 100;
    format!("{minutes:02}:{seconds:02}.{centiseconds:02}")
}

fn known_value(value: &str, unknown_value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != unknown_value).then(|| value.to_string())
}

pub(crate) fn current_lyrics_hash(lyrics_cache_path: &str) -> Result<Option<String>, String> {
    let lyrics_cache_path = lyrics_cache_path.trim();
    if lyrics_cache_path.is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(lyrics_cache_path);
    if !path.is_file() {
        return Ok(None);
    }

    let hash_path = hash_sidecar_path(&path);
    if hash_path.is_file() {
        let hash = fs::read_to_string(&hash_path)
            .map_err(|err| format!("无法读取歌词哈希缓存: {err}"))?
            .trim()
            .to_string();
        if !hash.is_empty() {
            return Ok(Some(hash));
        }
    }

    let lyrics = fs::read_to_string(&path).map_err(|err| format!("无法读取歌词缓存: {err}"))?;
    let hash = lyrics_hash(&lyrics);
    write_hash_sidecar(&path, &hash)?;
    Ok(Some(hash))
}

pub(crate) fn lyrics_hash(lyrics: &str) -> String {
    let digest = Sha256::digest(lyrics.trim().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn write_hash_sidecar(lyrics_path: &Path, hash: &str) -> Result<(), String> {
    let hash_path = hash_sidecar_path(lyrics_path);
    if let Some(parent) = hash_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建歌词哈希目录: {err}"))?;
    }
    fs::write(&hash_path, hash).map_err(|err| format!("无法写入歌词哈希缓存: {err}"))
}

fn hash_sidecar_path(lyrics_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.sha256", lyrics_path.to_string_lossy()))
}

fn cache_key(
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    duration: Option<u64>,
) -> String {
    format!(
        "{}|{}|{}|{}",
        title.trim().to_lowercase(),
        artist.unwrap_or_default().trim().to_lowercase(),
        album.unwrap_or_default().trim().to_lowercase(),
        duration.unwrap_or_default()
    )
}
