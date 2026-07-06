<script setup lang="ts">
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  watchEffect,
} from "vue";
import { storeToRefs } from "pinia";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  currentMonitor,
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  primaryMonitor,
} from "@tauri-apps/api/window";
import repeat_one_icon from "./assets/icons/repeat-one.svg";
import repeat_icon from "./assets/icons/repeat.svg";
import shuffle_icon from "./assets/icons/shuffle.svg";
import ContextMenu from "./components/ContextMenu.vue";
import ContentArea from "./components/ContentArea.vue";
import GlobalNotification from "./components/GlobalNotification.vue";
import LibrarySidebar from "./components/LibrarySidebar.vue";
import PlayerBar from "./components/PlayerBar.vue";
import TrackDetailDialog from "./components/TrackDetailDialog.vue";
import TrackContextMenu from "./components/TrackContextMenu.vue";
import TopBar from "./components/TopBar.vue";
import { use_app_config_store } from "./stores/app_config";
import { use_library_store } from "./stores/library";
import { use_playback_store } from "./stores/playback";
import { use_player_queue_store } from "./stores/player_queue";
import type {
  AppConfig,
  AppStartup,
  AlbumItem,
  ArtistItem,
  DecoderRunSummary,
  LibraryRefreshResult,
  PlaybackMode,
  PlaybackModeItem,
  PlayTrackResult,
  PlayStatistics,
  PlaybackStatus,
  PlaylistBundle,
  PlaylistCache,
  QueueSource,
  Track,
  ViewKey,
} from "./types/music";
import { display_album, display_artist, display_title, is_missing_track } from "./utils/track";

const ConfirmDialog = defineAsyncComponent(() => import("./components/ConfirmDialog.vue"));
const LibraryScanDialog = defineAsyncComponent(() => import("./components/LibraryScanDialog.vue"));
const NowPlayingPage = defineAsyncComponent(() => import("./components/NowPlayingPage.vue"));
const PlaybackQueuePanel = defineAsyncComponent(
  () => import("./components/PlaybackQueuePanel.vue"),
);
const SettingsPanel = defineAsyncComponent(() => import("./components/SettingsPanel.vue"));

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

type TrackContextMenuState = {
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

type LibraryScanDialogState = {
  visible: boolean;
  status: "loading" | "success" | "error";
  title: string;
  message: string;
  detail: string;
};

const player_queue = use_player_queue_store();
const playback_store = use_playback_store();
const app_config_store = use_app_config_store();
const library_store = use_library_store();
const { library_tracks: tracks, current_queue, playback_mode, queue_source } = storeToRefs(player_queue);
const { status, current_track, progress_dragging } = storeToRefs(playback_store);
const { config: app_config } = storeToRefs(app_config_store);
const { selected_directories, library_loaded, playlists, play_statistics } = storeToRefs(library_store);
const loading = ref(false);
const error_message = ref("");
const query = ref("");
const active_view = ref<ViewKey>("all");
const selected_artist = ref("");
const selected_album = ref("");
const selected_playlist_id = ref("my_playlist");
const settings_open = ref(false);
const playback_queue_open = ref(false);
const now_playing_open = ref(false);
const locate_playing_track_request = ref(0);
const track_context_menu = ref<TrackContextMenuState | null>(null);
const track_detail_dialog = ref<Track | null>(null);
const playlist_context_menu = ref<PlaylistContextMenu | null>(null);
const pending_delete_playlist = ref<PlaylistCache | null>(null);
const library_scan_dialog = ref<LibraryScanDialogState>({
  visible: false,
  status: "loading",
  title: "加载曲库",
  message: "正在扫描音乐目录...",
  detail: "",
});
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
let window_resize_unlisten: UnlistenFn | null = null;
let window_close_unlisten: UnlistenFn | null = null;
let media_shortcut_listeners_disposed = false;
let media_shortcuts_scheduled = false;
let restored_playback_pending = false;
let restoring_player_cache = false;
let listening_track_id: string | null = null;
let listening_started_at = 0;
let closing_window = false;
let error_message_timer: number | undefined;
let app_window_shown = false;

const sidebar_min_width = 72;
const sidebar_max_width = 420;
const sidebar_compact_width = 100;
const app_min_width = 600;
const app_min_height = 700;
const player_cache_storage_key = "music_box_player_cache";
const app_window = getCurrentWindow();
const playback_modes: PlaybackModeItem[] = [
  { mode: "shuffle", icon: shuffle_icon, label: "随机播放" },
  { mode: "repeat", icon: repeat_icon, label: "循环播放" },
  { mode: "repeat_one", icon: repeat_one_icon, label: "单曲循环" },
];

const tracks_by_id = computed(() => new Map(tracks.value.map((track) => [track.id, track])));

const display_tracks = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return display_tracks_for_view(active_view.value);

  return display_tracks_for_view(active_view.value).filter((track) => track_matches_query(track, keyword));
});

function track_matches_query(track: Track, keyword: string) {
  if (active_view.value === "albums" && !selected_album.value) {
    return display_album(track).toLowerCase().includes(keyword);
  }
  if (active_view.value === "artists" && !selected_artist.value) {
    return display_artist(track).toLowerCase().includes(keyword);
  }
  return `${display_title(track)} ${display_artist(track)} ${display_album(track)}`.toLowerCase().includes(keyword);
}

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

const total_size = computed(() =>
  tracks.value.reduce((total, track) => total + (track.file_size ?? 0), 0),
);

const sidebar_compact = computed(() => sidebar_width.value < sidebar_compact_width);

const app_background_color = computed(
  () => app_config.value?.style.background_color?.trim() || "#ffffff",
);

const app_background_image = computed(() => {
  const image_path = app_config.value?.style.background_image?.trim();
  return image_path ? `url("${convertFileSrc(image_path)}")` : "none";
});

