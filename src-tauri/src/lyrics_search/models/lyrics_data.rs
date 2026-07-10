use super::{LineInfo, TrackMetadata};

/// 歌词数据。
#[derive(Debug, Clone, Default)]
pub struct LyricsData {
    /// 主歌词行。
    pub lines: Vec<LineInfo>,
    /// 翻译歌词行。
    ///
    /// 注意：翻译歌词只按行展示，不包含逐字信息。
    pub tlines: Option<Vec<LineInfo>>,
    /// 搜索结果关联的曲目信息。
    pub track_metadata: Option<TrackMetadata>,
}
