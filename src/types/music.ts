export type Track = {
  id: string;
  title: string;
  artist: string;
  album: string;
  path: string;
  duration?: number | null;
  cover_cache_path?: string | null;
  lyrics_cache_path: string;
  metadata: TrackMetadata;
};

export type TrackMetadata = {
  title: string;
  artist: string;
  album: string;
  duration?: number | null;
  bitrate?: number | null;
  sample_rate?: number | null;
  year?: number | null;
  genre: string[];
  track_number?: number | null;
  disk_number?: number | null;
  cover_cache_path?: string | null;
  lyrics_cache_path: string;
  metadata_source: "embedded" | "embedded_with_filename_fallback" | "filename";
};

export type AppConfig = {
  music_directory: string[];
  library_cache_dir: string;
  cover_cache_dir: string;
  lyrics_cache_dir: string;
  my_playlist_cache_dir: string;
};

export type AppStartup = {
  config: AppConfig;
  tracks: Track[];
  playlists: PlaylistBundle;
};

export type PlaybackStatus = {
  path?: string | null;
  playing: boolean;
  volume: number;
  elapsed: number;
};

export type ViewKey = "all" | "artists" | "albums" | "stats" | "recent" | "playlist_1";

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
