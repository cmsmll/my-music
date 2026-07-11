<script setup lang="ts">
import { nextTick, onActivated, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import lyrics_copy_icon from "../assets/icons/lyrics-copy.svg";
import next_icon from "../assets/icons/next.svg";
import pause_icon from "../assets/icons/pause.svg";
import play_icon from "../assets/icons/play.svg";
import playlist_icon from "../assets/icons/playlist.svg";
import previous_icon from "../assets/icons/previous.svg";
import volume_icon from "../assets/icons/volume.svg";
import { use_playback_store } from "../stores/playback";
import { use_app_actions_store } from "../stores/app_actions";
import { use_ui_store } from "../stores/ui";
import type { PlaybackModeItem } from "../types/music";
import { cover_src, display_artist, display_title, format_duration, icon_style } from "../utils/track";

withDefaults(defineProps<{
  playback_mode_button: PlaybackModeItem;
  show_cover?: boolean;
}>(), {
  show_cover: true,
});

const progress_fill_element = ref<HTMLElement | null>(null);
const progress_handle_element = ref<HTMLElement | null>(null);
const progress_tooltip_element = ref<HTMLElement | null>(null);
const progress_tooltip_visible = ref(false);
const playback_store = use_playback_store();
const app_actions = use_app_actions_store();
const ui_store = use_ui_store();
const { current_track, status, progress_dragging, progress_percent, visual_elapsed } = storeToRefs(playback_store);

function render_progress(percent: number, _seconds: number) {
  const safe_percent = Math.min(Math.max(percent, 0), 100);
  if (progress_fill_element.value) {
    progress_fill_element.value.style.transform = `scaleX(${safe_percent / 100})`;
  }
  if (progress_handle_element.value) {
    progress_handle_element.value.style.left = `${safe_percent}%`;
  }
}

async function sync_current_progress() {
  await nextTick();
  render_progress(playback_store.progress_percent, playback_store.visual_elapsed);
}

function render_progress_tooltip(event: PointerEvent) {
  if (!progress_tooltip_element.value) return;

  const duration = current_track.value?.duration ?? 0;
  if (duration <= 0) {
    progress_tooltip_element.value.style.left = "0%";
    progress_tooltip_element.value.textContent = "0:00";
    return;
  }

  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
  progress_tooltip_element.value.style.left = `${ratio * 100}%`;
  progress_tooltip_element.value.textContent = format_duration(Math.round(duration * ratio));
}

function show_progress_tooltip(event: PointerEvent) {
  progress_tooltip_visible.value = true;
  render_progress_tooltip(event);
}

function hide_progress_tooltip() {
  progress_tooltip_visible.value = false;
}

function begin_progress_interaction(event: PointerEvent) {
  show_progress_tooltip(event);
  app_actions.begin_progress_drag(event);
}

function move_progress_interaction(event: PointerEvent) {
  show_progress_tooltip(event);
  app_actions.drag_progress(event);
}

function end_progress_interaction(event: PointerEvent) {
  show_progress_tooltip(event);
  app_actions.end_progress_drag(event);
}

function cancel_progress_interaction(event: PointerEvent) {
  hide_progress_tooltip();
  app_actions.cancel_progress_drag(event);
}

onActivated(() => {
  void sync_current_progress();
});

watch([progress_percent, visual_elapsed], () => {
  render_progress(progress_percent.value, visual_elapsed.value);
});
</script>

<template>
  <footer class="player_bar">
    <div
      class="player_progress"
      :class="{ dragging: progress_dragging, tooltip_visible: progress_tooltip_visible }"
      role="slider"
      :aria-valuemin="0"
      :aria-valuemax="current_track?.duration ?? 0"
      :aria-valuenow="status.elapsed"
      aria-label="播放进度"
      @pointerdown="begin_progress_interaction"
      @pointermove="move_progress_interaction"
      @pointerup="end_progress_interaction"
      @pointercancel="cancel_progress_interaction"
      @pointerenter="show_progress_tooltip"
      @pointerleave="hide_progress_tooltip"
    >
      <div class="progress_bar_container">
        <div class="progress_track">
          <div ref="progress_fill_element" class="progress_fill" />
          <div ref="progress_handle_element" class="progress_handle" />
          <div ref="progress_tooltip_element" class="progress_tooltip">0:00</div>
        </div>
      </div>
    </div>

    <div class="now_track">
      <button
        v-if="show_cover"
        class="player_cover_button"
        type="button"
        title="打开播放页"
        :disabled="!current_track"
        @click="ui_store.open_now_playing()"
      >
        <span class="player_cover" :class="{ spinning_cover: status.playing && current_track }">
          <img v-if="current_track?.cover_cache_path" :src="cover_src(current_track)" alt="" />
          <span v-else>♪</span>
        </span>
      </button>
      <span class="now_text">
        <strong>{{ display_title(current_track) }}</strong>
        <small>{{ display_artist(current_track) }}</small>
      </span>
    </div>

    <div class="player_center">
      <div class="control_row">
        <button class="hover_border_control" type="button" title="上一首" @click="app_actions.previous_track()">
          <span class="svg_icon" :style="icon_style(previous_icon)" />
        </button>
        <button class="play_button hover_border_control" type="button" title="播放或暂停" @click="app_actions.toggle_playback()">
          <span class="svg_icon" :style="icon_style(status.playing ? pause_icon : play_icon)" />
        </button>
        <button class="hover_border_control" type="button" title="下一首" @click="app_actions.next_track()">
          <span class="svg_icon" :style="icon_style(next_icon)" />
        </button>
      </div>
    </div>

    <div class="player_tools">
      <button class="hover_border_control" type="button" title="播放队列" @click="ui_store.open_playback_queue()">
        <span class="svg_icon" :style="icon_style(playlist_icon)" />
      </button>
      <button
        class="hover_border_control"
        type="button"
        :title="playback_mode_button.label"
        :aria-label="playback_mode_button.label"
        @click="app_actions.cycle_playback_mode()"
      >
        <span class="svg_icon" :style="icon_style(playback_mode_button.icon)" />
      </button>
      <button class="hover_border_control" type="button" title="桌面歌词" @click="app_actions.toggle_desktop_lyrics()">
        <span class="svg_icon" :style="icon_style(lyrics_copy_icon)" />
      </button>
      <span class="volume_icon svg_icon" :style="icon_style(volume_icon)" />
      <input type="range" min="0" max="1.5" step="0.01" :value="status.volume" @input="app_actions.change_volume($event)" />
    </div>
  </footer>
</template>

<style>
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
  background: transparent;
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
  background: var(--theme-border-color, #e8e8e8);
}

.progress_fill {
  width: 100%;
  height: 100%;
  border-radius: 15px;
  background: var(--theme-highlight-color, #426dff);
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

.player_progress.tooltip_visible .progress_tooltip,
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
  display: grid;
  place-items: center;
  overflow: hidden;
  flex: 0 0 auto;
  width: 62px;
  height: 62px;
  border-radius: 8px;
  color: #ffffff;
  font-size: 1.8rem;
  font-weight: 900;
}

.player_cover_button {
  display: grid;
  width: 62px;
  height: 62px;
  place-items: center;
  border: 0;
  padding: 0;
  color: inherit;
  background: transparent;
  cursor: pointer;
}

.player_cover_button:disabled {
  cursor: default;
}

.player_cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.now_text {
  display: grid;
  min-width: 0;
  gap: 4px;
}

.now_text strong,
.now_text small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.now_text strong {
  font-size: 1.05rem;
}

.now_text small {
  color: var(--theme-subtitle-color, #8b919c);
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
  color: var(--theme-control-color, #1e2026);
  background: transparent;
  font-size: 1.35rem;
}

.control_row .play_button .svg_icon {
  width: 19px;
  height: 19px;
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
  color: var(--theme-control-color, #1e2026);
  width: 19px;
  height: 19px;
}

.player_tools input {
  width: 122px;
  accent-color: var(--theme-control-color, #1e2026);
  cursor: pointer;
}

.player_tools input::-webkit-slider-thumb {
  cursor: pointer;
}

.player_tools input::-moz-range-thumb {
  cursor: pointer;
}
</style>
