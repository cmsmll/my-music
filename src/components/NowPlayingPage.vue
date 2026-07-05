<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import maximize_icon from "../assets/icons/maximize.svg";
import minimize_icon from "../assets/icons/minimize.svg";
import x_icon from "../assets/icons/x.svg";
import PlayerBar from "./PlayerBar.vue";
import type { PlaybackModeItem, PlaybackStatus, Track } from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration, icon_style } from "../utils/track";

type PlayerBarExpose = {
  render_progress: (percent: number, seconds: number) => void;
};

const props = defineProps<{
  current_track?: Track | null;
  status: PlaybackStatus;
  progress_dragging: boolean;
  playback_mode_button: PlaybackModeItem;
}>();

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
const lyrics_lines = ref<string[]>([]);

const lyric_placeholder = [
  "暂未获取到歌词",
  "可以重新加载曲库，或检查音频文件是否包含内嵌歌词。",
];

const display_lyrics = computed(() =>
  lyrics_lines.value.length ? lyrics_lines.value : lyric_placeholder,
);

function normalize_lyrics(content: string) {
  return content
    .split(/\r?\n/)
    .map((line) => line.replace(/^(\[[^\]]+\])+\s*/u, "").trim())
    .filter(Boolean);
}

async function load_lyrics(track?: Track | null) {
  lyrics_lines.value = [];
  const path = track?.lyrics_cache_path?.trim();
  if (!path) return;

  lyrics_loading.value = true;
  try {
    const content = await invoke<string | null>("read_lyrics_cache", { path });
    lyrics_lines.value = content ? normalize_lyrics(content) : [];
  } catch (error) {
    console.warn("读取歌词失败", error);
    lyrics_lines.value = [];
  } finally {
    lyrics_loading.value = false;
  }
}

watch(
  () => props.current_track?.id,
  () => {
    void load_lyrics(props.current_track);
  },
  { immediate: true },
);

function close_on_escape(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
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

defineExpose({ render_progress });
</script>

<template>
  <section class="now_playing_page">
    <header class="now_playing_header" @mousedown="emit('start_window_drag', $event)">
      <button class="now_playing_back" type="button" title="返回" @click="emit('close')">
        <span />
      </button>
      <div class="now_playing_window_tools">
        <span>播放器模式</span>
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
      <section class="record_stage" aria-label="歌曲封面">
        <div class="tonearm" :class="{ tonearm_playing: status.playing && current_track }">
          <span class="tonearm_head" />
          <span class="tonearm_body" />
          <span class="tonearm_needle" />
        </div>
        <div class="record_disc" :class="{ spinning_cover: status.playing && current_track }">
          <div class="record_grooves" />
          <div class="record_label">
            <img v-if="current_track?.cover_cache_path" :src="cover_src(current_track)" alt="" />
            <span v-else>♪</span>
          </div>
        </div>
      </section>

      <section class="now_playing_info">
        <div class="track_identity">
          <h1>{{ display_title(current_track) }}</h1>
          <p>
            <span>专辑：{{ display_album(current_track) }}</span>
            <span>歌手：{{ display_artist(current_track) }}</span>
            <span>时长：{{ format_duration(current_track?.duration) }}</span>
          </p>
        </div>

        <div class="info_tabs" aria-label="播放信息">
          <button class="active" type="button">歌词</button>
          <button type="button" disabled>百科</button>
          <button type="button" disabled>相似推荐</button>
        </div>

        <div class="lyrics_panel">
          <p v-if="lyrics_loading" class="lyrics_hint">正在读取歌词...</p>
          <p
            v-for="(line, index) in display_lyrics"
            v-else
            :key="`${line}-${index}`"
            :class="{ lyrics_hint: !lyrics_lines.length }"
          >
            {{ line }}
          </p>
        </div>
      </section>
    </main>

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
  border: 0;
  padding: 0;
  color: #f5f6f8;
  background: transparent;
  cursor: pointer;
}

