<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import repeat_one_icon from "./assets/icons/repeat-one.svg";
import repeat_icon from "./assets/icons/repeat.svg";
import shuffle_icon from "./assets/icons/shuffle.svg";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ContentArea from "./components/ContentArea.vue";
import LibrarySidebar from "./components/LibrarySidebar.vue";
import PlayerBar from "./components/PlayerBar.vue";
import PlaybackQueuePanel from "./components/PlaybackQueuePanel.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import TopBar from "./components/TopBar.vue";
import { use_player_queue_store } from "./stores/player_queue";
import type {
  AppConfig,
  AppStartup,
  AlbumItem,
  ArtistItem,
  PlaybackMode,
  PlaybackModeItem,
  PlaybackStatus,
  PlaylistBundle,
  PlaylistCache,
  QueueSource,
  Track,
  ViewKey,
} from "./types/music";
import { display_album, display_artist } from "./utils/track";

type PlayerBarExpose = {
  render_progress: (percent: number, seconds: number) => void;
};

type PlayerCache = {
  version: 1;
  queue_source: QueueSource;
  track_ids: string[];
  current_track_id: string | null;
  current_track_path: string | null;
  elapsed: number;
  playback_mode: PlaybackMode;
  updated_at: number;
};

type TrackContextMenu = {
  track: Track;
  x: number;
  y: number;
  remove_playlist_id: string | null;
};

type PlaylistContextMenu = {
  playlist: PlaylistCache;
  x: number;
  y: number;
};

const player_queue = use_player_queue_store();
const { library_tracks: tracks, current_queue, playback_mode, queue_source } = storeToRefs(player_queue);
const status = ref<PlaybackStatus>({
  path: null,
  playing: false,
  volume: 1,
  elapsed: 0,
});
const selected_directories = ref<string[]>([]);
const loading = ref(false);
const error_message = ref("");
const query = ref("");
const app_config = ref<AppConfig | null>(null);
const playlists = ref<PlaylistBundle>(empty_playlist_bundle());
const active_view = ref<ViewKey>("all");
const selected_artist = ref("");
const selected_album = ref("");
const selected_playlist_id = ref("my_playlist");
const settings_open = ref(false);
const playback_queue_open = ref(false);
const track_context_menu = ref<TrackContextMenu | null>(null);
const playlist_context_menu = ref<PlaylistContextMenu | null>(null);
const pending_delete_playlist = ref<PlaylistCache | null>(null);
const progress_dragging = ref(false);
const player_bar = ref<PlayerBarExpose | null>(null);
const sidebar_width = ref(250);
const sidebar_resizing = ref(false);

let status_timer: number | undefined;
let progress_frame: number | undefined;
let progress_sync_started_at = performance.now();
let progress_preview_seconds = 0;
let progress_preview_percent = 0;
let progress_drag_rect: DOMRect | null = null;
let pending_seek_seconds: number | null = null;
let pending_seek_path: string | null = null;
let pending_seek_started_at = performance.now();
let visual_elapsed = 0;
let handled_completion_path = "";
let sidebar_resize_start_x = 0;
let sidebar_resize_start_width = 250;
let media_shortcut_unlisteners: UnlistenFn[] = [];
let media_shortcut_listeners_disposed = false;
let restored_playback_pending = false;
let restoring_player_cache = false;

const sidebar_min_width = 72;
const sidebar_max_width = 420;
const sidebar_compact_width = 100;
const sidebar_width_storage_key = "music_box_sidebar_width";
const player_cache_storage_key = "music_box_player_cache";
const app_window = getCurrentWindow();
const playback_modes: PlaybackModeItem[] = [
  { mode: "shuffle", icon: shuffle_icon, label: "随机播放" },
  { mode: "repeat", icon: repeat_icon, label: "循环播放" },
  { mode: "repeat_one", icon: repeat_one_icon, label: "单曲循环" },
];

const current_track = computed(() =>
  tracks.value.find((track) => track.path === status.value.path),
);

const tracks_by_id = computed(() => new Map(tracks.value.map((track) => [track.id, track])));

const display_tracks = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return queue_tracks_for_view(active_view.value);

  return tracks.value.filter((track) =>
    `${track.title} ${track.artist} ${track.album}`.toLowerCase().includes(keyword),
  );
});

const album_count = computed(() => {
  const albums = new Set(
    tracks.value
      .map((track) => display_album(track))
      .filter((album) => album !== "未知专辑"),
  );
  return albums.size;
});

const artist_count = computed(() => {
  const artists = new Set(
    tracks.value
      .map((track) => display_artist(track))
      .filter((artist) => artist !== "未知歌手"),
  );
  return artists.size;
});

const artist_items = computed<ArtistItem[]>(() => {
  const artists = new Map<string, ArtistItem>();

  for (const track of tracks.value) {
    const name = display_artist(track);
    if (name === "未知歌手") continue;

    const item =
      artists.get(name) ??
      ({
        name,
        track_count: 0,
        total_duration: 0,
        cover_track: undefined,
      } satisfies ArtistItem);

    item.track_count += 1;
    item.total_duration += track.duration ?? 0;
    if (!item.cover_track || (!item.cover_track.cover_cache_path && track.cover_cache_path)) {
      item.cover_track = track;
    }

    artists.set(name, item);
  }

  return Array.from(artists.values()).sort((left, right) =>
    left.name.localeCompare(right.name, "zh-Hans-CN"),
  );
});

const total_duration = computed(() =>
  tracks.value.reduce((total, track) => total + (track.duration ?? 0), 0),
);

const sidebar_compact = computed(() => sidebar_width.value < sidebar_compact_width);

const app_shell_style = computed(() => ({
  "--sidebar_width": `${sidebar_width.value}px`,
}));

const playback_mode_button = computed(() => {
  return playback_modes.find((item) => item.mode === playback_mode.value) ?? playback_modes[0];
});

const user_playlist_items = computed<PlaylistCache[]>(() => {
  return playlists.value.my_playlists ?? [];
});

const selected_user_playlist = computed(() => {
  return (
    user_playlist_items.value.find((playlist) => playlist.id === selected_playlist_id.value) ??
    user_playlist_items.value[0] ??
    playlists.value.my_playlist
  );
});

async function choose_music_directory() {
  error_message.value = "";
  const selected = await open({
    directory: true,
    multiple: true,
    title: "选择本地音乐文件夹",
  });

  const directories = Array.isArray(selected)
    ? selected
    : typeof selected === "string"
      ? [selected]
      : [];
  if (!directories.length) return;

  selected_directories.value = directories;
  await scan_directory(directories);
}

