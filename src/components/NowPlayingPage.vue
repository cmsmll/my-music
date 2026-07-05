<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import compact_left_icon from "../assets/icons/compact-left.svg";
import compact_right_icon from "../assets/icons/compact-right.svg";
import maximize_icon from "../assets/icons/maximize.svg";
import minimize_icon from "../assets/icons/minimize.svg";
import tonearm_icon from "../assets/tonearm-minimal-white.svg";
import x_icon from "../assets/icons/x.svg";
import LineLyricsRenderer from "./LineLyricsRenderer.vue";
import PlayerBar from "./PlayerBar.vue";
import { use_app_config_store } from "../stores/app_config";
import { use_notification_store } from "../stores/notifications";
import { use_playback_store } from "../stores/playback";
import { use_player_queue_store } from "../stores/player_queue";
import type {
  LyricsSearchResponse,
  LyricsSearchResult,
  LyricsUseResult,
  PlaybackModeItem,
  PlaybackStatus,
  Track,
} from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration, icon_style } from "../utils/track";

type PlayerBarExpose = {
  render_progress: (percent: number, seconds: number) => void;
};

const props = defineProps<{
  current_track?: Track | null;
  status: PlaybackStatus;
  visual_elapsed: number;
  progress_dragging: boolean;
  playback_mode_button: PlaybackModeItem;
}>();

const player_queue = use_player_queue_store();
const playback_store = use_playback_store();
const app_config_store = use_app_config_store();
const notification = use_notification_store();

const emit = defineEmits<{
  close: [];
  start_window_drag: [event: MouseEvent];
  minimize_window: [];
  toggle_maximize_window: [];
  close_window: [];
  begin_progress_drag: [event: PointerEvent];
  drag_progress: [event: PointerEvent];
  end_progress_drag: [event: PointerEvent];
  cancel_progress_drag: [event: PointerEvent];
  previous_track: [];
  toggle_playback: [];
  next_track: [];
  open_queue: [];
  cycle_playback_mode: [];
  change_volume: [event: Event];
}>();

const player_bar = ref<PlayerBarExpose | null>(null);
const lyrics_loading = ref(false);
const lyrics_text = ref("");
const lyrics_lines = ref<string[]>([]);
const compact_panel = ref<"record" | "lyrics">("record");
const lyrics_search_open = ref(false);
const lyrics_search_loading = ref(false);
const lyrics_search_error = ref("");
const lyrics_search_results = ref<LyricsSearchResult[]>([]);
const current_lyrics_hash = ref<string | null>(null);
const lyrics_use_pending_hash = ref("");
const auto_lyrics_attempted_track_ids = ref(new Set<string>());
let lyrics_search_request_id = 0;
let auto_lyrics_request_id = 0;
let lyrics_load_request_id = 0;

const lyric_placeholder = [
  "暂未获取到歌词",
  "可以重新加载曲库，或检查音频文件是否包含内嵌歌词。",
];


const auto_lyrics_enabled = computed({
  get() {
    return app_config_store.config?.state.auto_lyrics_enabled ?? false;
  },
  set(value: boolean) {
    app_config_store.update_state({ auto_lyrics_enabled: value });
  },
});

function normalize_lyrics(content: string) {
  return content
    .split(/\r?\n/)
    .map((line) => line.replace(/^(\[[^\]]+\])+\s*/u, "").trim())
    .filter(Boolean);
}

async function load_lyrics(track?: Track | null) {
  const request_id = ++lyrics_load_request_id;
  lyrics_text.value = "";
  lyrics_lines.value = [];
  const path = track?.lyrics_cache_path?.trim();
  if (!path) return false;

  lyrics_loading.value = true;
  try {
    const content = await invoke<string | null>("read_lyrics_cache", { path });
    const lines = content ? normalize_lyrics(content) : [];
    if (request_id === lyrics_load_request_id) {
      lyrics_text.value = content ?? "";
      lyrics_lines.value = lines;
    }
    return lines.length > 0;
  } catch (error) {
    console.warn("读取歌词失败", error);
    if (request_id === lyrics_load_request_id) {
      lyrics_text.value = "";
      lyrics_lines.value = [];
    }
    return false;
  } finally {
    if (request_id === lyrics_load_request_id) {
      lyrics_loading.value = false;
    }
  }
}

