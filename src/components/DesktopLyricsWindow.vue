<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import next_icon from "../assets/icons/next.svg";
import pause_icon from "../assets/icons/pause.svg";
import play_icon from "../assets/icons/play.svg";
import previous_icon from "../assets/icons/previous.svg";
import random_icon from "../assets/icons/disorder.svg";
import repeat_one_icon from "../assets/icons/repeat-one.svg";
import repeat_icon from "../assets/icons/repeat.svg";
import rotate_icon from "../assets/icons/refresh.svg";
import shuffle_icon from "../assets/icons/shuffle.svg";
import type { DesktopLyricsAction, DesktopLyricsState, PlaybackMode } from "../types/music";
import { has_timed_lyric_lines, lyric_index_for_elapsed, parse_lyric_lines } from "../utils/lyrics";
import { icon_style } from "../utils/track";

const desktop_lyrics_orientation_key = "my_music_desktop_lyrics_vertical";

const playback_mode_icons: Record<PlaybackMode, string> = {
  shuffle: shuffle_icon,
  random: random_icon,
  repeat: repeat_icon,
  repeat_one: repeat_one_icon,
};

const playback_mode_titles: Record<PlaybackMode, string> = {
  shuffle: "随机列表",
  random: "随机播放",
  repeat: "循环播放",
  repeat_one: "单曲循环",
};

const current_window = getCurrentWindow();
const state = ref<DesktopLyricsState>({
  track: null,
  elapsed: 0,
  playing: false,
  playback_mode: "repeat",
  theme: {
    title_color: "#f5f6f8",
    subtitle_color: "rgba(245, 246, 248, 0.58)",
    highlight_color: "#3bce82",
  },
});
const lyrics_text = ref("");
const lyrics_loading = ref(false);
const locked = ref(false);
const vertical = ref(localStorage.getItem(desktop_lyrics_orientation_key) === "1");
let lyrics_request_id = 0;
let unlisteners: UnlistenFn[] = [];

const lyric_lines = computed(() => parse_lyric_lines(lyrics_text.value));
const has_timed_lyrics = computed(() => has_timed_lyric_lines(lyric_lines.value));
const active_index = computed(() => lyric_index_for_elapsed(lyric_lines.value, state.value.elapsed));
const theme_style = computed(() => ({
  "--desktop-title-color": state.value.theme.title_color,
  "--desktop-subtitle-color": state.value.theme.subtitle_color,
  "--desktop-highlight-color": state.value.theme.highlight_color,
}));
const current_mode_icon = computed(() => playback_mode_icons[state.value.playback_mode]);
const current_mode_title = computed(() => playback_mode_titles[state.value.playback_mode]);

const display_lines = computed(() => {
  if (lyrics_loading.value) {
    return [{ key: "loading", text: "正在读取歌词...", active: true, single: true }];
  }

  if (!lyric_lines.value.length) {
    return [{ key: "empty", text: "暂无歌词", active: true, single: true }];
  }

  if (!has_timed_lyrics.value) {
    return [{ key: lyric_lines.value[0].key, text: lyric_lines.value[0].text, active: true, single: true }];
  }

  const index = Math.max(active_index.value, 0);
  const current = lyric_lines.value[index];
  if (!current) {
    return [{ key: "empty", text: "暂无歌词", active: true, single: true }];
  }

  const next = lyric_lines.value[index + 1];
  if (!next) {
    return [{ key: current.key, text: current.text, active: true, single: true }];
  }

  let lines: Array<{ key: string; text: string; active: boolean; single: boolean }> = [];
  if (index % 2 === 0) {
    lines = [
      { key: current.key, text: current.text, active: true, single: false },
      { key: next.key, text: next.text, active: false, single: false },
    ];
  } else {
    lines = [
      { key: next.key, text: next.text, active: false, single: false },
      { key: current.key, text: current.text, active: true, single: false },
    ];
  }

  // 竖排时反转两行顺序，保证高亮位置视觉和横向一致
  if (vertical.value && lines.length === 2) {
    lines = lines.reverse();
  }
  return lines;
});

async function load_lyrics(path?: string | null) {
  const request_id = ++lyrics_request_id;
  lyrics_text.value = "";
  const lyrics_path = path?.trim();
  if (!lyrics_path) return;

  lyrics_loading.value = true;
  try {
    const content = await invoke<string | null>("read_lyrics_cache", { path: lyrics_path });
    if (request_id === lyrics_request_id) {
      lyrics_text.value = content ?? "";
    }
  } catch (error) {
    console.warn("读取桌面歌词失败", error);
    if (request_id === lyrics_request_id) {
      lyrics_text.value = "";
    }
  } finally {
    if (request_id === lyrics_request_id) {
      lyrics_loading.value = false;
    }
  }
}

