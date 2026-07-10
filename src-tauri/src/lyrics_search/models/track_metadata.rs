use serde::{Deserialize, Serialize};

/// 曲目元数据 trait。
///
/// 用于把上层歌曲信息传入不同歌词源搜索器做匹配。
pub trait ITrackMetadata: Send + Sync {
    fn title(&self) -> Option<&str>;
    fn artist(&self) -> Option<&str>;
    fn album(&self) -> Option<&str>;
    fn album_artist(&self) -> Option<&str> {
        None
    }
    fn duration_ms(&self) -> Option<u32> {
        None
    }
}

impl ITrackMetadata for TrackMetadata {
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }
    fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }
    fn album_artist(&self) -> Option<&str> {
        self.album_artist.as_deref()
    }
    fn duration_ms(&self) -> Option<u32> {
        self.duration_ms
    }
}

/// 歌词搜索使用的曲目元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackMetadata {
    /// 歌曲标题。
    pub title: Option<String>,
    /// 歌手。
    pub artist: Option<String>,
    /// 专辑。
    pub album: Option<String>,
    /// 专辑艺术家。
    pub album_artist: Option<String>,
    /// 时长，单位毫秒。
    pub duration_ms: Option<u32>,
    /// 试听片段，格式为 `[起始毫秒, 持续毫秒]`。
    pub trial: Option<[u32; 2]>,
    /// 是否为试听结果。
    pub is_trial: bool,
}