async function scan_directory(directories: string[]) {
  loading.value = true;
  error_message.value = "";

  try {
    const scanned_tracks = await invoke<Track[]>("scan_music_dir", { dirs: directories });
    player_queue.set_library_tracks(scanned_tracks);
    await load_startup_state();
  } catch (error) {
    error_message.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function reload_library() {
  const directories = app_config.value?.music_directory ?? selected_directories.value;
  if (!directories.length) {
    await choose_music_directory();
    return;
  }
  await scan_directory(directories);
}

async function load_startup_state() {
  loading.value = true;
  error_message.value = "";

  try {
    const startup = await invoke<AppStartup>("get_startup_state");
    app_config.value = startup.config;
    playlists.value = startup.playlists;
    ensure_selected_playlist();
    restoring_player_cache = true;
    player_queue.set_library_tracks(startup.tracks);
    selected_directories.value = startup.config.music_directory;
    const restored_player_cache = restore_player_cache();
    restoring_player_cache = false;
    if (!restored_player_cache) {
      set_queue_for_current_view();
    }
  } catch (error) {
    restoring_player_cache = false;
    error_message.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function play(track: Track) {
  try {
    restored_playback_pending = false;
    handled_completion_path = "";
    clear_pending_seek();
    apply_playback_status(await invoke<PlaybackStatus>("play_track", { path: track.path }));
    add_recent_track(track);
    start_status_polling();
  } catch (error) {
    error_message.value = String(error);
  }
}

async function toggle_playback() {
  if (!status.value.path) {
    set_queue_for_current_view();
  }
  const first_track = current_queue.value[0] ?? display_tracks.value[0];
  if (!status.value.path && first_track) {
    await play(first_track);
    return;
  }

  if (restored_playback_pending && !status.value.playing) {
    const track = current_track.value ?? first_track;
    if (track) {
      await play_from_cached_position(track, status.value.elapsed);
      return;
    }
  }

  apply_playback_status(
    await invoke<PlaybackStatus>(status.value.playing ? "pause_track" : "resume_track"),
  );
  start_status_polling();
}

async function previous_track() {
  const queue = playback_queue();
  if (!queue.length) return;
  const index = queue_index(queue);
  const previous_index = index <= 0 ? queue.length - 1 : index - 1;
  await play(queue[previous_index]);
}

async function next_track() {
  await play_next_track(false);
}

async function stop_playback() {
  try {
    restored_playback_pending = false;
    clear_pending_seek();
    apply_playback_status(await invoke<PlaybackStatus>("stop_track"));
  } catch (error) {
    error_message.value = String(error);
  }
}

function minimize_window() {
  void app_window.minimize();
}

function toggle_maximize_window() {
  void app_window.toggleMaximize();
}

function close_window() {
  void app_window.close();
}

function start_window_drag(event: MouseEvent) {
  if (event.button !== 0) return;

  const target = event.target as HTMLElement | null;
  if (target?.closest("button, input, label, a, [role='button'], .sidebar_resize_handle")) {
    return;
  }

  void app_window.startDragging();
}

async function change_volume(event: Event) {
  const target = event.target as HTMLInputElement;
  apply_playback_status(
    await invoke<PlaybackStatus>("set_volume", {
      volume: Number(target.value),
    }),
  );
}

function update_progress_preview(event: PointerEvent) {
  const duration = current_track.value?.duration;
  if (!duration) return;

  const target = event.currentTarget as HTMLElement;
  const rect = progress_drag_rect ?? target.getBoundingClientRect();
  const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  progress_preview_percent = ratio * 100;
  progress_preview_seconds = Math.round(duration * ratio);
  render_progress(progress_preview_percent, progress_preview_seconds);
}

async function commit_progress_seek(seconds = progress_preview_seconds) {
  const duration = current_track.value?.duration;
  if (!duration || !status.value.path) return;

  const target_seconds = Math.min(Math.max(seconds, 0), duration);
  if (restored_playback_pending && !status.value.playing) {
    status.value = {
      ...status.value,
      elapsed: target_seconds,
    };
    sync_visual_elapsed(status.value);
    save_player_cache();
    return;
  }

  try {
    handled_completion_path = "";
    hold_progress_at_seek_target(target_seconds);
    apply_playback_status(await invoke<PlaybackStatus>("seek_track", { seconds: target_seconds }));
  } catch (error) {
    clear_pending_seek();
    error_message.value = String(error);
  }
}

function begin_progress_drag(event: PointerEvent) {
  if (!current_track.value?.duration) return;
  progress_dragging.value = true;
  progress_drag_rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  update_progress_preview(event);
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function drag_progress(event: PointerEvent) {
  if (!progress_dragging.value) return;
  update_progress_preview(event);
}

async function end_progress_drag(event: PointerEvent) {
  if (!progress_dragging.value) return;
  update_progress_preview(event);
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  progress_drag_rect = null;
  progress_dragging.value = false;
  await commit_progress_seek(progress_preview_seconds);
}

function cancel_progress_drag(event: PointerEvent) {
  if (!progress_dragging.value) return;
  progress_dragging.value = false;
  progress_drag_rect = null;
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
}

function start_status_polling() {
  if (status_timer) window.clearInterval(status_timer);
  status_timer = window.setInterval(async () => {
    const next_status = await invoke<PlaybackStatus>("get_playback_status");
    apply_playback_status(next_status);
    await handle_playback_completion(next_status);
  }, 1000);
}

function apply_playback_status(next_status: PlaybackStatus) {
  status.value = next_status;
  player_queue.set_current_track_path(next_status.path);
  if (hold_pending_seek_progress(next_status)) {
    save_player_cache();
    if (next_status.playing) request_progress_frame();
    return;
  }
  sync_visual_elapsed(next_status);
  save_player_cache();
  if (next_status.playing) request_progress_frame();
}

function sync_visual_elapsed(next_status: PlaybackStatus) {
  visual_elapsed = next_status.elapsed;
  progress_sync_started_at = performance.now();
  if (!progress_dragging.value) {
    render_progress(progress_percent_for(next_status.elapsed), next_status.elapsed);
  }
}

function hold_progress_at_seek_target(seconds: number) {
  pending_seek_seconds = seconds;
  pending_seek_path = status.value.path ?? null;
  pending_seek_started_at = performance.now();
  visual_elapsed = seconds;
  render_progress(progress_percent_for(seconds), seconds);
}

function hold_pending_seek_progress(next_status: PlaybackStatus) {
  if (pending_seek_seconds === null) return false;

  if (next_status.path !== pending_seek_path) {
    clear_pending_seek();
    return false;
  }

  const matched = Math.abs(next_status.elapsed - pending_seek_seconds) <= 1;
  if (matched) {
    clear_pending_seek();
    return false;
  }

  visual_elapsed = playback_elapsed_from_pending_seek();
  progress_sync_started_at = pending_seek_started_at;
  render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
  return true;
}

function playback_elapsed_from_pending_seek() {
  const base = pending_seek_seconds ?? status.value.elapsed;
  if (!status.value.playing) return base;
  return base + (performance.now() - pending_seek_started_at) / 1000;
}

function clear_pending_seek() {
  pending_seek_seconds = null;
  pending_seek_path = null;
}

function request_progress_frame() {
  if (progress_frame) return;
  progress_frame = window.requestAnimationFrame(update_progress_frame);
}

function update_progress_frame(now: number) {
  progress_frame = undefined;

  const duration = current_track.value?.duration ?? 0;
  if (!duration || !status.value.path) {
    visual_elapsed = 0;
    render_progress(0, 0);
    return;
  }

  if (status.value.playing) {
    const base_elapsed = pending_seek_seconds ?? status.value.elapsed;
    const started_at = pending_seek_seconds === null ? progress_sync_started_at : pending_seek_started_at;
    const elapsed = base_elapsed + (now - started_at) / 1000;
    visual_elapsed = Math.min(elapsed, duration);
    if (!progress_dragging.value) {
      render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
    }
    if (visual_elapsed >= duration) {
      void handle_playback_completion({
        ...status.value,
        playing: false,
        elapsed: Math.floor(duration),
      });
      return;
    }
    request_progress_frame();
    return;
  }

  visual_elapsed = Math.min(status.value.elapsed, duration);
  if (!progress_dragging.value) {
    render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
  }
}

function progress_percent_for(seconds: number) {
  const duration = current_track.value?.duration ?? 0;
  if (!duration) return 0;
  return Math.min(Math.max((seconds / duration) * 100, 0), 100);
}

function render_progress(percent: number, seconds: number) {
  player_bar.value?.render_progress(percent, seconds);
}

function cache_elapsed_seconds() {
  const duration = current_track.value?.duration ?? Number.POSITIVE_INFINITY;
  const seconds =
    pending_seek_seconds !== null
      ? playback_elapsed_from_pending_seek()
      : status.value.playing
        ? visual_elapsed
        : status.value.elapsed;

  return Math.max(0, Math.floor(Math.min(seconds, duration)));
}

function save_player_cache() {
  if (restoring_player_cache) return;

  const cache: PlayerCache = {
    version: 1,
    queue_source: queue_source.value,
    track_ids: current_queue.value.map((track) => track.id),
    current_track_id: current_track.value?.id ?? null,
    current_track_path: status.value.path ?? null,
    elapsed: cache_elapsed_seconds(),
    playback_mode: playback_mode.value,
    updated_at: Date.now(),
  };

  localStorage.setItem(player_cache_storage_key, JSON.stringify(cache));
}

function read_player_cache() {
  try {
    const raw_cache = localStorage.getItem(player_cache_storage_key);
    if (!raw_cache) return null;

    const cache = JSON.parse(raw_cache) as Partial<PlayerCache>;
    if (cache.version !== 1 || !Array.isArray(cache.track_ids) || !cache.queue_source) {
      return null;
    }

    return cache as PlayerCache;
  } catch (error) {
    console.warn("无法读取播放缓存", error);
    return null;
  }
}

function restore_player_cache() {
  const cache = read_player_cache();
  if (!cache) return false;

  const was_restoring_player_cache = restoring_player_cache;
  restoring_player_cache = true;
  try {
    player_queue.set_playback_mode(cache.playback_mode);

    const track_by_id = new Map(tracks.value.map((track) => [track.id, track]));
    const restored_queue = cache.track_ids
      .map((track_id) => track_by_id.get(track_id))
      .filter((track): track is Track => Boolean(track));

    if (!restored_queue.length) return false;

    player_queue.set_current_queue(cache.queue_source, restored_queue);

    const restored_track =
      (cache.current_track_id ? track_by_id.get(cache.current_track_id) : undefined) ??
      tracks.value.find((track) => track.path === cache.current_track_path) ??
      restored_queue[0];

    if (!restored_track) return true;

    const elapsed = Math.min(Math.max(cache.elapsed ?? 0, 0), restored_track.duration ?? cache.elapsed ?? 0);
    restored_playback_pending = true;
    status.value = {
      path: restored_track.path,
      playing: false,
      volume: status.value.volume,
      elapsed,
    };
    player_queue.set_current_track_path(restored_track.path);
    sync_visual_elapsed(status.value);
    return true;
  } finally {
    restoring_player_cache = was_restoring_player_cache;
  }
}

async function play_from_cached_position(track: Track, seconds: number) {
  const target_seconds = Math.min(Math.max(Math.floor(seconds), 0), track.duration ?? seconds);
  await play(track);

  if (target_seconds > 0 && status.value.path === track.path) {
    hold_progress_at_seek_target(target_seconds);
    apply_playback_status(await invoke<PlaybackStatus>("seek_track", { seconds: target_seconds }));
  }
}

function playback_queue() {
  return current_queue.value.length ? current_queue.value : display_tracks.value;
}

function queue_index(queue: Track[]) {
  if (!status.value.path) return -1;
  return queue.findIndex((track) => track.path === status.value.path);
}

function cycle_playback_mode() {
  const index = playback_modes.findIndex((item) => item.mode === playback_mode.value);
  player_queue.set_playback_mode(playback_modes[(index + 1) % playback_modes.length].mode);
}

function random_queue_index(queue: Track[], current_index: number) {
  if (queue.length <= 1) return 0;
  let next_index = current_index;
  while (next_index === current_index) {
    next_index = Math.floor(Math.random() * queue.length);
  }
  return next_index;
}

async function play_next_track(from_completion: boolean) {
  const queue = playback_queue();
  if (!queue.length) return false;

  const current_index = queue_index(queue);
  if (playback_mode.value === "shuffle") {
    await play(queue[random_queue_index(queue, current_index)]);
    return true;
  }

  if (playback_mode.value === "repeat_one" && from_completion && current_track.value) {
    await play(current_track.value);
    return true;
  }

  const next_index = current_index < 0 ? 0 : current_index + 1;
  if (next_index < queue.length) {
    await play(queue[next_index]);
    return true;
  }

  if (playback_mode.value === "repeat") {
    await play(queue[0]);
    return true;
  }

  return false;
}

async function handle_playback_completion(next_status: PlaybackStatus) {
  const track = current_track.value;
  if (!track?.duration || !next_status.path) return;
  if (next_status.path !== track.path) return;
  if (next_status.elapsed < Math.max(track.duration - 1, 0)) return;
  if (handled_completion_path === next_status.path) return;

  handled_completion_path = next_status.path;
  const played_next = await play_next_track(true);
  if (!played_next) {
    await stop_playback();
  }
}

function show_view(view: ViewKey) {
  active_view.value = view;
  if (view !== "all") query.value = "";
  selected_artist.value = "";
  selected_album.value = "";
}

function show_playlist(playlist_id: string) {
  selected_playlist_id.value = playlist_id;
  show_view("playlist_1");
}

function queue_source_for_view(view: ViewKey): QueueSource {
  if (view === "artists" && selected_artist.value) {
    return { type: "artist", id: selected_artist.value, label: selected_artist.value };
  }
  if (view === "albums" && selected_album.value) {
    return { type: "album", id: selected_album.value, label: selected_album.value };
  }
  if (view === "playlist_1") {
    const playlist = selected_user_playlist.value;
    return {
      type: "playlist",
      id: playlist.id,
      label: playlist.name,
    };
  }

  const labels: Record<ViewKey, string> = {
    all: "全部",
    artists: "歌手",
    albums: "专辑",
    stats: "统计",
    recent: "最近播放",
    playlist_1: "我的歌单",
  };
  return { type: view, id: view, label: labels[view] };
}

function queue_tracks_for_view(view: ViewKey) {
  if (view === "artists" && selected_artist.value) {
    return tracks.value.filter((track) => display_artist(track) === selected_artist.value);
  }
  if (view === "albums" && selected_album.value) {
    return tracks.value.filter((track) => display_album(track) === selected_album.value);
  }
  if (view === "recent") return tracks_from_ids(playlists.value.recent.track_ids);
  if (view === "playlist_1") return tracks_from_ids(selected_user_playlist.value.track_ids);
  return tracks.value;
}

function playlist_track_ids_for_source(source: QueueSource) {
  if (source.type === "recent") return playlists.value.recent.track_ids;
  if (source.type !== "playlist") return [];

  return user_playlist_items.value.find((playlist) => playlist.id === source.id)?.track_ids ?? [];
}

function queue_tracks_for_source(source: QueueSource) {
  if (source.type === "artist") {
    return tracks.value.filter((track) => display_artist(track) === source.id);
  }
  if (source.type === "album") {
    return tracks.value.filter((track) => display_album(track) === source.id);
  }
  if (source.type === "recent" || source.type === "playlist") {
    return tracks_from_ids(playlist_track_ids_for_source(source));
  }
  if (source.type === "search") {
    const keyword = source.id.trim().toLowerCase();
    if (!keyword) return [];

    return tracks.value.filter((track) =>
      `${track.title} ${track.artist} ${track.album}`.toLowerCase().includes(keyword),
    );
  }

  return tracks.value;
}

function queue_source_with_latest_label(source: QueueSource) {
  if (source.type === "playlist") {
    const playlist = user_playlist_items.value.find((item) => item.id === source.id);
    if (playlist) return { ...source, label: playlist.name };
  }
  return { ...source };
}

function refresh_current_queue_source() {
  player_queue.set_current_queue(
    queue_source_with_latest_label(queue_source.value),
    queue_tracks_for_source(queue_source.value),
  );
}

function tracks_from_ids(track_ids: string[]) {
  return track_ids
    .map((track_id) => tracks_by_id.value.get(track_id))
    .filter((track): track is Track => Boolean(track));
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

function empty_playlist_bundle(): PlaylistBundle {
  const empty_playlist = (id: string, name: string, kind: string) => ({
    id,
    name,
    kind,
    generated_at: 0,
    metadata: {
      track_count: 0,
      total_duration: 0,
      item_count: 0,
      cover_cache_path: null,
    },
    track_ids: [],
    children: [],
  });

  return {
    recent: empty_playlist("recent", "最近播放", "recent"),
    my_playlist: empty_playlist("my_playlist", "我的歌单", "user"),
    my_playlists: [empty_playlist("my_playlist", "我的歌单", "user")],
    artists: empty_playlist("artists", "歌手", "artists"),
    albums: empty_playlist("albums", "专辑", "albums"),
  };
}

function ensure_selected_playlist() {
  if (user_playlist_items.value.some((playlist) => playlist.id === selected_playlist_id.value)) {
    return;
  }

  selected_playlist_id.value = user_playlist_items.value[0]?.id ?? playlists.value.my_playlist.id;
}

function set_queue_for_current_view() {
  if (query.value.trim()) {
    player_queue.set_current_queue(
      {
        type: "search",
        id: query.value.trim(),
        label: "搜索结果",
      },
      display_tracks.value,
    );
    return;
  }

  player_queue.set_current_queue(queue_source_for_view(active_view.value), queue_tracks_for_view(active_view.value));
}

function open_artist_playlist(name: string) {
  active_view.value = "artists";
  selected_artist.value = name;
  selected_album.value = "";
  query.value = "";
}

function open_album_playlist(name: string) {
  active_view.value = "albums";
  selected_album.value = name;
  selected_artist.value = "";
  query.value = "";
}

function close_detail_playlist() {
  selected_artist.value = "";
  selected_album.value = "";
}

function update_query(value: string) {
  query.value = value;
  active_view.value = "all";
  selected_artist.value = "";
  selected_album.value = "";
}

function focus_search() {
  active_view.value = "all";
}

async function play_track_from_view(track: Track) {
  set_queue_for_current_view();
  await play(track);
}

function active_record_playlist_id() {
  if (query.value.trim() || selected_artist.value || selected_album.value) return "";
  if (active_view.value === "recent") return "recent";
  if (active_view.value === "playlist_1") return selected_user_playlist.value.id;
  return "";
}

function open_track_context_menu(track: Track, event: MouseEvent) {
  const menu_width = 220;
  const menu_min_height = 64;
  track_context_menu.value = {
    track,
    x: Math.min(event.clientX, Math.max(window.innerWidth - menu_width - 12, 12)),
    y: Math.min(event.clientY, Math.max(window.innerHeight - menu_min_height - 12, 12)),
    remove_playlist_id: active_record_playlist_id(),
  };
}

function close_track_context_menu() {
  track_context_menu.value = null;
}

function open_playlist_context_menu(playlist: PlaylistCache, event: MouseEvent) {
  const menu_width = 170;
  const menu_min_height = 96;
  playlist_context_menu.value = {
    playlist,
    x: Math.min(event.clientX, Math.max(window.innerWidth - menu_width - 12, 12)),
    y: Math.min(event.clientY, Math.max(window.innerHeight - menu_min_height - 12, 12)),
  };
}

function close_playlist_context_menu() {
  playlist_context_menu.value = null;
}

function apply_playlist_bundle(next_playlists: PlaylistBundle) {
  playlists.value = next_playlists;
  ensure_selected_playlist();
}

async function create_playlist(name: string) {
  const trimmed_name = name.trim();
  if (!trimmed_name) return;

  const existing_ids = new Set(user_playlist_items.value.map((playlist) => playlist.id));
  try {
    const next_playlists = await invoke<PlaylistBundle>("create_user_playlist", {
      name: trimmed_name,
    });
    apply_playlist_bundle(next_playlists);

    const created_playlist =
      next_playlists.my_playlists.find((playlist) => !existing_ids.has(playlist.id)) ??
      next_playlists.my_playlists.find((playlist) => playlist.name === trimmed_name);
    if (created_playlist) show_playlist(created_playlist.id);
  } catch (error) {
    error_message.value = String(error);
  }
}

async function rename_context_playlist() {
  const playlist = playlist_context_menu.value?.playlist;
  if (!playlist) return;

  close_playlist_context_menu();
  const name = window.prompt("请输入新的歌单名称", playlist.name);
  if (name === null) return;

  const trimmed_name = name.trim();
  if (!trimmed_name || trimmed_name === playlist.name) return;

  try {
    apply_playlist_bundle(
      await invoke<PlaylistBundle>("rename_user_playlist", {
        playlistId: playlist.id,
        name: trimmed_name,
      }),
    );
    if (queue_source.value.type === "playlist" && queue_source.value.id === playlist.id) {
      refresh_current_queue_source();
    }
  } catch (error) {
    error_message.value = String(error);
  }
}

function context_playlist_can_be_deleted() {
  return Boolean(playlist_context_menu.value?.playlist);
}

async function delete_context_playlist() {
  const playlist = playlist_context_menu.value?.playlist;
  if (!playlist || !context_playlist_can_be_deleted()) return;

  close_playlist_context_menu();
  pending_delete_playlist.value = playlist;
}

function cancel_delete_playlist() {
  pending_delete_playlist.value = null;
}

async function confirm_delete_playlist() {
  const playlist = pending_delete_playlist.value;
  if (!playlist) return;

  pending_delete_playlist.value = null;

  try {
    const deleted_playlist_id = playlist.id;
    const deleted_selected_playlist = selected_playlist_id.value === deleted_playlist_id;
    apply_playlist_bundle(
      await invoke<PlaylistBundle>("delete_user_playlist", {
        playlistId: deleted_playlist_id,
      }),
    );

    if (deleted_selected_playlist) {
      ensure_selected_playlist();
      if (active_view.value === "playlist_1") {
        if (user_playlist_items.value.length) {
          show_playlist(selected_user_playlist.value.id);
        } else {
          show_view("all");
        }
      }
    }
    if (queue_source.value.type === "playlist" && queue_source.value.id === deleted_playlist_id) {
      set_queue_for_current_view();
    }
  } catch (error) {
    error_message.value = String(error);
  }
}

function context_track_in_playlist(playlist: PlaylistCache) {
  const track_id = track_context_menu.value?.track.id;
  return Boolean(track_id && playlist.track_ids.includes(track_id));
}

async function add_context_track_to_playlist(playlist: PlaylistCache) {
  const track = track_context_menu.value?.track;
  if (!track || context_track_in_playlist(playlist)) return;

  try {
    playlists.value = await invoke<PlaylistBundle>("add_track_to_playlist", {
      playlistId: playlist.id,
      trackId: track.id,
    });
    close_track_context_menu();
    if (queue_source.value.type === "playlist" && queue_source.value.id === playlist.id) {
      refresh_current_queue_source();
    }
  } catch (error) {
    error_message.value = String(error);
  }
}

function context_track_can_be_removed() {
  return Boolean(track_context_menu.value?.remove_playlist_id);
}

async function remove_context_track_record() {
  const track = track_context_menu.value?.track;
  const playlist_id = track_context_menu.value?.remove_playlist_id ?? "";
  if (!track) return;
  if (!playlist_id) return;

  try {
    playlists.value = await invoke<PlaylistBundle>("remove_track_from_playlist", {
      playlistId: playlist_id,
      trackId: track.id,
    });
    close_track_context_menu();
    if (
      (queue_source.value.type === "recent" && playlist_id === "recent") ||
      (queue_source.value.type === "playlist" && queue_source.value.id === playlist_id)
    ) {
      refresh_current_queue_source();
    }
  } catch (error) {
    error_message.value = String(error);
  }
}

async function play_track_from_queue(track: Track) {
  await play(track);
}

function open_queue_source() {
  const source = queue_source.value;
  playback_queue_open.value = false;

  if (source.type === "artist") {
    open_artist_playlist(source.id);
    return;
  }
  if (source.type === "album") {
    open_album_playlist(source.id);
    return;
  }
  if (source.type === "search") {
    update_query(source.id);
    return;
  }
  if (source.type === "recent" || source.type === "playlist_1" || source.type === "all") {
    show_view(source.type);
    return;
  }
  if (source.type === "playlist") {
    show_playlist(source.id);
  }
}

function clamp_sidebar_width(width: number) {
  return Math.min(Math.max(width, sidebar_min_width), sidebar_max_width);
}

function load_sidebar_width() {
  const saved_width = Number(localStorage.getItem(sidebar_width_storage_key));
  if (Number.isFinite(saved_width)) {
    sidebar_width.value = clamp_sidebar_width(saved_width);
  }
}

function save_sidebar_width() {
  localStorage.setItem(sidebar_width_storage_key, String(Math.round(sidebar_width.value)));
}

function resize_sidebar(event: PointerEvent) {
  const offset = event.clientX - sidebar_resize_start_x;
  sidebar_width.value = clamp_sidebar_width(sidebar_resize_start_width + offset);
}

function end_sidebar_resize() {
  sidebar_resizing.value = false;
  document.body.classList.remove("resizing_sidebar");
  window.removeEventListener("pointermove", resize_sidebar);
  window.removeEventListener("pointerup", end_sidebar_resize);
  window.removeEventListener("pointercancel", end_sidebar_resize);
  save_sidebar_width();
}

function begin_sidebar_resize(event: PointerEvent) {
  event.preventDefault();
  sidebar_resizing.value = true;
  sidebar_resize_start_x = event.clientX;
  sidebar_resize_start_width = sidebar_width.value;
  document.body.classList.add("resizing_sidebar");
  window.addEventListener("pointermove", resize_sidebar);
  window.addEventListener("pointerup", end_sidebar_resize);
  window.addEventListener("pointercancel", end_sidebar_resize);
}

async function listen_media_shortcuts() {
  try {
    const unlisteners = await Promise.all([
      listen("media-play-pause", () => {
        void toggle_playback();
      }),
      listen("media-previous", () => {
        void previous_track();
      }),
      listen("media-next", () => {
        void next_track();
      }),
    ]);

    if (media_shortcut_listeners_disposed) {
      unlisteners.forEach((unlisten) => unlisten());
      return;
    }

    media_shortcut_unlisteners = unlisteners;
  } catch (error) {
    console.warn("无法监听系统媒体热键", error);
  }
}

function handle_before_unload() {
  save_player_cache();
}

function close_track_context_menu_on_pointer(event: PointerEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest(".track_context_menu")) return;
  if (target?.closest(".playlist_context_menu")) return;
  close_track_context_menu();
  close_playlist_context_menu();
}

function close_track_context_menu_on_key(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close_track_context_menu();
    close_playlist_context_menu();
  }
}

onBeforeUnmount(() => {
  save_player_cache();
  if (status_timer) window.clearInterval(status_timer);
  if (progress_frame) window.cancelAnimationFrame(progress_frame);
  media_shortcut_listeners_disposed = true;
  media_shortcut_unlisteners.forEach((unlisten) => unlisten());
  media_shortcut_unlisteners = [];
  window.removeEventListener("pointermove", resize_sidebar);
  window.removeEventListener("pointerup", end_sidebar_resize);
  window.removeEventListener("pointercancel", end_sidebar_resize);
  window.removeEventListener("beforeunload", handle_before_unload);
  window.removeEventListener("pointerdown", close_track_context_menu_on_pointer);
  window.removeEventListener("keydown", close_track_context_menu_on_key);
  document.body.classList.remove("resizing_sidebar");
});

onMounted(() => {
  media_shortcut_listeners_disposed = false;
  load_sidebar_width();
  window.addEventListener("beforeunload", handle_before_unload);
  window.addEventListener("pointerdown", close_track_context_menu_on_pointer);
  window.addEventListener("keydown", close_track_context_menu_on_key);
  void listen_media_shortcuts();
  void load_startup_state();
});

const album_items = computed<AlbumItem[]>(() => {
  const albums = new Map<string, AlbumItem>();

  for (const track of tracks.value) {
    const name = display_album(track);
    if (name === "未知专辑") continue;

    const item =
      albums.get(name) ??
      ({
        name,
        artist: display_artist(track),
        track_count: 0,
        total_duration: 0,
        cover_track: undefined,
      } satisfies AlbumItem);

    item.track_count += 1;
    item.total_duration += track.duration ?? 0;
    if (!item.cover_track || (!item.cover_track.cover_cache_path && track.cover_cache_path)) {
      item.cover_track = track;
    }

    albums.set(name, item);
  }

  return Array.from(albums.values()).sort((left, right) =>
    left.name.localeCompare(right.name, "zh-Hans-CN"),
  );
});

watch(display_tracks, () => {
  if (query.value.trim() && queue_source.value.type === "search") set_queue_for_current_view();
});

watch([current_queue, queue_source, playback_mode], () => {
  save_player_cache();
}, { deep: true });
</script>

<template>
  <main class="app_shell" :class="{ sidebar_compact, sidebar_resizing }" :style="app_shell_style">
    <LibrarySidebar
      :active_view="active_view"
      :has_query="Boolean(query.trim())"
      :track_count="tracks.length"
      :artist_count="artist_count"
      :album_count="album_count"
      :recent_count="playlists.recent.metadata.track_count"
      :playlist_items="user_playlist_items"
      :active_playlist_id="selected_user_playlist.id"
      @show_view="show_view"
      @show_playlist="show_playlist"
      @create_playlist="create_playlist"
      @open_playlist_menu="open_playlist_context_menu"
      @begin_resize="begin_sidebar_resize"
    />

    <section class="workspace">
      <TopBar
        :query="query"
        @update:query="update_query"
        @focus_search="focus_search"
        @reload_library="reload_library"
        @open_settings="settings_open = true"
        @minimize_window="minimize_window"
        @toggle_maximize_window="toggle_maximize_window"
        @close_window="close_window"
        @start_window_drag="start_window_drag"
      />

      <ContentArea
        :active_view="active_view"
        :query="query"
        :loading="loading"
        :error_message="error_message"
        :tracks="tracks"
        :display_tracks="display_tracks"
        :status_path="status.path"
        :is_playing="status.playing"
        :selected_artist="selected_artist"
        :selected_album="selected_album"
        :selected_playlist_id="selected_user_playlist.id"
        :playback_queue_source="queue_source"
        :artist_items="artist_items"
        :album_items="album_items"
        :album_count="album_count"
        :artist_count="artist_count"
        :total_duration="total_duration"
        @play_track="play_track_from_view"
        @open_track_menu="open_track_context_menu"
        @open_artist="open_artist_playlist"
        @open_album="open_album_playlist"
        @close_detail="close_detail_playlist"
      />
    </section>

    <div
      v-if="track_context_menu"
      class="track_context_menu"
      :style="{ left: `${track_context_menu.x}px`, top: `${track_context_menu.y}px` }"
      @click.stop
      @contextmenu.prevent
    >
      <div class="track_context_menu_header">
        <p>添加到歌单</p>
        <button
          v-if="context_track_can_be_removed()"
          class="track_context_delete_button"
          type="button"
          title="删除记录"
          @click="remove_context_track_record"
        >
          删除
        </button>
      </div>
      <button
        v-for="playlist in user_playlist_items"
        class="track_context_playlist_button"
        :key="playlist.id"
        type="button"
        :title="playlist.name"
        :disabled="context_track_in_playlist(playlist)"
        @click="add_context_track_to_playlist(playlist)"
      >
        {{ playlist.name }}
      </button>
    </div>

    <div
      v-if="playlist_context_menu"
      class="playlist_context_menu"
      :style="{ left: `${playlist_context_menu.x}px`, top: `${playlist_context_menu.y}px` }"
      @click.stop
      @contextmenu.prevent
    >
      <button class="context_menu_button" type="button" @click="rename_context_playlist">
        重命名
      </button>
      <button
        class="context_menu_button danger"
        type="button"
        :disabled="!context_playlist_can_be_deleted()"
        @click="delete_context_playlist"
      >
        删除
      </button>
    </div>

    <PlayerBar
      ref="player_bar"
      :current_track="current_track"
      :status="status"
      :progress_dragging="progress_dragging"
      :playback_mode_button="playback_mode_button"
      @begin_progress_drag="begin_progress_drag"
      @drag_progress="drag_progress"
      @end_progress_drag="end_progress_drag"
      @cancel_progress_drag="cancel_progress_drag"
      @previous_track="previous_track"
      @toggle_playback="toggle_playback"
      @next_track="next_track"
      @open_queue="playback_queue_open = true"
      @cycle_playback_mode="cycle_playback_mode"
      @change_volume="change_volume"
    />

    <PlaybackQueuePanel
      v-if="playback_queue_open"
      :queue_source="queue_source"
      :tracks="current_queue"
      :active_track_id="current_track?.id"
      :status_path="status.path"
      :is_playing="status.playing"
      @close="playback_queue_open = false"
      @open_source="open_queue_source"
      @play_track="play_track_from_queue"
    />

    <SettingsPanel
      v-if="settings_open"
      :app_config="app_config"
      @close="settings_open = false"
      @choose_music_directory="choose_music_directory"
    />

    <ConfirmDialog
      v-if="pending_delete_playlist"
      title="删除歌单"
      :message="`确定删除歌单“${pending_delete_playlist.name}”吗？`"
      detail="只会删除歌单记录，不会删除音乐文件。"
      confirm_label="删除"
      cancel_label="取消"
      @confirm="confirm_delete_playlist"
      @cancel="cancel_delete_playlist"
    />
  </main>
</template>

<style>
:root {
  color: #1e2026;
  background: #f6f7fa;
  font-family:
    Inter, "Segoe UI", "Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
  font-size: 16px;
  font-synthesis: none;
  line-height: 1.5;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
  user-select: none;
  -webkit-user-select: none;
}

body {
  margin: 0;
  min-width: 900px;
  min-height: 100vh;
  overflow: hidden;
}

button,
input {
  font: inherit;
}

input,
textarea,
[contenteditable="true"] {
  user-select: text;
  -webkit-user-select: text;
}

button {
  border: 0;
  cursor: pointer;
}

.app_shell {
  display: grid;
  grid-template-areas:
    "sidebar workspace"
    "player player";
  grid-template-columns: var(--sidebar_width, 250px) minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr) 86px;
  height: 100vh;
  color: #1e2026;
  background: #ffffff;
}

.sidebar {
  grid-area: sidebar;
  display: flex;
  flex-direction: column;
  position: relative;
  min-height: 0;
  border-right: 1px solid #ebedf2;
  background: #fbfcfe;
}

.sidebar_resize_handle {
  position: absolute;
  top: 0;
  right: -4px;
  z-index: 20;
  width: 8px;
  height: 100%;
  cursor: col-resize;
}

.sidebar_resize_handle:hover,
.sidebar_resizing .sidebar_resize_handle {
  background: rgba(66, 109, 255, 0.12);
}

.tool_button,
.window_button,
.player_tools button,
.control_row button {
  display: grid;
  width: 38px;
  height: 38px;
  place-items: center;
  border-radius: 8px;
  color: #16181d;
  background: transparent;
  font-size: 1.25rem;
  line-height: 1;
}

.svg_icon {
  display: inline-block;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  background: currentColor;
  -webkit-mask: var(--icon) center / contain no-repeat;
  mask: var(--icon) center / contain no-repeat;
}

.sidebar_nav {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 28px 28px;
  scrollbar-width: none;
}

.sidebar_nav::-webkit-scrollbar {
  display: none;
}

.nav_group {
  display: grid;
  gap: 12px;
}

.playlist_group {
  flex: 1;
  align-content: start;
  margin-top: 28px;
}

.nav_group h2,
.settings_panel h2,
.settings_section h3,
p {
  margin: 0;
}

.nav_group h2 {
  margin-bottom: 12px;
  color: #7d828c;
  font-size: 1rem;
  font-weight: 800;
}

.nav_item {
  display: flex;
  align-items: center;
  gap: 16px;
  width: 100%;
  min-height: 48px;
  border-radius: 8px;
  padding: 0 22px;
  color: #202329;
  background: transparent;
  font-size: 1.05rem;
  font-weight: 700;
  text-align: left;
}

.nav_item span:not(.nav_icon) {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav_item:hover,
.nav_item.active {
  color: #426dff;
  background: #eaf0ff;
}

.nav_icon {
  width: 24px;
  height: 24px;
}

.sidebar_compact .sidebar_nav {
  padding-inline: 12px;
}

.sidebar_compact .nav_group h2,
.sidebar_compact .nav_item span:not(.nav_icon) {
  display: none;
}

.sidebar_compact .nav_item {
  justify-content: center;
  gap: 0;
  padding-inline: 0;
}

.resizing_sidebar,
.resizing_sidebar * {
  cursor: col-resize !important;
  user-select: none;
}

.create_playlist {
  margin-top: auto;
}

.create_playlist_input_row {
  cursor: text;
}

.create_playlist_input {
  min-width: 0;
  width: 100%;
  border: 0;
  outline: 0;
  padding: 0;
  color: #202329;
  background: transparent;
  font-size: 1.05rem;
  font-weight: 700;
}

.create_playlist_input::placeholder {
  color: #a0a5af;
}

.workspace {
  grid-area: workspace;
  display: grid;
  grid-template-rows: 78px minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  background: #ffffff;
}

.top_bar {
  display: grid;
  justify-content: center;
  align-items: center;
  grid-template-columns: 1fr 220px;
  border-bottom: 1px solid #eef0f4;
  padding-right: 28px;
  cursor: move;
  user-select: none;
}

.search_box {
  display: flex;
  align-items: center;
  justify-content: center;
  justify-self: center;
  gap: 12px;
  flex: 1;
  width: 60%;
  min-width: 300px;
  height: 52px;
  border-radius: 8px;
  padding: 0 18px;
  color: #858b96;
  background: #f4f5f8;
  cursor: text;
  user-select: auto;
}

.search_box input {
  width: 100%;
  border: 0;
  outline: 0;
  color: #1e2026;
  background: transparent;
  font-size: 1rem;
  user-select: none;
  -webkit-user-select: none;
}

.search_box .svg_icon {
  width: 18px;
  height: 18px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-shrink: 0;
  cursor: default;
}

.tool_button:hover,
.window_button:hover,
.player_tools button:hover,
.control_row button:hover {
  background: #f0f2f6;
}

.window_button.close {
  font-weight: 900;
}

.tool_button .svg_icon,
.window_button .svg_icon,
.player_tools .svg_icon,
.control_row .svg_icon {
  width: 20px;
  height: 20px;
}

.content_area {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 22px 22px 0;
}

.status_line,
.muted {
  color: #8b919c;
  font-size: 0.92rem;
}

.primary_button {
  min-height: 38px;
  border-radius: 8px;
  padding: 0 16px;
  color: #ffffff;
  background: #426dff;
  font-size: 0.95rem;
  font-weight: 800;
}

.status_line {
  padding: 8px;
}

.error_line {
  padding: 4px;
  color: #c33131;
}

.track_table {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 12px 18px 0;
}

.table_head,
.table_row {
  display: grid;
  /* grid-template-columns: 68px minmax(200px, 1fr) minmax(150px, 0.8fr) 86px; */
  grid-template-columns: 68px 1.35fr 1fr 86px;
  align-items: center;
  gap: 18px;
  width: 100%;
}

.table_head {
  position: sticky;
  top: 0;
  z-index: 1;
  height: 36px;
  color: #a0a5af;
  background: #ffffff;
  font-size: 0.82rem;
  font-weight: 800;
}

.table_row {
  min-height: 74px;
  border-radius: 8px;
  padding: 8px 0;
  color: #1e2026;
  background: transparent;
  text-align: left;
}

.table_row:hover,
.table_row.active {
  background: #f5f7ff;
}

.track_context_menu,
.playlist_context_menu {
  position: fixed;
  z-index: 1000;
  display: grid;
  gap: 4px;
  border: 1px solid #eef0f4;
  border-radius: 8px;
  padding: 8px;
  background: #ffffff;
  box-shadow: 0 12px 34px rgba(19, 24, 34, 0.16);
}

.track_context_menu {
  min-width: 190px;
  max-width: 240px;
}

.playlist_context_menu {
  min-width: 150px;
}

.track_context_menu_header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  padding: 4px 8px 6px;
}

.track_context_menu_header p {
  min-width: 0;
  color: #8b919c;
  font-size: 0.78rem;
  font-weight: 800;
}

.track_context_delete_button {
  flex: 0 0 auto;
  min-height: 26px;
  border-radius: 8px;
  padding: 0 8px;
  color: #b65b5b;
  background: transparent;
  font-size: 0.78rem;
  font-weight: 800;
}

.track_context_delete_button:hover {
  color: #c33131;
  background: #fff0f0;
}

.track_context_playlist_button {
  overflow: hidden;
  min-height: 34px;
  border-radius: 8px;
  padding: 0 10px;
  color: #1e2026;
  background: transparent;
  font-size: 0.92rem;
  font-weight: 800;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_context_playlist_button:hover {
  color: #426dff;
  background: #eaf0ff;
}

.track_context_playlist_button:disabled {
  color: #b3b8c2;
  background: transparent;
  cursor: default;
}

.track_context_playlist_button:disabled:hover {
  color: #b3b8c2;
  background: transparent;
}

.context_menu_button {
  min-height: 34px;
  border-radius: 8px;
  padding: 0 10px;
  color: #1e2026;
  background: transparent;
  font-size: 0.92rem;
  font-weight: 800;
  text-align: left;
}

.context_menu_button:hover {
  color: #426dff;
  background: #eaf0ff;
}

.context_menu_button.danger {
  color: #b65b5b;
}

.context_menu_button.danger:hover {
  color: #c33131;
  background: #fff0f0;
}

.context_menu_button:disabled,
.context_menu_button:disabled:hover {
  color: #b3b8c2;
  background: transparent;
  cursor: default;
}

.index_cell,
.duration_cell {
  color: #8b919c;
  text-align: center;
}

.song_cell {
  display: flex;
  align-items: center;
  gap: 18px;
  min-width: 0;
}

.cover_thumb,
.player_cover,
.album_art {
  display: grid;
  place-items: center;
  overflow: hidden;
  flex: 0 0 auto;
  border-radius: 8px;
  color: #ffffff;
  background:
    linear-gradient(145deg, #21242b, #426dff),
    #21242b;
  font-weight: 900;
}

.cover_thumb {
  width: 52px;
  height: 52px;
  font-size: 1.5rem;
}

.cover_thumb img,
.player_cover img,
.album_art img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.song_text {
  display: grid;
  min-width: 0;
  gap: 3px;
}

.song_text strong,
.song_text small,
.album_cell,
.now_text strong,
.now_text small,
.path_list p,
.settings_section input {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.song_text strong {
  font-size: 1rem;
}

.song_text small,
.album_cell {
  color: #a0a5af;
  font-size: 0.95rem;
}

.empty_state {
  display: grid;
  min-height: 220px;
  place-items: center;
  color: #8b919c;
}

.placeholder_view {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 18px 8px;
}

.placeholder_grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 18px;
}

.album_tile {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.media_tile {
  border: 0;
  padding: 0;
  color: inherit;
  background: transparent;
  text-align: left;
}

.media_tile:hover strong {
  color: #426dff;
}

.detail_header {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px;
  min-height: 44px;
  padding: 0 0 12px;
}

.detail_header button {
  min-height: 32px;
  border-radius: 8px;
  padding: 0 12px;
  color: #426dff;
  background: #eaf0ff;
  font-size: 0.9rem;
  font-weight: 800;
}

.detail_title {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.detail_title strong,
.detail_meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail_meta {
  justify-self: end;
  color: #8b919c;
  font-size: 0.88rem;
  font-weight: 700;
  text-align: right;
}

.album_art,
.artist_art {
  width: 100%;
  aspect-ratio: 1;
  font-size: 3rem;
}

.artist_grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 18px;
}

.artist_tile {
  display: grid;
  gap: 7px;
  min-width: 0;
}

.artist_art {
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: 8px;
  color: #ffffff;
  background:
    linear-gradient(145deg, #21242b, #426dff),
    #21242b;
  font-weight: 900;
}

.artist_art img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.album_tile strong,
.album_tile small,
.artist_tile strong,
.artist_tile small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.album_tile small,
.artist_tile small {
  color: #8b919c;
}

.stats_view {
  display: grid;
  grid-template-columns: repeat(4, minmax(120px, 1fr));
  gap: 16px;
  padding: 18px 8px;
}

.stats_view article {
  display: grid;
  gap: 8px;
  border: 1px solid #eef0f4;
  border-radius: 8px;
  padding: 24px;
  background: #fbfcfe;
}

.stats_view strong {
  font-size: 1.8rem;
}

.stats_view span {
  color: #8b919c;
}

.spinning_cover {
  border-radius: 50%;
  animation: cover_spin 16s linear infinite;
  will-change: transform;
}

@keyframes cover_spin {
  from {
    transform: rotate(0deg);
  }

  to {
    transform: rotate(360deg);
  }
}

.player_bar {
  grid-area: player;
  display: grid;
  grid-template-columns: minmax(240px, 360px) minmax(340px, 1fr) minmax(280px, 420px);
  grid-template-rows: 2px minmax(0, 1fr);
  align-items: center;
  column-gap: 28px;
  row-gap: 0;
  min-width: 0;
  padding: 0 38px;
  background: #ffffff;
}

.player_bar button {
  appearance: none;
  -webkit-appearance: none;
  outline: 0;
  box-shadow: none;
  -webkit-tap-highlight-color: transparent;
}

.player_bar button:focus,
.player_bar button:focus-visible,
.player_bar button:active {
  outline: 0;
  box-shadow: none;
}

.player_progress {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  width: calc(100% + 76px);
  height: 14px;
  align-self: stretch;
  cursor: pointer;
  transform: translateX(-38px) translateY(-6px);
  touch-action: none;
  user-select: none;
  z-index: 99;
}

.progress_bar_container {
  display: flex;
  align-items: center;
  width: 100%;
  cursor: pointer;
  position: relative;
}

.progress_track {
  position: relative;
  width: 100%;
  height: 2px;
  overflow: visible;
  border-radius: 15px;
  background: rgba(128, 128, 128, 0.18);
}

.progress_fill {
  width: 100%;
  height: 100%;
  border-radius: 15px;
  background: #426dff;
  transform: scaleX(0);
  transform-origin: left center;
  will-change: transform;
}

.progress_handle {
  position: absolute;
  top: 50%;
  left: 0%;
  z-index: 2;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow: 0.5px 0.5px 2px 1px rgba(0, 0, 0, 0.12);
  cursor: pointer;
  transform: translate(-50%, -50%);
  visibility: hidden;
  will-change: left;
}

.progress_tooltip {
  position: absolute;
  bottom: 25px;
  left: 0%;
  z-index: 3;
  border-radius: 5px;
  padding: 2px 6px;
  color: #000000;
  background: #ffffff;
  box-shadow: 0.5px 0.5px 2px 1px rgba(0, 0, 0, 0.08);
  font-size: 0.75rem;
  font-weight: 700;
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
  transform: translateX(-50%);
  transition: opacity 120ms linear;
  will-change: left;
}

.progress_tooltip::after {
  content: "";
  position: absolute;
  top: 100%;
  left: 50%;
  border: 4px solid transparent;
  border-top-color: #ffffff;
  transform: translateX(-50%);
}

.player_progress:hover .progress_handle,
.player_progress.dragging .progress_handle {
  visibility: visible;
}

.player_progress.dragging .progress_tooltip {
  opacity: 1;
}

.now_track {
  grid-row: 2;
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
}

.player_cover {
  width: 62px;
  height: 62px;
  font-size: 1.8rem;
}

.now_text {
  display: grid;
  min-width: 0;
  gap: 4px;
}

.now_text strong {
  font-size: 1.05rem;
}

.now_text small {
  color: #8b919c;
}

.player_center {
  grid-row: 2;
  display: grid;
  align-items: center;
  justify-items: center;
  min-width: 0;
}

.control_row {
  display: flex;
  align-items: center;
  gap: 24px;
}

.control_row .play_button {
  width: 48px;
  height: 48px;
  color: #111318;
  background: transparent;
  font-size: 1.35rem;
}

.control_row .play_button .svg_icon {
  width: 19px;
  height: 19px;
}

.control_row .play_button:hover {
  background: #f0f2f6;
}

.player_tools {
  grid-row: 2;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  min-width: 0;
}

.volume_icon {
  color: #111318;
  width: 19px;
  height: 19px;
}

.player_tools input {
  width: 122px;
  accent-color: #111318;
}

.settings_overlay {
  position: fixed;
  inset: 0;
  z-index: 5;
  display: flex;
  justify-content: flex-end;
  background: rgba(18, 21, 28, 0.22);
}

.queue_overlay {
  position: fixed;
  inset: 0;
  z-index: 999;
  display: flex;
  justify-content: flex-end;
  background: rgba(18, 21, 28, 0.16);
}

.settings_panel {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: min(440px, 100vw);
  height: 100%;
  padding: 28px;
  background: #ffffff;
  box-shadow: -20px 0 60px rgba(19, 24, 34, 0.16);
}

.queue_panel {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  width: min(420px, 100vw);
  height: 100%;
  padding: 24px;
  background: #ffffff;
  box-shadow: -20px 0 60px rgba(19, 24, 34, 0.14);
}

.settings_panel header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.queue_panel header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px;
  min-width: 0;
  padding-bottom: 18px;
}

.settings_panel h2 {
  font-size: 1.45rem;
}

.queue_title_button {
  overflow: hidden;
  margin: 0;
  min-width: 0;
  border: 0;
  padding: 0;
  color: #1e2026;
  background: transparent;
  font-size: 1.24rem;
  font-weight: 800;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue_title_button:hover {
  color: #426dff;
}

.settings_panel header p {
  color: #8b919c;
}

.queue_panel header p {
  justify-self: end;
  overflow: hidden;
  margin: 0;
  color: #8b919c;
  font-size: 0.9rem;
  font-weight: 700;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue_panel header > div {
  min-width: 0;
}

.queue_list {
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 6px;
}

.queue_item {
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  width: 100%;
  min-height: 58px;
  border-radius: 8px;
  padding: 8px;
  color: #1e2026;
  background: transparent;
  text-align: left;
}

.queue_item:hover,
.queue_item.active {
  background: #f5f7ff;
}

.queue_cover {
  display: grid;
  place-items: center;
  overflow: hidden;
  width: 42px;
  height: 42px;
  border-radius: 8px;
  color: #ffffff;
  background:
    linear-gradient(145deg, #21242b, #426dff),
    #21242b;
  font-weight: 900;
}

.queue_cover.spinning_cover {
  border-radius: 50%;
  animation: cover_spin 16s linear infinite;
  will-change: transform;
}

.queue_panel.playing .queue_item.active .queue_cover {
  border-radius: 50%;
  animation: cover_spin 16s linear infinite;
  will-change: transform;
}

.queue_cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.queue_text {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.queue_text strong,
.queue_text small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue_text strong {
  font-size: 0.94rem;
}

.queue_text small,
.queue_duration {
  color: #8b919c;
  font-size: 0.84rem;
}

.queue_duration {
  justify-self: end;
}

.settings_section {
  display: grid;
  gap: 12px;
}

.settings_section h3 {
  font-size: 1rem;
}

.path_list {
  display: grid;
  gap: 8px;
}

.path_list p,
.settings_section input {
  width: 100%;
  border: 1px solid #e5e8ef;
  border-radius: 8px;
  padding: 10px 12px;
  color: #505763;
  background: #f8f9fb;
}

.settings_section label {
  display: grid;
  gap: 6px;
  min-width: 0;
  color: #8b919c;
  font-size: 0.84rem;
  font-weight: 800;
}
</style>