function send_action(action: DesktopLyricsAction) {
  void emitTo("main", "desktop-lyrics-action", action);
}

async function toggle_orientation() {
  vertical.value = !vertical.value;
  localStorage.setItem(desktop_lyrics_orientation_key, vertical.value ? "1" : "0");
  await sync_window_orientation_size();
}

async function sync_window_orientation_size() {
  try {
    const size = await current_window.innerSize();
    const min_side = 120;
    let w = size.width;
    let h = size.height;

    const should_swap_to_vertical = vertical.value && w > h;
    const should_swap_to_horizontal = !vertical.value && h > w;
    if (should_swap_to_vertical || should_swap_to_horizontal) {
      [w, h] = [h, w];
      w = Math.max(w, min_side);
      h = Math.max(h, min_side);
      await current_window.setSize(new PhysicalSize(w, h));
    }
  } catch (error) {
    console.warn("同步桌面歌词窗口方向尺寸失败", error);
  }
}

async function lock_desktop_lyrics(event?: MouseEvent) {
  const target = event?.target as HTMLElement | null;
  if (target?.closest("button")) return;
  if (locked.value) return;

  event?.preventDefault();
  event?.stopPropagation();

  locked.value = true;
  try {
    await current_window.setIgnoreCursorEvents(true);
  } catch (error) {
    locked.value = false;
    console.warn("锁定桌面歌词失败", error);
  }
}

async function unlock_desktop_lyrics() {
  locked.value = false;
  try {
    await current_window.setIgnoreCursorEvents(false);
  } catch (error) {
    console.warn("解锁桌面歌词失败", error);
  }
}

function start_drag(event: MouseEvent) {
  if (locked.value || event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("button")) return;
  if (event.detail >= 2) {
    void lock_desktop_lyrics(event);
    return;
  }
  void current_window.startDragging();
}

watch([
  () => state.value.track?.lyrics_cache_path ?? "",
  () => state.value.track?.lyrics_cache_hash ?? "",
], ([path]) => {
  void load_lyrics(path);
}, { immediate: true });

onMounted(async () => {
  document.documentElement.classList.add("desktop_lyrics_root");
  document.body.classList.add("desktop_lyrics_body");
  await sync_window_orientation_size();
  try {
    await current_window.setIgnoreCursorEvents(false);
  } catch (error) {
    console.warn("初始化桌面歌词点击状态失败", error);
  }

  unlisteners = await Promise.all([
    listen<DesktopLyricsState>("desktop-lyrics-state", (event) => {
      state.value = event.payload;
    }),
    listen("desktop-lyrics-unlock", () => {
      void unlock_desktop_lyrics();
    }),
  ]);
  await emitTo("main", "desktop-lyrics-ready");
});

onBeforeUnmount(() => {
  document.documentElement.classList.remove("desktop_lyrics_root");
  document.body.classList.remove("desktop_lyrics_body");
  unlisteners.forEach((unlisten) => unlisten());
  unlisteners = [];
});
</script>

<template>
  <main
    class="desktop_lyrics_shell"
    :class="{ vertical, locked }"
    :style="theme_style"
    @mousedown="start_drag"
    @dblclick="lock_desktop_lyrics"
  >
    <section class="desktop_lyrics_surface" aria-label="桌面歌词">
      <div class="desktop_lyrics_controls" aria-label="播放控制">
        <button type="button" title="切换方向" @click="toggle_orientation">
          <span class="svg_icon" :style="icon_style(rotate_icon)" />
        </button>

        <button type="button" title="上一首" @click="send_action('previous')">
          <span class="svg_icon" :style="icon_style(previous_icon)" />
        </button>
        <button type="button" title="播放或暂停" @click="send_action('toggle_playback')">
          <span class="svg_icon" :style="icon_style(state.playing ? pause_icon : play_icon)" />
        </button>
        <button type="button" title="下一首" @click="send_action('next')">
          <span class="svg_icon" :style="icon_style(next_icon)" />
        </button>
        <button type="button" :title="current_mode_title" @click="send_action('cycle_mode')">
          <span class="svg_icon" :style="icon_style(current_mode_icon)" />
        </button>
      </div>

      <div class="desktop_lyrics_text_frame">
        <div class="desktop_lyrics_text" :class="{ single: display_lines.length === 1 }">
          <p
            v-for="line in display_lines"
            :key="line.key"
            :class="{ active: line.active, single: line.single }"
          >
            {{ line.text }}
          </p>
        </div>
      </div>
    </section>
  </main>
