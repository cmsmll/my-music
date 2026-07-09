import { defineStore } from "pinia";
import { ref } from "vue";
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

export const use_library_store = defineStore("library", () => {
  const selected_directories = ref<string[]>([]);
  const library_loaded = ref(false);
  const playlists = ref<PlaylistBundle>(empty_playlist_bundle());
  const play_statistics = ref<PlayStatistics>(empty_play_statistics());

  function set_selected_directories(directories: string[]) {
    selected_directories.value = [...directories];
  }

  function set_library_loaded(loaded: boolean) {
    library_loaded.value = loaded;
  }

  function set_playlists(next_playlists: PlaylistBundle) {
    playlists.value = next_playlists;
  }

  function set_play_statistics(statistics: PlayStatistics) {
    play_statistics.value = statistics;
  }

  function add_recent_track(track: Track) {
    const track_ids = playlists.value.recent.track_ids.filter((track_id) => track_id !== track.id);
    track_ids.unshift(track.id);
    playlists.value.recent = {
      ...playlists.value.recent,
      track_ids: track_ids.slice(0, 100),
      metadata: {
        ...playlists.value.recent.metadata,
        track_count: Math.min(track_ids.length, 100),
      },
    };
  }

  return {
    selected_directories,
    library_loaded,
    playlists,
    play_statistics,
    set_selected_directories,
    set_library_loaded,
    set_playlists,
    set_play_statistics,
    add_recent_track,
  };
});
