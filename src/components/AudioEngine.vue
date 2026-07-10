<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { use_playback_store } from "../stores/playback";
import { use_player_queue_store } from "../stores/player_queue";
import type { PlaybackStatus, Track } from "../types/music";

const emit = defineEmits<{
  ended: [status: PlaybackStatus];
  playback_error: [error: unknown];
}>();

type AudioError = {
  path: string | null;
  source: string;
  code: number | null;
  message: string;
  elapsed: number;
  ready_state: number;
  network_state: number;
};

const audio_element = ref<HTMLAudioElement | null>(null);
const playback_store = use_playback_store();
const player_queue = use_player_queue_store();
const { current_track, progress_dragging, status } = storeToRefs(playback_store);

let current_path: string | null = null;
let volume_level = 1;
let play_request_id = 0;
let status_timer: number | undefined;
let progress_frame: number | undefined;
let progress_sync_started_at = performance.now();
let pending_seek_seconds: number | null = null;
let pending_seek_path: string | null = null;
let pending_seek_started_at = performance.now();
let visual_elapsed = 0;
let last_saved_playback_elapsed = -1;

function current_audio() {
  if (!audio_element.value) {
    throw new Error("音频元素未初始化");
  }
  return audio_element.value;
}

function is_audio_play_interrupted(error: unknown) {
  if (error instanceof DOMException && error.name === "AbortError") return true;
  const message = (error instanceof Error ? error.message : String(error)).toLowerCase();
  return (
    message.includes("play() request was interrupted") ||
    message.includes("interrupted by a new load request") ||
    message.includes("interrupted by a call to pause") ||
    message.includes("播放请求已被新的播放加载打断")
  );
}

function is_current_play_request(request_id: number) {
  return request_id === play_request_id;
}

function safe_audio_volume(volume: number) {
  return Math.min(Math.max(volume, 0), 1);
}

function audio_status(): PlaybackStatus {
  const audio = current_audio();
  return {
    path: current_path,
    playing: !audio.paused && !audio.ended,
    volume: volume_level,
    elapsed: Math.max(audio.currentTime || 0, 0),
  };
}

function emit_status() {
  apply_playback_status(audio_status());
}

async function play_audio_for_request(request_id: number) {
  if (!is_current_play_request(request_id)) return;

  try {
    await current_audio().play();
  } catch (error) {
    if (!is_current_play_request(request_id) || is_audio_play_interrupted(error)) return;
    throw error;
  }

  if (!is_current_play_request(request_id)) return;
  emit_status();
}

function wait_for_metadata(request_id: number) {
  const audio = current_audio();
  if (audio.readyState >= HTMLMediaElement.HAVE_METADATA) {
    return Promise.resolve();
  }

  return new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      audio.removeEventListener("loadedmetadata", handle_loaded);
      audio.removeEventListener("error", handle_error);
    };
    const handle_loaded = () => {
      cleanup();
      if (is_current_play_request(request_id)) resolve();
    };
    const handle_error = () => {
      cleanup();
      if (is_current_play_request(request_id)) reject(new Error(audio_error_message()));
    };
    audio.addEventListener("loadedmetadata", handle_loaded, { once: true });
    audio.addEventListener("error", handle_error, { once: true });
    audio.load();
  });
}

async function play(track: Track, seconds = 0) {
  const audio = current_audio();
  const request_id = ++play_request_id;
  clear_pending_seek();
  current_path = track.path;
  audio.src = convertFileSrc(track.path);
  audio.volume = safe_audio_volume(volume_level);
  if (seconds > 0) {
    await wait_for_metadata(request_id);
    if (is_current_play_request(request_id)) {
      audio.currentTime = Math.max(seconds, 0);
    }
  }
  await play_audio_for_request(request_id);
  start_status_polling();
}

function pause() {
  play_request_id += 1;
  current_audio().pause();
  emit_status();
  start_status_polling();
}

async function resume() {
  if (!current_path) return;
  const request_id = ++play_request_id;
  await play_audio_for_request(request_id);
  start_status_polling();
}

function stop() {
  const audio = current_audio();
  play_request_id += 1;
  clear_pending_seek();
  audio.pause();
  audio.removeAttribute("src");
  audio.load();
  current_path = null;
  emit_status();
}

function set_volume(volume: number) {
  volume_level = Math.min(Math.max(volume, 0), 1.5);
  current_audio().volume = safe_audio_volume(volume_level);
  emit_status();
}

function seek(seconds: number) {
  hold_progress_at_seek_target(seconds);
  current_audio().currentTime = Math.max(seconds, 0);
  emit_status();
}

function apply_external_status(next_status: PlaybackStatus) {
  current_path = next_status.path ?? null;
  apply_playback_status(next_status);
}

function preview_seek(seconds: number) {
  set_visual_elapsed(seconds);
}

function cancel_seek_preview() {
  set_visual_elapsed(visual_elapsed);
}

function start_status_polling() {
  if (status_timer) window.clearInterval(status_timer);
  status_timer = window.setInterval(() => {
    emit_status();
  }, 1000);
}

function apply_playback_status(next_status: PlaybackStatus) {
  playback_store.set_status(next_status);
  player_queue.set_current_track_path(next_status.path);
  if (hold_pending_seek_progress(next_status)) {
    if (next_status.playing) request_progress_frame();
    return;
  }
  sync_visual_elapsed(next_status);
  if (next_status.playing) request_progress_frame();
}

function sync_visual_elapsed(next_status: PlaybackStatus) {
  set_visual_elapsed(next_status.elapsed);
  progress_sync_started_at = performance.now();
}

function hold_progress_at_seek_target(seconds: number) {
  pending_seek_seconds = seconds;
  pending_seek_path = status.value.path ?? null;
  pending_seek_started_at = performance.now();
  set_visual_elapsed(seconds);
}

