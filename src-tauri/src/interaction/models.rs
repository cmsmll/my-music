//! 前后端共享的数据结构。
//!
//! 这些结构会被序列化到 Tauri command 响应、配置文件或缓存文件中。
//! 注意：字段改名会影响前端和已有缓存，新增字段优先使用 `serde(default)` 保持兼容。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 曲库中的单首歌曲数据。
///
/// 注意：`id` 当前使用歌曲文件路径，前端歌单和播放队列都通过它引用“全部”曲库里的歌曲对象。
pub(crate) struct Track {
    /// 歌曲唯一标识，当前等同于文件路径。
    pub(crate) id: String,
    /// 歌曲名称。
    pub(crate) title: String,
    /// 歌手名称，解析失败时为“未知歌手”。
    pub(crate) artist: String,
    /// 专辑名称，解析失败时为“未知专辑”。
    pub(crate) album: String,
    /// 本地音频文件路径。
    pub(crate) path: String,
    /// 歌曲时长，单位秒。
    pub(crate) duration: Option<u64>,
    /// 文件大小，单位字节。
    #[serde(default)]
    pub(crate) file_size: Option<u64>,
    /// 音频码率。
    #[serde(default)]
    pub(crate) bitrate: Option<u32>,
    /// 音频采样率。
    #[serde(default)]
    pub(crate) sample_rate: Option<u32>,
    /// 年份。
    #[serde(default)]
    pub(crate) year: Option<u16>,
    /// 流派标签。
    #[serde(default)]
    pub(crate) genre: Vec<String>,
    /// 曲目序号。
    #[serde(default)]
    pub(crate) track_number: Option<u32>,
    /// 碟片序号。
    #[serde(default)]
    pub(crate) disk_number: Option<u32>,
    /// 封面缓存路径。
    pub(crate) cover_cache_path: Option<String>,
    /// 歌词缓存路径。
    pub(crate) lyrics_cache_path: String,
    /// 当前歌词缓存内容哈希，用于判断搜索结果是否和本地歌词同源。
    #[serde(default)]
    pub(crate) lyrics_cache_hash: String,
    /// 元数据来源。
    #[serde(default = "default_metadata_source")]
    pub(crate) metadata_source: MetadataSource,
    /// 旧版缓存中的嵌套 metadata 字段。
    ///
    /// 注意：只用于读取旧缓存迁移，保存新缓存时不会再写出。
    #[serde(default, rename = "metadata", skip_serializing)]
    pub(crate) legacy_metadata: Option<TrackMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 从音频文件中解析出的原始元数据。
pub(crate) struct TrackMetadata {
    /// 标题。
    pub(crate) title: String,
    /// 歌手。
    pub(crate) artist: String,
    /// 专辑。
    pub(crate) album: String,
    /// 时长，单位秒。
    pub(crate) duration: Option<u64>,
    /// 码率。
    pub(crate) bitrate: Option<u32>,
    /// 采样率。
    pub(crate) sample_rate: Option<u32>,
    /// 年份。
    pub(crate) year: Option<u16>,
    /// 流派。
    pub(crate) genre: Vec<String>,
    /// 曲目序号。
    pub(crate) track_number: Option<u32>,
    /// 碟片序号。
    pub(crate) disk_number: Option<u32>,
    /// 封面缓存路径。
    pub(crate) cover_cache_path: Option<String>,
    /// 歌词缓存路径。
    pub(crate) lyrics_cache_path: String,
    /// 歌词缓存哈希。
    #[serde(default)]
    pub(crate) lyrics_cache_hash: String,
    /// 元数据来源。
    pub(crate) metadata_source: MetadataSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// 歌曲元数据来源。
pub(crate) enum MetadataSource {
    /// 全部来自音频内嵌标签。
    Embedded,
    /// 部分来自内嵌标签，缺失字段使用文件名或目录回退。
    EmbeddedWithFilenameFallback,
    /// 只从文件名和目录推断。
    Filename,
}

/// 老缓存没有 metadata 来源字段时，默认按文件名解析来源处理。
fn default_metadata_source() -> MetadataSource {
    MetadataSource::Filename
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 应用完整配置，对应 `config.toml` 规范化后的结构。
pub(crate) struct AppConfig {
    /// 曲库扫描目录。
    pub(crate) music_directory: Vec<String>,
    /// 解码器配置。
    pub(crate) decoder: DecoderConfig,
    /// 缓存目录配置。
    pub(crate) cache: CacheConfig,
    /// 前端样式配置。
    pub(crate) style: StyleConfig,
    /// 前端运行状态配置。
    pub(crate) state: AppStateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 缓存目录配置。
pub(crate) struct CacheConfig {
    /// 曲库缓存目录。
    pub(crate) library_cache_dir: String,
    /// 封面缓存目录。
    pub(crate) cover_cache_dir: String,
    /// 歌词缓存目录。
    pub(crate) lyrics_cache_dir: String,
    /// 用户歌单和最近播放缓存目录。
    pub(crate) playlist_cache_dir: String,
    /// 频谱缓存目录。
    pub(crate) spectrum_cache_dir: String,
    /// 日志缓存目录。
    pub(crate) log_cache_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 解码器配置。
pub(crate) struct DecoderConfig {
    /// 解码输出目录。
    pub(crate) output_dir: String,
    /// 需要处理的格式列表，使用逗号分隔。
    pub(crate) process_formats: String,
    /// 加密音频扫描目录。
    pub(crate) scan_directory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端主题和样式配置。
pub(crate) struct StyleConfig {
    /// 背景颜色。
    pub(crate) background_color: String,
    /// 背景图片路径。
    pub(crate) background_image: String,
    /// 背景图片透明度。
    pub(crate) background_image_opacity: f32,
    /// 标题色。
    pub(crate) title_color: String,
    /// 副标题色。
    pub(crate) subtitle_color: String,
    /// 高亮色。
    pub(crate) highlight_color: String,
    /// 边框色。
    pub(crate) border_color: String,
    /// 是否显示主要布局边框。
    pub(crate) show_border: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端运行状态配置。
///
/// 注意：这些值由前端 Pinia 防抖同步，变化频率可能较高。
pub(crate) struct AppStateConfig {
    /// 窗口宽度。
    pub(crate) width: u32,
    /// 窗口高度。
    pub(crate) height: u32,
    /// 音量。
    pub(crate) volume: f32,
    /// 左侧栏宽度。
    pub(crate) sidebar_width: u32,
    /// 是否开启自动搜索歌词。
    pub(crate) auto_lyrics_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
/// `config.toml` 的反序列化结构。
///
/// 注意：这个结构同时兼容旧版扁平缓存目录字段和新版分组字段。
pub(crate) struct ConfigFile {
    pub(crate) music_directory: Option<MusicDirectoryConfig>,
    pub(crate) decoder: Option<DecoderConfigFile>,
    pub(crate) cache: Option<CacheConfigFile>,
    // 兼容旧版扁平配置，保存后会迁移到 [cache]。
    pub(crate) library_cache_dir: Option<String>,
    pub(crate) cover_cache_dir: Option<String>,
    pub(crate) lyrics_cache_dir: Option<String>,
    pub(crate) playlist_cache_dir: Option<String>,
    pub(crate) spectrum_cache_dir: Option<String>,
    pub(crate) log_cache_dir: Option<String>,
    pub(crate) my_playlist_cache_dir: Option<String>,
    pub(crate) log_dir: Option<String>,
    pub(crate) style: Option<StyleConfigFile>,
    pub(crate) state: Option<AppStateConfigFile>,
}

#[derive(Debug, Clone, Deserialize)]
/// 配置文件中的 `[cache]` 分组。
///
/// 注意：`my_playlist_cache_dir` 和 `log_dir` 是旧配置兼容字段。
pub(crate) struct CacheConfigFile {
    pub(crate) library_cache_dir: Option<String>,
    pub(crate) cover_cache_dir: Option<String>,
    pub(crate) lyrics_cache_dir: Option<String>,
    pub(crate) playlist_cache_dir: Option<String>,
    pub(crate) spectrum_cache_dir: Option<String>,
    pub(crate) log_cache_dir: Option<String>,
    pub(crate) my_playlist_cache_dir: Option<String>,
    pub(crate) log_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// 配置文件中的 `[decoder]` 分组。
pub(crate) struct DecoderConfigFile {
    pub(crate) output_dir: Option<String>,
    pub(crate) process_formats: Option<String>,
    pub(crate) scan_directory: Option<MusicDirectoryConfig>,
}

#[derive(Debug, Clone, Deserialize)]
/// 配置文件中的 `[style]` 分组。
///
/// 注意：`control_color` 是旧版高亮色字段，解析后会迁移到 `highlight_color`。
pub(crate) struct StyleConfigFile {
    pub(crate) background_color: Option<String>,
    pub(crate) background_image: Option<String>,
    pub(crate) background_image_opacity: Option<f32>,
    pub(crate) title_color: Option<String>,
    pub(crate) subtitle_color: Option<String>,
    pub(crate) highlight_color: Option<String>,
    pub(crate) border_color: Option<String>,
    pub(crate) control_color: Option<String>,
    pub(crate) show_border: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
/// 配置文件中的 `[state]` 分组。
pub(crate) struct AppStateConfigFile {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) volume: Option<f32>,
    pub(crate) sidebar_width: Option<u32>,
    pub(crate) auto_lyrics_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
/// 音乐目录配置兼容格式。
pub(crate) enum MusicDirectoryConfig {
    /// 旧版单目录字符串。
    Single(String),
    /// 新版多目录数组。
    Multiple(Vec<String>),
}

impl MusicDirectoryConfig {
    /// 兼容旧版单字符串配置和新版数组配置。
    pub(crate) fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(directory) => vec![directory],
            Self::Multiple(directories) => directories,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 单个音乐目录对应的曲库缓存。
pub(crate) struct LibraryCache {
    /// 该缓存对应的音乐目录。
    pub(crate) music_directory: String,
    /// 扫描时使用的封面缓存目录。
    pub(crate) cover_cache_dir: String,
    /// 扫描时使用的歌词缓存目录。
    pub(crate) lyrics_cache_dir: String,
    /// 缓存生成时间戳。
    pub(crate) generated_at: u64,
    /// 歌曲列表。
    pub(crate) tracks: TrackCacheEntries,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
/// 曲库缓存中的歌曲集合格式。
pub(crate) enum TrackCacheEntries {
    /// 新版格式：按歌曲 id 存储，前端可直接索引。
    ById(BTreeMap<String, Track>),
    /// 旧版格式：数组列表。
    List(Vec<Track>),
}

impl TrackCacheEntries {
    /// 兼容旧版数组缓存和新版按歌曲 id 存储的对象缓存。
    pub(crate) fn into_tracks(self) -> Vec<Track> {
        match self {
            Self::ById(tracks) => tracks.into_values().collect(),
            Self::List(tracks) => tracks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 歌单摘要，用于侧边栏和详情入口。
pub(crate) struct PlaylistSummary {
    /// 歌单 id。
    pub(crate) id: String,
    /// 歌单名称。
    pub(crate) name: String,
    /// 歌单类型。
    pub(crate) kind: String,
    /// 该歌单缓存文件路径。
    pub(crate) cache_path: String,
    /// 歌曲数量。
    pub(crate) track_count: usize,
    /// 总时长，单位秒。
    pub(crate) total_duration: u64,
    /// 展示封面路径。
    pub(crate) cover_cache_path: Option<String>,
    /// 歌曲 id 列表。
    ///
    /// 注意：系统汇总歌单通常只通过 children 展示，这里可能为空。
    #[serde(default)]
    pub(crate) track_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 歌单元数据。
pub(crate) struct PlaylistMetadata {
    /// 歌曲数量。
    pub(crate) track_count: usize,
    /// 总时长，单位秒。
    pub(crate) total_duration: u64,
    /// 子项数量，例如歌手数量或专辑数量。
    pub(crate) item_count: usize,
    /// 展示封面路径。
    pub(crate) cover_cache_path: Option<String>,
    /// 用户歌单排序索引。
    #[serde(default)]
    pub(crate) index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 单个歌单缓存。
///
/// 注意：用户歌单主要使用 `track_ids`，歌手/专辑等系统汇总歌单主要使用 `children`。
pub(crate) struct PlaylistCache {
    /// 歌单 id。
    pub(crate) id: String,
    /// 歌单名称。
    pub(crate) name: String,
    /// 歌单类型。
    pub(crate) kind: String,
    /// 缓存生成或更新时间戳。
    pub(crate) generated_at: u64,
    /// 歌单元数据。
    pub(crate) metadata: PlaylistMetadata,
    /// 引用“全部”曲库中歌曲 id 的列表。
    #[serde(default)]
    pub(crate) track_ids: Vec<String>,
    /// 系统汇总歌单的子项列表。
    #[serde(default)]
    pub(crate) children: Vec<PlaylistSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// “全部”歌单缓存。
///
/// 注意：这是前端曲库数据库，其他歌单通过歌曲 id 引用这里的 `tracks`。
pub(crate) struct AllPlaylistCache {
    /// 固定 id，通常为 `all`。
    pub(crate) id: String,
    /// 展示名称。
    pub(crate) name: String,
    /// 类型。
    pub(crate) kind: String,
    /// 当前配置的音乐目录。
    pub(crate) music_directory: Vec<String>,
    /// 封面缓存目录。
    pub(crate) cover_cache_dir: String,
    /// 歌词缓存目录。
    pub(crate) lyrics_cache_dir: String,
    /// 缓存生成时间戳。
    pub(crate) generated_at: u64,
    /// 所有歌曲，按歌曲 id 存储。
    pub(crate) tracks: BTreeMap<String, Track>,
    /// 顶层歌单摘要列表。
    pub(crate) playlists: Vec<PlaylistSummary>,
}

#[derive(Debug, Clone, Serialize)]
/// 前端一次性加载的歌单集合。
pub(crate) struct PlaylistBundle {
    /// 最近播放。
    pub(crate) recent: PlaylistCache,
    /// 默认“我的歌单”。
    pub(crate) my_playlist: PlaylistCache,
    /// 所有用户歌单。
    pub(crate) my_playlists: Vec<PlaylistCache>,
    /// 歌手汇总歌单。
    pub(crate) artists: PlaylistCache,
    /// 专辑汇总歌单。
    pub(crate) albums: PlaylistCache,
}

#[derive(Debug, Clone, Serialize)]
/// 应用启动时返回给前端的完整启动数据。
pub(crate) struct AppStartup {
    /// 当前配置。
    pub(crate) config: AppConfig,
    /// 默认配置。
    pub(crate) default_config: AppConfig,
    /// 启动时从缓存读取的全部歌曲。
    pub(crate) tracks: Vec<Track>,
    /// 启动时从缓存读取的歌单。
    pub(crate) playlists: PlaylistBundle,
    /// 播放统计。
    pub(crate) play_statistics: PlayStatistics,
    /// 最近一次播放记录，用于重启后恢复播放列表、歌曲和进度。
    pub(crate) playback_record: Option<PlaybackRecord>,
}

#[derive(Debug, Clone, Serialize)]
/// 手动刷新曲库后返回给前端的数据。
pub(crate) struct LibraryRefreshResult {
    /// 最新歌曲列表。
    pub(crate) tracks: Vec<Track>,
    /// 最新歌单集合。
    pub(crate) playlists: PlaylistBundle,
    /// 最新播放统计。
    pub(crate) play_statistics: PlayStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 播放记录来源。
///
/// 注意：一级来源表示来自哪个主列表；歌手、专辑详情会额外写入二级来源。
pub(crate) struct PlaybackRecordSource {
    /// 来源类型，例如 all、recent、playlist、artists、artist。
    pub(crate) source_type: String,
    /// 来源 id。
    pub(crate) id: String,
    /// 来源显示名称。
    pub(crate) label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 轻量播放记录缓存。
///
/// 注意：随机播放不会缓存随机后的队列顺序，恢复时只保证当前歌曲、进度和来源一致。
pub(crate) struct PlaybackRecord {
    /// 缓存结构版本。
    pub(crate) version: u8,
    /// 当前歌曲 id。
    pub(crate) track_id: String,
    /// 当前播放进度，单位秒。
    pub(crate) elapsed: u64,
    /// 当前播放模式。
    pub(crate) playback_mode: String,
    /// 一级播放列表来源。
    pub(crate) playlist: PlaybackRecordSource,
    /// 二级列表来源，主要用于歌手/专辑详情。
    #[serde(default)]
    pub(crate) secondary_playlist: Option<PlaybackRecordSource>,
    /// 最近更新时间戳。
    pub(crate) updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
/// 歌词搜索响应。
pub(crate) struct LyricsSearchResponse {
    /// 当前本地歌词缓存哈希。
    pub(crate) current_lyrics_hash: Option<String>,
    /// 搜索候选列表。
    pub(crate) results: Vec<LyricsSearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 前端歌词搜索请求。
pub(crate) struct LyricsSearchRequest {
    /// 歌曲 id。
    pub(crate) track_id: String,
    /// 歌曲名。
    pub(crate) title: String,
    /// 歌手名。
    pub(crate) artist: String,
    /// 专辑名。
    pub(crate) album: String,
    /// 歌曲时长，单位秒。
    pub(crate) duration: Option<u64>,
    /// 本地歌词缓存路径。
    pub(crate) lyrics_cache_path: String,
    /// 前端已知的歌词缓存哈希。
    pub(crate) lyrics_cache_hash: Option<String>,
    /// 是否强制刷新，绕过后端 moka 搜索缓存。
    pub(crate) force_refresh: bool,
}

#[derive(Debug, Clone, Serialize)]
/// 单个歌词搜索候选。
pub(crate) struct LyricsSearchResult {
    /// 歌词来源。
    pub(crate) source: String,
    /// 候选 id。
    pub(crate) id: String,
    /// 候选歌曲名。
    pub(crate) track_name: String,
    /// 候选歌手名。
    pub(crate) artist_name: String,
    /// 候选专辑名。
    pub(crate) album_name: String,
    /// 候选歌曲时长，单位秒。
    pub(crate) duration: Option<u64>,
    /// 歌词内容哈希，用于前端判断“同源”。
    pub(crate) lyrics_hash: String,
    /// LRC 或逐行同步歌词。
    pub(crate) synced_lyrics: Option<String>,
    /// 普通歌词文本。
    pub(crate) plain_lyrics: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
/// 使用歌词搜索结果后的返回值。
pub(crate) struct LyricsUseResult {
    /// 写入的歌词缓存路径。
    pub(crate) lyrics_cache_path: String,
    /// 写入歌词的哈希。
    pub(crate) lyrics_hash: String,
    /// 写入的歌词内容。
    pub(crate) lyrics: String,
    /// 同步更新后的歌曲对象。
    pub(crate) track: Option<Track>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// 前端 audio 标签播放错误记录。
pub(crate) struct AudioErrorRecord {
    /// 音频文件路径。
    pub(crate) path: Option<String>,
    /// 前端实际加载的媒体地址。
    pub(crate) source: String,
    /// HTMLMediaElement 错误码。
    pub(crate) code: Option<u16>,
    /// 前端错误描述。
    pub(crate) message: String,
    /// 错误发生时的播放进度，单位秒。
    pub(crate) elapsed: u64,
    /// HTMLMediaElement readyState。
    pub(crate) ready_state: u16,
    /// HTMLMediaElement networkState。
    pub(crate) network_state: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// 播放统计总表。
pub(crate) struct PlayStatistics {
    /// 累计播放次数。
    pub(crate) total_play_count: u64,
    /// 累计聆听时长，单位秒。
    pub(crate) total_listening_seconds: u64,
    /// 单曲统计，按歌曲 id 存储。
    #[serde(default)]
    pub(crate) tracks: BTreeMap<String, TrackPlayStatistic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 单首歌曲播放统计。
pub(crate) struct TrackPlayStatistic {
    /// 歌曲 id。
    pub(crate) track_id: String,
    /// 歌名快照。
    pub(crate) title: String,
    /// 歌手快照。
    pub(crate) artist: String,
    /// 专辑快照。
    pub(crate) album: String,
    /// 文件路径快照。
    pub(crate) path: String,
    /// 播放次数。
    pub(crate) play_count: u64,
    /// 聆听时长，单位秒。
    pub(crate) listening_seconds: u64,
    /// 最近播放时间戳。
    pub(crate) last_played_at: u64,
}
