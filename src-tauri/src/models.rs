use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) path: String,
    pub(crate) duration: Option<u64>,
    #[serde(default)]
    pub(crate) file_size: Option<u64>,
    #[serde(default)]
    pub(crate) bitrate: Option<u32>,
    #[serde(default)]
    pub(crate) sample_rate: Option<u32>,
    #[serde(default)]
    pub(crate) year: Option<u16>,
    #[serde(default)]
    pub(crate) genre: Vec<String>,
    #[serde(default)]
    pub(crate) track_number: Option<u32>,
    #[serde(default)]
    pub(crate) disk_number: Option<u32>,
    pub(crate) cover_cache_path: Option<String>,
    pub(crate) lyrics_cache_path: String,
    #[serde(default)]
    pub(crate) lyrics_cache_hash: String,
    #[serde(default = "default_metadata_source")]
    pub(crate) metadata_source: MetadataSource,
    #[serde(default, rename = "metadata", skip_serializing)]
    pub(crate) legacy_metadata: Option<TrackMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrackMetadata {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) duration: Option<u64>,
    pub(crate) bitrate: Option<u32>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) year: Option<u16>,
    pub(crate) genre: Vec<String>,
    pub(crate) track_number: Option<u32>,
    pub(crate) disk_number: Option<u32>,
    pub(crate) cover_cache_path: Option<String>,
    pub(crate) lyrics_cache_path: String,
    #[serde(default)]
    pub(crate) lyrics_cache_hash: String,
    pub(crate) metadata_source: MetadataSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MetadataSource {
    Embedded,
    EmbeddedWithFilenameFallback,
    Filename,
}

