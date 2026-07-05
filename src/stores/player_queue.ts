import { defineStore } from "pinia";
import type { PlaybackMode, QueueSource, Track } from "../types/music";

const default_queue_source: QueueSource = {
  type: "all",
  id: "all",
  label: "全部",
};

export const use_player_queue_store = defineStore("player_queue", {
  state: () => ({
    library_tracks: [] as Track[],
    current_queue: [] as Track[],
    queue_source: default_queue_source,
    current_track_path: null as string | null,
    playback_mode: "repeat" as PlaybackMode,
  }),
  getters: {
    current_index(state) {
      if (!state.current_track_path) return -1;
      return state.current_queue.findIndex((track) => track.path === state.current_track_path);
    },
  },
  actions: {
    set_library_tracks(tracks: Track[]) {
      this.library_tracks = tracks;
      if (this.queue_source.type === "all" || !this.current_queue.length) {
        this.set_current_queue(default_queue_source, tracks);
      }
    },
    set_current_queue(source: QueueSource, tracks: Track[]) {
      this.queue_source = source;
      this.current_queue = [...tracks];
    },
    set_current_track_path(path?: string | null) {
      this.current_track_path = path ?? null;
    },
    upsert_track(track: Track) {
      const update_track = (current: Track) => (current.id === track.id ? { ...current, ...track } : current);
      this.library_tracks = this.library_tracks.map(update_track);
      this.current_queue = this.current_queue.map(update_track);
    },
    set_playback_mode(mode: PlaybackMode) {
      this.playback_mode = mode;
    },
  },
});
