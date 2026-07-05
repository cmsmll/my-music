<script setup lang="ts">
import { computed, ref } from "vue";
import type { PlaylistCache, Track } from "../types/music";
import {
  display_album,
  display_artist,
  display_title,
  format_duration,
  format_file_size,
  is_missing_track,
} from "../utils/track";
import ContextMenu from "./ContextMenu.vue";

const props = defineProps<{
  track: Track;
  x: number;
  y: number;
  playlists: PlaylistCache[];
  can_remove: boolean;
}>();

const emit = defineEmits<{
  add_playlist: [playlist: PlaylistCache];
  remove: [];
}>();

const detail_open = ref(false);

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

function playlist_disabled(playlist: PlaylistCache) {
  return is_missing_track(props.track) || playlist.track_ids.includes(props.track.id);
}

function toggle_detail() {
  detail_open.value = !detail_open.value;
}
</script>

<template>
  <ContextMenu class="track_context_menu" :x="x" :y="y">
    <div class="track_context_menu_header">
      <p>添加到歌单</p>
      <div class="track_context_actions">
        <button
          v-if="can_remove"
          class="track_context_delete_button"
          type="button"
          title="删除记录"
          @click="emit('remove')"
        >
          删除
        </button>
        <button
          class="track_context_detail_button"
          :class="{ active: detail_open }"
          type="button"
          title="查看歌曲详情"
          @click="toggle_detail"
        >
          详情
        </button>
      </div>
    </div>

    <section v-if="detail_open" class="track_detail_panel">
      <dl>
        <div v-for="[label, value] in track_details" :key="label">
          <dt>{{ label }}</dt>
          <dd :title="value">{{ value }}</dd>
        </div>
      </dl>
    </section>

    <button
      v-for="playlist in playlists"
      :key="playlist.id"
      class="track_context_playlist_button"
      type="button"
      :title="playlist.name"
      :disabled="playlist_disabled(playlist)"
      @click="emit('add_playlist', playlist)"
    >
      {{ playlist.name }}
    </button>
  </ContextMenu>
</template>

<style scoped>
.track_context_menu {
  min-width: 220px;
  max-width: min(360px, calc(100vw - 24px));
  max-height: calc(100vh - 24px);
  overflow-y: auto;
}

.track_context_menu_header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  padding: 4px 8px 6px;
}

.track_context_menu_header p {
  min-width: 0;
  color: #8b919c;
  font-size: 0.78rem;
  font-weight: 800;
}

.track_context_actions {
  display: flex;
  flex: 0 0 auto;
  gap: 4px;
}

.track_context_delete_button,
.track_context_detail_button {
  min-height: 26px;
  border-radius: 8px;
  padding: 0 8px;
  background: transparent;
  font-size: 0.78rem;
  font-weight: 800;
}

.track_context_delete_button {
  color: #b65b5b;
}

.track_context_delete_button:hover {
  color: #c33131;
  background: #fff0f0;
}

.track_context_detail_button {
  color: #426dff;
}

.track_context_detail_button:hover,
.track_context_detail_button.active {
  background: #eaf0ff;
}

.track_context_playlist_button {
  overflow: hidden;
  min-height: 34px;
  border-radius: 8px;
  padding: 0 10px;
  color: #1e2026;
  background: transparent;
  font-size: 0.92rem;
  font-weight: 800;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_context_playlist_button:hover {
  color: #426dff;
  background: #eaf0ff;
}

.track_context_playlist_button:disabled,
.track_context_playlist_button:disabled:hover {
  color: #b3b8c2;
  background: transparent;
  cursor: default;
}

.track_detail_panel {
  min-width: 0;
  border-radius: 8px;
  padding: 8px;
  background: #f8f9fb;
}

.track_detail_panel dl {
  display: grid;
  gap: 6px;
  margin: 0;
}

.track_detail_panel div {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
}

.track_detail_panel dt,
.track_detail_panel dd {
  overflow: hidden;
  margin: 0;
  font-size: 0.78rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_detail_panel dt {
  color: #8b919c;
  font-weight: 800;
}

.track_detail_panel dd {
  color: #1e2026;
}
</style>
