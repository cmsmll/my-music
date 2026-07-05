import { defineStore } from "pinia";
import type { PlaybackStatus, Track } from "../types/music";

const default_status: PlaybackStatus = {
  path: null,
  playing: false,
  volume: 1,
  elapsed: 0,
};

export const use_playback_store = defineStore("playback", {
  state: () => ({
    tracks_by_id: {} as Record<string, Track>,
    tracks_by_path: {} as Record<string, Track>,
    current_track_id: null as string | null,
    current_track_path: null as string | null,
    status: { ...default_status } as PlaybackStatus,
    visual_elapsed: 0,
    progress_dragging: false,
  }),
  getters: {
    current_track(state) {
      if (state.current_track_id) {
        const track = state.tracks_by_id[state.current_track_id];
        if (track) return track;
      }
      if (state.current_track_path) {
        return state.tracks_by_path[state.current_track_path] ?? null;
      }
      return null;
    },
    duration(): number {
      return this.current_track?.duration ?? 0;
    },
    progress_percent(): number {
      if (!this.duration) return 0;
      return Math.min(Math.max((this.visual_elapsed / this.duration) * 100, 0), 100);
    },
  },
  actions: {
    set_library_tracks(tracks: Track[]) {
      this.tracks_by_id = Object.fromEntries(tracks.map((track) => [track.id, track]));
      this.tracks_by_path = Object.fromEntries(tracks.map((track) => [track.path, track]));
      this.sync_current_track_from_path(this.status.path ?? this.current_track_path);
    },
    upsert_track(track: Track) {
      this.tracks_by_id = {
        ...this.tracks_by_id,
        [track.id]: track,
      };
      if (track.path) {
        this.tracks_by_path = {
          ...this.tracks_by_path,
          [track.path]: track,
        };
      }
      this.sync_current_track_from_path(this.status.path ?? this.current_track_path);
    },
    set_status(status: PlaybackStatus) {
      this.status = { ...status };
      this.sync_current_track_from_path(status.path);
    },
    patch_status(status: Partial<PlaybackStatus>) {
      this.set_status({
        ...this.status,
        ...status,
      });
    },
    set_visual_elapsed(seconds: number) {
      this.visual_elapsed = Math.max(0, seconds);
    },
    set_progress_dragging(dragging: boolean) {
      this.progress_dragging = dragging;
    },
    sync_current_track_from_path(path?: string | null) {
      this.current_track_path = path ?? null;
      const track = path ? this.tracks_by_path[path] : null;
      this.current_track_id = track?.id ?? null;
    },
    reset() {
      this.status = { ...default_status };
      this.current_track_id = null;
      this.current_track_path = null;
      this.visual_elapsed = 0;
      this.progress_dragging = false;
    },
  },
});
