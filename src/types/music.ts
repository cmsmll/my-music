export type Track = {
  id: string;
  title: string;
  artist: string;
  album: string;
  path: string;
  duration?: number | null;
  file_size?: number | null;
  bitrate?: number | null;
  sample_rate?: number | null;
  year?: number | null;
  genre?: string[];
  track_number?: number | null;
  disk_number?: number | null;
  cover_cache_path?: string | null;
  lyrics_cache_path: string;
  lyrics_cache_hash: string;
  metadata_source: "embedded" | "embedded_with_filename_fallback" | "filename";
  missing?: boolean;
};

export type AppConfig = {
  music_directory: string[];
  decoder: DecoderConfig;
  cache: CacheConfig;
  style: StyleConfig;
  state: AppStateConfig;
};

export type CacheConfig = {
  library_cache_dir: string;
  playlist_cache_dir: string;
  lyrics_cache_dir: string;
  cover_cache_dir: string;
  spectrum_cache_dir: string;
  log_cache_dir: string;
};

export type DecoderConfig = {
  output_dir: string;
  process_formats: string;
  scan_directory: string[];
};

export type DecoderRunSummary = {
  executed: boolean;
  scanned: number;
  processed: number;
  skipped: number;
  failed: number;
  output_dir: string;
  scan_directory_count: number;
  message: string;
};

export type StyleConfig = {
  background_color: string;
  background_image: string;
  background_image_opacity: number;
  title_color: string;
  subtitle_color: string;
  highlight_color: string;
  border_color: string;
  show_border: boolean;
};

export type AppStateConfig = {
  width: number;
  height: number;
  volume: number;
  sidebar_width: number;
  auto_lyrics_enabled: boolean;
};

export type AppStartup = {
  config: AppConfig;
  default_config: AppConfig;
  tracks: Track[];
  playlists: PlaylistBundle;
  play_statistics: PlayStatistics;
};

export type LibraryRefreshResult = {
  tracks: Track[];
  playlists: PlaylistBundle;
  play_statistics: PlayStatistics;
};

export type PlaybackStatus = {
  path?: string | null;
  playing: boolean;
  volume: number;
  elapsed: number;
};

export type PlayTrackResult = {
  status: PlaybackStatus;
  play_statistics: PlayStatistics;
};

export type LyricsSearchResult = {
  source: string;
  id: string;
  track_name: string;
  artist_name: string;
  album_name: string;
  duration?: number | null;
  lyrics_hash: string;
  synced_lyrics?: string | null;
  plain_lyrics?: string | null;
};

export type LyricsSearchResponse = {
  current_lyrics_hash?: string | null;
  results: LyricsSearchResult[];
};

export type LyricsUseResult = {
  lyrics_cache_path: string;
  lyrics_hash: string;
  lyrics: string;
  track?: Track | null;
};

export type ViewKey = "all" | "artists" | "albums" | "stats" | "recent" | "user_playlist";

export type PlaybackMode = "shuffle" | "repeat" | "repeat_one";

export type PlaybackModeItem = {
  mode: PlaybackMode;
  icon: string;
  label: string;
};

export type ArtistItem = {
  name: string;
  track_count: number;
  total_duration: number;
  cover_track?: Track;
};

export type AlbumItem = {
  name: string;
  artist: string;
  track_count: number;
  total_duration: number;
  cover_track?: Track;
};

export type QueueSourceType = ViewKey | "search" | "artist" | "album" | "playlist";

export type QueueSource = {
  type: QueueSourceType;
  id: string;
  label: string;
};

export type PlaylistMetadata = {
  track_count: number;
  total_duration: number;
  item_count: number;
  cover_cache_path?: string | null;
  index: number;
};

export type PlaylistSummary = {
  id: string;
  name: string;
  kind: string;
  cache_path: string;
  track_count: number;
  total_duration: number;
  cover_cache_path?: string | null;
  track_ids: string[];
};

export type PlaylistCache = {
  id: string;
  name: string;
  kind: string;
  generated_at: number;
  metadata: PlaylistMetadata;
  track_ids: string[];
  children: PlaylistSummary[];
};

export type PlaylistBundle = {
  recent: PlaylistCache;
  my_playlist: PlaylistCache;
  my_playlists: PlaylistCache[];
  artists: PlaylistCache;
  albums: PlaylistCache;
};

export type TrackPlayStatistic = {
  track_id: string;
  title: string;
  artist: string;
  album: string;
  path: string;
  play_count: number;
  listening_seconds: number;
  last_played_at: number;
};

export type PlayStatistics = {
  total_play_count: number;
  total_listening_seconds: number;
  tracks: Record<string, TrackPlayStatistic>;
};