const app_background_image_opacity = computed(() =>
  Math.min(Math.max(app_config.value?.style.background_image_opacity ?? 1, 0), 1),
);

const theme_title_color = computed(
  () => app_config.value?.style.title_color?.trim() || "#1e2026",
);

const theme_subtitle_color = computed(
  () => app_config.value?.style.subtitle_color?.trim() || "#8b919c",
);

const theme_highlight_color = computed(
  () => app_config.value?.style.highlight_color?.trim() || "#22a05a",
);

const theme_border_color = computed(
  () => app_config.value?.style.border_color?.trim() || "#e8e8e8",
);

const app_border_width = computed(() =>
  (app_config.value?.style.show_border ?? true) ? "2px" : "0px",
);

const app_shell_style = computed(() => ({
  "--sidebar_width": `${sidebar_width.value}px`,
  "--app_border_width": app_border_width.value,
  "--app_background_color": app_background_color.value,
  "--app_background_image": app_background_image.value,
  "--app_background_image_opacity": `${app_background_image_opacity.value}`,
  "--theme-title-color": theme_title_color.value,
  "--theme-subtitle-color": theme_subtitle_color.value,
  "--theme-highlight-color": theme_highlight_color.value,
  "--theme-border-color": theme_border_color.value,
  "--theme-control-color": theme_title_color.value,
}));

watchEffect(() => {
  document.documentElement.style.setProperty("--theme-title-color", theme_title_color.value);
  document.documentElement.style.setProperty("--theme-subtitle-color", theme_subtitle_color.value);
  document.documentElement.style.setProperty("--theme-highlight-color", theme_highlight_color.value);
  document.documentElement.style.setProperty("--theme-border-color", theme_border_color.value);
  document.documentElement.style.setProperty("--theme-control-color", theme_title_color.value);
});

const playback_mode_button = computed(() => {
  return playback_modes.find((item) => item.mode === playback_mode.value) ?? playback_modes[0];
});

const user_playlist_items = computed<PlaylistCache[]>(() => {
  return playlists.value.my_playlists ?? [];
});

const ordered_user_playlist_items = computed<PlaylistCache[]>(() => {
  return [...user_playlist_items.value].sort((left, right) => {
    const left_index = left.metadata.index ?? 0;
    const right_index = right.metadata.index ?? 0;
    if (left_index !== right_index) return left_index - right_index;
    return left.name.localeCompare(right.name, "zh-Hans-CN");
  });
});

const selected_user_playlist = computed(() => {
  return (
    ordered_user_playlist_items.value.find((playlist) => playlist.id === selected_playlist_id.value) ??
    ordered_user_playlist_items.value[0] ??
    playlists.value.my_playlist
  );
});

function clear_error_message() {
  if (error_message_timer) {
    window.clearTimeout(error_message_timer);
    error_message_timer = undefined;
  }
  error_message.value = "";
}

function show_error_message(error: unknown) {
  if (error_message_timer) {
    window.clearTimeout(error_message_timer);
  }
  error_message.value = String(error);
  error_message_timer = window.setTimeout(() => {
    error_message.value = "";
    error_message_timer = undefined;
  }, 5000);
}

async function choose_music_directory() {
  clear_error_message();
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
  try {
    const next_config = await invoke<AppConfig>("add_music_dirs", { dirs: directories });
    app_config_store.hydrate_config(next_config, app_config_store.default_config ?? undefined);
    selected_directories.value = next_config.music_directory;
  } catch (error) {
    show_error_message(error);
  }
}

async function scan_directory(directories: string[]) {
  loading.value = true;
  clear_error_message();
  library_scan_dialog.value = {
    visible: true,
    status: "loading",
    title: "加载曲库",
    message: "正在扫描音乐目录...",
    detail: `共 ${directories.length} 个目录`,
  };

  try {
    const result = await invoke<LibraryRefreshResult>("reload_music_library", { dirs: directories });
    library_loaded.value = true;
    player_queue.set_library_tracks(result.tracks);
    playback_store.set_library_tracks(result.tracks);
    playlists.value = result.playlists;
    play_statistics.value = result.play_statistics;
    ensure_selected_playlist();
    const restored_player_cache = restore_player_cache();
    if (!restored_player_cache) {
      set_queue_for_current_view();
    }
    library_scan_dialog.value = {
      visible: true,
      status: "success",
      title: "曲库加载完成",
      message: `成功加载 ${result.tracks.length} 首歌曲`,
      detail: `已扫描 ${directories.length} 个目录`,
    };
  } catch (error) {
    show_error_message(error);
    library_scan_dialog.value = {
      visible: true,
      status: "error",
      title: "曲库加载失败",
      message: "扫描音乐目录时发生错误",
      detail: String(error),
    };
  } finally {
    loading.value = false;
  }
}

function close_library_scan_dialog() {
  if (library_scan_dialog.value.status === "loading") return;
  library_scan_dialog.value.visible = false;
}

async function reload_library() {
  const directories = app_config.value?.music_directory ?? selected_directories.value;
  if (!directories.length) {
    await choose_music_directory();
    return;
  }
  await scan_directory(directories);
}

async function decode_music_files() {
  loading.value = true;
  clear_error_message();
  const decoder_config = app_config.value?.decoder;
  const scan_directory_count = decoder_config?.scan_directory.filter((directory) => directory.trim()).length ?? 0;
  library_scan_dialog.value = {
    visible: true,
    status: "loading",
    title: "解码",
    message: "正在处理解码目录...",
    detail: `共 ${scan_directory_count} 个目录`,
  };

  try {
    const summary = await invoke<DecoderRunSummary>("run_decoder");
    const status = !summary.executed || summary.failed > 0 ? "error" : "success";
    library_scan_dialog.value = {
      visible: true,
      status,
      title: summary.executed
        ? summary.failed > 0
          ? "解码完成但有失败"
          : "解码完成"
        : "解码未执行",
      message: summary.message,
      detail: summary.output_dir
        ? `输出目录：${summary.output_dir}`
        : "请在配置中填写解码输出目录和扫描目录",
    };
  } catch (error) {
    show_error_message(error);
    library_scan_dialog.value = {
      visible: true,
      status: "error",
      title: "解码失败",
      message: "执行解码时发生错误",
      detail: String(error),
    };
  } finally {
    loading.value = false;
  }
}

