mod kugou;
mod netease;
mod qqmusic;

use crate::lyrics_search::error::{GeneralError, LyrixResult, SearcherError};
use crate::lyrics_search::models::{
    ITrackMetadata, LineInfo, LyricsData, MusicPlayer, TrackMetadata,
};
use crate::lyrics_search::parsers::lrc::LrcParser;
use crate::lyrics_search::parsers::IParsers;
use crate::lyrics_search::searchers::{ISearchResult, ISearcher};
use async_trait::async_trait;
use reqwest::Client;

use kugou::KugouProvider;
use netease::NeteaseProvider;
use qqmusic::QQMusicProvider;

/// Provider 拉取到的原始歌词。
pub struct RawLyrics {
    /// 主歌词内容。
    pub lyrics: String,
    /// 翻译歌词内容。
    pub tlyrics: Option<String>,
}

/// Provider 只负责搜索和获取原始歌词，解析由播放器类型统一选择。
#[async_trait]
pub(crate) trait LyrixProvider {
    type Searcher: ISearcher;
    type Api: Send + Sync;
    type SearchResult: ISearchResult + 'static;

    async fn create_searcher(&self) -> LyrixResult<Self::Searcher>;
    async fn create_api(&self) -> LyrixResult<Self::Api>;
    fn label() -> &'static str;
    async fn fetch(api: &Self::Api, best: &Self::SearchResult) -> LyrixResult<RawLyrics>;
}

async fn fetch_raw_lyrics<P: LyrixProvider>(
    provider: &P,
    track: &dyn ITrackMetadata,
) -> LyrixResult<RawLyrics> {
    let label = P::label();

    let searcher = provider.create_searcher().await?;
    let result =
        searcher
            .search_for_result(track)
            .await?
            .ok_or_else(|| SearcherError::NoMatch {
                label: label.to_string(),
                title: track.title().unwrap_or_default().to_string(),
            })?;

    let best = result
        .as_any()
        .downcast_ref::<P::SearchResult>()
        .ok_or_else(|| GeneralError::Internal {
            detail: format!("{}: search result type mismatch", label),
        })?;

    let api = provider.create_api().await?;
    P::fetch(&api, best).await
}

pub(crate) fn parse_lyrics_for_player(
    player: &MusicPlayer,
    raw: RawLyrics,
) -> LyrixResult<LyricsData> {
    let lyrics = raw.lyrics;
    let tlyrics = raw.tlyrics;
    let lines: Vec<LineInfo> = match player {
        MusicPlayer::Netease => parse_netease_lyrics(lyrics)?,
        MusicPlayer::QQMusic => {
            crate::lyrics_search::parsers::qqmusic::QQMusicParser {}.decrypt_and_parse(lyrics)?
        }
        MusicPlayer::Kugou => {
            crate::lyrics_search::parsers::kugou::KugouParser {}.decrypt_and_parse(lyrics)?
        }
    };

    if lines.is_empty() {
        return Err(GeneralError::MissingField {
            field: format!("{}: no lyrics content", player.display_name()),
        }
        .into());
    }

    let tlines: Option<Vec<LineInfo>> = tlyrics.filter(|s| !s.trim().is_empty()).and_then(|s| {
        (crate::lyrics_search::parsers::lrc::UniversalLrcParser {})
            .parse(s)
            .ok()
    });

    Ok(LyricsData {
        lines,
        tlines,
        track_metadata: None,
    })
}

fn parse_netease_lyrics(content: String) -> LyrixResult<Vec<LineInfo>> {
    if let Ok(lines) =
        (crate::lyrics_search::parsers::netease::NeteaseParser {}).parse(content.clone())
    {
        if !lines.is_empty() {
            return Ok(lines);
        }
    }

    crate::lyrics_search::parsers::netease::NeteaseLrcParser { version: 3 }.parse(content)
}

pub(crate) async fn fetch_raw_lyrics_from_player(
    player: &MusicPlayer,
    track: &dyn ITrackMetadata,
    client: &Client,
) -> LyrixResult<RawLyrics> {
    match player {
        MusicPlayer::Netease => {
            fetch_raw_lyrics(
                &NeteaseProvider {
                    client: client.clone(),
                },
                track,
            )
            .await
        }
        MusicPlayer::QQMusic => {
            fetch_raw_lyrics(
                &QQMusicProvider {
                    client: client.clone(),
                },
                track,
            )
            .await
        }
        MusicPlayer::Kugou => {
            fetch_raw_lyrics(
                &KugouProvider {
                    client: client.clone(),
                },
                track,
            )
            .await
        }
    }
}

pub(crate) async fn fetch_lyrics_from_player(
    player: &MusicPlayer,
    track: TrackMetadata,
    client: &Client,
) -> LyrixResult<LyricsData> {
    let raw = fetch_raw_lyrics_from_player(player, &track, client).await?;
    let lyrics = parse_lyrics_for_player(player, raw)?;
    Ok(LyricsData {
        track_metadata: Some(track),
        ..lyrics
    })
}
