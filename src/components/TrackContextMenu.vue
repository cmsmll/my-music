<script setup lang="ts">
import type { PlaylistCache, Track } from "../types/music";
import { is_missing_track } from "../utils/track";
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
  detail: [];
}>();

function playlist_disabled(playlist: PlaylistCache) {
  return is_missing_track(props.track) || playlist.track_ids.includes(props.track.id);
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
          type="button"
          title="查看歌曲详情"
          @click="emit('detail')"
        >
          详情
        </button>
      </div>
    </div>

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
  color: var(--theme-subtitle-color, #8b919c);
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
  color: var(--theme-control-color, #426dff);
}

.track_context_detail_button:hover {
  background: #eaf0ff;
}

.track_context_playlist_button {
  overflow: hidden;
  min-height: 34px;
  border-radius: 8px;
  padding: 0 10px;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 0.92rem;
  font-weight: 800;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.track_context_playlist_button:hover {
  color: var(--theme-control-color, #426dff);
  background: #eaf0ff;
}

.track_context_playlist_button:disabled,
.track_context_playlist_button:disabled:hover {
  color: #b3b8c2;
  background: transparent;
  cursor: default;
}

</style>