function hold_pending_seek_progress(next_status: PlaybackStatus) {
  if (pending_seek_seconds === null) return false;

  if (next_status.path !== pending_seek_path) {
    clear_pending_seek();
    return false;
  }

  const matched = Math.abs(next_status.elapsed - pending_seek_seconds) <= 1;
  if (matched) {
    clear_pending_seek();
    return false;
  }

  set_visual_elapsed(playback_elapsed_from_pending_seek());
  progress_sync_started_at = pending_seek_started_at;
  return true;
}

function playback_elapsed_from_pending_seek() {
  const base = pending_seek_seconds ?? status.value.elapsed;
  if (!status.value.playing) return base;
  return base + (performance.now() - pending_seek_started_at) / 1000;
}

function clear_pending_seek() {
  pending_seek_seconds = null;
  pending_seek_path = null;
}

function request_progress_frame() {
  if (progress_frame) return;
  progress_frame = window.requestAnimationFrame(update_progress_frame);
}

function update_progress_frame(now: number) {
  progress_frame = undefined;

  const duration = current_track.value?.duration ?? 0;
  if (!duration || !status.value.path) {
    set_visual_elapsed(0);
    return;
  }

  if (status.value.playing) {
    const base_elapsed = pending_seek_seconds ?? status.value.elapsed;
    const started_at = pending_seek_seconds === null ? progress_sync_started_at : pending_seek_started_at;
    const elapsed = base_elapsed + (now - started_at) / 1000;
    if (!progress_dragging.value) {
      set_visual_elapsed(Math.min(elapsed, duration));
    }
    if (!progress_dragging.value && visual_elapsed >= duration) {
      emit("ended", {
        ...status.value,
        playing: false,
        elapsed: Math.floor(duration),
      });
      return;
    }
    request_progress_frame();
    return;
  }

  set_visual_elapsed(Math.min(status.value.elapsed, duration));
}

function set_visual_elapsed(seconds: number) {
  visual_elapsed = Math.max(0, seconds);
  playback_store.set_visual_elapsed(visual_elapsed);
}

function cache_elapsed_seconds() {
  const duration = current_track.value?.duration ?? Number.POSITIVE_INFINITY;
  const seconds =
    pending_seek_seconds !== null
      ? playback_elapsed_from_pending_seek()
      : status.value.playing
        ? visual_elapsed
        : status.value.elapsed;

  return Math.max(0, Math.floor(Math.min(seconds, duration)));
}

function save_playback_elapsed_cache(elapsed = cache_elapsed_seconds(), force = false) {
  if (!force && Math.abs(last_saved_playback_elapsed - elapsed) <= 1) return;

  last_saved_playback_elapsed = elapsed;
  player_queue.save_playback_elapsed(elapsed);
}

function handle_audio_time_update(event: Event) {
  emit_status();
  if (progress_dragging.value) return;
  const audio = event.currentTarget as HTMLAudioElement;
  save_playback_elapsed_cache(audio.currentTime || 0);
}

function handle_audio_ended() {
  emit_status();
  emit("ended", audio_status());
}

function handle_audio_error() {
  const error = audio_error();
  if (is_audio_error_ignorable(error)) {
    emit_status();
    return;
  }

  emit("playback_error", error.message);
  void invoke("record_audio_error", {
    request: {
      path: error.path,
      source: error.source,
      code: error.code,
      message: error.message,
      elapsed: error.elapsed,
      readyState: error.ready_state,
      networkState: error.network_state,
    },
  }).catch((log_error) => {
    console.warn("无法记录音频错误", log_error);
  });
  emit_status();
}

function is_audio_error_ignorable(error: AudioError) {
  return error.code === MediaError.MEDIA_ERR_ABORTED || is_audio_play_interrupted(error.message);
}

function audio_error_message() {
  const error = current_audio().error;
  if (!error) return "音频播放失败";
  switch (error.code) {
    case MediaError.MEDIA_ERR_ABORTED:
      return "音频播放被中止";
    case MediaError.MEDIA_ERR_NETWORK:
      return "音频文件加载失败";
    case MediaError.MEDIA_ERR_DECODE:
      return "音频格式无法解码";
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      return "当前音频格式不支持播放";
    default:
      return "音频播放失败";
  }
}

function audio_error(): AudioError {
  const audio = current_audio();
  return {
    path: current_path,
    source: audio.currentSrc || audio.src,
    code: audio.error?.code ?? null,
    message: audio_error_message(),
    elapsed: Math.max(Math.floor(audio.currentTime || 0), 0),
    ready_state: audio.readyState,
    network_state: audio.networkState,
  };
}

watch(() => current_track.value?.id ?? null, () => {
  last_saved_playback_elapsed = -1;
  save_playback_elapsed_cache(cache_elapsed_seconds(), true);
});

onBeforeUnmount(() => {
  if (status_timer) window.clearInterval(status_timer);
  if (progress_frame) window.cancelAnimationFrame(progress_frame);
  try {
    stop();
  } catch {
    // 组件卸载时音频节点可能已经被浏览器释放。
  }
});

defineExpose({
  play,
  pause,
  resume,
  stop,
  seek,
  set_volume,
  status: audio_status,
  preview_seek,
  cancel_seek_preview,
  apply_external_status,
  cache_elapsed_seconds,
  save_playback_elapsed_cache,
});
</script>

<template>
  <audio
    ref="audio_element"
    class="audio_element"
    preload="auto"
    @play="emit_status"
    @pause="emit_status"
    @timeupdate="handle_audio_time_update"
    @ended="handle_audio_ended"
    @error="handle_audio_error"
  />
</template>

<style scoped>
.audio_element {
  display: none;
}
</style>
