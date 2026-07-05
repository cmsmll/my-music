<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import maximize_icon from "../assets/icons/maximize.svg";
import minimize_icon from "../assets/icons/minimize.svg";
import tonearm_icon from "../assets/tonearm-minimal-white.svg";
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
const compact_panel = ref<"record" | "lyrics">("record");

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
      <button
        class="compact_switch compact_switch_record"
        :class="{ active: compact_panel === 'record' }"
        type="button"
        title="唱片"
        @click="compact_panel = 'record'"
      >
        唱片
      </button>

      <section class="record_stage" :class="{ compact_active: compact_panel === 'record' }" aria-label="歌曲封面">
        <div class="tonearm" :class="{ tonearm_playing: status.playing && current_track }">
          <img :src="tonearm_icon" alt="" />
        </div>
        <div class="record_disc" :class="{ spinning_cover: status.playing && current_track }">
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

      <button
        class="compact_switch compact_switch_lyrics"
        :class="{ active: compact_panel === 'lyrics' }"
        type="button"
        title="歌词"
        @click="compact_panel = 'lyrics'"
      >
        歌词
      </button>
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
    border: 1px solid rgba(245, 246, 248, 0.24);
    border-radius: 20px;
    padding: 0;
    color: rgba(245, 246, 248, 0.72);
    background: rgba(255, 255, 255, 0.07);
    font-size: 0.82rem;
    font-weight: 900;
    line-height: 1;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 180ms ease,
      border-color 180ms ease,
      color 180ms ease;
    writing-mode: vertical-rl;
    cursor: pointer;
  }

  .now_playing_content:hover .compact_switch,
  .compact_switch:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  .compact_switch.active {
    border-color: rgba(245, 246, 248, 0.72);
    color: #ffffff;
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
