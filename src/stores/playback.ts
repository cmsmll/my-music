import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { use_library_catalog_store } from "./library_catalog";
import type { PlaybackStatus } from "../types/music";

const default_status: PlaybackStatus = {
  path: null,
  playing: false,
  volume: 1,
  elapsed: 0,
};

export const use_playback_store = defineStore("playback", () => {
  const catalog = use_library_catalog_store();
  const current_track_id = ref<string | null>(null);
  const current_track_path = ref<string | null>(null);
  const status = ref<PlaybackStatus>({ ...default_status });
  const visual_elapsed = ref(0);
  const progress_dragging = ref(false);

  const current_track = computed(() => {
    if (current_track_id.value) {
      const track = catalog.track_by_id(current_track_id.value);
      if (track) return track;
    }
    if (current_track_path.value) {
      return catalog.track_by_path(current_track_path.value);
    }
    return null;
  });

  const duration = computed(() => current_track.value?.duration ?? 0);
  const progress_percent = computed(() => {
    if (!duration.value) return 0;
    return Math.min(Math.max((visual_elapsed.value / duration.value) * 100, 0), 100);
  });

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
    const track = catalog.track_by_path(path);
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

    current_track_id,
    current_track_path,
    status,
    visual_elapsed,
    progress_dragging,
    current_track,
    duration,
    progress_percent,

    set_status,
    patch_status,
    set_visual_elapsed,
    set_progress_dragging,
    sync_current_track_from_path,
    reset,
  };
});