async function open_lyrics_search() {
  lyrics_search_open.value = true;
  await search_current_lyrics();
}

async function search_current_lyrics() {
  const track = props.current_track;
  lyrics_search_error.value = "";
  lyrics_search_results.value = [];
  current_lyrics_hash.value = track?.lyrics_cache_hash?.trim() || null;

  if (!track) {
    lyrics_search_error.value = "当前没有正在播放的歌曲。";
    return;
  }
  const title = display_title(track);
  if (title === "未知歌曲") {
    lyrics_search_error.value = "歌曲名称为空，无法搜索歌词。";
    return;
  }

  const request_id = ++lyrics_search_request_id;
  lyrics_search_loading.value = true;
  try {
    const response = await search_lyrics_for_track(track);
    if (request_id === lyrics_search_request_id) {
      current_lyrics_hash.value = response.current_lyrics_hash ?? null;
      lyrics_search_results.value = response.results;
    }
  } catch (error) {
    if (request_id === lyrics_search_request_id) {
      lyrics_search_error.value = error instanceof Error ? error.message : String(error);
    }
  } finally {
    if (request_id === lyrics_search_request_id) {
      lyrics_search_loading.value = false;
    }
  }
}

async function search_lyrics_for_track(track: Track) {
  return await invoke<LyricsSearchResponse>("search_lyrics", {
    trackId: track.id,
    title: display_title(track),
    artist: display_artist(track),
    album: display_album(track),
    duration: track.duration ? Math.round(track.duration) : null,
    lyricsCachePath: track.lyrics_cache_path,
    lyricsCacheHash: track.lyrics_cache_hash,
  });
}

async function use_lyrics_result(result: LyricsSearchResult) {
  const track = props.current_track;
  if (!track) {
    lyrics_search_error.value = "当前没有正在播放的歌曲。";
    return;
  }

  lyrics_search_error.value = "";
  lyrics_use_pending_hash.value = result.lyrics_hash;
  try {
    await apply_lyrics_result(track, result);
  } catch (error) {
    lyrics_search_error.value = error instanceof Error ? error.message : String(error);
  } finally {
    lyrics_use_pending_hash.value = "";
  }
}

async function apply_lyrics_result(track: Track, result: LyricsSearchResult) {
  const lyrics = result.synced_lyrics || result.plain_lyrics || "";
  if (!lyrics.trim()) {
    throw new Error("此搜索结果没有可用歌词。");
  }

  const used = await invoke<LyricsUseResult>("use_lyrics_search_result", {
    trackId: track.id,
    lyricsCachePath: track.lyrics_cache_path,
    lyrics,
  });
  lyrics_text.value = used.lyrics;
  lyrics_lines.value = normalize_lyrics(used.lyrics);
  current_lyrics_hash.value = used.lyrics_hash;
  if (used.track) {
    playback_store.upsert_track(used.track);
    player_queue.upsert_track(used.track);
  }
}

function lyrics_result_name(result: LyricsSearchResult) {
  return `${result.track_name || "未知歌曲"} - ${result.artist_name || "未知歌手"}`;
}

function lyrics_result_meta(result: LyricsSearchResult) {
  const parts = [];
  if (result.album_name) parts.push(result.album_name);
  return parts.join(" / ");
}

function is_current_lyrics_result(result: LyricsSearchResult) {
  return Boolean(current_lyrics_hash.value && current_lyrics_hash.value === result.lyrics_hash);
}

function toggle_auto_lyrics() {
  auto_lyrics_enabled.value = !auto_lyrics_enabled.value;
  if (auto_lyrics_enabled.value) {
    void maybe_auto_load_lyrics(props.current_track, lyrics_lines.value.length > 0);
  }
}

function has_auto_attempted(track: Track) {
  return auto_lyrics_attempted_track_ids.value.has(track.id);
}