.now_playing_back span {
  width: 18px;
  height: 18px;
  border-right: 3px solid currentColor;
  border-bottom: 3px solid currentColor;
  transform: rotate(45deg) translateY(-3px);
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

.now_playing_content {
  display: grid;
  grid-template-columns: minmax(360px, 0.94fr) minmax(420px, 1.06fr);
  align-items: center;
  gap: clamp(42px, 7vw, 120px);
  min-width: 0;
  min-height: 0;
  padding: 0 clamp(58px, 9vw, 150px) 20px;
}

.record_stage {
  position: relative;
  display: grid;
  justify-items: center;
  align-items: center;
  min-width: 0;
  min-height: 0;
}

.record_disc {
  position: relative;
  display: grid;
  width: min(36vw, 470px);
  min-width: 300px;
  aspect-ratio: 1;
  place-items: center;
  border-radius: 50%;
  background:
    radial-gradient(circle, transparent 0 27%, rgba(0, 0, 0, 0.72) 28% 100%),
    repeating-radial-gradient(circle, #1f2022 0 3px, #0e0f11 4px 7px);
  box-shadow:
    0 28px 70px rgba(0, 0, 0, 0.38),
    inset 0 0 0 16px rgba(255, 255, 255, 0.03),
    inset 0 0 0 24px rgba(0, 0, 0, 0.28);
}

.record_grooves {
  position: absolute;
  inset: 6%;
  border-radius: 50%;
  background: repeating-radial-gradient(circle, transparent 0 5px, rgba(255, 255, 255, 0.035) 6px 7px);
}

.record_label {
  position: relative;
  z-index: 1;
  display: grid;
  overflow: hidden;
  width: 52%;
  aspect-ratio: 1;
  place-items: center;
  border: 10px solid rgba(0, 0, 0, 0.24);
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
  position: absolute;
  top: 4%;
  right: 16%;
  z-index: 2;
  width: 230px;
  height: 142px;
  pointer-events: none;
  transform: translateY(-13px) rotate(-9deg);
  transform-origin: 32px 30px;
  transition:
    transform 420ms cubic-bezier(0.2, 0.9, 0.28, 1),
    filter 420ms ease;
  filter: drop-shadow(0 16px 18px rgba(0, 0, 0, 0.28));
}

.tonearm_playing {
  transform: translateY(0) rotate(13deg);
  filter: drop-shadow(0 7px 9px rgba(0, 0, 0, 0.34));
}

.tonearm_head {
  position: absolute;
  top: 0;
  left: 0;
  width: 28px;
  height: 28px;
  border: 8px solid #f4f5f7;
  border-radius: 50%;
  box-shadow: 0 0 0 10px rgba(255, 255, 255, 0.04);
}

.tonearm_body {
  position: absolute;
  top: 22px;
  left: 22px;
  width: 190px;
  height: 86px;
  border-top: 10px solid #f4f5f7;
  border-right: 10px solid #f4f5f7;
  border-radius: 0 0 32px 0;
  transform: skewX(22deg);
  transition: border-color 240ms ease;
}

.tonearm_body::after {
  position: absolute;
  right: -18px;
  bottom: -18px;
  width: 32px;
  height: 22px;
  border-radius: 5px;
  background: #f4f5f7;
  content: "";
}

.tonearm_needle {
  position: absolute;
  right: -2px;
  bottom: 14px;
  width: 9px;
  height: 38px;
  border-radius: 6px;
  background: linear-gradient(180deg, #f4f5f7 0 62%, #26282d 63% 100%);
  transform: rotate(-22deg) translateY(-12px);
  transform-origin: 50% 0;
  transition: transform 420ms cubic-bezier(0.2, 0.9, 0.28, 1);
}

.tonearm_playing .tonearm_needle {
  transform: rotate(-22deg) translateY(0);
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

.info_tabs {
  display: flex;
  width: max-content;
  max-width: 100%;
  gap: 4px;
  border-radius: 22px;
  padding: 4px;
  background: rgba(255, 255, 255, 0.08);
}

.info_tabs button {
  border: 0;
  border-radius: 18px;
  padding: 5px 14px;
  color: rgba(245, 246, 248, 0.68);
  background: transparent;
  font-size: 0.95rem;
  font-weight: 900;
}

.info_tabs button.active {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.16);
}

.info_tabs button:disabled {
  cursor: default;
}

.lyrics_panel {
  display: grid;
  align-content: start;
  gap: 18px;
  height: min(44vh, 430px);
  overflow-y: auto;
  color: rgba(245, 246, 248, 0.48);
  font-size: clamp(1.2rem, 1.6vw, 1.7rem);
  font-weight: 900;
  line-height: 1.55;
  scrollbar-width: none;
}

.lyrics_panel::-webkit-scrollbar {
  display: none;
}

.lyrics_panel p {
  margin: 0;
}

.lyrics_panel .lyrics_hint {
  color: rgba(245, 246, 248, 0.36);
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

@media (max-width: 980px) {
  .now_playing_content {
    grid-template-columns: 1fr;
    align-content: start;
    overflow-y: auto;
    padding: 0 32px 24px;
  }

  .record_disc {
    width: min(68vw, 360px);
  }

  .tonearm {
    right: 22%;
    scale: 0.72;
  }

  .track_identity p {
    flex-wrap: wrap;
    gap: 10px 18px;
  }
}
</style>
