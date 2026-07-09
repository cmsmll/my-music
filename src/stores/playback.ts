import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { PlaybackStatus, Track } from "../types/music";

const default_status: PlaybackStatus = {
  path: null,
  playing: false,
  volume: 1,
  elapsed: 0,
};

export const use_playback_store = defineStore("playback", () => {
  const tracks_by_id = ref<Record<string, Track>>({});
  const tracks_by_path = ref<Record<string, Track>>({});
  const current_track_id = ref<string | null>(null);
  const current_track_path = ref<string | null>(null);
  const status = ref<PlaybackStatus>({ ...default_status });
  const visual_elapsed = ref(0);
  const progress_dragging = ref(false);

  const current_track = computed(() => {
    if (current_track_id.value) {
      const track = tracks_by_id.value[current_track_id.value];
      if (track) return track;
    }
    if (current_track_path.value) {
      return tracks_by_path.value[current_track_path.value] ?? null;
    }
    return null;
  });

  const duration = computed(() => current_track.value?.duration ?? 0);
  const progress_percent = computed(() => {
    if (!duration.value) return 0;
    return Math.min(Math.max((visual_elapsed.value / duration.value) * 100, 0), 100);
  });

  function set_library_tracks(tracks: Track[]) {
    tracks_by_id.value = Object.fromEntries(tracks.map((track) => [track.id, track]));
    tracks_by_path.value = Object.fromEntries(tracks.map((track) => [track.path, track]));
    sync_current_track_from_path(status.value.path ?? current_track_path.value);
  }

  function upsert_track(track: Track) {
    tracks_by_id.value = {
      ...tracks_by_id.value,
      [track.id]: track,
    };
    if (track.path) {
      tracks_by_path.value = {
        ...tracks_by_path.value,
        [track.path]: track,
      };
    }
    sync_current_track_from_path(status.value.path ?? current_track_path.value);
  }

  function set_status(next_status: PlaybackStatus) {
    status.value = { ...next_status };
    sync_current_track_from_path(next_status.path);
  }

  function patch_status(next_status: Partial<PlaybackStatus>) {
    set_status({
      ...status.value,
      ...next_status,
    });
  }

  function set_visual_elapsed(seconds: number) {
    visual_elapsed.value = Math.max(0, seconds);
  }

  function set_progress_dragging(dragging: boolean) {
    progress_dragging.value = dragging;
  }

  function sync_current_track_from_path(path?: string | null) {
    current_track_path.value = path ?? null;
    const track = path ? tracks_by_path.value[path] : null;
    current_track_id.value = track?.id ?? null;
  }

  function reset() {
    status.value = { ...default_status };
    current_track_id.value = null;
    current_track_path.value = null;
    visual_elapsed.value = 0;
    progress_dragging.value = false;
  }

  return {
    tracks_by_id,
    tracks_by_path,
    current_track_id,
    current_track_path,
    status,
    visual_elapsed,
    progress_dragging,
    current_track,
    duration,
    progress_percent,
    set_library_tracks,
    upsert_track,
    set_status,
    patch_status,
    set_visual_elapsed,
    set_progress_dragging,
    sync_current_track_from_path,
    reset,
  };
});