function set_auto_attempted(track: Track, attempted: boolean) {
  const next = new Set(auto_lyrics_attempted_track_ids.value);
  if (attempted) {
    next.add(track.id);
  } else {
    next.delete(track.id);
  }
  auto_lyrics_attempted_track_ids.value = next;
}

function select_auto_lyrics_result(results: LyricsSearchResult[]) {
  return (
    results.find((result) => {
      const source = result.source.toLowerCase();
      return result.source.includes("酷狗") || source.includes("kugou");
    }) ?? results[0]
  );
}

async function maybe_auto_load_lyrics(track?: Track | null, has_local_lyrics = lyrics_lines.value.length > 0) {
  if (!auto_lyrics_enabled.value || !track || has_local_lyrics || has_auto_attempted(track)) return;
  if (display_title(track) === "未知歌曲") return;

  const request_id = ++auto_lyrics_request_id;
  set_auto_attempted(track, true);
  notification.info("Auto 正在搜索歌词");

  try {
    const response = await search_lyrics_for_track(track);
    if (request_id !== auto_lyrics_request_id || props.current_track?.id !== track.id) {
      set_auto_attempted(track, false);
      return;
    }

    current_lyrics_hash.value = response.current_lyrics_hash ?? track.lyrics_cache_hash?.trim() ?? null;
    const result = select_auto_lyrics_result(response.results);
    if (!result) {
      notification.warning("Auto 未找到歌词");
      return;
    }

    await apply_lyrics_result(track, result);
    notification.success(`Auto 已加载${result.source ? `：${result.source}` : ""}`);
  } catch (error) {
    console.warn("自动获取歌词失败", error);
    if (request_id === auto_lyrics_request_id && props.current_track?.id === track.id) {
      notification.error("Auto 获取歌词失败");
    }
  }
}

watch(
  () => props.current_track?.id,
  async () => {
    const has_lyrics = await load_lyrics(props.current_track);
    await maybe_auto_load_lyrics(props.current_track, has_lyrics);
  },
  { immediate: true },
);

watch(
  () => auto_lyrics_enabled.value,
  (enabled) => {
    if (enabled) {
      void maybe_auto_load_lyrics(props.current_track, lyrics_lines.value.length > 0);
    }
  },
);

function close_on_escape(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  if (lyrics_search_open.value) {
    lyrics_search_open.value = false;
    return;
  }
  emit("close");
}

onMounted(() => {
  window.addEventListener("keydown", close_on_escape);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", close_on_escape);
});

function render_progress(percent: number, seconds: number) {
  player_bar.value?.render_progress(percent, seconds);
}

function toggle_compact_panel() {
  compact_panel.value = compact_panel.value === "record" ? "lyrics" : "record";
}

defineExpose({ render_progress });
</script>

