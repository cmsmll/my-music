<script setup lang="ts">
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import CustomScrollbar from "./CustomScrollbar.vue";
import { use_playback_store } from "../stores/playback";
import { use_player_queue_store } from "../stores/player_queue";
import { use_ui_store } from "../stores/ui";
import { use_app_actions_store } from "../stores/app_actions";
import type { Track } from "../types/music";
import { cover_src, display_artist, display_title, format_duration } from "../utils/track";

const player_queue = use_player_queue_store();
const app_actions = use_app_actions_store();
const playback_store = use_playback_store();
const ui_store = use_ui_store();
const { active_queue, queue_source } = storeToRefs(player_queue);
const { current_track, status } = storeToRefs(playback_store);
const { playback_queue_open } = storeToRefs(ui_store);
type CustomScrollbarExpose = {
  set_scroll_top: (value: number) => void;
  get_scroll_top: () => number;
  get_client_height: () => number;
};

const queue_list = ref<CustomScrollbarExpose | null>(null);
const queue_scroll_top = ref(0);
const queue_viewport_height = ref(640);
const queue_row_height = 58;
const queue_virtual_overscan = 8;

const queue_title = computed(() => {
  if (queue_source.value.type === "artist") return `歌手·${queue_source.value.label}`;
  if (queue_source.value.type === "album") return `专辑·${queue_source.value.label}`;
  return queue_source.value.label;
});

const queue_total_duration = computed(() =>
  active_queue.value.reduce((total, track) => total + (track.duration ?? 0), 0),
);

const is_playing = computed(() => status.value.playing);
const virtual_queue_start_index = computed(() =>
  Math.max(Math.floor(queue_scroll_top.value / queue_row_height) - queue_virtual_overscan, 0),
);
const virtual_queue_visible_count = computed(() =>
  Math.ceil(queue_viewport_height.value / queue_row_height) + queue_virtual_overscan * 2,
);
const virtual_queue_items = computed(() => {
  const start = virtual_queue_start_index.value;
  return active_queue.value.slice(start, start + virtual_queue_visible_count.value);
});
const virtual_queue_top_padding = computed(() => virtual_queue_start_index.value * queue_row_height);
const virtual_queue_bottom_padding = computed(() =>
  Math.max(
    (active_queue.value.length - virtual_queue_start_index.value - virtual_queue_items.value.length) *
      queue_row_height,
    0,
  ),
);

function track_is_active(track: Track) {
  if (current_track.value?.id) return track.id === current_track.value.id;
  return Boolean(status.value.path && track.path === status.value.path);
}

function track_should_spin(track: Track) {
  return is_playing.value && track_is_active(track);
}

function update_queue_virtual_viewport() {
  if (!queue_list.value) return;
  queue_viewport_height.value = queue_list.value.get_client_height() || queue_viewport_height.value;
  queue_scroll_top.value = queue_list.value.get_scroll_top();
}

function handle_queue_scroll() {
  if (!queue_list.value) return;
  queue_scroll_top.value = queue_list.value.get_scroll_top();
}

async function scroll_active_track_into_view() {
  await nextTick();
  if (!queue_list.value) return;
  const active_index = active_queue.value.findIndex(track_is_active);
  if (active_index < 0) return;
  const client_height = queue_list.value.get_client_height();
  const top = Math.max(active_index * queue_row_height - client_height / 2 + queue_row_height / 2, 0);
  queue_list.value.set_scroll_top(top);
  update_queue_virtual_viewport();
}

function close_on_escape(event: KeyboardEvent) {
  if (event.key === "Escape" && playback_queue_open.value) ui_store.close_playback_queue();
}

onMounted(() => {
  window.addEventListener("keydown", close_on_escape);
  update_queue_virtual_viewport();
});

onActivated(() => {
  update_queue_virtual_viewport();
  void scroll_active_track_into_view();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", close_on_escape);
});
</script>

<template>
  <div class="queue_overlay" @click.self="ui_store.close_playback_queue()">
    <aside class="queue_panel" :class="{ playing: is_playing }" aria-label="播放队列">
      <header>
        <button class="queue_title_button" type="button" :title="queue_title" @click="app_actions.open_queue_source()">
          {{ queue_title }}
        </button>
        <p>{{ active_queue.length }} 首歌曲 {{ format_duration(queue_total_duration) }}</p>
      </header>

      <CustomScrollbar
        ref="queue_list"
        class="queue_list"
        content_class="queue_list_content"
        @scroll="handle_queue_scroll"
        @resize="update_queue_virtual_viewport"
      >
        <div class="virtual_track_spacer" :style="{ height: `${virtual_queue_top_padding}px` }" />
        <button
          v-for="track in virtual_queue_items"
          :key="track.id"
          class="queue_item"
          :class="{ active: track_is_active(track) }"
          type="button"
          @click="app_actions.play_queue_track(track)"
        >
          <span class="queue_cover" :class="{ spinning_cover: track_should_spin(track) }">
            <img v-if="track.cover_cache_path" :src="cover_src(track)" alt="" />
            <span v-else>♪</span>
          </span>
          <span class="queue_text">
            <strong>{{ display_title(track) }}</strong>
            <small>{{ display_artist(track) }}</small>
          </span>
          <span class="queue_duration">{{ format_duration(track.duration) }}</span>
        </button>
        <div class="virtual_track_spacer" :style="{ height: `${virtual_queue_bottom_padding}px` }" />

        <p v-if="!active_queue.length" class="empty_state">当前播放队列为空。</p>
      </CustomScrollbar>
    </aside>
  </div>
</template>
