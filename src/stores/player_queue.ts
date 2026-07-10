import { defineStore } from "pinia";
import { computed, ref } from "vue";
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

function shuffle_tracks(tracks: Track[]) {
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
  const library_tracks = ref<Track[]>([]);
  const current_queue = ref<Track[]>([]);
  const shuffle_queue = ref<Track[]>([]);
  const queue_source = ref<QueueSource>(default_queue_source);
  const current_track_path = ref<string | null>(null);
  const playback_mode = ref<PlaybackMode>("repeat");
  const playback_record = ref<PlaybackRecord | null>(null);
  const active_queue = computed(() =>
    playback_mode.value === "shuffle" ? shuffle_queue.value : current_queue.value,
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
    shuffle_queue.value = build_shuffle_queue(tracks);
  }

  function set_current_track_path(path?: string | null) {
    current_track_path.value = path ?? null;
  }

  function upsert_track(track: Track) {
    const update_track = (current: Track) => (current.id === track.id ? { ...current, ...track } : current);
    library_tracks.value = library_tracks.value.map(update_track);
  }

  function set_playback_mode(mode: PlaybackMode) {
    if (mode === "shuffle" && playback_mode.value !== "shuffle") {
      shuffle_queue.value = build_shuffle_queue(current_queue.value);
    }
    playback_mode.value = mode;
  }

  function hydrate_playback_record(record?: PlaybackRecord | null) {
    playback_record.value = record ?? read_playback_record_from_storage();
  }

  function set_playback_record(record: PlaybackRecord) {
    playback_record.value = { ...record };
  }

  function save_playback_record(record: PlaybackRecord) {
    set_playback_record(record);
    write_playback_record_metadata(record);
    write_playback_elapsed(record.elapsed);
  }

  function save_playback_record_metadata(record: PlaybackRecord) {
    set_playback_record(record);
    write_playback_record_metadata(record);
  }

  function save_playback_elapsed(seconds: number) {
    const elapsed = Math.max(0, Math.floor(seconds));
    write_playback_elapsed(elapsed);
    if (playback_record.value) {
      playback_record.value = { ...playback_record.value, elapsed };
    }
  }

  function build_shuffle_queue(tracks: Track[], anchor_track_id?: string | null) {
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

  function shuffle_current_queue(anchor_track_id?: string | null) {
    shuffle_queue.value = build_shuffle_queue(current_queue.value, anchor_track_id);
    return shuffle_queue.value;
  }

  function reshuffle_queue(excluded_first_track_id?: string | null) {
    const source_queue = current_queue.value.length ? current_queue.value : shuffle_queue.value;
    const next_queue = shuffle_tracks(source_queue);
    if (excluded_first_track_id && next_queue.length > 1 && next_queue[0]?.id === excluded_first_track_id) {
      const swap_index = next_queue.findIndex((track, index) => index > 0 && track.id !== excluded_first_track_id);
      if (swap_index > 0) {
        [next_queue[0], next_queue[swap_index]] = [next_queue[swap_index], next_queue[0]];
      }
    }

    shuffle_queue.value = next_queue;
    return shuffle_queue.value;
  }

  return {
    library_tracks,
    current_queue,
    shuffle_queue,
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
    save_playback_record_metadata,
    save_playback_elapsed,
    shuffle_current_queue,
    reshuffle_queue,
  };
});