<template>
  <section class="now_playing_page">
    <header class="now_playing_header" @mousedown="emit('start_window_drag', $event)">
      <button class="now_playing_back" type="button" title="返回" @click="emit('close')">
        <span />
      </button>
      <div class="now_playing_window_tools">
        <button class="lyrics_search_button" type="button" @click="open_lyrics_search">搜索歌词</button>
        <button
          class="lyrics_search_button lyrics_auto_button"
          :class="{ active: auto_lyrics_enabled }"
          type="button"
          :title="auto_lyrics_enabled ? '关闭自动获取歌词' : '开启自动获取歌词'"
          @click="toggle_auto_lyrics"
        >
          Auto
        </button>
        <button class="window_button hover_border_control" type="button" title="最小化" @click="emit('minimize_window')">
          <span class="svg_icon" :style="icon_style(minimize_icon)" />
        </button>
        <button
          class="window_button hover_border_control"
          type="button"
          title="最大化"
          @click="emit('toggle_maximize_window')"
        >
          <span class="svg_icon" :style="icon_style(maximize_icon)" />
        </button>
        <button class="window_button close hover_border_control" type="button" title="关闭" @click="emit('close_window')">
          <span class="svg_icon" :style="icon_style(x_icon)" />
        </button>
      </div>
    </header>

    <main class="now_playing_content">
      <button
        class="compact_switch compact_switch_record"
        :class="{ active: compact_panel === 'record' }"
        type="button"
        :title="compact_panel === 'record' ? '切换到歌词' : '切换到唱片'"
        @click="toggle_compact_panel"
      >
        <span class="svg_icon" :style="icon_style(compact_left_icon)" />
      </button>

      <section class="record_stage" :class="{ compact_active: compact_panel === 'record' }" aria-label="歌曲封面">
        <div class="tonearm" :class="{ tonearm_playing: status.playing && current_track }">
          <img :src="tonearm_icon" alt="" />
        </div>
        <div
          class="record_disc"
          :class="{
            now_playing_record_spin: current_track,
            now_playing_record_spin_running: status.playing && current_track,
          }"
        >
          <div class="record_grooves" />
          <div class="record_label">
            <img v-if="current_track?.cover_cache_path" :src="cover_src(current_track)" alt="" />
            <span v-else>♪</span>
          </div>
        </div>
      </section>

      <section class="now_playing_info" :class="{ compact_active: compact_panel === 'lyrics' }">
        <div class="track_identity">
          <h1>{{ display_title(current_track) }}</h1>
          <p>
            <span>专辑：{{ display_album(current_track) }}</span>
            <span>歌手：{{ display_artist(current_track) }}</span>
          </p>
        </div>

        <LineLyricsRenderer
          :lyrics="lyrics_text"
          :elapsed="visual_elapsed"
          :loading="lyrics_loading"
          :placeholder="lyric_placeholder"
          :seeking="progress_dragging"
        />
      </section>

      <button
        class="compact_switch compact_switch_lyrics"
        :class="{ active: compact_panel === 'lyrics' }"
        type="button"
        :title="compact_panel === 'record' ? '切换到歌词' : '切换到唱片'"
        @click="toggle_compact_panel"
      >
        <span class="svg_icon" :style="icon_style(compact_right_icon)" />
      </button>
    </main>

    <div v-if="lyrics_search_open" class="lyrics_search_overlay" @click.self="lyrics_search_open = false">
      <section class="lyrics_search_dialog" role="dialog" aria-modal="true" aria-label="搜索歌词">
        <header>
          <strong>搜索歌词</strong>
          <span class="lyrics_search_duration">{{ format_duration(current_track?.duration) }}</span>
          <button type="button" title="关闭" @click="lyrics_search_open = false">×</button>
        </header>
        <div class="lyrics_result_head">
          <span>歌词来源</span>
          <span>歌词名称</span>
          <span>操作</span>
        </div>
        <div class="lyrics_result_body">
          <p v-if="lyrics_search_loading" class="lyrics_result_state">正在搜索歌词...</p>
          <p v-else-if="lyrics_search_error" class="lyrics_result_state error">{{ lyrics_search_error }}</p>
          <p v-else-if="!lyrics_search_results.length" class="lyrics_result_state">暂无搜索结果</p>
          <template v-else>
            <div
              v-for="result in lyrics_search_results"
              :key="`${result.source}-${result.id}`"
              class="lyrics_result_row"
            >
              <span>{{ result.source }}</span>
              <strong :title="lyrics_result_meta(result)">
                <span>{{ lyrics_result_name(result) }}</span>
                <small>{{ lyrics_result_meta(result) }}</small>
              </strong>
              <button
                type="button"
                :class="{ current: is_current_lyrics_result(result) }"
                :disabled="is_current_lyrics_result(result) || lyrics_use_pending_hash === result.lyrics_hash"
                @click="use_lyrics_result(result)"
              >
                {{
                  is_current_lyrics_result(result)
                    ? "已使用"
                    : lyrics_use_pending_hash === result.lyrics_hash
                      ? "使用中"
                      : "使用"
                }}
              </button>
            </div>
          </template>
        </div>
      </section>
    </div>

    <PlayerBar
      ref="player_bar"
      :current_track="current_track"
      :status="status"
      :progress_dragging="progress_dragging"
      :playback_mode_button="playback_mode_button"
      :show_cover="false"
      @begin_progress_drag="emit('begin_progress_drag', $event)"
      @drag_progress="emit('drag_progress', $event)"
      @end_progress_drag="emit('end_progress_drag', $event)"
      @cancel_progress_drag="emit('cancel_progress_drag', $event)"
      @previous_track="emit('previous_track')"
      @toggle_playback="emit('toggle_playback')"
      @next_track="emit('next_track')"
      @open_queue="emit('open_queue')"
      @cycle_playback_mode="emit('cycle_playback_mode')"
      @change_volume="emit('change_volume', $event)"
    />
  </section>
