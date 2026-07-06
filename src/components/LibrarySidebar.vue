<script setup lang="ts">
import { nextTick, ref } from "vue";
import album_icon from "../assets/icons/album.svg";
import artist_icon from "../assets/icons/artist.svg";
import clock_fill_icon from "../assets/icons/clock-fill.svg";
import music_note_icon from "../assets/icons/music-note.svg";
import playlist_grid_icon from "../assets/icons/playlist-grid.svg";
import plus_icon from "../assets/icons/plus.svg";
import statistics_icon from "../assets/icons/statistics.svg";
import type { PlaylistCache, ViewKey } from "../types/music";
import { icon_style } from "../utils/track";

const props = defineProps<{
  active_view: ViewKey;
  has_query: boolean;
  track_count: number;
  artist_count: number;
  album_count: number;
  recent_count: number;
  playlist_items: PlaylistCache[];
  active_playlist_id: string;
}>();

const emit = defineEmits<{
  show_view: [view: ViewKey];
  show_playlist: [playlist_id: string];
  create_playlist: [name: string];
  reorder_playlists: [playlist_ids: string[]];
  open_playlist_menu: [playlist: PlaylistCache, event: MouseEvent];
  begin_resize: [event: PointerEvent];
}>();

const creating_playlist = ref(false);
const new_playlist_name = ref("");
const new_playlist_input = ref<HTMLInputElement | null>(null);
const dragging_playlist_id = ref("");
const drag_over_playlist_id = ref("");
const drag_over_after = ref(false);

let playlist_drag_start_x = 0;
let playlist_drag_start_y = 0;
let playlist_drag_candidate_id = "";
let playlist_drag_pointer_id = -1;
let playlist_drag_started = false;
let suppress_playlist_click = false;

async function start_create_playlist() {
  creating_playlist.value = true;
  new_playlist_name.value = "";
  await nextTick();
  new_playlist_input.value?.focus();
}

function cancel_create_playlist() {
  creating_playlist.value = false;
  new_playlist_name.value = "";
}

function submit_create_playlist() {
  const name = new_playlist_name.value.trim();
  if (name) {
    emit("create_playlist", name);
  }
  cancel_create_playlist();
}

function click_playlist(playlist_id: string, event: MouseEvent) {
  if (suppress_playlist_click) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  emit("show_playlist", playlist_id);
}

