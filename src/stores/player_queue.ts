import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import type { PlaybackMode, PlaybackRecord, QueueSource, Track } from "../types/music";

const default_queue_source: QueueSource = {
  type: "all",
  id: "all",
  label: "全部",
};

function shuffle_tracks(tracks: Track[]) {
  const shuffled = [...tracks];
  for (let index = shuffled.length - 1; index > 0; index -= 1) {
    const random_index = Math.floor(Math.random() * (index + 1));
    [shuffled[index], shuffled[random_index]] = [shuffled[random_index], shuffled[index]];
  }
  return shuffled;
}

export const use_player_queue_store = defineStore("player_queue", () => {
  const library_tracks = ref<Track[]>([]);
  const current_queue = ref<Track[]>([]);
  const random_queue = ref<Track[]>([]);
  const queue_source = ref<QueueSource>(default_queue_source);
  const current_track_path = ref<string | null>(null);
  const playback_mode = ref<PlaybackMode>("repeat");
  const playback_record = ref<PlaybackRecord | null>(null);
  const active_queue = computed(() =>
    playback_mode.value === "random" ? random_queue.value : current_queue.value,
  );

  const current_index = computed(() => {
    if (!current_track_path.value) return -1;
    return active_queue.value.findIndex((track) => track.path === current_track_path.value);
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
    random_queue.value = build_random_queue(tracks);
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

  function hydrate_playback_record(record?: PlaybackRecord | null) {
    playback_record.value = record ?? null;
  }

  function set_playback_record(record: PlaybackRecord) {
    playback_record.value = { ...record };
  }

  async function save_playback_record(record: PlaybackRecord) {
    set_playback_record(record);
    await invoke("save_playback_record", { record });
  }

  function build_random_queue(tracks: Track[], anchor_track_id?: string | null) {
    const anchor_track = anchor_track_id
      ? tracks.find((track) => track.id === anchor_track_id)
      : undefined;
    const other_tracks = anchor_track
      ? tracks.filter((track) => track.id !== anchor_track.id)
      : tracks;
    return anchor_track
      ? [anchor_track, ...shuffle_tracks(other_tracks)]
      : shuffle_tracks(other_tracks);
  }

  function randomize_random_queue(anchor_track_id?: string | null) {
    random_queue.value = build_random_queue(current_queue.value, anchor_track_id);
    return random_queue.value;
  }

  function rerandomize_random_queue(excluded_first_track_id?: string | null) {
    const source_queue = current_queue.value.length ? current_queue.value : random_queue.value;
    const next_queue = shuffle_tracks(source_queue);
    if (excluded_first_track_id && next_queue.length > 1 && next_queue[0]?.id === excluded_first_track_id) {
      const swap_index = next_queue.findIndex((track, index) => index > 0 && track.id !== excluded_first_track_id);
      if (swap_index > 0) {
        [next_queue[0], next_queue[swap_index]] = [next_queue[swap_index], next_queue[0]];
      }
    }

    random_queue.value = next_queue;
    return random_queue.value;
  }

  return {
    library_tracks,
    current_queue,
    random_queue,
    active_queue,
    queue_source,
    current_track_path,
    playback_mode,
    playback_record,
    current_index,
    set_library_tracks,
    set_current_queue,
    set_current_track_path,
    upsert_track,
    set_playback_mode,
    hydrate_playback_record,
    set_playback_record,
    save_playback_record,
    randomize_random_queue,
    rerandomize_random_queue,
  };
});
