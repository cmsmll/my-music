<script setup lang="ts">
import { ref } from "vue";
import lyrics_copy_icon from "../assets/icons/lyrics-copy.svg";
import next_icon from "../assets/icons/next.svg";
import pause_icon from "../assets/icons/pause.svg";
import play_icon from "../assets/icons/play.svg";
import playlist_icon from "../assets/icons/playlist.svg";
import previous_icon from "../assets/icons/previous.svg";
import volume_icon from "../assets/icons/volume.svg";
import type { PlaybackModeItem, PlaybackStatus, Track } from "../types/music";
import { cover_src, display_artist, display_title, format_duration, icon_style } from "../utils/track";

defineProps<{
  current_track?: Track | null;
  status: PlaybackStatus;
  progress_dragging: boolean;
  playback_mode_button: PlaybackModeItem;
}>();

const emit = defineEmits<{
  begin_progress_drag: [event: PointerEvent];
  drag_progress: [event: PointerEvent];
  end_progress_drag: [event: PointerEvent];
  cancel_progress_drag: [event: PointerEvent];
  previous_track: [];
  toggle_playback: [];
  next_track: [];
  open_queue: [];
  cycle_playback_mode: [];
  change_volume: [event: Event];
}>();

const progress_fill_element = ref<HTMLElement | null>(null);
const progress_handle_element = ref<HTMLElement | null>(null);
const progress_tooltip_element = ref<HTMLElement | null>(null);

function render_progress(percent: number, seconds: number) {
  const safe_percent = Math.min(Math.max(percent, 0), 100);
  if (progress_fill_element.value) {
    progress_fill_element.value.style.transform = `scaleX(${safe_percent / 100})`;
  }
  if (progress_handle_element.value) {
    progress_handle_element.value.style.left = `${safe_percent}%`;
  }
  if (progress_tooltip_element.value) {
    progress_tooltip_element.value.style.left = `${safe_percent}%`;
    progress_tooltip_element.value.textContent = format_duration(seconds);
  }
}

defineExpose({ render_progress });
</script>

<template>
  <footer class="player_bar">
    <div
      class="player_progress"
      :class="{ dragging: progress_dragging }"
      role="slider"
      :aria-valuemin="0"
      :aria-valuemax="current_track?.duration ?? 0"
      :aria-valuenow="status.elapsed"
      aria-label="播放进度"
      @pointerdown="emit('begin_progress_drag', $event)"
      @pointermove="emit('drag_progress', $event)"
      @pointerup="emit('end_progress_drag', $event)"
      @pointercancel="emit('cancel_progress_drag', $event)"
    >
      <div class="progress_bar_container">
        <div class="progress_track">
          <div ref="progress_fill_element" class="progress_fill" />
          <div ref="progress_handle_element" class="progress_handle" />
          <div ref="progress_tooltip_element" class="progress_tooltip">0:00</div>
        </div>
      </div>
    </div>

    <div class="now_track">
      <span class="player_cover" :class="{ spinning_cover: status.playing && current_track }">
        <img v-if="current_track?.cover_cache_path" :src="cover_src(current_track)" alt="" />
        <span v-else>♪</span>
      </span>
      <span class="now_text">
        <strong>{{ display_title(current_track) }}</strong>
        <small>{{ display_artist(current_track) }}</small>
      </span>
    </div>

    <div class="player_center">
      <div class="control_row">
        <button type="button" title="上一首" @click="emit('previous_track')">
          <span class="svg_icon" :style="icon_style(previous_icon)" />
        </button>
        <button class="play_button" type="button" title="播放或暂停" @click="emit('toggle_playback')">
          <span class="svg_icon" :style="icon_style(status.playing ? pause_icon : play_icon)" />
        </button>
        <button type="button" title="下一首" @click="emit('next_track')">
          <span class="svg_icon" :style="icon_style(next_icon)" />
        </button>
      </div>
    </div>

    <div class="player_tools">
      <button type="button" title="播放队列" @click="emit('open_queue')">
        <span class="svg_icon" :style="icon_style(playlist_icon)" />
      </button>
      <button
        type="button"
        :title="playback_mode_button.label"
        :aria-label="playback_mode_button.label"
        @click="emit('cycle_playback_mode')"
      >
        <span class="svg_icon" :style="icon_style(playback_mode_button.icon)" />
      </button>
      <button type="button" title="桌面歌词">
        <span class="svg_icon" :style="icon_style(lyrics_copy_icon)" />
      </button>
      <span class="volume_icon svg_icon" :style="icon_style(volume_icon)" />
      <input type="range" min="0" max="1.5" step="0.01" :value="status.volume" @input="emit('change_volume', $event)" />
    </div>
  </footer>
</template>
