import { defineStore } from "pinia";
import type { PlayStatistics, PlaylistBundle, Track } from "../types/music";

function empty_playlist(id: string, name: string, kind: string) {
  return {
    id,
    name,
    kind,
    generated_at: 0,
    metadata: {
      track_count: 0,
      total_duration: 0,
      item_count: 0,
      cover_cache_path: null,
      index: 0,
    },
    track_ids: [],
    children: [],
  };
}

export function empty_playlist_bundle(): PlaylistBundle {
  const my_playlist = empty_playlist("my_playlist", "我的歌单", "user");
  return {
    recent: empty_playlist("recent", "最近播放", "recent"),
    my_playlist,
    my_playlists: [my_playlist],
    artists: empty_playlist("artists", "歌手", "artists"),
    albums: empty_playlist("albums", "专辑", "albums"),
  };
}

export function empty_play_statistics(): PlayStatistics {
  return {
    total_play_count: 0,
    total_listening_seconds: 0,
    tracks: {},
  };
}

export const use_library_store = defineStore("library", {
  state: () => ({
    selected_directories: [] as string[],
    library_loaded: false,
    playlists: empty_playlist_bundle(),
    play_statistics: empty_play_statistics(),
  }),
  actions: {
    set_selected_directories(directories: string[]) {
      this.selected_directories = [...directories];
    },
    set_library_loaded(loaded: boolean) {
      this.library_loaded = loaded;
    },
    set_playlists(playlists: PlaylistBundle) {
      this.playlists = playlists;
    },
    set_play_statistics(statistics: PlayStatistics) {
      this.play_statistics = statistics;
    },
    add_recent_track(track: Track) {
      const track_ids = this.playlists.recent.track_ids.filter((track_id) => track_id !== track.id);
      track_ids.unshift(track.id);
      this.playlists.recent = {
        ...this.playlists.recent,
        track_ids: track_ids.slice(0, 100),
        metadata: {
          ...this.playlists.recent.metadata,
          track_count: Math.min(track_ids.length, 100),
        },
      };
    },
  },
});
