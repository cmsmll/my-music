<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import album_icon from "./assets/icons/album.svg";
import artist_icon from "./assets/icons/artist.svg";
import clock_fill_icon from "./assets/icons/clock-fill.svg";
import lyrics_copy_icon from "./assets/icons/lyrics-copy.svg";
import maximize_icon from "./assets/icons/maximize.svg";
import minimize_icon from "./assets/icons/minimize.svg";
import music_note_icon from "./assets/icons/music-note.svg";
import next_icon from "./assets/icons/next.svg";
import pause_icon from "./assets/icons/pause.svg";
import play_icon from "./assets/icons/play.svg";
import playlist_grid_icon from "./assets/icons/playlist-grid.svg";
import playlist_icon from "./assets/icons/playlist.svg";
import plus_icon from "./assets/icons/plus.svg";
import previous_icon from "./assets/icons/previous.svg";
import refresh_icon from "./assets/icons/refresh.svg";
import repeat_one_icon from "./assets/icons/repeat-one.svg";
import repeat_icon from "./assets/icons/repeat.svg";
import search_icon from "./assets/icons/search.svg";
import settings_icon from "./assets/icons/settings.svg";
import shuffle_icon from "./assets/icons/shuffle.svg";
import statistics_icon from "./assets/icons/statistics.svg";
import volume_icon from "./assets/icons/volume.svg";
import x_icon from "./assets/icons/x.svg";

type Track = {
  id: string;
  title: string;
  artist: string;
  album: string;
  path: string;
  duration?: number | null;
  cover_cache_path?: string | null;
  lyrics_cache_path: string;
  metadata: TrackMetadata;
};

type TrackMetadata = {
  title: string;
  artist: string;
  album: string;
  duration?: number | null;
  bitrate?: number | null;
  sample_rate?: number | null;
  year?: number | null;
  genre: string[];
  track_number?: number | null;
  disk_number?: number | null;
  cover_cache_path?: string | null;
  lyrics_cache_path: string;
  metadata_source: "embedded" | "embedded_with_filename_fallback" | "filename";
};

type AppConfig = {
  music_directory: string[];
  library_cache_dir: string;
  cover_cache_dir: string;
  lyrics_cache_dir: string;
};

type AppStartup = {
  config: AppConfig;
  tracks: Track[];
};

type PlaybackStatus = {
  path?: string | null;
  playing: boolean;
  volume: number;
  elapsed: number;
};

type ViewKey = "all" | "artists" | "albums" | "stats" | "recent" | "playlist_1";
type PlaybackMode = "shuffle" | "repeat" | "repeat_one";
type PlaybackModeItem = {
  mode: PlaybackMode;
  icon: string;
  label: string;
};
type ArtistItem = {
  name: string;
  track_count: number;
  total_duration: number;
  cover_track?: Track;
};

const tracks = ref<Track[]>([]);
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
const active_view = ref<ViewKey>("all");
const settings_open = ref(false);
const playback_mode = ref<PlaybackMode>("repeat");
const progress_dragging = ref(false);
const progress_fill_element = ref<HTMLElement | null>(null);
const progress_handle_element = ref<HTMLElement | null>(null);
const progress_tooltip_element = ref<HTMLElement | null>(null);
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

const sidebar_min_width = 72;
const sidebar_max_width = 420;
const sidebar_compact_width = 100;
const sidebar_width_storage_key = "music_box_sidebar_width";
const app_window = getCurrentWindow();
const playback_modes: PlaybackModeItem[] = [
  { mode: "shuffle", icon: shuffle_icon, label: "随机播放" },
  { mode: "repeat", icon: repeat_icon, label: "循环播放" },
  { mode: "repeat_one", icon: repeat_one_icon, label: "单曲循环" },
];

const current_track = computed(() =>
  tracks.value.find((track) => track.path === status.value.path),
);