async function load_startup_state() {
  loading.value = true;
  clear_error_message();

  try {
    const startup = await invoke<AppStartup>("get_startup_state");
    app_config_store.hydrate_config(startup.config, startup.default_config);
    await apply_config_state(startup.config);
    playlists.value = startup.playlists;
    play_statistics.value = startup.play_statistics;
    restoring_player_cache = true;
    library_loaded.value = startup.tracks.length > 0;
    player_queue.set_library_tracks(startup.tracks);
    playback_store.set_library_tracks(startup.tracks);
    selected_directories.value = startup.config.music_directory;
    ensure_selected_playlist();
    const restored_player_cache = restore_player_cache();
    restoring_player_cache = false;
    if (!restored_player_cache) {
      set_queue_for_current_view();
    }
  } catch (error) {
    restoring_player_cache = false;
    show_error_message(error);
    await show_app_window();
  } finally {
    loading.value = false;
  }
}

async function apply_config_state(config: AppConfig) {
  sidebar_width.value = clamp_sidebar_width(config.state.sidebar_width);
  playback_store.patch_status({ volume: config.state.volume });

  void invoke<PlaybackStatus>("set_volume", {
    volume: config.state.volume,
  })
    .then((next_status) => {
      playback_store.patch_status({ volume: next_status.volume });
    })
    .catch((error) => {
      console.warn("无法同步配置音量", error);
    });

  if (config.state.width > 0 && config.state.height > 0) {
    try {
      const width = Math.max(config.state.width, app_min_width);
      const height = Math.max(config.state.height, app_min_height);
      await app_window.setSize(new PhysicalSize(width, height));
      await center_app_window(width, height);
    } catch (error) {
      console.warn("无法同步配置窗口大小", error);
    }
  }

  await show_app_window();
}

async function center_app_window(width: number, height: number) {
  const monitor = (await currentMonitor()) ?? (await primaryMonitor());
  if (!monitor) return;

  const work_area = monitor.workArea;
  const left = work_area.position.x + Math.round((work_area.size.width - width) / 2);
  const top = work_area.position.y + Math.round((work_area.size.height - height) / 2);
  await app_window.setPosition(
    new PhysicalPosition(
      Math.max(work_area.position.x, left),
      Math.max(work_area.position.y, top),
    ),
  );
}

async function show_app_window() {
  if (app_window_shown) return;
  app_window_shown = true;
  try {
    await app_window.show();
    schedule_deferred_startup_work();
  } catch (error) {
    console.warn("无法显示窗口", error);
  }
}

function schedule_deferred_startup_work() {
  if (media_shortcuts_scheduled) return;
  media_shortcuts_scheduled = true;

  window.setTimeout(() => {
    void listen_media_shortcuts();
  }, 0);
}

async function play(track: Track) {
  try {
    await flush_listening_time();
    restored_playback_pending = false;
    handled_completion_path = "";
    clear_pending_seek();
    const result = await invoke<PlayTrackResult>("play_track", { path: track.path });
    apply_playback_status(result.status);
    library_store.add_recent_track(track);
    play_statistics.value = result.play_statistics;
    start_listening_session(track);
    start_status_polling();
  } catch (error) {
    show_error_message(error);
  }
}

async function toggle_playback() {
  if (!status.value.path) {
    set_queue_for_current_view();
  }
  const first_track = current_queue.value[0] ?? display_tracks.value.find((track) => !is_missing_track(track));
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

  const was_playing = status.value.playing;
  if (was_playing) {
    await flush_listening_time();
  }

  apply_playback_status(await invoke<PlaybackStatus>(was_playing ? "pause_track" : "resume_track"));
  if (!was_playing && status.value.playing && current_track.value) {
    start_listening_session(current_track.value);
  }
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
    await flush_listening_time();
    restored_playback_pending = false;
    clear_pending_seek();
    apply_playback_status(await invoke<PlaybackStatus>("stop_track"));
  } catch (error) {
    show_error_message(error);
  }
}

function start_listening_session(track: Track) {
  if (!status.value.playing || is_missing_track(track)) return;
  listening_track_id = track.id;
  listening_started_at = performance.now();
}

async function flush_listening_time() {
  if (!listening_track_id || !listening_started_at) return;

  const track_id = listening_track_id;
  const seconds = Math.floor((performance.now() - listening_started_at) / 1000);
  listening_track_id = null;
  listening_started_at = 0;

  if (seconds <= 0) return;

  try {
    play_statistics.value = await invoke<PlayStatistics>("record_listening_time", {
      trackId: track_id,
      seconds,
    });
  } catch (error) {
    console.warn("无法记录聆听时长", error);
  }
}

function minimize_window() {
  void app_window.minimize();
}

function toggle_maximize_window() {
  void app_window.toggleMaximize();
}

async function close_window() {
  await close_window_after_config_save();
}

function start_window_drag(event: MouseEvent) {
  if (event.button !== 0) return;

  const target = event.target as HTMLElement | null;
  if (target?.closest("button, input, label, a, [role='button'], .sidebar_resize_handle")) {
    return;
  }

  void app_window.startDragging();
}