</template>

<style>
.now_playing_page {
  position: fixed;
  inset: 0;
  z-index: 850;
  display: grid;
  grid-template-rows: 92px minmax(0, 1fr) 86px;
  overflow: hidden;
  color: #f5f6f8;
  background:
    radial-gradient(circle at 24% 34%, rgba(255, 255, 255, 0.08), transparent 30%),
    linear-gradient(135deg, rgba(24, 25, 28, 0.98), rgba(16, 17, 19, 0.98));
  user-select: none;
}

.now_playing_header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 52px;
  cursor: move;
}

.now_playing_back {
  display: grid;
  width: 44px;
  height: 44px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 0;
  color: #f5f6f8;
  background: transparent;
  cursor: pointer;
}

.now_playing_back:hover,
.now_playing_back:focus-visible {
  border-color: #ffffff;
  color: #ffffff;
}

.now_playing_back span {
  display: block;
  width: 18px;
  height: 18px;
  border-right: 3px solid currentColor;
  border-bottom: 3px solid currentColor;
  transform: translateY(-5px) rotate(45deg);
  transform-origin: center;
}

.now_playing_window_tools {
  display: flex;
  align-items: center;
  gap: 14px;
  color: rgba(245, 246, 248, 0.64);
  font-weight: 800;
  cursor: default;
}

.now_playing_window_tools .window_button {
  color: rgba(245, 246, 248, 0.72);
}

.now_playing_page .hover_border_control:hover,
.now_playing_page .hover_border_control:focus-visible {
  border-color: #ffffff;
  border-radius: 6px;
  color: #ffffff;
}

.lyrics_search_button {
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 5px 12px;
  color: rgba(245, 246, 248, 0.72);
  background: transparent;
  font-size: 0.96rem;
  font-weight: 900;
  cursor: pointer;
}

.lyrics_search_button:hover,
.lyrics_search_button:focus-visible {
  border-color: #ffffff;
  color: #ffffff;
}

.lyrics_search_button.active {
  border-color: rgba(38, 201, 107, 0.62);
  color: #26c96b;
}

.now_playing_content {
  display: grid;
  grid-template-columns: minmax(360px, 0.94fr) minmax(420px, 1.06fr);
  align-items: center;
  gap: clamp(42px, 7vw, 120px);
  min-width: 0;
  min-height: 0;
  padding: 0 clamp(58px, 9vw, 150px) 20px;
}

.compact_switch {
  display: none;
}

.record_stage {
  align-self: center;
  justify-self: center;
  display: grid;
  grid-template-areas: "turntable";
  place-items: center;
  width: min(28vw, 430px);
  min-width: 320px;
  aspect-ratio: 0.76;
  min-height: 0;
  transform: translateY(-10%);
}

