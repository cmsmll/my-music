#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// 支持的歌词来源。
pub enum MusicPlayer {
    /// 酷狗音乐。
    Kugou,
    /// 网易云音乐。
    Netease,
    /// QQ 音乐。
    QQMusic,
}

impl MusicPlayer {
    pub fn display_name(&self) -> &str {
        match self {
            MusicPlayer::Kugou => "酷狗音乐",
            MusicPlayer::Netease => "网易云音乐",
            MusicPlayer::QQMusic => "QQ音乐",
        }
    }
}
