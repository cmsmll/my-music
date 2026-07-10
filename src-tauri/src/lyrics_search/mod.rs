#![allow(dead_code)]
#![allow(clippy::all)]

//! 本地歌词搜索适配层。
//!
//! 这里保留酷狗、网易云和 QQ 音乐的搜索/拉取/解析能力，
//! 上层 `interaction::lyrics` 负责缓存、哈希和前端响应结构。

pub mod error;
pub mod fetchers;
pub mod models;
pub mod parsers;
pub mod providers;
pub mod searchers;

pub use self::models::MusicPlayer;

use crate::lyrics_search::error::{GeneralError, LyrixResult};
use crate::lyrics_search::models::{LineInfo, LyricsData, TrackMetadata};
use crate::lyrics_search::providers::fetch_lyrics_from_player;
use reqwest::Client;

/// 歌词搜索入口客户端。
pub struct Lyrix {
    /// 共享 HTTP 客户端。
    client: Client,
}

impl Lyrix {
    /// 创建歌词搜索客户端。
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 使用指定歌词源搜索并获取歌词。
    pub async fn get_lyrics_with_player(
        &self,
        player: &MusicPlayer,
        title: &str,
        artist: Option<&str>,
        album: Option<&str>,
        album_artist: Option<&str>,
        duration_ms: u32,
    ) -> LyrixResult<LyricsData> {
        let track = TrackMetadata {
            title: Some(title.to_string()),
            artist: artist.map(|s| s.to_string()),
            album: album.map(|s| s.to_string()),
            album_artist: album_artist.map(|s| s.to_string()),
            duration_ms: Some(duration_ms),
            ..Default::default()
        };
        fetch_lyrics_from_player(player, track, &self.client).await
    }

    /// 从试听歌词中截取真实可用片段。
    pub fn get_trial_part(&self, raw: LyricsData) -> LyrixResult<LyricsData> {
        let (st, du) = match &raw.track_metadata {
            Some(op) => match &op.trial {
                Some(trial) => (trial[0], trial[1]),
                None => {
                    return Err(GeneralError::MissingField {
                        field: "trial info".to_string(),
                    }
                    .into())
                }
            },
            None => {
                return Err(GeneralError::MissingField {
                    field: "track_metadata".to_string(),
                }
                .into())
            }
        };
        let raw_lines = raw.lines;
        let mut new_lines: Vec<LineInfo> = Vec::new();
        for x in raw_lines {
            if x.start_time < st {
                continue;
            }
            if x.start_time > st + du {
                break;
            }
            new_lines.push(LineInfo {
                start_time: x.start_time - st,
                ..x
            });
        }
        Ok(LyricsData {
            lines: new_lines,
            ..raw
        })
    }
}