const display_tracks = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return tracks.value;

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
    tracks.value = await invoke<Track[]>("scan_music_dir", { dirs: directories });
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
    tracks.value = startup.tracks;
    selected_directories.value = startup.config.music_directory;
  } catch (error) {
    error_message.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function play(track: Track) {
  try {
    handled_completion_path = "";
    clear_pending_seek();
    apply_playback_status(await invoke<PlaybackStatus>("play_track", { path: track.path }));
    start_status_polling();
  } catch (error) {
    error_message.value = String(error);
  }
}

async function toggle_playback() {
  if (!status.value.path && display_tracks.value[0]) {
    await play(display_tracks.value[0]);
    return;
  }

  apply_playback_status(
    await invoke<PlaybackStatus>(status.value.playing ? "pause_track" : "resume_track"),
  );
  start_status_polling();
}

async function previous_track() {
  const queue = current_queue();
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
  if (hold_pending_seek_progress(next_status)) {
    if (next_status.playing) request_progress_frame();
    return;
  }
  sync_visual_elapsed(next_status);
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
  const safe_percent = Math.min(Math.max(percent, 0), 100);
  if (progress_fill_element.value) {
    progress_fill_element.value.style.transform = `scaleX(${safe_percent / 100})`;
  }
  if (progress_handle_element.value) {
    progress_handle_element.value.style.left = `${safe_percent}%`;
  }
  if (progress_tooltip_element.value) {
    progress_tooltip_element.value.style.left = `${safe_percent}%`;
    progress_tooltip_element.value.textContent = format_duration(seconds);
  }
}

function current_queue() {
  return display_tracks.value.length ? display_tracks.value : tracks.value;
}

function queue_index(queue: Track[]) {
  if (!status.value.path) return -1;
  return queue.findIndex((track) => track.path === status.value.path);
}

function cycle_playback_mode() {
  const index = playback_modes.findIndex((item) => item.mode === playback_mode.value);
  playback_mode.value = playback_modes[(index + 1) % playback_modes.length].mode;
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
  const queue = current_queue();
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
}

function display_title(track?: Track | null) {
  return track?.title?.trim() || "未知歌曲";
}

function display_artist(track?: Track | null) {
  return track?.artist?.trim() || "未知歌手";
}

function display_album(track?: Track | null) {
  return track?.album?.trim() || "未知专辑";
}

function format_duration(seconds?: number | null) {
  if (!seconds) return "--:--";
  const minutes = Math.floor(seconds / 60);
  const rest = Math.floor(seconds % 60);
  return `${minutes}:${String(rest).padStart(2, "0")}`;
}

function cover_src(track?: Track | null) {
  return track?.cover_cache_path ? convertFileSrc(track.cover_cache_path) : "";
}

function icon_style(icon: string) {
  return { "--icon": `url("${icon}")` };
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

onBeforeUnmount(() => {
  if (status_timer) window.clearInterval(status_timer);
  if (progress_frame) window.cancelAnimationFrame(progress_frame);
  window.removeEventListener("pointermove", resize_sidebar);
  window.removeEventListener("pointerup", end_sidebar_resize);
  window.removeEventListener("pointercancel", end_sidebar_resize);
  document.body.classList.remove("resizing_sidebar");
});

onMounted(() => {
  load_sidebar_width();
  void load_startup_state();
});
</script>

<template>
  <main class="app_shell" :class="{ sidebar_compact, sidebar_resizing }" :style="app_shell_style">
    <aside class="sidebar">
      <nav class="sidebar_nav" aria-label="主导航">
        <section class="nav_group">
          <h2>音乐曲库</h2>
          <button class="nav_item" :class="{ active: active_view === 'all' && !query }" type="button"
            :title="String(tracks.length)"
            @click="show_view('all')">
            <span class="nav_icon svg_icon" :style="icon_style(music_note_icon)" />
            <span>全部</span>
          </button>
          <button class="nav_item" :class="{ active: active_view === 'artists' }" type="button"
            :title="String(artist_count)"
            @click="show_view('artists')">
            <span class="nav_icon svg_icon" :style="icon_style(artist_icon)" />
            <span>歌手</span>
          </button>
          <button class="nav_item" :class="{ active: active_view === 'albums' }" type="button"
            :title="String(album_count)"
            @click="show_view('albums')">
            <span class="nav_icon svg_icon" :style="icon_style(album_icon)" />
            <span>专辑</span>
          </button>
          <button class="nav_item" :class="{ active: active_view === 'stats' }" type="button"
            @click="show_view('stats')">
            <span class="nav_icon svg_icon" :style="icon_style(statistics_icon)" />
            <span>统计</span>
          </button>
        </section>

        <section class="nav_group playlist_group">
          <h2>播放列表</h2>
          <button class="nav_item" :class="{ active: active_view === 'recent' }" type="button" title="0"
            @click="show_view('recent')">
            <span class="nav_icon svg_icon" :style="icon_style(clock_fill_icon)" />
            <span>最近播放</span>
          </button>
          <button class="nav_item" :class="{ active: active_view === 'playlist_1' }" type="button" title="0"
            @click="show_view('playlist_1')">
            <span class="nav_icon svg_icon" :style="icon_style(playlist_grid_icon)" />
            <span>我的歌单</span>
          </button>
          <button class="nav_item create_playlist" type="button">
            <span class="nav_icon svg_icon" :style="icon_style(plus_icon)" />
            <span>新建歌单</span>
          </button>
        </section>
      </nav>
      <div class="sidebar_resize_handle" role="separator" aria-orientation="vertical" aria-label="调整侧边栏宽度"
        @pointerdown="begin_sidebar_resize" />
    </aside>

    <section class="workspace">
      <header class="top_bar" @mousedown="start_window_drag">
        <label class="search_box">
          <span class="svg_icon" :style="icon_style(search_icon)" />
          <input v-model="query" type="search" placeholder="搜索歌曲、歌手、专辑" @focus="active_view = 'all'" />
        </label>

        <div class="toolbar">
          <button class="tool_button" type="button" title="重新加载曲库" @click="reload_library">
            <span class="svg_icon" :style="icon_style(refresh_icon)" />
          </button>
          <button class="tool_button" type="button" title="设置" @click="settings_open = true">
            <span class="svg_icon" :style="icon_style(settings_icon)" />
          </button>
          <button class="window_button" type="button" title="最小化" @click="minimize_window">
            <span class="svg_icon" :style="icon_style(minimize_icon)" />
          </button>
          <button class="window_button" type="button" title="最大化" @click="toggle_maximize_window">
            <span class="svg_icon" :style="icon_style(maximize_icon)" />
          </button>
          <button class="window_button close" type="button" title="关闭" @click="close_window">
            <span class="svg_icon" :style="icon_style(x_icon)" />
          </button>
        </div>
      </header>

      <section class="content_area">
        <p v-if="loading" class="status_line">正在加载曲库...</p>
        <p v-if="error_message" class="error_line">{{ error_message }}</p>

        <section v-if="active_view === 'all' || query.trim()" class="track_table" aria-label="歌曲列表">
          <div class="table_head">
            <span class="index_cell">#</span>
            <span>歌曲</span>
            <span>专辑</span>
            <span class="duration_cell">时长</span>
          </div>

          <button v-for="(track, index) in display_tracks" :key="track.id" class="table_row"
            :class="{ active: track.path === status.path }" type="button" @click="play(track)">
            <span class="index_cell">{{ index + 1 }}</span>
            <span class="song_cell">
              <span class="cover_thumb">
                <img v-if="track.cover_cache_path" :src="cover_src(track)" alt="" />
                <span v-else>♪</span>
              </span>
              <span class="song_text">
                <strong>{{ display_title(track) }}</strong>
                <small>{{ display_artist(track) }}</small>
              </span>
            </span>
            <span class="album_cell">{{ display_album(track) }}</span>
            <span class="duration_cell">{{ format_duration(track.duration) }}</span>
          </button>

          <p v-if="!loading && !display_tracks.length" class="empty_state">
            没有找到歌曲，先添加音乐目录或调整搜索内容。
          </p>
        </section>

        <section v-else-if="active_view === 'albums'" class="placeholder_view">
          <div class="placeholder_grid">
            <article v-for="track in tracks.slice(0, 8)" :key="track.id" class="album_tile">
              <span class="album_art">
                <img v-if="track.cover_cache_path" :src="cover_src(track)" alt="" />
                <span v-else>♪</span>
              </span>
              <strong>{{ display_album(track) }}</strong>
              <small>{{ display_artist(track) }}</small>
            </article>
          </div>
          <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示专辑。</p>
        </section>

        <section v-else-if="active_view === 'artists'" class="placeholder_view">
          <div class="artist_grid">
            <article v-for="artist in artist_items" :key="artist.name" class="artist_tile">
              <span class="artist_art">
                <img v-if="artist.cover_track?.cover_cache_path" :src="cover_src(artist.cover_track)" alt="" />
                <span v-else>♪</span>
              </span>
              <strong :title="artist.name">{{ artist.name }}</strong>
              <small>{{ artist.track_count }} 首歌曲 {{ format_duration(artist.total_duration) }}</small>
            </article>
          </div>
          <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示歌手。</p>
        </section>

        <section v-else-if="active_view === 'stats'" class="stats_view">
          <article>
            <strong>{{ tracks.length }}</strong>
            <span>歌曲</span>
          </article>
          <article>
            <strong>{{ album_count }}</strong>
            <span>专辑</span>
          </article>
          <article>
            <strong>{{ artist_count }}</strong>
            <span>歌手</span>
          </article>
          <article>
            <strong>{{ format_duration(total_duration) }}</strong>
            <span>总时长</span>
          </article>
        </section>

        <section v-else class="placeholder_view">
          <p class="empty_state">这个播放列表界面已经预留，后续会接入播放记录和自定义歌单。</p>
        </section>
      </section>
    </section>

    <footer class="player_bar">
      <div class="player_progress" :class="{ dragging: progress_dragging }" role="slider" :aria-valuemin="0"
        :aria-valuemax="current_track?.duration ?? 0" :aria-valuenow="status.elapsed" aria-label="播放进度"
        @pointerdown="begin_progress_drag" @pointermove="drag_progress" @pointerup="end_progress_drag"
        @pointercancel="cancel_progress_drag">
        <div class="progress_bar_container">
          <div class="progress_track">
            <div ref="progress_fill_element" class="progress_fill" />
            <div ref="progress_handle_element" class="progress_handle" />
            <div ref="progress_tooltip_element" class="progress_tooltip">0:00</div>
          </div>
        </div>
      </div>

      <div class="now_track">
        <span class="player_cover">
          <img v-if="current_track?.cover_cache_path" :src="cover_src(current_track)" alt="" />
          <span v-else>♪</span>
        </span>
        <span class="now_text">
          <strong>{{ display_title(current_track) }}</strong>
          <small>{{ display_artist(current_track) }}</small>
        </span>
      </div>

      <div class="player_center">
        <div class="control_row">
          <button type="button" title="上一首" @click="previous_track">
            <span class="svg_icon" :style="icon_style(previous_icon)" />
          </button>
          <button class="play_button" type="button" title="播放或暂停" @click="toggle_playback">
            <span class="svg_icon" :style="icon_style(status.playing ? pause_icon : play_icon)" />
          </button>
          <button type="button" title="下一首" @click="next_track">
            <span class="svg_icon" :style="icon_style(next_icon)" />
          </button>
        </div>
      </div>

      <div class="player_tools">
        <button type="button" title="播放队列">
          <span class="svg_icon" :style="icon_style(playlist_icon)" />
        </button>
        <button type="button" :title="playback_mode_button.label" :aria-label="playback_mode_button.label"
          @click="cycle_playback_mode">
          <span class="svg_icon" :style="icon_style(playback_mode_button.icon)" />
        </button>
        <button type="button" title="桌面歌词">
          <span class="svg_icon" :style="icon_style(lyrics_copy_icon)" />
        </button>
        <span class="volume_icon svg_icon" :style="icon_style(volume_icon)" />
        <input type="range" min="0" max="1.5" step="0.01" :value="status.volume" @input="change_volume" />
      </div>
    </footer>

    <div v-if="settings_open" class="settings_overlay" @click.self="settings_open = false">
      <aside class="settings_panel" aria-label="设置">
        <header>
          <div>
            <h2>设置</h2>
            <p>配置文件内容</p>
          </div>
          <button class="tool_button" type="button" title="关闭设置" @click="settings_open = false">
            <span class="svg_icon" :style="icon_style(x_icon)" />
          </button>
        </header>

        <section class="settings_section">
          <h3>音乐目录</h3>
          <div v-if="app_config?.music_directory.length" class="path_list">
            <p v-for="directory in app_config.music_directory" :key="directory">{{ directory }}</p>
          </div>
          <p v-else class="muted">尚未选择音乐目录。</p>
          <button class="primary_button" type="button" @click="choose_music_directory">添加音乐目录</button>
        </section>

        <section class="settings_section">
          <h3>缓存位置</h3>
          <label>
            <span>library_cache_dir</span>
            <input :value="app_config?.library_cache_dir ?? ''" readonly />
          </label>
          <label>
            <span>cover_cache_dir</span>
            <input :value="app_config?.cover_cache_dir ?? ''" readonly />
          </label>
          <label>
            <span>lyrics_cache_dir</span>
            <input :value="app_config?.lyrics_cache_dir ?? ''" readonly />
          </label>
        </section>
      </aside>
    </div>
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
  overflow: hidden;
  padding: 28px 28px;
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
  overflow: auto;
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

.settings_panel header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.settings_panel h2 {
  font-size: 1.45rem;
}

.settings_panel header p {
  color: #8b919c;
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
