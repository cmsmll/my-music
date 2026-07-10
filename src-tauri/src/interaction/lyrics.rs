//! 歌词搜索和歌词缓存写入服务。
//!
//! 前端的手动搜索和 Auto 自动搜索都走这里。搜索结果使用 moka 做短期缓存，
//! 用户点击“使用”后才会写入本地歌词缓存并同步歌曲缓存中的歌词哈希。

use super::models::*;
use crate::logger::{self, LogKind};
use crate::lyrics_search::{models, Lyrix, MusicPlayer};
use crate::utils::atomic_write;
use moka::future::Cache;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::Arc, time::Duration};

const LYRICS_CACHE_CAPACITY: u64 = 10;
const PROVIDER_TIMEOUT_SECONDS: u64 = 12;

#[derive(Clone)]
/// 歌词搜索服务。
///
/// 注意：搜索结果缓存只保存在内存里，用于降低短时间重复请求公开歌词源的概率。
pub(crate) struct LyricsSearchService {
    /// 本地歌词源适配器。
    lyrix: Arc<Lyrix>,
    /// 歌词搜索结果缓存。
    cache: Cache<String, Vec<LyricsSearchResult>>,
}

impl LyricsSearchService {
    /// 创建歌词搜索服务和搜索结果缓存。
    pub(crate) fn new() -> Self {
        Self {
            lyrix: Arc::new(Lyrix::new()),
            cache: Cache::builder()
                .max_capacity(LYRICS_CACHE_CAPACITY)
                .time_to_live(Duration::from_secs(30 * 60))
                .build(),
        }
    }

    /// 搜索当前歌曲的歌词候选，并返回当前本地歌词哈希。
    ///
    /// 注意：`force_refresh` 为 true 时会绕过 moka 缓存重新请求公开歌词源。
    pub(crate) async fn search(
        &self,
        request: LyricsSearchRequest,
    ) -> Result<LyricsSearchResponse, String> {
        let LyricsSearchRequest {
            track_id,
            title,
            artist,
            album,
            duration,
            lyrics_cache_path,
            lyrics_cache_hash,
            force_refresh,
        } = request;
        let title = title.trim().to_string();
        if title.is_empty() || title == "未知歌曲" {
            return Err("歌曲名称为空，无法搜索歌词".to_string());
        }

        let artist = known_value(&artist, "未知歌手");
        let album = known_value(&album, "未知专辑");
        let cache_key = cache_key(
            &track_id,
            &title,
            artist.as_deref(),
            album.as_deref(),
            duration,
        );
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
            .search_results(
                ProviderSearchRequest {
                    cache_key,
                    lyrix,
                    title,
                    artist,
                    album,
                    duration,
                },
                force_refresh,
            )
            .await?;

        Ok(LyricsSearchResponse {
            current_lyrics_hash: current_hash,
            results,
        })
    }

    /// 从缓存读取或向公开歌词源请求搜索结果。
    async fn search_results(
        &self,
        request: ProviderSearchRequest,
        force_refresh: bool,
    ) -> Result<Vec<LyricsSearchResult>, String> {
        let ProviderSearchRequest {
            cache_key,
            lyrix,
            title,
            artist,
            album,
            duration,
        } = request;
        if force_refresh {
            let results = search_from_providers(lyrix, title, artist, album, duration).await?;
            self.cache.insert(cache_key, results.clone()).await;
            return Ok(results);
        }

        self.cache
            .try_get_with(cache_key, async move {
                search_from_providers(lyrix, title, artist, album, duration).await
            })
            .await
            .map_err(|err| err.to_string())
    }

    /// 将前端选中的歌词写入固定歌词缓存文件。
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

        atomic_write(&path, lyrics.as_bytes(), "歌词缓存")?;
        let lyrics_hash = lyrics_hash(&lyrics);

        Ok(LyricsUseResult {
            lyrics_cache_path: path.to_string_lossy().to_string(),
            lyrics_hash,
            lyrics,
            track: None,
        })
    }
}

/// 向具体歌词源发起搜索时使用的内部请求。
struct ProviderSearchRequest {
    /// moka 缓存 key。
    cache_key: String,
    /// 共享 Lyrix 客户端。
    lyrix: Arc<Lyrix>,
    /// 搜索标题。
    title: String,
    /// 搜索歌手。
    artist: Option<String>,
    /// 搜索专辑。
    album: Option<String>,
    /// 歌曲时长，单位秒。
    duration: Option<u64>,
}

/// 向所有启用的歌词源并行按顺序尝试搜索。
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
                logger::warn(
                    LogKind::Lyrics,
                    format!(
                        "歌词搜索失败 | 来源={} | 原因=\"{}\"",
                        player.display_name(),
                        &err.to_string(),
                    ),
                );
            }
            Err(_) => {
                logger::warn(
                    LogKind::Lyrics,
                    format!("歌词搜索超时 | 来源={}", player.display_name()),
                );
            }
        }
    }

    Ok(results)
}

/// 将 Lyrix 返回的数据转换成前端搜索结果结构。
fn map_lyrics_result(
    player: MusicPlayer,
    data: models::LyricsData,
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

/// 将逐行歌词格式化为 LRC 文本。
fn format_lrc(lines: &[models::LineInfo]) -> String {
    lines
        .iter()
        .filter_map(|line| {
            let text = line_text(line);
            (!text.is_empty()).then(|| format!("[{}] {}", format_lrc_time(line.start_time), text))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 将逐行歌词格式化为普通纯文本。
fn format_plain_lyrics(lines: &[models::LineInfo]) -> String {
    lines
        .iter()
        .map(line_text)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 取一行歌词的文本；逐字歌词会拼接 syllable 文本。
fn line_text(line: &models::LineInfo) -> String {
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

/// 将毫秒时间格式化为 LRC 时间戳。
fn format_lrc_time(milliseconds: u32) -> String {
    let total_centiseconds = milliseconds / 10;
    let minutes = total_centiseconds / 6000;
    let seconds = (total_centiseconds / 100) % 60;
    let centiseconds = total_centiseconds % 100;
    format!("{minutes:02}:{seconds:02}.{centiseconds:02}")
}

/// 过滤“未知歌手/未知专辑”等占位值，减少歌词源误匹配。
fn known_value(value: &str, unknown_value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != unknown_value).then(|| value.to_string())
}

/// 读取当前歌词缓存文件并计算哈希。
pub(crate) fn current_lyrics_hash(lyrics_cache_path: &str) -> Result<Option<String>, String> {
    let lyrics_cache_path = lyrics_cache_path.trim();
    if lyrics_cache_path.is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(lyrics_cache_path);
    if !path.is_file() {
        return Ok(None);
    }

    let lyrics = fs::read_to_string(&path).map_err(|err| format!("无法读取歌词缓存: {err}"))?;
    Ok(Some(lyrics_hash(&lyrics)))
}

/// 计算歌词文本的 SHA-256 哈希。
pub(crate) fn lyrics_hash(lyrics: &str) -> String {
    let digest = Sha256::digest(lyrics.trim().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 为歌词搜索结果缓存生成 key。
fn cache_key(
    track_id: &str,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    duration: Option<u64>,
) -> String {
    let track_id = track_id.trim();
    if !track_id.is_empty() {
        return format!("track:{track_id}");
    }

    format!(
        "{}|{}|{}|{}",
        title.trim().to_lowercase(),
        artist.unwrap_or_default().trim().to_lowercase(),
        album.unwrap_or_default().trim().to_lowercase(),
        duration.unwrap_or_default()
    )
}
