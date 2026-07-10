use crate::lyrics_search::error::{GeneralError, LyrixResult};
use crate::lyrics_search::providers::{LyrixProvider, RawLyrics};
use async_trait::async_trait;
use memchr::memmem::{find, Finder};
use reqwest::Client;
use std::sync::LazyLock;

static FI: LazyLock<Finder<'static>> = LazyLock::new(|| Finder::new(b"CDATA["));
/// QQ 音乐歌词 Provider。
pub(crate) struct QQMusicProvider {
    /// 共享 HTTP 客户端。
    pub(crate) client: Client,
}

#[async_trait]
impl LyrixProvider for QQMusicProvider {
    type Searcher = crate::lyrics_search::searchers::qqmusic::QQMusicSearcher;
    type Api = crate::lyrics_search::fetchers::qqmusic::QQMusicFetcher;
    type SearchResult = crate::lyrics_search::searchers::qqmusic::QQMusicSearchResult;

    async fn create_searcher(&self) -> LyrixResult<Self::Searcher> {
        Ok(
            crate::lyrics_search::searchers::qqmusic::QQMusicSearcher::with_client(
                self.client.clone(),
            ),
        )
    }

    async fn create_api(&self) -> LyrixResult<Self::Api> {
        Ok(
            crate::lyrics_search::fetchers::qqmusic::QQMusicFetcher::with_client(
                self.client.clone(),
            ),
        )
    }

    fn label() -> &'static str {
        "QQ音乐"
    }

    async fn fetch(api: &Self::Api, best: &Self::SearchResult) -> LyrixResult<RawLyrics> {
        let xml = api.get_lyrics_qrc(&best.id.to_string()).await?;
        let bytes = xml.as_bytes();
        let mut pos = 0usize;

        pos += FI
            .find(&bytes[pos..])
            .ok_or_else(|| GeneralError::MissingField {
                field: "QQMusic network qrc content".to_string(),
            })?
            + 6; //落到加密正文
        let end = find(&bytes[pos..], b"]]>").ok_or_else(|| GeneralError::MissingField {
            field: "QQMusic network qrc content".to_string(),
        })?;
        let lyrics = xml[pos..pos + end].to_string();
        if lyrics.is_empty() {
            return Err(GeneralError::MissingField {
                field: "QQMusic network qrc content".to_string(),
            }
            .into());
        }
        pos += end;
        pos += FI
            .find(&bytes[pos..])
            .ok_or_else(|| GeneralError::MissingField {
                field: "QQMusic network qrc content".to_string(),
            })?
            + 6; //落到lrc歌词
        let tlyrics = if let Some(end) = find(&bytes[pos..], b"]]>") {
            Some(xml[pos..pos + end].to_string())
        } else {
            None
        };
        //前面这个是加密了的 后面没有加密
        Ok(RawLyrics { lyrics, tlyrics })
    }
}