async function capture_app_state() {
  const current_state = {
    volume: status.value.volume,
    sidebar_width: Math.round(sidebar_width.value),
  };

  try {
    const size = await app_window.innerSize();
    app_config_store.update_state({
      ...current_state,
      width: Math.max(Math.round(size.width), app_min_width),
      height: Math.max(Math.round(size.height), app_min_height),
    });
  } catch {
    app_config_store.update_state(current_state);
  }
}

async function flush_app_config() {
  await capture_app_state();
  await app_config_store.flush_config_save();
}

async function close_window_after_config_save() {
  if (closing_window) return;

  closing_window = true;
  try {
    await flush_app_config();
  } finally {
    await app_window.destroy();
  }
}

async function change_volume(event: Event) {
  const target = event.target as HTMLInputElement;
  const next_status = await invoke<PlaybackStatus>("set_volume", {
    volume: Number(target.value),
  });
  apply_playback_status(next_status);
  app_config_store.update_state({ volume: next_status.volume });
}

function update_progress_preview(event: PointerEvent) {
  const duration = current_track.value?.duration;
  if (!duration) return;

  const target = event.currentTarget as HTMLElement;
  const rect = progress_drag_rect ?? target.getBoundingClientRect();
  const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  progress_preview_percent = ratio * 100;
  progress_preview_seconds = Math.round(duration * ratio);
  set_visual_elapsed(progress_preview_seconds);
  render_progress(progress_preview_percent, progress_preview_seconds);
}

async function commit_progress_seek(seconds = progress_preview_seconds) {
  const duration = current_track.value?.duration;
  if (!duration || !status.value.path) return;

  const target_seconds = Math.min(Math.max(seconds, 0), duration);
  if (restored_playback_pending && !status.value.playing) {
    playback_store.patch_status({ elapsed: target_seconds });
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
    show_error_message(error);
  }
}

function begin_progress_drag(event: PointerEvent) {
  if (!current_track.value?.duration) return;
  playback_store.set_progress_dragging(true);
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
  playback_store.set_progress_dragging(false);
  await commit_progress_seek(progress_preview_seconds);
}

function cancel_progress_drag(event: PointerEvent) {
  if (!progress_dragging.value) return;
  playback_store.set_progress_dragging(false);
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
  playback_store.set_status(next_status);
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
  set_visual_elapsed(next_status.elapsed);
  progress_sync_started_at = performance.now();
  if (!progress_dragging.value) {
    render_progress(progress_percent_for(next_status.elapsed), next_status.elapsed);
  }
}