.record_disc {
  grid-area: turntable;
  align-self: end;
  justify-self: center;
  display: grid;
  width: min(90%, 395px);
  min-width: 270px;
  aspect-ratio: 1;
  place-items: center;
  border-radius: 50%;
  background:
    radial-gradient(circle, transparent 0 26%, rgba(0, 0, 0, 0.82) 27% 100%),
    repeating-radial-gradient(circle, #1b1c1d 0 2px, #101112 3px 5px);
  box-shadow:
    0 0 0 12px rgba(255, 255, 255, 0.04),
    0 0 0 15px rgba(0, 0, 0, 0.18),
    0 28px 70px rgba(0, 0, 0, 0.38),
    inset 0 0 0 8px rgba(255, 255, 255, 0.024),
    inset 0 0 0 14px rgba(0, 0, 0, 0.32);
}

.now_playing_record_spin {
  animation: cover_spin 16s linear infinite;
  animation-play-state: paused;
  will-change: transform;
}

.now_playing_record_spin_running {
  animation-play-state: running;
}

.record_grooves {
  grid-area: 1 / 1;
  width: 94%;
  aspect-ratio: 1;
  place-self: center;
  border-radius: 50%;
  background: repeating-radial-gradient(circle, transparent 0 5px, rgba(255, 255, 255, 0.035) 6px 7px);
}

.record_label {
  grid-area: 1 / 1;
  z-index: 1;
  display: grid;
  overflow: hidden;
  width: 64%;
  aspect-ratio: 1;
  place-items: center;
  border: 7px solid rgba(0, 0, 0, 0.24);
  border-radius: 50%;
  color: #ffffff;
  background:
    linear-gradient(145deg, #252a32, var(--theme-highlight-color, #426dff)),
    #252a32;
  font-size: 4rem;
  font-weight: 900;
}

.record_label img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.tonearm {
  grid-area: turntable;
  align-self: start;
  justify-self: center;
  z-index: 2;
  --tonearm-x: 12%;
  --tonearm-y: -8%;
  --tonearm-rest-angle: -10deg;
  --tonearm-play-angle: 30deg;
  width: min(72%, 310px);
  aspect-ratio: 520 / 300;
  pointer-events: none;
  transform: translate(var(--tonearm-x), var(--tonearm-y)) rotate(var(--tonearm-rest-angle));
  transform-origin: 14.23% 25.33%;
  transition:
    transform 520ms cubic-bezier(0.2, 0.9, 0.28, 1),
    filter 420ms ease;
  filter: drop-shadow(0 16px 18px rgba(0, 0, 0, 0.28));
}

.tonearm_playing {
  transform: translate(var(--tonearm-x), var(--tonearm-y)) rotate(var(--tonearm-play-angle));
  filter: drop-shadow(0 7px 9px rgba(0, 0, 0, 0.34));
}

.tonearm img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.now_playing_info {
  display: grid;
  align-content: center;
  gap: 28px;
  min-width: 0;
  min-height: 0;
}

.track_identity {
  display: grid;
  gap: 14px;
  text-align: center;
}

.track_identity h1 {
  overflow: hidden;
  margin: 0;
  color: #ffffff;
  font-size: clamp(2rem, 3vw, 3.4rem);
  font-weight: 900;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_identity p {
  display: flex;
  justify-content: center;
  gap: 28px;
  min-width: 0;
  color: rgba(245, 246, 248, 0.58);
  font-size: 1.02rem;
  font-weight: 800;
}

.track_identity p span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lyrics_search_overlay {
  position: fixed;
  inset: 0;
  z-index: 880;
  display: grid;
  place-items: center;
  padding: 28px;
  background: rgba(10, 11, 13, 0.48);
}

.lyrics_search_dialog {
  display: grid;
  width: min(560px, 100%);
  max-height: min(480px, calc(100vh - 56px));
  overflow: hidden;
  border: 1px solid rgba(245, 246, 248, 0.14);
  border-radius: 6px;
  background: rgba(28, 30, 34, 0.96);
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.38);
}

.lyrics_search_dialog header,
.lyrics_result_head,
.lyrics_result_row {
  display: grid;
  grid-template-columns: 1fr minmax(0, 1.4fr) 90px;
  align-items: center;
  gap: 14px;
}

.lyrics_search_dialog header {
  grid-template-columns: minmax(0, 1fr) auto 36px;
  padding: 18px 20px 12px;
}

.lyrics_search_dialog header strong {
  overflow: hidden;
  color: #ffffff;
  font-size: 1.16rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lyrics_search_duration {
  justify-self: center;
  color: rgba(245, 246, 248, 0.64);
  font-size: 0.95rem;
  font-weight: 900;
  white-space: nowrap;
}

.lyrics_search_dialog header button {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 6px;
  color: rgba(245, 246, 248, 0.72);
  background: transparent;
  font-size: 1.35rem;
  line-height: 1;
  cursor: pointer;
}

.lyrics_search_dialog header button:hover {
  border-color: #ffffff;
  color: #ffffff;
}

.lyrics_result_head {
  padding: 12px 20px;
  color: rgba(245, 246, 248, 0.48);
  font-size: 0.88rem;
  font-weight: 900;
}
.lyrics_result_head span:nth-child(3) {
  text-align: center;
}

.lyrics_result_body {
  min-height: 120px;
  overflow-y: auto;
  padding: 0 14px 16px;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.lyrics_result_body::-webkit-scrollbar {
  display: none;
}

.lyrics_result_state {
  display: grid;
  min-height: 116px;
  margin: 0;
  place-items: center;
  color: rgba(245, 246, 248, 0.56);
  font-weight: 900;
}

.lyrics_result_state.error {
  color: #ffb4b4;
}

.lyrics_result_row {
  margin: 0 0 8px;
  border-radius: 6px;
  padding: 12px 6px;
  color: rgba(245, 246, 248, 0.74);
  font-weight: 800;
}

.lyrics_result_row strong {
  display: grid;
  gap: 4px;
  overflow: hidden;
  color: #ffffff;
}

.lyrics_result_row strong span,
.lyrics_result_row strong small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lyrics_result_row strong small {
  color: rgba(245, 246, 248, 0.46);
  font-size: 0.78rem;
  font-weight: 800;
}

.lyrics_result_row button {
  min-height: 32px;
  border: 1px solid rgba(245, 246, 248, 0.22);
  border-radius: 6px;
  color: rgba(245, 246, 248, 0.54);
  background: transparent;
  font-weight: 900;
  cursor: pointer;
}

.lyrics_result_row button:disabled {
  cursor: default;
}

.lyrics_result_row button.current {
  border-color: rgba(38, 201, 107, 0.62);
  color: #26c96b;
}

.now_playing_page .player_bar {
  grid-area: auto;
  grid-row: 3;
  background: rgba(18, 19, 21, 0.84);
}

.now_playing_page .now_text strong {
  color: #ffffff;
}

.now_playing_page .now_text small,
.now_playing_page .player_tools,
.now_playing_page .volume_icon {
  color: rgba(245, 246, 248, 0.64);
}

.now_playing_page .control_row button,
.now_playing_page .control_row .play_button,
.now_playing_page .player_tools button {
  color: rgba(245, 246, 248, 0.78);
}

.now_playing_page .player_tools input {
  accent-color: rgba(245, 246, 248, 0.78);
}

@media (max-width: 1120px) {
  .now_playing_content {
    grid-template-columns: 46px minmax(0, 1fr) 46px;
    grid-template-areas: "record_switch compact_panel lyrics_switch";
    align-content: center;
    align-items: center;
    gap: 12px;
    overflow: hidden;
    padding: 0 18px 24px;
  }

  .compact_switch {
    display: grid;
    width: 38px;
    min-width: 0;
    height: 76px;
    place-items: center;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 0;
    color: rgba(245, 246, 248, 0.72);
    background: transparent;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 180ms ease,
      border-color 180ms ease,
      color 180ms ease;
    cursor: pointer;
  }

  .compact_switch .svg_icon {
    width: 22px;
    height: 22px;
  }

  .now_playing_content:hover .compact_switch,
  .compact_switch:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  .compact_switch.active {
    color: #ffffff;
  }

  .compact_switch:hover,
  .compact_switch:focus-visible {
    border-color: #ffffff;
    border-radius: 6px;
  }

  .compact_switch_record {
    grid-area: record_switch;
  }

  .compact_switch_lyrics {
    grid-area: lyrics_switch;
  }

  .record_stage,
  .now_playing_info {
    display: none;
    grid-area: compact_panel;
    width: 100%;
    min-width: 0;
  }

  .record_stage.compact_active,
  .now_playing_info.compact_active {
    display: grid;
  }

  .record_stage {
    justify-self: center;
    width: min(72vw, 350px);
  }

  .record_disc {
    width: min(60vw, 310px);
  }

  .tonearm {
    width: min(76%, 260px);
  }

  .track_identity p {
    flex-wrap: wrap;
    gap: 10px 18px;
  }
}
</style>