fn default_metadata_source() -> MetadataSource {
    MetadataSource::Filename
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
    pub(crate) music_directory: Vec<String>,
    pub(crate) decoder: DecoderConfig,
    pub(crate) cache: CacheConfig,
    pub(crate) style: StyleConfig,
    pub(crate) state: AppStateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CacheConfig {
    pub(crate) library_cache_dir: String,
    pub(crate) cover_cache_dir: String,
    pub(crate) lyrics_cache_dir: String,
    pub(crate) my_playlist_cache_dir: String,
    pub(crate) log_dir: String,
    pub(crate) play_statistics_cache_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DecoderConfig {
    pub(crate) output_dir: String,
    pub(crate) process_formats: String,
    pub(crate) scan_directory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StyleConfig {
    pub(crate) background_color: String,
    pub(crate) background_image: String,
    pub(crate) background_image_opacity: f32,
    pub(crate) title_color: String,
    pub(crate) subtitle_color: String,
    pub(crate) highlight_color: String,
    pub(crate) show_border: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppStateConfig {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) volume: f32,
    pub(crate) sidebar_width: u32,
    pub(crate) auto_lyrics_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ConfigFile {
    pub(crate) music_directory: Option<MusicDirectoryConfig>,
    pub(crate) decoder: Option<DecoderConfigFile>,
    pub(crate) cache: Option<CacheConfigFile>,
    // 兼容旧版扁平配置，保存后会迁移到 [cache]。
    pub(crate) library_cache_dir: Option<String>,
    pub(crate) cover_cache_dir: Option<String>,
    pub(crate) lyrics_cache_dir: Option<String>,
    pub(crate) my_playlist_cache_dir: Option<String>,
    pub(crate) log_dir: Option<String>,
    pub(crate) play_statistics_cache_path: Option<String>,
    pub(crate) style: Option<StyleConfigFile>,
    pub(crate) state: Option<AppStateConfigFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CacheConfigFile {
    pub(crate) library_cache_dir: Option<String>,
    pub(crate) cover_cache_dir: Option<String>,
    pub(crate) lyrics_cache_dir: Option<String>,
    pub(crate) my_playlist_cache_dir: Option<String>,
    pub(crate) log_dir: Option<String>,
    pub(crate) play_statistics_cache_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DecoderConfigFile {
    pub(crate) output_dir: Option<String>,
    pub(crate) process_formats: Option<String>,
    pub(crate) scan_directory: Option<MusicDirectoryConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StyleConfigFile {
    pub(crate) background_color: Option<String>,
    pub(crate) background_image: Option<String>,
    pub(crate) background_image_opacity: Option<f32>,
    pub(crate) title_color: Option<String>,
    pub(crate) subtitle_color: Option<String>,
    pub(crate) highlight_color: Option<String>,
    pub(crate) control_color: Option<String>,
    pub(crate) show_border: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AppStateConfigFile {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) volume: Option<f32>,
    pub(crate) sidebar_width: Option<u32>,
    pub(crate) auto_lyrics_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum MusicDirectoryConfig {
    Single(String),
    Multiple(Vec<String>),
}

impl MusicDirectoryConfig {
    pub(crate) fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(directory) => vec![directory],
            Self::Multiple(directories) => directories,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LibraryCache {
    pub(crate) music_directory: String,
    pub(crate) cover_cache_dir: String,
    pub(crate) lyrics_cache_dir: String,
    pub(crate) generated_at: u64,
    pub(crate) tracks: TrackCacheEntries,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum TrackCacheEntries {
    ById(BTreeMap<String, Track>),
    List(Vec<Track>),
}

impl TrackCacheEntries {
    pub(crate) fn into_tracks(self) -> Vec<Track> {
        match self {
            Self::ById(tracks) => tracks.into_values().collect(),
            Self::List(tracks) => tracks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) cache_path: String,
    pub(crate) track_count: usize,
    pub(crate) total_duration: u64,
    pub(crate) cover_cache_path: Option<String>,
    #[serde(default)]
    pub(crate) track_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistMetadata {
    pub(crate) track_count: usize,
    pub(crate) total_duration: u64,
    pub(crate) item_count: usize,
    pub(crate) cover_cache_path: Option<String>,
    #[serde(default)]
    pub(crate) index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlaylistCache {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) generated_at: u64,
    pub(crate) metadata: PlaylistMetadata,
    pub(crate) track_ids: Vec<String>,
    pub(crate) children: Vec<PlaylistSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AllPlaylistCache {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) music_directory: Vec<String>,
    pub(crate) cover_cache_dir: String,
    pub(crate) lyrics_cache_dir: String,
    pub(crate) generated_at: u64,
    pub(crate) tracks: BTreeMap<String, Track>,
    pub(crate) playlists: Vec<PlaylistSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlaylistBundle {
    pub(crate) recent: PlaylistCache,
    pub(crate) my_playlist: PlaylistCache,
    pub(crate) my_playlists: Vec<PlaylistCache>,
    pub(crate) artists: PlaylistCache,
    pub(crate) albums: PlaylistCache,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppStartup {
    pub(crate) config: AppConfig,
    pub(crate) default_config: AppConfig,
    pub(crate) tracks: Vec<Track>,
    pub(crate) playlists: PlaylistBundle,
    pub(crate) play_statistics: PlayStatistics,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LibraryRefreshResult {
    pub(crate) tracks: Vec<Track>,
    pub(crate) playlists: PlaylistBundle,
    pub(crate) play_statistics: PlayStatistics,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlaybackStatus {
    pub(crate) path: Option<String>,
    pub(crate) playing: bool,
    pub(crate) volume: f32,
    pub(crate) elapsed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlayTrackResult {
    pub(crate) status: PlaybackStatus,
    pub(crate) play_statistics: PlayStatistics,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LyricsSearchResponse {
    pub(crate) current_lyrics_hash: Option<String>,
    pub(crate) results: Vec<LyricsSearchResult>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LyricsSearchResult {
    pub(crate) source: String,
    pub(crate) id: String,
    pub(crate) track_name: String,
    pub(crate) artist_name: String,
    pub(crate) album_name: String,
    pub(crate) duration: Option<u64>,
    pub(crate) lyrics_hash: String,
    pub(crate) synced_lyrics: Option<String>,
    pub(crate) plain_lyrics: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LyricsUseResult {
    pub(crate) lyrics_cache_path: String,
    pub(crate) lyrics_hash: String,
    pub(crate) lyrics: String,
    pub(crate) track: Option<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PlayStatistics {
    pub(crate) total_play_count: u64,
    pub(crate) total_listening_seconds: u64,
    #[serde(default)]
    pub(crate) tracks: BTreeMap<String, TrackPlayStatistic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrackPlayStatistic {
    pub(crate) track_id: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) path: String,
    pub(crate) play_count: u64,
    pub(crate) listening_seconds: u64,
    pub(crate) last_played_at: u64,
}
