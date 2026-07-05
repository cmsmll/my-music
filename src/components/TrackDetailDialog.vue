<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import type { Track } from "../types/music";
import {
  display_album,
  display_artist,
  display_title,
  format_duration,
  format_file_size,
} from "../utils/track";

const props = defineProps<{
  track: Track;
}>();

const emit = defineEmits<{
  close: [];
}>();

const track_details = computed(() => {
  const metadata = props.track.metadata;
  return [
    ["歌曲", display_title(props.track)],
    ["歌手", display_artist(props.track)],
    ["专辑", display_album(props.track)],
    ["时长", format_duration(props.track.duration)],
    ["文件大小", format_file_size(props.track.file_size)],
    ["文件路径", props.track.path || "--"],
    ["歌曲 ID", props.track.id],
    ["码率", metadata.bitrate ? `${metadata.bitrate} kbps` : "--"],
    ["采样率", metadata.sample_rate ? `${metadata.sample_rate} Hz` : "--"],
    ["年份", metadata.year ? String(metadata.year) : "--"],
    ["流派", metadata.genre.length ? metadata.genre.join(", ") : "--"],
    ["封面缓存", props.track.cover_cache_path || "--"],
    ["歌词缓存", props.track.lyrics_cache_path || "--"],
    ["元数据来源", metadata.metadata_source],
  ];
});

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
  <div class="track_detail_overlay" @click.self="emit('close')">
    <section class="track_detail_dialog" aria-label="歌曲详情">
      <header>
        <div>
          <h2>歌曲详情</h2>
          <p>{{ display_title(track) }}</p>
        </div>
        <button type="button" @click="emit('close')">关闭</button>
      </header>

      <dl>
        <div v-for="[label, value] in track_details" :key="label">
          <dt>{{ label }}</dt>
          <dd :title="value">{{ value }}</dd>
        </div>
      </dl>
    </section>
  </div>
</template>

<style scoped>
.track_detail_overlay {
  position: fixed;
  inset: 0;
  z-index: 1001;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(18, 21, 28, 0.16);
}

.track_detail_dialog {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 18px;
  width: min(620px, 100%);
  height: min(720px, calc(100vh - 48px));
  max-height: min(720px, calc(100vh - 48px));
  overflow: hidden;
  border: 1px solid #eef0f4;
  border-radius: 10px;
  padding: 22px;
  background: #ffffff;
  box-shadow: 0 18px 54px rgba(19, 24, 34, 0.18);
}

.track_detail_dialog header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.track_detail_dialog header > div {
  min-width: 0;
}

.track_detail_dialog h2,
.track_detail_dialog p {
  overflow: hidden;
  margin: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_detail_dialog h2 {
  color: #1e2026;
  font-size: 1.18rem;
  font-weight: 900;
}

.track_detail_dialog p {
  margin-top: 2px;
  color: #8b919c;
  font-size: 0.9rem;
  font-weight: 800;
}

.track_detail_dialog button {
  flex: 0 0 auto;
  min-height: 34px;
  border-radius: 8px;
  padding: 0 12px;
  color: #426dff;
  background: #eaf0ff;
  font-size: 0.9rem;
  font-weight: 800;
}

.track_detail_dialog dl {
  display: grid;
  align-content: start;
  gap: 8px;
  min-height: 0;
  overflow-y: auto;
  margin: 0;
  padding-right: 4px;
  scrollbar-width: none;
}

.track_detail_dialog dl::-webkit-scrollbar {
  display: none;
}

.track_detail_dialog dl > div {
  display: grid;
  grid-template-columns: 90px minmax(0, 1fr);
  gap: 12px;
  min-width: 0;
  border-radius: 8px;
  padding: 10px 12px;
  background: #f8f9fb;
}

.track_detail_dialog dt,
.track_detail_dialog dd {
  overflow: hidden;
  margin: 0;
  font-size: 0.88rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_detail_dialog dt {
  color: #8b919c;
  font-weight: 800;
}

.track_detail_dialog dd {
  color: #1e2026;
}
</style>