function begin_playlist_pointer_drag(playlist_id: string, event: PointerEvent) {
  if (event.button !== 0) return;
  playlist_drag_candidate_id = playlist_id;
  playlist_drag_pointer_id = event.pointerId;
  playlist_drag_start_x = event.clientX;
  playlist_drag_start_y = event.clientY;
  playlist_drag_started = false;
  drag_over_playlist_id.value = "";
  drag_over_after.value = false;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function move_playlist_pointer_drag(event: PointerEvent) {
  if (event.pointerId !== playlist_drag_pointer_id || !playlist_drag_candidate_id) return;

  const distance = Math.hypot(event.clientX - playlist_drag_start_x, event.clientY - playlist_drag_start_y);
  if (!playlist_drag_started && distance < 6) return;

  event.preventDefault();
  if (!playlist_drag_started) {
    playlist_drag_started = true;
    suppress_playlist_click = true;
    dragging_playlist_id.value = playlist_drag_candidate_id;
  }

  update_drag_target(event);
}

function end_playlist_pointer_drag(event: PointerEvent) {
  if (event.pointerId !== playlist_drag_pointer_id) return;

  const dragged_playlist_id = playlist_drag_candidate_id;
  const target_playlist_id = drag_over_playlist_id.value;
  const insert_after = drag_over_after.value;
  const should_reorder = playlist_drag_started && dragged_playlist_id && target_playlist_id && dragged_playlist_id !== target_playlist_id;

  try {
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  } catch {
    // Pointer capture can already be released by the browser.
  }

  reset_playlist_drag();
  if (should_reorder) reorder_playlist_ids(dragged_playlist_id, target_playlist_id, insert_after);

  window.setTimeout(() => {
    suppress_playlist_click = false;
  });
}

function cancel_playlist_pointer_drag(event: PointerEvent) {
  if (event.pointerId !== playlist_drag_pointer_id) return;
  reset_playlist_drag();
  window.setTimeout(() => {
    suppress_playlist_click = false;
  });
}

function update_drag_target(event: PointerEvent) {
  const target = document
    .elementFromPoint(event.clientX, event.clientY)
    ?.closest<HTMLElement>("[data-playlist-id]");
  if (!target) {
    drag_over_playlist_id.value = "";
    drag_over_after.value = false;
    return;
  }

  const playlist_id = target?.dataset.playlistId ?? "";
  if (!playlist_id || playlist_id === playlist_drag_candidate_id) {
    drag_over_playlist_id.value = "";
    drag_over_after.value = false;
    return;
  }

  const rect = target.getBoundingClientRect();
  drag_over_playlist_id.value = playlist_id;
  drag_over_after.value = event.clientY > rect.top + rect.height / 2;
}

function reorder_playlist_ids(dragged_playlist_id: string, target_playlist_id: string, insert_after: boolean) {
  const playlist_ids = props.playlist_items.map((playlist) => playlist.id);
  const from_index = playlist_ids.indexOf(dragged_playlist_id);
  if (from_index < 0) return;

  const [dragged_id] = playlist_ids.splice(from_index, 1);
  const target_index = playlist_ids.indexOf(target_playlist_id);
  if (target_index < 0) return;

  playlist_ids.splice(target_index + (insert_after ? 1 : 0), 0, dragged_id);
  emit("reorder_playlists", playlist_ids);
}

function reset_playlist_drag() {
  playlist_drag_candidate_id = "";
  playlist_drag_pointer_id = -1;
  playlist_drag_started = false;
  dragging_playlist_id.value = "";
  drag_over_playlist_id.value = "";
  drag_over_after.value = false;
}
</script>

<template>
  <aside class="sidebar">
    <nav class="sidebar_nav" aria-label="主导航">
      <section class="nav_group">
        <h2>音乐曲库</h2>
        <button
          class="nav_item"
          :class="{ active: active_view === 'all' && !has_query }"
          type="button"
          :title="String(track_count)"
          @click="emit('show_view', 'all')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(music_note_icon)" />
          <span>全部</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'artists' }"
          type="button"
          :title="String(artist_count)"
          @click="emit('show_view', 'artists')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(artist_icon)" />
          <span>歌手</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'albums' }"
          type="button"
          :title="String(album_count)"
          @click="emit('show_view', 'albums')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(album_icon)" />
          <span>专辑</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'stats' }"
          type="button"
          @click="emit('show_view', 'stats')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(statistics_icon)" />
          <span>统计</span>
        </button>
      </section>

      <section class="nav_group playlist_group">
        <h2>播放列表</h2>
        <button
          class="nav_item"
          :class="{ active: active_view === 'recent' }"
          type="button"
          :title="String(recent_count)"
          @click="emit('show_view', 'recent')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(clock_fill_icon)" />
          <span>最近播放</span>
        </button>
        <button
          v-for="playlist in playlist_items"
          :key="playlist.id"
          class="nav_item playlist_item"
          :class="{
            active: active_view === 'user_playlist' && active_playlist_id === playlist.id,
            dragging: dragging_playlist_id === playlist.id,
            drag_over: drag_over_playlist_id === playlist.id,
            drag_over_after: drag_over_playlist_id === playlist.id && drag_over_after,
          }"
          type="button"
          :data-playlist-id="playlist.id"
          :title="String(playlist.metadata.track_count)"
          @click="click_playlist(playlist.id, $event)"
          @contextmenu.prevent="emit('open_playlist_menu', playlist, $event)"
          @pointerdown="begin_playlist_pointer_drag(playlist.id, $event)"
          @pointermove="move_playlist_pointer_drag"
          @pointerup="end_playlist_pointer_drag"
          @pointercancel="cancel_playlist_pointer_drag"
        >
          <span class="nav_icon svg_icon" :style="icon_style(playlist_grid_icon)" />
          <span>{{ playlist.name }}</span>
        </button>
        <label v-if="creating_playlist" class="nav_item create_playlist create_playlist_input_row">
          <span class="nav_icon svg_icon" :style="icon_style(plus_icon)" />
          <input
            ref="new_playlist_input"
            v-model="new_playlist_name"
            class="create_playlist_input"
            type="text"
            placeholder="新建歌单"
            @blur="submit_create_playlist"
            @keydown.esc.prevent="cancel_create_playlist"
            @keydown.enter.prevent="submit_create_playlist"
          />
        </label>
        <button v-else class="nav_item create_playlist" type="button" @click="start_create_playlist">
          <span class="nav_icon svg_icon" :style="icon_style(plus_icon)" />
          <span>新建歌单</span>
        </button>
      </section>
    </nav>
    <div
      class="sidebar_resize_handle"
      role="separator"
      aria-orientation="vertical"
      aria-label="调整侧边栏宽度"
      @pointerdown="emit('begin_resize', $event)"
    />
  </aside>
</template>

<style>
.sidebar {
  grid-area: sidebar;
  display: flex;
  flex-direction: column;
  position: relative;
  min-height: 0;
  border-right: var(--app_border_width, 2px) solid var(--progress_track_background, rgba(128, 128, 128, 0.18));
  background: transparent;
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

.nav_group h2 {
  margin: 0 0 12px;
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
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 0 22px;
  color: var(--theme-title-color, #1e2026);
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

.nav_item.active {
  border-color: var(--theme-highlight-color, #426dff);
  color: var(--theme-highlight-color, #426dff);
  background: transparent;
}

.nav_item:hover {
  border-color: var(--theme-highlight-color, #426dff);
  color: var(--theme-highlight-color, #426dff);
  background: transparent;
}

.nav_item.dragging {
  opacity: 0.45;
}

.playlist_item {
  cursor: grab;
  touch-action: none;
}

.playlist_item:active {
  cursor: grabbing;
}

.nav_item.drag_over {
  color: var(--theme-highlight-color, #426dff);
  background: #f1f5ff;
  box-shadow: inset 0 3px 0 var(--theme-highlight-color, #426dff);
}

.nav_item.drag_over_after {
  box-shadow: inset 0 -3px 0 var(--theme-highlight-color, #426dff);
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
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 1.05rem;
  font-weight: 700;
}

.create_playlist_input::placeholder {
  color: var(--theme-subtitle-color, #a0a5af);
}
</style>