</template>

<style>
html.desktop_lyrics_root {
  background: transparent;
}

body.desktop_lyrics_body {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: transparent;
}

body.desktop_lyrics_body #app {
  width: 100vw;
  height: 100vh;
  background: transparent;
}

.desktop_lyrics_shell {
  display: grid;
  width: 100vw;
  height: 100vh;
  padding: 10px;
  color: var(--desktop-title-color, #f5f6f8);
  background: transparent;
  user-select: none;
}

.desktop_lyrics_shell:hover{
  cursor: move;
}

.desktop_lyrics_surface {
  display: grid;
  grid-template-rows: 34px minmax(0, 1fr);
  align-items: center;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 8px;
  padding: 6px 20px 14px;
  background: rgba(18, 27, 43, 0);
  transition: background-color 160ms ease;
}

.desktop_lyrics_shell:hover .desktop_lyrics_surface {
  background: rgba(18, 27, 43, 0.38);
}

.desktop_lyrics_shell.locked:hover .desktop_lyrics_surface,
.desktop_lyrics_shell.locked .desktop_lyrics_surface {
  background: rgba(18, 27, 43, 0);
}

.desktop_lyrics_controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 140ms ease;
}

.desktop_lyrics_shell:hover .desktop_lyrics_controls {
  opacity: 1;
  pointer-events: auto;
}

.desktop_lyrics_shell.locked:hover .desktop_lyrics_controls,
.desktop_lyrics_shell.locked .desktop_lyrics_controls {
  opacity: 0;
  pointer-events: none;
}

.desktop_lyrics_controls button {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 6px;
  color: rgba(245, 246, 248, 0.86);
  background: rgba(255, 255, 255, 0.04);
  cursor: pointer;
}

.desktop_lyrics_controls button:hover,
.desktop_lyrics_controls button:focus-visible {
  border-color: rgba(255, 255, 255, 0.72);
  color: #ffffff;
}

.desktop_lyrics_controls .svg_icon {
  width: 18px;
  height: 18px;
}

.desktop_lyrics_text_frame {
  display: grid;
  align-items: stretch;
  justify-items: stretch;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.desktop_lyrics_text {
  display: grid;
  grid-template-rows: repeat(2, minmax(0, 1fr));
  align-items: center;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  gap: 4px;
  text-align: center;
}

.desktop_lyrics_text.single {
  grid-template-rows: minmax(0, 1fr);
}

.desktop_lyrics_text p {
  letter-spacing: 5px;
  overflow: hidden;
  margin: 0;
  color: #ffffff;
  font-size: 2rem;
  font-weight: 900;
  line-height: 1.25;
  text-overflow: ellipsis;
  -webkit-text-stroke: 1px rgba(0, 0, 0, 0.82);
  paint-order: stroke fill;
  white-space: nowrap;
  transition:
    color 180ms ease,
    transform 180ms ease,
    opacity 180ms ease;
}

.desktop_lyrics_text p.active {
  color: var(--desktop-highlight-color, #3bce82);
  -webkit-text-stroke-color: rgba(0, 0, 0, 0.88);
  opacity: 1;
  transform: scale(1.04);
}

.desktop_lyrics_text p.single {
  align-self: center;
  font-size: 2.16rem;
}

/* ========== 修复后的纵向竖排样式（无rotate，使用writing-mode） ========== */
.desktop_lyrics_shell.vertical .desktop_lyrics_surface {
  grid-template-columns: 34px minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
  gap: 12px;
  padding: 14px 14px 14px 8px;
}

.desktop_lyrics_shell.vertical .desktop_lyrics_controls {
  flex-direction: column;
  align-self: center;
  grid-column: 1;
  grid-row: 1;
}

.desktop_lyrics_shell.vertical .desktop_lyrics_text_frame {
  grid-column: 2;
  grid-row: 1;
  display: grid;
  place-items: center;
  overflow: hidden;
}

.desktop_lyrics_shell.vertical .desktop_lyrics_text {
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-template-rows: minmax(0, 1fr);
  width: 100%;
  height: 100%;
  gap: 12px;
  justify-items: center;
  align-items: center;
}

.desktop_lyrics_shell.vertical .desktop_lyrics_text.single {
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
}

.desktop_lyrics_shell.vertical .desktop_lyrics_text p {
  max-width: none;
  max-height: 100%;
  writing-mode: vertical-rl;
  text-orientation: upright;
  white-space: nowrap;
  text-align: center;
}
</style>
