/// 逐字歌词片段。
#[derive(Debug, Clone, Default)]
pub struct TextInfo {
    /// 相对当前歌词行的起始偏移，单位毫秒。
    pub start_time: u16,
    /// 片段持续时间，单位毫秒。
    pub duration: u16,
    /// 片段文本。
    pub text: String,
}

/// 歌词行信息。
///
/// 注意：普通逐行歌词使用 `text`，逐字歌词使用 `syllables`。
#[derive(Debug, Clone, Default)]
pub struct LineInfo {
    /// 行起始时间，单位毫秒。
    pub start_time: u32,
    /// 行持续时间，单位毫秒。
    pub duration: u16,
    /// 普通歌词行文本。
    pub text: String,
    /// 逐字歌词片段列表。
    pub syllables: Vec<TextInfo>,
}
