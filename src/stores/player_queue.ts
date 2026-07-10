import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";
import { use_library_catalog_store } from "./library_catalog";
import type { PlaybackMode, PlaybackRecord, QueueSource, Track } from "../types/music";

const default_queue_source: QueueSource = {
  type: "all",
  id: "all",
  label: "全部",
};

const playback_record_storage_key = "my_music_playback_record";
const playback_elapsed_storage_key = "my_music_playback_elapsed";
const playback_mode_semantics_storage_key = "my_music_playback_mode_semantics";
const playback_mode_semantics_version = "shuffle-random-v2";

type PlaybackRecordMetadata = Omit<PlaybackRecord, "elapsed">;

function migrate_playback_mode_semantics(mode: unknown): PlaybackMode | undefined {
  if (mode !== "random" && mode !== "shuffle" && mode !== "repeat" && mode !== "repeat_one") {
    return undefined;
  }

  if (localStorage.getItem(playback_mode_semantics_storage_key) === playback_mode_semantics_version) {
    return mode;
  }

  if (mode === "random") return "shuffle";
  if (mode === "shuffle") return "random";
  return mode;
}

function shuffle_tracks(tracks: string[]) {
  const shuffled = [...tracks];
  for (let index = shuffled.length - 1; index > 0; index -= 1) {
    const random_index = Math.floor(Math.random() * (index + 1));
    [shuffled[index], shuffled[random_index]] = [shuffled[random_index], shuffled[index]];
  }
  return shuffled;
}

function read_playback_record_from_storage(): PlaybackRecord | null {
  try {
    const raw_record = localStorage.getItem(playback_record_storage_key);
    if (!raw_record) return null;

    const metadata = JSON.parse(raw_record) as Partial<PlaybackRecordMetadata>;
    const playback_mode = migrate_playback_mode_semantics(metadata.playback_mode);
    if (metadata.version !== 1 || !metadata.track_id || !playback_mode || !metadata.playlist) {
      return null;
    }

    return {
      version: 1,
      track_id: metadata.track_id,
      playback_mode,
      playlist: metadata.playlist,
      secondary_playlist: metadata.secondary_playlist ?? null,
      elapsed: read_playback_elapsed_from_storage(),
    };
  } catch (error) {
    console.warn("无法读取播放记录缓存", error);
    return null;
  }
}

function read_playback_elapsed_from_storage() {
  const elapsed = Number(localStorage.getItem(playback_elapsed_storage_key));
  return Number.isFinite(elapsed) && elapsed > 0 ? Math.floor(elapsed) : 0;
}

function write_playback_record_metadata(record: PlaybackRecord) {
  const { elapsed: _elapsed, ...metadata } = record;
  localStorage.setItem(playback_mode_semantics_storage_key, playback_mode_semantics_version);
  localStorage.setItem(playback_record_storage_key, JSON.stringify(metadata));
}

function write_playback_elapsed(seconds: number) {
  localStorage.setItem(playback_elapsed_storage_key, String(Math.max(0, Math.floor(seconds))));
}

export const use_player_queue_store = defineStore("player_queue", () => {
  const catalog = use_library_catalog_store();
  const current_queue_ids = shallowRef<string[]>([]);
  const shuffle_queue_ids = shallowRef<string[]>([]);
  const queue_source = ref<QueueSource>(default_queue_source);
  const current_track_path = ref<string | null>(null);
  const playback_mode = ref<PlaybackMode>("repeat");
  const playback_record = ref<PlaybackRecord | null>(null);

  const library_tracks = computed(() => catalog.tracks);
  const current_queue = computed(() => catalog.resolve_track_ids(current_queue_ids.value));
  const shuffle_queue = computed(() => catalog.resolve_track_ids(shuffle_queue_ids.value));
  const active_queue = computed(() =>
    playback_mode.value === "shuffle" ? shuffle_queue.value : current_queue.value,
  );
  const current_index = computed(() => {
    if (!current_track_path.value) return -1;
    return active_queue.value.findIndex((track) => track.path === current_track_path.value);
  });

  function set_library_tracks(tracks: Track[]) {
    catalog.set_tracks(tracks);
    if (queue_source.value.type === "all" || !current_queue_ids.value.length) {
      set_current_queue(default_queue_source, tracks);
    }
  }

  function set_current_queue(source: QueueSource, tracks: Track[]) {
    queue_source.value = source;
    current_queue_ids.value = tracks.map((track) => track.id);
    shuffle_queue_ids.value = build_shuffle_queue(current_queue_ids.value);
  }

  function set_current_track_path(path?: string | null) { current_track_path.value = path ?? null; }
  function upsert_track(track: Track) { catalog.upsert_track(track); }
  function set_playback_mode(mode: PlaybackMode) {
    if (mode === "shuffle" && playback_mode.value !== "shuffle") {
      shuffle_queue_ids.value = build_shuffle_queue(current_queue_ids.value);
    }
    playback_mode.value = mode;
  }
  function hydrate_playback_record(record?: PlaybackRecord | null) {
    playback_record.value = record ?? read_playback_record_from_storage();
  }
  function set_playback_record(record: PlaybackRecord) { playback_record.value = { ...record }; }
  function save_playback_record(record: PlaybackRecord) {
    set_playback_record(record); write_playback_record_metadata(record); write_playback_elapsed(record.elapsed);
  }
  function save_playback_record_metadata(record: PlaybackRecord) {
    set_playback_record(record); write_playback_record_metadata(record);
  }
  function save_playback_elapsed(seconds: number) {
    const elapsed = Math.max(0, Math.floor(seconds));
    write_playback_elapsed(elapsed);
    if (playback_record.value) playback_record.value = { ...playback_record.value, elapsed };
  }
  function build_shuffle_queue(ids: string[], anchor_track_id?: string | null) {
    const anchor = anchor_track_id && ids.includes(anchor_track_id) ? anchor_track_id : null;
    const others = anchor ? ids.filter((id) => id !== anchor) : ids;
    return anchor ? [anchor, ...shuffle_tracks(others)] : shuffle_tracks(others);
  }
  function shuffle_current_queue(anchor_track_id?: string | null) {
    shuffle_queue_ids.value = build_shuffle_queue(current_queue_ids.value, anchor_track_id);
    return shuffle_queue.value;
  }
  function reshuffle_queue(excluded_first_track_id?: string | null) {
    const source = current_queue_ids.value.length ? current_queue_ids.value : shuffle_queue_ids.value;
    const next = shuffle_tracks(source);
    if (excluded_first_track_id && next.length > 1 && next[0] === excluded_first_track_id) {
      const index = next.findIndex((id, current) => current > 0 && id !== excluded_first_track_id);
      if (index > 0) [next[0], next[index]] = [next[index], next[0]];
    }
    shuffle_queue_ids.value = next;
    return shuffle_queue.value;
  }

  return { library_tracks, current_queue, shuffle_queue, active_queue, queue_source, current_track_path,
    playback_mode, playback_record, current_index, set_library_tracks, set_current_queue,
    set_current_track_path, upsert_track, set_playback_mode, hydrate_playback_record,
    set_playback_record, save_playback_record, save_playback_record_metadata,
    save_playback_elapsed, shuffle_current_queue, reshuffle_queue };
});