use crate::models::LyricsSearchResult;
use lyrix::{Lyrix, MusicPlayer};
use moka::future::Cache;
use std::{sync::Arc, time::Duration};

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
    ) -> Result<Vec<LyricsSearchResult>, String> {
        let title = title.trim().to_string();
        if title.is_empty() || title == "未知歌曲" {
            return Err("歌曲名称为空，无法搜索歌词".to_string());
        }

        let artist = known_value(&artist, "未知歌手");
        let album = known_value(&album, "未知专辑");
        let cache_key = cache_key(&title, artist.as_deref(), album.as_deref(), duration);
        let lyrix = self.lyrix.clone();

        self.cache
            .try_get_with(cache_key, async move {
                search_from_providers(lyrix, title, artist, album, duration).await
            })
            .await
            .map_err(|err| err.to_string())
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
