import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { PlaybackMode, QueueSource, Track } from "../types/music";

const default_queue_source: QueueSource = {
  type: "all",
  id: "all",
  label: "全部",
};

export const use_player_queue_store = defineStore("player_queue", () => {
  const library_tracks = ref<Track[]>([]);
  const current_queue = ref<Track[]>([]);
  const queue_source = ref<QueueSource>(default_queue_source);
  const current_track_path = ref<string | null>(null);
  const playback_mode = ref<PlaybackMode>("repeat");

  const current_index = computed(() => {
    if (!current_track_path.value) return -1;
    return current_queue.value.findIndex((track) => track.path === current_track_path.value);
  });

  function set_library_tracks(tracks: Track[]) {
    library_tracks.value = tracks;
    if (queue_source.value.type === "all" || !current_queue.value.length) {
      set_current_queue(default_queue_source, tracks);
    }
  }

  function set_current_queue(source: QueueSource, tracks: Track[]) {
    queue_source.value = source;
    current_queue.value = [...tracks];
  }

  function set_current_track_path(path?: string | null) {
    current_track_path.value = path ?? null;
  }

  function upsert_track(track: Track) {
    const update_track = (current: Track) => (current.id === track.id ? { ...current, ...track } : current);
    library_tracks.value = library_tracks.value.map(update_track);
  }

  function set_playback_mode(mode: PlaybackMode) {
    playback_mode.value = mode;
  }

  return {
    library_tracks,
    current_queue,
    queue_source,
    current_track_path,
    playback_mode,
    current_index,
    set_library_tracks,
    set_current_queue,
    set_current_track_path,
    upsert_track,
    set_playback_mode,
  };
});
