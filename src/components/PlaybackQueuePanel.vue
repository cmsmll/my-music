<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { use_playback_store } from "../stores/playback";
import { use_player_queue_store } from "../stores/player_queue";
import { use_ui_store } from "../stores/ui";
import type { Track } from "../types/music";
import { cover_src, display_artist, display_title, format_duration } from "../utils/track";

const emit = defineEmits<{
  open_source: [];
  play_track: [track: Track];
}>();

const player_queue = use_player_queue_store();
const playback_store = use_playback_store();
const ui_store = use_ui_store();
const { current_queue, queue_source } = storeToRefs(player_queue);
const { current_track, status } = storeToRefs(playback_store);
const { playback_queue_open } = storeToRefs(ui_store);
const queue_list = ref<HTMLElement | null>(null);

const queue_title = computed(() => {
  if (queue_source.value.type === "artist") return `歌手·${queue_source.value.label}`;
  if (queue_source.value.type === "album") return `专辑·${queue_source.value.label}`;
  return queue_source.value.label;
});

const queue_total_duration = computed(() =>
  current_queue.value.reduce((total, track) => total + (track.duration ?? 0), 0),
);

const is_playing = computed(() => status.value.playing);

function track_is_active(track: Track) {
  if (current_track.value?.id) return track.id === current_track.value.id;
  return Boolean(status.value.path && track.path === status.value.path);
}

function track_should_spin(track: Track) {
  return is_playing.value && track_is_active(track);
}

async function scroll_active_track_into_view() {
  await nextTick();
  const active_item = queue_list.value?.querySelector<HTMLElement>(".queue_item.active");
  active_item?.scrollIntoView({ block: "center" });
}

function close_on_escape(event: KeyboardEvent) {
  if (event.key === "Escape" && playback_queue_open.value) ui_store.close_playback_queue();
}

onMounted(() => {
  window.addEventListener("keydown", close_on_escape);
  void scroll_active_track_into_view();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", close_on_escape);
});

watch(
  () => [current_track.value?.id, status.value.path, current_queue.value.length, playback_queue_open.value],
  () => {
    void scroll_active_track_into_view();
  },
);
</script>

<template>
  <div class="queue_overlay" @click.self="ui_store.close_playback_queue()">
    <aside class="queue_panel" :class="{ playing: is_playing }" aria-label="播放队列">
      <header>
        <button class="queue_title_button" type="button" :title="queue_title" @click="emit('open_source')">
          {{ queue_title }}
        </button>
        <p>{{ current_queue.length }} 首歌曲 {{ format_duration(queue_total_duration) }}</p>
      </header>

      <section ref="queue_list" class="queue_list">
        <button
          v-for="track in current_queue"
          :key="track.id"
          class="queue_item"
          :class="{ active: track_is_active(track) }"
          type="button"
          @click="emit('play_track', track)"
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

        <p v-if="!current_queue.length" class="empty_state">当前播放队列为空。</p>
      </section>
    </aside>
  </div>
</template>
