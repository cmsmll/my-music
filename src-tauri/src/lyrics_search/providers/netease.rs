use crate::lyrics_search::error::{GeneralError, LyrixResult};
use crate::lyrics_search::providers::{LyrixProvider, RawLyrics};
use async_trait::async_trait;
use reqwest::Client;

/// 网易云歌词 Provider。
pub(crate) struct NeteaseProvider {
    /// 共享 HTTP 客户端。
    pub(crate) client: Client,
}

#[async_trait]
impl LyrixProvider for NeteaseProvider {
    type Searcher = crate::lyrics_search::searchers::netease::NeteaseSearcher;
    type Api = crate::lyrics_search::fetchers::netease::NeteaseFetcher;
    type SearchResult = crate::lyrics_search::searchers::netease::NeteaseSearchResult;

    async fn create_searcher(&self) -> LyrixResult<Self::Searcher> {
        Ok(
            crate::lyrics_search::searchers::netease::NeteaseSearcher::with_client(
                self.client.clone(),
            ),
        )
    }

    async fn create_api(&self) -> LyrixResult<Self::Api> {
        Ok(
            crate::lyrics_search::fetchers::netease::NeteaseFetcher::with_client(
                self.client.clone(),
            ),
        )
    }

    fn label() -> &'static str {
        "网易云"
    }

    async fn fetch(api: &Self::Api, best: &Self::SearchResult) -> LyrixResult<RawLyrics> {
        let lyric_result = api.get_lyric(&best.id).await?;
        if let Some(lyrics) = lyric_result.yrc.and_then(|y| y.lyric) {
            if !lyrics.is_empty() {
                let tlyrics = lyric_result.tlyric.and_then(|y| y.lyric);
                return Ok(RawLyrics { lyrics, tlyrics });
            }
        }

        let lrc = lyric_result.lrc.ok_or_else(|| GeneralError::MissingField {
            field: "网易云: LRC也没有哟".to_string(),
        })?;
        let lyrics = lrc.lyric.ok_or_else(|| GeneralError::MissingField {
            field: "网易云: LRC也没有哟".to_string(),
        })?;

        Ok(RawLyrics {
            lyrics,
            tlyrics: None,
        })
    }
}