function hold_progress_at_seek_target(seconds: number) {
  pending_seek_seconds = seconds;
  pending_seek_path = status.value.path ?? null;
  pending_seek_started_at = performance.now();
  set_visual_elapsed(seconds);
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

  set_visual_elapsed(playback_elapsed_from_pending_seek());
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
    set_visual_elapsed(0);
    render_progress(0, 0);
    return;
  }

  if (status.value.playing) {
    const base_elapsed = pending_seek_seconds ?? status.value.elapsed;
    const started_at = pending_seek_seconds === null ? progress_sync_started_at : pending_seek_started_at;
    const elapsed = base_elapsed + (now - started_at) / 1000;
    if (!progress_dragging.value) {
      set_visual_elapsed(Math.min(elapsed, duration));
      render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
    }
    if (!progress_dragging.value && visual_elapsed >= duration) {
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

  set_visual_elapsed(Math.min(status.value.elapsed, duration));
  if (!progress_dragging.value) {
    render_progress(progress_percent_for(visual_elapsed), visual_elapsed);
  }
}

function set_visual_elapsed(seconds: number) {
  visual_elapsed = seconds;
  playback_store.set_visual_elapsed(seconds);
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
  if (!library_loaded.value && !status.value.path) return;

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
    const restored_queue_source =
      cache.queue_source.type === "search" ? queue_source_for_view("all") : cache.queue_source;

    const track_by_id = new Map(tracks.value.map((track) => [track.id, track]));
    const restored_queue =
      restored_queue_source.type === "all"
        ? tracks.value
        : cache.track_ids
            .map((track_id) => track_by_id.get(track_id))
            .filter((track): track is Track => Boolean(track));

    if (!restored_queue.length) return false;

    player_queue.set_current_queue(restored_queue_source, restored_queue);

    const restored_track =
      (cache.current_track_id ? track_by_id.get(cache.current_track_id) : undefined) ??
      tracks.value.find((track) => track.path === cache.current_track_path) ??
      restored_queue[0];

    if (!restored_track) return true;

    const elapsed = Math.min(Math.max(cache.elapsed ?? 0, 0), restored_track.duration ?? cache.elapsed ?? 0);
    restored_playback_pending = true;
    playback_store.set_status({
      path: restored_track.path,
      playing: false,
      volume: status.value.volume,
      elapsed,
    });
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
  show_view("user_playlist");
}

function queue_source_for_view(view: ViewKey): QueueSource {
  if (view === "artists" && selected_artist.value) {
    return { type: "artist", id: selected_artist.value, label: selected_artist.value };
  }
  if (view === "albums" && selected_album.value) {
    return { type: "album", id: selected_album.value, label: selected_album.value };
  }
  if (view === "user_playlist") {
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
    user_playlist: "我的歌单",
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
  if (view === "user_playlist") return tracks_from_ids(selected_user_playlist.value.track_ids);
  return tracks.value;
}

function display_tracks_for_view(view: ViewKey) {
  if (view === "user_playlist") return tracks_from_ids(selected_user_playlist.value.track_ids, true);
  return queue_tracks_for_view(view);
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

function tracks_from_ids(track_ids: string[], include_missing = false) {
  return track_ids
    .map((track_id) => tracks_by_id.value.get(track_id) ?? (include_missing ? missing_track_from_id(track_id) : null))
    .filter((track): track is Track => Boolean(track));
}

function missing_track_from_id(track_id: string): Track {
  return {
    id: track_id,
    title: track_id,
    artist: "",
    album: "",
    path: "",
    duration: null,
    file_size: null,
    bitrate: null,
    sample_rate: null,
    year: null,
    genre: [],
    track_number: null,
    disk_number: null,
    cover_cache_path: null,
    lyrics_cache_path: "",
    lyrics_cache_hash: "",
    metadata_source: "filename",
    missing: true,
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
    player_queue.set_current_queue(queue_source_for_view(active_view.value), display_tracks.value);
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
  if (active_view.value === "stats") {
    active_view.value = "all";
    selected_artist.value = "";
    selected_album.value = "";
  }
}

function focus_search() {
  if (active_view.value === "stats") {
    active_view.value = "all";
    selected_artist.value = "";
    selected_album.value = "";
  }
}

async function play_track_from_view(track: Track) {
  if (is_missing_track(track)) return;
  set_queue_for_current_view();
  await play(track);
}

function active_record_playlist_id() {
  if (query.value.trim() || selected_artist.value || selected_album.value) return "";
  if (active_view.value === "recent") return "recent";
  if (active_view.value === "user_playlist") return selected_user_playlist.value.id;
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

function open_track_detail_dialog() {
  const track = track_context_menu.value?.track;
  if (!track) return;
  track_detail_dialog.value = track;
  close_track_context_menu();
}

function close_track_detail_dialog() {
  track_detail_dialog.value = null;
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

async function reorder_playlists(playlist_ids: string[]) {
  try {
    apply_playlist_bundle(
      await invoke<PlaylistBundle>("reorder_user_playlists", {
        playlistIds: playlist_ids,
      }),
    );
  } catch (error) {
    show_error_message(error);
  }
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
    show_error_message(error);
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
    show_error_message(error);
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
      if (active_view.value === "user_playlist") {
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
    show_error_message(error);
  }
}

async function add_context_track_to_playlist(playlist: PlaylistCache) {
  const track = track_context_menu.value?.track;
  if (!track || is_missing_track(track) || playlist.track_ids.includes(track.id)) return;

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
    show_error_message(error);
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
    show_error_message(error);
  }
}

async function play_track_from_queue(track: Track) {
  if (is_missing_track(track)) return;
  await play(track);
}

async function open_queue_source() {
  const source = queue_source.value;
  playback_queue_open.value = false;

  if (source.type === "artist") {
    open_artist_playlist(source.id);
  } else if (source.type === "album") {
    open_album_playlist(source.id);
  } else if (source.type === "search") {
    show_view("all");
  } else if (source.type === "recent" || source.type === "user_playlist" || source.type === "all") {
    show_view(source.type);
  } else if (source.type === "playlist") {
    show_playlist(source.id);
  }

  await nextTick();
  locate_playing_track_request.value += 1;
}

function clamp_sidebar_width(width: number) {
  return Math.min(Math.max(width, sidebar_min_width), sidebar_max_width);
}

function save_sidebar_width() {
  app_config_store.update_state({
    sidebar_width: Math.round(sidebar_width.value),
  });
}

function resize_sidebar(event: PointerEvent) {
  const offset = event.clientX - sidebar_resize_start_x;
  sidebar_width.value = clamp_sidebar_width(sidebar_resize_start_width + offset);
  save_sidebar_width();
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
    await invoke("register_media_shortcuts");
  } catch (error) {
    console.warn("无法监听系统媒体热键", error);
  }
}

async function listen_window_resize() {
  try {
    window_resize_unlisten = await app_window.onResized(({ payload }) => {
      app_config_store.update_state({
        width: Math.round(payload.width),
        height: Math.round(payload.height),
      });
    });
  } catch (error) {
    console.warn("无法监听窗口尺寸变化", error);
  }
}

async function listen_window_close() {
  try {
    window_close_unlisten = await app_window.onCloseRequested((event) => {
      event.preventDefault();
      void close_window_after_config_save();
    });
  } catch (error) {
    console.warn("无法监听窗口关闭事件", error);
  }
}

function handle_before_unload() {
  save_player_cache();
  void flush_listening_time();
  void flush_app_config();
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
  void flush_listening_time();
  void flush_app_config();
  if (status_timer) window.clearInterval(status_timer);
  if (progress_frame) window.cancelAnimationFrame(progress_frame);
  if (error_message_timer) window.clearTimeout(error_message_timer);
  media_shortcut_listeners_disposed = true;
  media_shortcut_unlisteners.forEach((unlisten) => unlisten());
  media_shortcut_unlisteners = [];
  window_resize_unlisten?.();
  window_resize_unlisten = null;
  window_close_unlisten?.();
  window_close_unlisten = null;
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
  window.addEventListener("beforeunload", handle_before_unload);
  window.addEventListener("pointerdown", close_track_context_menu_on_pointer);
  window.addEventListener("keydown", close_track_context_menu_on_key);
  void listen_window_resize();
  void listen_window_close();
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
      :playlist_items="ordered_user_playlist_items"
      :active_playlist_id="selected_user_playlist.id"
      @show_view="show_view"
      @show_playlist="show_playlist"
      @create_playlist="create_playlist"
      @reorder_playlists="reorder_playlists"
      @open_playlist_menu="open_playlist_context_menu"
      @begin_resize="begin_sidebar_resize"
    />

    <section class="workspace">
      <TopBar
        :query="query"
        @update:query="update_query"
        @focus_search="focus_search"
        @open_tools="decode_music_files"
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
        :locate_track_request="locate_playing_track_request"
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
        :total_size="total_size"
        :play_statistics="play_statistics"
        @play_track="play_track_from_view"
        @open_track_menu="open_track_context_menu"
        @open_artist="open_artist_playlist"
        @open_album="open_album_playlist"
        @close_detail="close_detail_playlist"
      />
    </section>

    <TrackContextMenu
      v-if="track_context_menu"
      :track="track_context_menu.track"
      :x="track_context_menu.x"
      :y="track_context_menu.y"
      :playlists="ordered_user_playlist_items"
      :can_remove="context_track_can_be_removed()"
      @remove="remove_context_track_record"
      @detail="open_track_detail_dialog"
      @add_playlist="add_context_track_to_playlist"
    />

    <ContextMenu
      v-if="playlist_context_menu"
      class="playlist_context_menu"
      :x="playlist_context_menu.x"
      :y="playlist_context_menu.y"
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
    </ContextMenu>

    <PlayerBar
      v-if="!now_playing_open"
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
      @open_now_playing="now_playing_open = true"
    />

    <Transition name="now_playing_slide">
      <NowPlayingPage
        v-if="now_playing_open"
        ref="player_bar"
        :current_track="current_track"
        :status="status"
        :progress_dragging="progress_dragging"
        :playback_mode_button="playback_mode_button"
        @close="now_playing_open = false"
        @start_window_drag="start_window_drag"
        @minimize_window="minimize_window"
        @toggle_maximize_window="toggle_maximize_window"
        @close_window="close_window"
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
    </Transition>

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

    <LibraryScanDialog
      v-if="library_scan_dialog.visible"
      :status="library_scan_dialog.status"
      :title="library_scan_dialog.title"
      :message="library_scan_dialog.message"
      :detail="library_scan_dialog.detail"
      @confirm="close_library_scan_dialog"
    />

    <TrackDetailDialog
      v-if="track_detail_dialog"
      :track="track_detail_dialog"
      @close="close_track_detail_dialog"
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

    <GlobalNotification />
  </main>
</template>

<style>
:root {
  color: var(--theme-title-color, #1e2026);
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
  min-width: 600px;
  min-height: 700px;
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
  appearance: none;
  -webkit-appearance: none;
  border: 0;
  cursor: pointer;
  outline: none;
  box-shadow: none;
  -webkit-tap-highlight-color: transparent;
}

button:focus,
button:focus-visible {
  outline: none;
  box-shadow: none;
}

.app_shell {
  position: relative;
  display: grid;
  grid-template-areas:
    "sidebar workspace"
    "player player";
  grid-template-columns: var(--sidebar_width, 250px) minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr) 86px;
  height: 100vh;
  min-width: 600px;
  min-height: 700px;
  overflow: hidden;
  color: var(--theme-title-color, #1e2026);
  background-color: var(--app_background_color, #ffffff);
}

.app_shell::before {
  position: absolute;
  inset: 0;
  z-index: 0;
  content: "";
  background-image: var(--app_background_image, none);
  background-position: center;
  background-size: cover;
  background-repeat: no-repeat;
  opacity: var(--app_background_image_opacity, 1);
  pointer-events: none;
}

.app_shell > * {
  position: relative;
  z-index: 1;
}

.tool_button,
.window_button,
.player_tools button,
.control_row button {
  display: grid;
  width: 38px;
  height: 38px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--theme-control-color, #1e2026);
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

.settings_panel h2,
.settings_section h3,
p {
  margin: 0;
}

.workspace {
  grid-area: workspace;
  display: grid;
  grid-template-rows: 78px minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  background: transparent;
}

.now_playing_slide-enter-active,
.now_playing_slide-leave-active {
  transition:
    transform 260ms cubic-bezier(0.22, 1, 0.36, 1),
    opacity 220ms ease;
  will-change: transform, opacity;
}

.now_playing_slide-enter-from {
  opacity: 0.8;
  transform: translateY(100%);
}

.now_playing_slide-enter-to,
.now_playing_slide-leave-from {
  opacity: 1;
  transform: translateY(0);
}

.now_playing_slide-leave-to {
  opacity: 0.82;
  transform: translateY(100%);
}

.tool_button .svg_icon,
.window_button .svg_icon,
.player_tools .svg_icon,
.control_row .svg_icon {
  width: 20px;
  height: 20px;
}

.hover_border_control:hover {
  border-color: var(--theme-title-color, #1e2026);
  color: var(--theme-title-color, #1e2026);
  background: transparent;
}

.content_area {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 22px 22px 0;
}

.status_line,
.muted {
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.92rem;
}

.primary_button {
  min-height: 38px;
  border-radius: 8px;
  padding: 0 16px;
  color: #ffffff;
  background: var(--theme-highlight-color, #426dff);
  font-size: 0.95rem;
  font-weight: 800;
}

.status_line {
  padding: 8px;
}

.error_line {
  margin-bottom: 8px;
  border-radius: 6px;
  padding: 4px;
  color: #b42318;
  background: #fff1f0;
  font-size: 0.92rem;
}

.track_table,
.placeholder_view,
.stats_view,
.settings_content,
.queue_list {
  scrollbar-color: rgba(136, 150, 176, 0.38) transparent;
  scrollbar-width: thin;
}

.track_table {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 12px 18px 0;
}

.track_table_view {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.track_table::-webkit-scrollbar,
.placeholder_view::-webkit-scrollbar,
.stats_view::-webkit-scrollbar,
.settings_content::-webkit-scrollbar,
.queue_list::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

.track_table::-webkit-scrollbar-track,
.placeholder_view::-webkit-scrollbar-track,
.stats_view::-webkit-scrollbar-track,
.settings_content::-webkit-scrollbar-track,
.queue_list::-webkit-scrollbar-track {
  background: transparent;
}

.track_table::-webkit-scrollbar-thumb,
.placeholder_view::-webkit-scrollbar-thumb,
.stats_view::-webkit-scrollbar-thumb,
.settings_content::-webkit-scrollbar-thumb,
.queue_list::-webkit-scrollbar-thumb {
  border: 3px solid transparent;
  border-radius: 999px;
  background: rgba(136, 150, 176, 0.42);
  background-clip: content-box;
}

.track_table::-webkit-scrollbar-thumb:hover,
.placeholder_view::-webkit-scrollbar-thumb:hover,
.stats_view::-webkit-scrollbar-thumb:hover,
.settings_content::-webkit-scrollbar-thumb:hover,
.queue_list::-webkit-scrollbar-thumb:hover {
  background: rgba(66, 109, 255, 0.58);
  background-clip: content-box;
}

.track_table::-webkit-scrollbar-corner,
.placeholder_view::-webkit-scrollbar-corner,
.stats_view::-webkit-scrollbar-corner,
.settings_content::-webkit-scrollbar-corner,
.queue_list::-webkit-scrollbar-corner {
  background: transparent;
}

.virtual_track_spacer {
  width: 100%;
  flex: 0 0 auto;
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
  flex: 0 0 auto;
  height: 36px;
  border-radius: 8px;
  padding: 0 20px 0 0;
  color: var(--theme-subtitle-color, #a0a5af);
  font-size: 0.82rem;
  font-weight: 800;
}

.table_row {
  min-height: 74px;
  border: var(--app_border_width, 2px) solid transparent;
  border-radius: 8px;
  padding: 8px 0;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  text-align: left;
}

.table_row:hover,
.table_row.active {
  /* background: #f5f7ff; */
  border-color: var(--theme-border-color, #e8e8e8);
}

.table_row.missing,
.table_row.missing:hover {
  background: #fff1f1;
}

.table_row.missing {
  cursor: default;
}

.table_row.missing .song_text strong {
  color: #a43838;
}

.playlist_context_menu {
  min-width: 150px;
}

.context_menu_button {
  min-height: 34px;
  border-radius: 8px;
  padding: 0 10px;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 0.92rem;
  font-weight: 800;
  text-align: left;
}

.context_menu_button:hover {
  color: var(--theme-highlight-color, #426dff);
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
  color: var(--theme-subtitle-color, #8b919c);
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
    linear-gradient(145deg, #21242b, var(--theme-highlight-color, #426dff)),
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
.table_row.active .song_text strong {
  color: var(--theme-highlight-color, #426dff);
}


.song_text small,
.album_cell {
  color: var(--theme-subtitle-color, #a0a5af);
  font-size: 0.95rem;
}

.empty_state {
  display: grid;
  min-height: 220px;
  place-items: center;
  color: var(--theme-subtitle-color, #8b919c);
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
  color: var(--theme-highlight-color, #426dff);
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
  color: var(--theme-highlight-color, #426dff);
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
  color: var(--theme-subtitle-color, #8b919c);
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
    linear-gradient(145deg, #21242b, var(--theme-highlight-color, #426dff)),
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
  color: var(--theme-subtitle-color, #8b919c);
}

.stats_view {
  display: grid;
  align-content: start;
  gap: 24px;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 18px 8px;
}

.stats_section {
  display: grid;
  gap: 14px;
  min-width: 0;
}

.stats_section h2 {
  margin: 0;
  color: var(--theme-title-color, #1e2026);
  font-size: 1.18rem;
  font-weight: 900;
}

.stats_card_grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(130px, 1fr));
  gap: 16px;
}

.music_stats_grid {
  grid-template-columns: repeat(5, minmax(120px, 1fr));
}

.stats_card_grid article {
  display: grid;
  gap: 8px;
  min-width: 0;
  border: 1px solid #eef0f4;
  border-radius: 8px;
  padding: 20px;
  background: #fbfcfe;
}

.stats_card_grid strong {
  overflow: hidden;
  color: var(--theme-highlight-color, #426dff);
  font-size: 1.55rem;
  font-weight: 900;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stats_card_grid span {
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.92rem;
  font-weight: 800;
}

.most_played_section {
  padding-bottom: 18px;
}

.most_played_list {
  display: grid;
  gap: 4px;
}

.most_played_row {
  display: grid;
  grid-template-columns: 52px minmax(180px, 1fr) minmax(140px, 0.7fr) 76px;
  align-items: center;
  gap: 16px;
  min-height: 54px;
  border-radius: 8px;
  padding: 6px 12px 6px 0;
  background: transparent;
}

.most_played_row:hover {
  background: #f5f7ff;
}

.most_played_song {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.most_played_song strong,
.most_played_song small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.most_played_song small,
.play_count_cell {
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.9rem;
}

.play_count_cell {
  justify-self: end;
  font-weight: 800;
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

.settings_overlay {
  position: fixed;
  inset: 0;
  z-index: 999;
  display: flex;
  overflow: hidden;
  background-color: var(--app_background_color, #ffffff);
}

.settings_overlay::before {
  position: absolute;
  inset: 0;
  z-index: 0;
  content: "";
  background-image: var(--app_background_image, none);
  background-position: center;
  background-size: cover;
  background-repeat: no-repeat;
  opacity: var(--app_background_image_opacity, 1);
  pointer-events: none;
}

.settings_overlay > * {
  position: relative;
  z-index: 1;
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
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 22px;
  width: 100%;
  height: 100%;
  padding: 28px;
  background: transparent;
}

.queue_panel {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  width: min(420px, 100vw);
  height: 100%;
  overflow: hidden;
  border-radius: 10px 0 0 10px;
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

.settings_panel .primary_button {
  border: 1px solid #e5e8ef;
  color: var(--theme-highlight-color, #426dff);
  background: transparent;
}

.settings_body {
  display: grid;
  grid-template-columns: 180px minmax(0, 1fr);
  gap: 28px;
  min-height: 0;
}

.settings_nav {
  display: grid;
  align-content: start;
  gap: 8px;
  min-height: 0;
  overflow-y: auto;
  scrollbar-width: none;
}

.settings_nav::-webkit-scrollbar {
  display: none;
}

.settings_nav_item {
  min-height: 48px;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 0 16px;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 1.08rem;
  font-weight: 900;
  text-align: left;
}

.settings_nav_item:hover,
.settings_nav_item.active {
  color: var(--theme-highlight-color, #426dff);
  background: transparent;
}

.settings_content {
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
}

.queue_title_button {
  overflow: hidden;
  margin: 0;
  min-width: 0;
  border: 0;
  padding: 0;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 1.24rem;
  font-weight: 800;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue_title_button:hover {
  color: var(--theme-highlight-color, #426dff);
}

.settings_panel header p {
  color: var(--theme-subtitle-color, #8b919c);
}

.queue_panel header p {
  justify-self: end;
  overflow: hidden;
  margin: 0;
  color: var(--theme-subtitle-color, #8b919c);
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
  color: var(--theme-title-color, #1e2026);
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
    linear-gradient(145deg, #21242b, var(--theme-highlight-color, #426dff)),
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

.queue_item.active .queue_text strong {
  color: var(--theme-highlight-color, #426dff);
}

.queue_text small,
.queue_duration {
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.84rem;
}

.queue_duration {
  justify-self: end;
}

.settings_section {
  display: grid;
  align-content: start;
  gap: 16px;
}

.settings_section h3 {
  font-size: 1.25rem;
  font-weight: 900;
}

.settings_field_group {
  display: grid;
  gap: 14px;
}

.settings_row,
.settings_placeholder {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
  border: 1px solid #e5e8ef;
  border-radius: 8px;
  padding: 14px;
  background: transparent;
}

.settings_row > div,
.settings_placeholder {
  min-width: 0;
}

.settings_row strong,
.settings_placeholder strong {
  display: block;
  color: var(--theme-title-color, #1e2026);
  font-size: 0.96rem;
}

.settings_row span,
.settings_placeholder span {
  display: block;
  overflow: hidden;
  margin-top: 2px;
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.82rem;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path_list {
  display: grid;
  gap: 8px;
}

.path_list_row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 40px;
  gap: 8px;
  min-width: 0;
}

.path_list p,
.settings_section input {
  width: 100%;
  border: 1px solid #e5e8ef;
  border-radius: 8px;
  padding: 10px 12px;
  color: #505763;
  background: transparent;
}

.settings_input_row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 40px 40px;
  gap: 8px;
  min-width: 0;
}

.settings_section .settings_color_picker {
  width: 40px;
  height: 40px;
  min-height: 40px;
  border-radius: 8px;
  padding: 4px;
  cursor: pointer;
}

.settings_radio_field {
  display: grid;
  gap: 6px;
  min-width: 0;
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.84rem;
  font-weight: 800;
}

.settings_radio_group {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  min-width: 0;
}

.settings_section .settings_radio_option {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  border: 1px solid #e5e8ef;
  border-radius: 8px;
  padding: 0 12px;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  cursor: pointer;
}

.settings_section .settings_radio_option.active {
  color: var(--theme-highlight-color, #426dff);
}

.settings_section .settings_radio_option input {
  width: 14px;
  height: 14px;
  border: 0;
  padding: 0;
  background: transparent;
  accent-color: var(--theme-highlight-color, #426dff);
}

.settings_color_picker::-webkit-color-swatch-wrapper {
  padding: 0;
}

.settings_color_picker::-webkit-color-swatch {
  border: 0;
  border-radius: 6px;
}

.settings_opacity_control {
  display: grid;
  grid-template-columns: 92px minmax(120px, 1fr) 40px;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.settings_opacity_control > span {
  margin: 0;
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.82rem;
  font-weight: 800;
  white-space: nowrap;
}

.settings_opacity_control input[type="range"] {
  width: 100%;
  height: 40px;
  border: 0;
  padding: 0;
  background: transparent;
  accent-color: var(--theme-highlight-color, #426dff);
}

.settings_opacity_control input[type="range"]::-webkit-slider-runnable-track {
  height: 6px;
  border-radius: 999px;
  background: #e5e8ef;
}

.settings_opacity_control input[type="range"]::-webkit-slider-thumb {
  width: 16px;
  height: 16px;
  margin-top: -5px;
  border: 3px solid #ffffff;
  border-radius: 50%;
  background: var(--theme-highlight-color, #426dff);
  box-shadow: 0 2px 8px rgba(66, 109, 255, 0.28);
  -webkit-appearance: none;
}

.settings_opacity_control input[type="range"]::-moz-range-track {
  height: 6px;
  border-radius: 999px;
  background: #e5e8ef;
}

.settings_opacity_control input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  border: 3px solid #ffffff;
  border-radius: 50%;
  background: var(--theme-highlight-color, #426dff);
  box-shadow: 0 2px 8px rgba(66, 109, 255, 0.28);
}

.settings_default_button,
.settings_file_button,
.settings_delete_button {
  min-height: 40px;
  border: 1px solid #e5e8ef;
  border-radius: 8px;
  color: var(--theme-control-color, #1e2026);
  background: transparent;
  font-size: 0.88rem;
  font-weight: 800;
}

.settings_default_button {
  display: grid;
  width: 40px;
  place-items: center;
}


.settings_file_button,
.settings_delete_button {
  display: grid;
  width: 40px;
  place-items: center;
}


.settings_default_button .svg_icon,
.settings_file_button .svg_icon,
.settings_delete_button .svg_icon {
  width: 18px;
  height: 18px;
}

.settings_section label {
  display: grid;
  gap: 6px;
  min-width: 0;
  color: var(--theme-subtitle-color, #8b919c);
  font-size: 0.84rem;
  font-weight: 800;
}
</style>
