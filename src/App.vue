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
import random_icon from "./assets/icons/disorder.svg";
import repeat_one_icon from "./assets/icons/repeat-one.svg";
import repeat_icon from "./assets/icons/repeat.svg";
import shuffle_icon from "./assets/icons/shuffle.svg";
import ContextMenu from "./components/ContextMenu.vue";
import ContentArea from "./components/ContentArea.vue";
import GlobalNotification from "./components/GlobalNotification.vue";
import LibrarySidebar from "./components/LibrarySidebar.vue";
import AudioEngine from "./components/AudioEngine.vue";
import PlayerBar from "./components/PlayerBar.vue";
import TrackDetailDialog from "./components/TrackDetailDialog.vue";
import TrackContextMenu from "./components/TrackContextMenu.vue";
import TopBar from "./components/TopBar.vue";
import { use_app_config_store } from "./stores/app_config";
import { use_app_actions_store } from "./stores/app_actions";
import { use_library_store } from "./stores/library";
import { use_library_catalog_store } from "./stores/library_catalog";
import { use_library_view_store } from "./stores/library_view";
import { use_notification_store } from "./stores/notifications";
import { use_playback_store } from "./stores/playback";
import { use_player_queue_store } from "./stores/player_queue";
import { use_ui_store } from "./stores/ui";
import type {
  AppConfig,
  AppStartup,
  DecoderRunSummary,
  LibraryRefreshResult,
  PlaybackMode,
  PlaybackModeItem,
  PlaybackRecord,
  PlaybackRecordSource,
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

type AudioEngineExpose = {
  play: (track: Track, seconds?: number) => Promise<void>;
  pause: () => void;
  resume: () => Promise<void>;
  stop: () => void;
  seek: (seconds: number) => void;
  set_volume: (volume: number) => void;
  status: () => PlaybackStatus;
  preview_seek: (seconds: number) => void;
  cancel_seek_preview: () => void;
  apply_external_status: (status: PlaybackStatus) => void;
  cache_elapsed_seconds: () => number;
  save_playback_elapsed_cache: (seconds?: number, force?: boolean) => void;
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
const app_actions = use_app_actions_store();
const library_store = use_library_store();
const library_catalog = use_library_catalog_store();
const library_view = use_library_view_store();
const notification = use_notification_store();
const ui_store = use_ui_store();
const {
  library_tracks: tracks,
  current_queue,
  active_queue,
  playback_mode,
  playback_record,
  queue_source,
} = storeToRefs(player_queue);
const { status, current_track, progress_dragging } = storeToRefs(playback_store);
const { tracks_by_id } = storeToRefs(library_catalog);
const { config: app_config } = storeToRefs(app_config_store);
const { selected_directories, library_loaded, playlists, play_statistics } = storeToRefs(library_store);
const { active_view, query, selected_album, selected_artist, selected_playlist_id } = storeToRefs(library_view);
const { settings_open, playback_queue_open, now_playing_open } = storeToRefs(ui_store);
const loading = ref(false);
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
const audio_engine = ref<AudioEngineExpose | null>(null);
const sidebar_width = ref(250);
const sidebar_resizing = ref(false);
let progress_preview_seconds = 0;
let progress_drag_rect: DOMRect | null = null;
let handled_completion_path = "";
let sidebar_resize_start_x = 0;
let sidebar_resize_start_width = 250;
let media_shortcut_unlisteners: UnlistenFn[] = [];
let window_resize_unlisten: UnlistenFn | null = null;
let window_close_unlisten: UnlistenFn | null = null;
let media_shortcut_listeners_disposed = false;
let media_shortcuts_scheduled = false;
let restored_playback_pending = false;
let restoring_playback_record = false;
let listening_track_id: string | null = null;
let listening_started_at = 0;
let closing_window = false;
let app_window_shown = false;

const sidebar_min_width = 72;
const sidebar_max_width = 420;
const sidebar_compact_width = 100;
const app_min_width = 600;
const app_min_height = 700;
const app_window = getCurrentWindow();
const playback_modes: PlaybackModeItem[] = [
  { mode: "shuffle", icon: shuffle_icon, label: "随机列表" },
  { mode: "random", icon: random_icon, label: "随机播放" },
  { mode: "repeat", icon: repeat_icon, label: "循环播放" },
  { mode: "repeat_one", icon: repeat_one_icon, label: "单曲循环" },
];

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
  () => app_config.value?.style.highlight_color?.trim() || "#3bce82",
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

function show_error_message(error: unknown) {
  notification.error(error instanceof Error ? error.message : String(error));
}

function ensure_audio_engine() {
  if (!audio_engine.value) {
    throw new Error("音频组件未初始化");
  }
  return audio_engine.value;
}

function audio_status() {
  return ensure_audio_engine().status();
}

async function choose_music_directory() {
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
    playlists.value = result.playlists;
    play_statistics.value = result.play_statistics;
    ensure_selected_playlist();
    const restored_playback_record = restore_playback_record_cache();
    if (!restored_playback_record) {
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

  try {
    const startup = await invoke<AppStartup>("get_startup_state");
    app_config_store.hydrate_config(startup.config, startup.default_config);
    await apply_config_state(startup.config);
    playlists.value = startup.playlists;
    play_statistics.value = startup.play_statistics;
    player_queue.hydrate_playback_record();
    restoring_playback_record = true;
    library_loaded.value = startup.tracks.length > 0;
    player_queue.set_library_tracks(startup.tracks);
    selected_directories.value = startup.config.music_directory;
    ensure_selected_playlist();
    const restored_playback_record = restore_playback_record_cache();
    restoring_playback_record = false;
    if (!restored_playback_record) {
      set_queue_for_current_view();
    }
  } catch (error) {
    restoring_playback_record = false;
    show_error_message(error);
    await show_app_window();
  } finally {
    loading.value = false;
  }
}

async function apply_config_state(config: AppConfig) {
  sidebar_width.value = clamp_sidebar_width(config.state.sidebar_width);
  playback_store.patch_status({ volume: config.state.volume });
  ensure_audio_engine().set_volume(config.state.volume);

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

async function play(track: Track, seconds = 0) {
  try {
    await flush_listening_time();
    restored_playback_pending = false;
    handled_completion_path = "";
    await ensure_audio_engine().play(track, seconds);
    library_store.add_recent_track(track);
    play_statistics.value = await invoke<PlayStatistics>("record_track_started", { path: track.path });
    start_listening_session(track);
  } catch (error) {
    show_error_message(error);
  }
}

async function toggle_playback() {
  if (!status.value.path) {
    set_queue_for_current_view();
  }
  const first_track = playback_queue()[0] ?? display_tracks.value.find((track) => !is_missing_track(track));
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

  try {
    if (was_playing) {
      ensure_audio_engine().pause();
    } else {
      await ensure_audio_engine().resume();
    }
  } catch (error) {
    show_error_message(error);
    return;
  }
  if (!was_playing && status.value.playing && current_track.value) {
    start_listening_session(current_track.value);
  }
}

async function previous_track() {
  const queue = playback_queue();
  if (!queue.length) return;

  if (playback_mode.value === "shuffle") {
    if (!active_queue.value.length) {
      set_queue_for_current_view();
    }
    const shuffle_mode_queue = playback_queue();
    if (!shuffle_mode_queue.length) return;

    const index = queue_index(shuffle_mode_queue);
    if (index > 0) {
      await play(shuffle_mode_queue[index - 1]);
      return;
    }

    const reshuffled_queue = player_queue.reshuffle_queue(current_track.value?.id ?? null);
    const first_track = reshuffled_queue[0];
    if (first_track) await play(first_track);
    return;
  }

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
    ensure_audio_engine().stop();
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
    flush_playback_record_cache();
    await flush_app_config();
  } finally {
    await app_window.destroy();
  }
}

async function change_volume(event: Event) {
  const target = event.target as HTMLInputElement;
  ensure_audio_engine().set_volume(Number(target.value));
  const next_status = audio_status();
  app_config_store.update_state({ volume: next_status.volume });
}

function update_progress_preview(event: PointerEvent) {
  const duration = current_track.value?.duration;
  if (!duration) return;

  const target = event.currentTarget as HTMLElement;
  const rect = progress_drag_rect ?? target.getBoundingClientRect();
  const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  progress_preview_seconds = Math.round(duration * ratio);
  ensure_audio_engine().preview_seek(progress_preview_seconds);
}

async function commit_progress_seek(seconds = progress_preview_seconds) {
  const duration = current_track.value?.duration;
  if (!duration || !status.value.path) return;

  const target_seconds = Math.min(Math.max(seconds, 0), duration);
  if (restored_playback_pending && !status.value.playing) {
    playback_store.patch_status({ elapsed: target_seconds });
    ensure_audio_engine().apply_external_status(status.value);
    save_playback_elapsed_cache(target_seconds, true);
    return;
  }

  try {
    handled_completion_path = "";
    ensure_audio_engine().seek(target_seconds);
  } catch (error) {
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
  ensure_audio_engine().cancel_seek_preview();
}

function cache_elapsed_seconds() {
  return audio_engine.value?.cache_elapsed_seconds() ?? Math.max(0, Math.floor(status.value.elapsed));
}

function playback_record_source_for_queue_source(source: QueueSource): PlaybackRecordSource {
  return {
    source_type: source.type,
    id: source.id,
    label: source.label,
  };
}

function playback_record_primary_source(source: QueueSource): PlaybackRecordSource {
  if (source.type === "artist") {
    return { source_type: "artists", id: "artists", label: "歌手" };
  }
  if (source.type === "album") {
    return { source_type: "albums", id: "albums", label: "专辑" };
  }
  if (source.type === "search") {
    return { source_type: "all", id: "all", label: "全部" };
  }
  return playback_record_source_for_queue_source(source);
}

function playback_record_secondary_source(source: QueueSource): PlaybackRecordSource | null {
  if (source.type !== "artist" && source.type !== "album") return null;
  return playback_record_source_for_queue_source(source);
}

function playback_mode_from_record(mode: string): PlaybackMode {
  return playback_modes.some((item) => item.mode === mode) ? (mode as PlaybackMode) : "repeat";
}

function queue_source_from_playback_record_source(source: PlaybackRecordSource): QueueSource {
  return {
    type: source.source_type,
    id: source.id,
    label: source.label,
  };
}

function queue_source_from_playback_record(record: PlaybackRecord): QueueSource {
  if (
    record.secondary_playlist?.source_type === "artist" ||
    record.secondary_playlist?.source_type === "album"
  ) {
    return queue_source_from_playback_record_source(record.secondary_playlist);
  }
  return queue_source_from_playback_record_source(record.playlist);
}

function current_playback_record(): PlaybackRecord | null {
  const track_id = current_track.value?.id;
  if (!track_id) return null;

  return {
    version: 1,
    track_id,
    elapsed: cache_elapsed_seconds(),
    playback_mode: playback_mode.value,
    playlist: playback_record_primary_source(queue_source.value),
    secondary_playlist: playback_record_secondary_source(queue_source.value),
  };
}

function save_playback_record_metadata_cache() {
  if (restoring_playback_record) return;
  if (!library_loaded.value && !status.value.path) return;

  const record = current_playback_record();
  if (!record) return;

  player_queue.save_playback_record_metadata(record);
}

function save_playback_elapsed_cache(seconds = cache_elapsed_seconds(), force = false) {
  if (restoring_playback_record) return;
  audio_engine.value?.save_playback_elapsed_cache(seconds, force);
}

function flush_playback_record_cache() {
  save_playback_record_metadata_cache();
  save_playback_elapsed_cache(cache_elapsed_seconds(), true);
}


function restore_playback_record_cache() {
  const record = playback_record.value;
  if (!record || record.version !== 1) return false;

  const restored_track = tracks_by_id.value[record.track_id];
  if (!restored_track) return false;

  const was_restoring_playback_record = restoring_playback_record;
  restoring_playback_record = true;
  try {
    const restored_mode = playback_mode_from_record(record.playback_mode);
    player_queue.set_playback_mode(restored_mode);

    const restored_queue_source = queue_source_with_latest_label(queue_source_from_playback_record(record));
    const restored_queue = queue_tracks_for_source(restored_queue_source);
    const restored_queue_has_track = restored_queue.some((track) => track.id === restored_track.id);
    const effective_queue_source = restored_queue_has_track ? restored_queue_source : queue_source_for_view("all");
    const effective_queue = restored_queue_has_track ? restored_queue : tracks.value;

    if (!effective_queue.length) return false;

    player_queue.set_current_queue(effective_queue_source, effective_queue);

    const elapsed = Math.min(Math.max(record.elapsed ?? 0, 0), restored_track.duration ?? record.elapsed ?? 0);
    restored_playback_pending = true;
    playback_store.set_status({
      path: restored_track.path,
      playing: false,
      volume: status.value.volume,
      elapsed,
    });
    player_queue.set_current_track_path(restored_track.path);
    ensure_audio_engine().apply_external_status(status.value);
    return true;
  } finally {
    restoring_playback_record = was_restoring_playback_record;
  }
}

async function play_from_cached_position(track: Track, seconds: number) {
  const target_seconds = Math.min(Math.max(Math.floor(seconds), 0), track.duration ?? seconds);
  await play(track, target_seconds);

}

function playback_queue() {
  return active_queue.value.length ? active_queue.value : display_tracks.value;
}

function queue_index(queue: Track[]) {
  if (!status.value.path) return -1;
  return queue.findIndex((track) => track.path === status.value.path);
}

function cycle_playback_mode() {
  const index = playback_modes.findIndex((item) => item.mode === playback_mode.value);
  const next_mode = playback_modes[(index + 1) % playback_modes.length].mode;
  player_queue.set_playback_mode(next_mode);
}

function random_track_index(queue: Track[], current_index: number) {
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
  if (playback_mode.value === "random") {
    await play(queue[random_track_index(queue, current_index)]);
    return true;
  }

  if (playback_mode.value === "shuffle") {
    if (!active_queue.value.length) {
      set_queue_for_current_view();
    }
    const shuffle_mode_queue = playback_queue();
    if (!shuffle_mode_queue.length) return false;

    const shuffle_current_index = queue_index(shuffle_mode_queue);
    const next_index = shuffle_current_index < 0 ? 0 : shuffle_current_index + 1;
    if (next_index < shuffle_mode_queue.length) {
      await play(shuffle_mode_queue[next_index]);
      return true;
    }

    const reshuffled_queue = player_queue.reshuffle_queue(current_track.value?.id ?? null);
    const first_track = reshuffled_queue[0];
    if (!first_track) return false;

    await play(first_track);
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
  library_view.show_view(view);
}

function show_playlist(playlist_id: string) {
  library_view.show_playlist(playlist_id);
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
    return library_catalog.tracks_for_artist(selected_artist.value);
  }
  if (view === "albums" && selected_album.value) {
    return library_catalog.tracks_for_album(selected_album.value);
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
    return library_catalog.tracks_for_artist(source.id);
  }
  if (source.type === "album") {
    return library_catalog.tracks_for_album(source.id);
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
    .map((track_id) => tracks_by_id.value[track_id] ?? (include_missing ? missing_track_from_id(track_id) : null))
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

  library_view.set_selected_playlist(user_playlist_items.value[0]?.id ?? playlists.value.my_playlist.id);
}

function set_queue_for_current_view() {
  if (query.value.trim()) {
    player_queue.set_current_queue(queue_source_for_view(active_view.value), display_tracks.value);
    return;
  }

  player_queue.set_current_queue(queue_source_for_view(active_view.value), queue_tracks_for_view(active_view.value));
}

function open_artist_playlist(name: string) {
  library_view.open_artist(name);
}

function open_album_playlist(name: string) {
  library_view.open_album(name);
}

function close_detail_playlist() {
  library_view.close_detail();
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
  close_playlist_context_menu();
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
  close_track_context_menu();
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
  ui_store.close_playback_queue();

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
  library_view.request_locate_playing_track();
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
  app_actions.reset();
  flush_playback_record_cache();
  void flush_listening_time();
  void flush_app_config();
}

function close_context_menus_on_pointer(event: PointerEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest(".context_menu")) return;
  close_track_context_menu();
  close_playlist_context_menu();
}

function prevent_default_context_menu(event: MouseEvent) {
  event.preventDefault();
}

function close_context_menus_on_key(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close_track_context_menu();
    close_playlist_context_menu();
    if (selected_artist.value || selected_album.value) {
      close_detail_playlist();
    }
  }
}

app_actions.register({
  begin_progress_drag,
  drag_progress,
  end_progress_drag,
  cancel_progress_drag,
  previous_track,
  toggle_playback,
  next_track,
  cycle_playback_mode,
  change_volume,
  play_track: play_track_from_view,
  play_queue_track: play_track_from_queue,
  open_queue_source,
  create_playlist,
  reorder_playlists,
  open_playlist_menu: open_playlist_context_menu,
  open_track_menu: open_track_context_menu,
  begin_sidebar_resize: begin_sidebar_resize,
  decode_music_files,
  reload_library,
  start_window_drag,
  minimize_window,
  toggle_maximize_window,
  close_window,
});
onBeforeUnmount(() => {
  app_actions.reset();
  flush_playback_record_cache();
  void flush_listening_time();
  void flush_app_config();
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
  window.removeEventListener("pointerdown", close_context_menus_on_pointer);
  window.removeEventListener("contextmenu", prevent_default_context_menu);
  window.removeEventListener("keydown", close_context_menus_on_key);
  document.body.classList.remove("resizing_sidebar");
});

onMounted(() => {
  media_shortcut_listeners_disposed = false;
  window.addEventListener("beforeunload", handle_before_unload);
  window.addEventListener("pointerdown", close_context_menus_on_pointer);
  window.addEventListener("contextmenu", prevent_default_context_menu);
  window.addEventListener("keydown", close_context_menus_on_key);
  void listen_window_resize();
  void listen_window_close();
  void load_startup_state();
});

watch([current_queue, queue_source, playback_mode], () => {
  save_playback_record_metadata_cache();
});

watch(() => current_track.value?.id ?? null, () => {
  save_playback_record_metadata_cache();
  save_playback_elapsed_cache(cache_elapsed_seconds(), true);
});

</script>

<template>
  <main class="app_shell" :class="{ sidebar_compact, sidebar_resizing }" :style="app_shell_style">
    <AudioEngine
      ref="audio_engine"
      @ended="handle_playback_completion"
      @playback_error="show_error_message"
    />
    <LibrarySidebar />


    <section class="workspace">
      <TopBar />


      <ContentArea :loading="loading" :display_tracks="display_tracks" />

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

    <KeepAlive>
      <PlayerBar v-if="!now_playing_open" :playback_mode_button="playback_mode_button" />

    </KeepAlive>

    <Transition name="now_playing_slide">
      <KeepAlive>
        <NowPlayingPage v-if="now_playing_open" :playback_mode_button="playback_mode_button" />

      </KeepAlive>
    </Transition>

    <KeepAlive>
      <PlaybackQueuePanel v-if="playback_queue_open" />

    </KeepAlive>

    <SettingsPanel
      v-if="settings_open"
      :app_config="app_config"
      @close="ui_store.close_settings()"
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
