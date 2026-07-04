<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import type { QueueSource, Track } from "../types/music";
import { cover_src, display_artist, display_title, format_duration } from "../utils/track";

const props = defineProps<{
  queue_source: QueueSource;
  tracks: Track[];
  active_track_id?: string | null;
  status_path?: string | null;
  is_playing: boolean;
}>();

const emit = defineEmits<{
  close: [];
  open_source: [];
  play_track: [track: Track];
}>();

function queue_title() {
  if (props.queue_source.type === "artist") return `歌手·${props.queue_source.label}`;
  if (props.queue_source.type === "album") return `专辑·${props.queue_source.label}`;
  return props.queue_source.label;
}

function queue_total_duration() {
  return props.tracks.reduce((total, track) => total + (track.duration ?? 0), 0);
}

function track_is_active(track: Track) {
  if (props.active_track_id) return track.id === props.active_track_id;
  return Boolean(props.status_path && track.path === props.status_path);
}

function track_should_spin(track: Track) {
  return props.is_playing && track_is_active(track);
}

function close_on_escape(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => {
  window.addEventListener("keydown", close_on_escape);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", close_on_escape);
});
</script>

<template>
  <div class="queue_overlay" @click.self="emit('close')">
    <aside class="queue_panel" :class="{ playing: is_playing }" aria-label="播放队列">
      <header>
        <button class="queue_title_button" type="button" :title="queue_title()" @click="emit('open_source')">
          {{ queue_title() }}
        </button>
        <p>{{ tracks.length }} 首歌曲 {{ format_duration(queue_total_duration()) }}</p>
      </header>

      <section class="queue_list">
        <button
          v-for="track in tracks"
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

        <p v-if="!tracks.length" class="empty_state">当前播放队列为空。</p>
      </section>
    </aside>
  </div>
</template>
