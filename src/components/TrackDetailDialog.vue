<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { computed, onBeforeUnmount, onMounted } from "vue";
import folder_open_icon from "../assets/icons/folder-open.svg";
import { use_notification_store } from "../stores/notifications";
import type { Track } from "../types/music";
import {
  display_album,
  display_artist,
  display_title,
  format_duration,
  format_file_size,
  icon_style,
} from "../utils/track";

type TrackDetailOpenMode = "reveal" | "directory";

type TrackDetailRow = {
  label: string;
  value: string;
  path?: string;
  open_mode?: TrackDetailOpenMode;
  is_path?: boolean;
};

const props = defineProps<{
  track: Track;
}>();

const emit = defineEmits<{
  close: [];
}>();

const notification = use_notification_store();

const track_details = computed<TrackDetailRow[]>(() => {
  return [
    { label: "歌曲", value: display_title(props.track) },
    { label: "歌手", value: display_artist(props.track) },
    { label: "专辑", value: display_album(props.track) },
    { label: "时长", value: format_duration(props.track.duration) },
    { label: "文件大小", value: format_file_size(props.track.file_size) },
    { label: "码率", value: props.track.bitrate ? `${props.track.bitrate} kbps` : "--" },
    { label: "采样率", value: props.track.sample_rate ? `${props.track.sample_rate} Hz` : "--" },
    { label: "信息来源", value: props.track.metadata_source },
    path_detail_row("文件路径", props.track.path),
    path_detail_row("封面缓存", props.track.cover_cache_path),
    path_detail_row("歌词缓存", props.track.lyrics_cache_path, "directory"),
  ];
});

function path_detail_row(label: string, path?: string | null, open_mode: TrackDetailOpenMode = "reveal"): TrackDetailRow {
  const value = path?.trim() || "--";
  return {
    label,
    value,
    path: path?.trim() || undefined,
    open_mode,
    is_path: true,
  };
}

async function open_directory(path: string) {
  await invoke("open_directory", { path });
}

async function open_track_folder(path?: string, open_mode: TrackDetailOpenMode = "reveal") {
  if (!path) return;

  if (open_mode === "directory") {
    try {
      await open_directory(path);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      notification.error(message);
      console.warn("无法打开文件夹", error);
    }
    return;
  }

  try {
    await revealItemInDir(path);
  } catch (error) {
    try {
      await open_directory(path);
    } catch (fallback_error) {
      const message = fallback_error instanceof Error ? fallback_error.message : String(fallback_error);
      notification.error(message);
      console.warn("无法打开文件夹", error, fallback_error);
    }
  }
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
  <div class="track_detail_overlay" @click.self="emit('close')">
    <section class="track_detail_dialog" aria-label="歌曲详情">
      <header>
        <div>
          <h2>歌曲详情</h2>
        </div>
        <button type="button" @click="emit('close')">关闭</button>
      </header>

      <dl>
        <div
          v-for="item in track_details"
          :key="item.label"
          :class="{ track_detail_path_row: item.is_path }"
        >
          <dt>{{ item.label }}</dt>
          <dd :title="item.value">{{ item.value }}</dd>
          <button
            v-if="item.is_path"
            class="track_detail_folder_button"
            type="button"
            title="打开文件夹"
            :disabled="!item.path"
            @click="open_track_folder(item.path, item.open_mode)"
          >
            <span class="track_detail_folder_icon" :style="icon_style(folder_open_icon)" />
          </button>
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

.track_detail_dialog dl > .track_detail_path_row {
  grid-template-columns: 90px minmax(0, 1fr) 34px;
  align-items: center;
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

.track_detail_dialog .track_detail_folder_button {
  display: grid;
  width: 34px;
  height: 34px;
  min-height: 34px;
  place-items: center;
  border: 1px solid transparent;
  padding: 0;
  color: #426dff;
  cursor: pointer;
}

.track_detail_dialog .track_detail_folder_button:disabled {
  cursor: default;
  opacity: 0.45;
}

.track_detail_dialog .track_detail_folder_button:not(:disabled):hover {
  border-color: #426dff;
}

.track_detail_folder_icon {
  display: inline-block;
  width: 18px;
  height: 18px;
  background: currentColor;
  -webkit-mask: var(--icon) center / contain no-repeat;
  mask: var(--icon) center / contain no-repeat;
}
</style>
