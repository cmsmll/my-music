<script setup lang="ts">
import { computed, nextTick, onActivated, onBeforeUnmount, ref, useId, watch } from "vue";
import { storeToRefs } from "pinia";
import { use_playback_store } from "../stores/playback";

type LineLyricItem = {
  key: string;
  time: number | null;
  text: string;
};

const props = withDefaults(defineProps<{
  lyrics: string;
  loading?: boolean;
  placeholder?: string[];
  active?: boolean;
}>(), {
  loading: false,
  placeholder: () => ["暂未获取到歌词"],
  active: true,
});

const playback_store = use_playback_store();
const { current_track_path, progress_dragging, visual_elapsed } = storeToRefs(playback_store);
const lyrics_viewport = ref<HTMLElement | null>(null);
const active_anchor_index = ref(-1);
const lyric_anchor_prefix = useId();
const manual_scroll_lock_ms = 3000;
const last_manual_scroll_at = ref(0);
let scroll_animation_frame: number | undefined;
let programmatic_scroll_active = false;
let programmatic_scroll_reset_timer: number | undefined;
let last_visual_elapsed = visual_elapsed.value;

const lyric_lines = computed(() => {
  const parsed: LineLyricItem[] = [];

  props.lyrics.split(/\r?\n/).forEach((source_line, source_index) => {
    const line = source_line.trim();
    if (!line) return;
    if (/^\[[a-z]+:/iu.test(line)) return;

    const time_matches = [...line.matchAll(/\[(\d{1,2}):(\d{2})(?:[.:](\d{1,3}))?\]/gu)];
    const text = line.replace(/^(\[[^\]]+\])+\s*/u, "").trim();
    if (!text) return;

    if (!time_matches.length) {
      parsed.push({
        key: `plain-${source_index}`,
        time: null,
        text,
      });
      return;
    }

    for (const match of time_matches) {
      const minute = Number(match[1]);
      const second = Number(match[2]);
      const fraction = match[3] ?? "0";
      const millisecond = Number(fraction.padEnd(3, "0").slice(0, 3));
      parsed.push({
        key: `${minute}-${second}-${millisecond}-${source_index}`,
        time: minute * 60 + second + millisecond / 1000,
        text,
      });
    }
  });

  return parsed.sort((left, right) => {
    if (left.time === null && right.time === null) return 0;
    if (left.time === null) return 1;
    if (right.time === null) return -1;
    return left.time - right.time;
  });
});

const has_timed_lyrics = computed(() =>
  lyric_lines.value.some((line) => line.time !== null),
);

const visible_lines = computed(() =>
  lyric_lines.value.length
    ? lyric_lines.value
    : props.placeholder.map((text, index) => ({
        key: `placeholder-${index}`,
        time: null,
        text,
      })),
);

function anchor_index_for_elapsed(seconds: number) {
  if (!has_timed_lyrics.value) return -1;

  let index = -1;
  for (let current = 0; current < lyric_lines.value.length; current += 1) {
    const time = lyric_lines.value[current].time;
    if (time === null) continue;
    if (time <= seconds + 0.08) {
      index = current;
    } else {
      break;
    }
  }
  if (index >= 0) return index;
  return lyric_lines.value.findIndex((line) => line.time !== null);
}

function next_timed_anchor_time(index: number) {
  for (let current = index + 1; current < lyric_lines.value.length; current += 1) {
    const time = lyric_lines.value[current].time;
    if (time !== null) return time;
  }
  return Number.POSITIVE_INFINITY;
}

function elapsed_in_anchor(index: number, seconds: number) {
  const line = lyric_lines.value[index];
  if (!line || line.time === null) return false;

  const first_timed_index = lyric_lines.value.findIndex((item) => item.time !== null);
  if (index === first_timed_index && seconds < line.time) return true;

  return seconds + 0.08 >= line.time && seconds < next_timed_anchor_time(index);
}

const active_index = computed(() => active_anchor_index.value);

function lyric_anchor_id(index: number) {
  return `${lyric_anchor_prefix}-line-${index}`;
}

function cancel_scroll_animation() {
  if (scroll_animation_frame === undefined) return;
  window.cancelAnimationFrame(scroll_animation_frame);
  scroll_animation_frame = undefined;
}

function reset_programmatic_scroll_later() {
  if (programmatic_scroll_reset_timer !== undefined) {
    window.clearTimeout(programmatic_scroll_reset_timer);
  }
  programmatic_scroll_reset_timer = window.setTimeout(() => {
    programmatic_scroll_active = false;
    programmatic_scroll_reset_timer = undefined;
  }, 80);
}

function begin_programmatic_scroll() {
  programmatic_scroll_active = true;
  if (programmatic_scroll_reset_timer !== undefined) {
    window.clearTimeout(programmatic_scroll_reset_timer);
    programmatic_scroll_reset_timer = undefined;
  }
}

function end_programmatic_scroll() {
  reset_programmatic_scroll_later();
}

function mark_manual_scroll() {
  last_manual_scroll_at.value = performance.now();
  cancel_scroll_animation();
  programmatic_scroll_active = false;
}

function handle_lyrics_scroll() {
  if (programmatic_scroll_active) return;
  last_manual_scroll_at.value = performance.now();
}

function should_skip_auto_scroll() {
  return performance.now() - last_manual_scroll_at.value < manual_scroll_lock_ms;
}

function clear_manual_scroll_lock() {
  last_manual_scroll_at.value = 0;
}

function is_seek_elapsed_change(seconds: number) {
  const elapsed_delta = Math.abs(seconds - last_visual_elapsed);
  last_visual_elapsed = seconds;
  return elapsed_delta > 1.5;
}

function ease_out_cubic(progress: number) {
  return 1 - (1 - progress) ** 3;
}

function scroll_to_anchor(anchor: HTMLElement, animate: boolean) {
  const viewport = lyrics_viewport.value;
  if (!viewport) return;

  const viewport_rect = viewport.getBoundingClientRect();
  const anchor_rect = anchor.getBoundingClientRect();
  const max_scroll_top = Math.max(viewport.scrollHeight - viewport.clientHeight, 0);
  const target_top = Math.min(
    Math.max(
      viewport.scrollTop + anchor_rect.top - viewport_rect.top - viewport.clientHeight / 2 + anchor_rect.height / 2,
      0,
    ),
    max_scroll_top,
  );

  cancel_scroll_animation();
  if (!animate) {
    begin_programmatic_scroll();
    viewport.scrollTop = target_top;
    end_programmatic_scroll();
    return;
  }

  const start_top = viewport.scrollTop;
  const distance = target_top - start_top;
  if (Math.abs(distance) < 1) return;

  const duration = 220;
  const started_at = performance.now();
  begin_programmatic_scroll();
  const step = (now: number) => {
    const progress = Math.min((now - started_at) / duration, 1);
    viewport.scrollTop = start_top + distance * ease_out_cubic(progress);
    if (progress < 1) {
      scroll_animation_frame = window.requestAnimationFrame(step);
      return;
    }
    scroll_animation_frame = undefined;
    end_programmatic_scroll();
  };
  scroll_animation_frame = window.requestAnimationFrame(step);
}

function wait_for_render_frame() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}

async function scroll_active_line(animate = true) {
  await nextTick();
  const anchor = document.getElementById(lyric_anchor_id(active_anchor_index.value));
  if (anchor) {
    scroll_to_anchor(anchor, animate);
  }
}

function sync_anchor_for_elapsed(seconds: number, force = false) {
  if (!has_timed_lyrics.value) {
    active_anchor_index.value = -1;
    cancel_scroll_animation();
    if (lyrics_viewport.value) {
      lyrics_viewport.value.scrollTop = 0;
    }
    return;
  }

  if (!force && elapsed_in_anchor(active_anchor_index.value, seconds)) return;

  const next_index = anchor_index_for_elapsed(seconds);
  if (!force && next_index === active_anchor_index.value) return;

  active_anchor_index.value = next_index;
  if (!force && should_skip_auto_scroll()) return;
  void scroll_active_line(!force);
}

async function sync_visible_anchor() {
  await nextTick();
  await wait_for_render_frame();
  sync_anchor_for_elapsed(visual_elapsed.value, true);
}

watch(visual_elapsed, (seconds) => {
  const force = progress_dragging.value || is_seek_elapsed_change(seconds);
  if (force) {
    clear_manual_scroll_lock();
  }
  sync_anchor_for_elapsed(seconds, force);
}, { immediate: true });

watch(current_track_path, () => {
  clear_manual_scroll_lock();
  last_visual_elapsed = visual_elapsed.value;
  void sync_visible_anchor();
});

onActivated(() => {
  void sync_visible_anchor();
});

watch(lyric_lines, async () => {
  await sync_visible_anchor();
});

watch(() => props.active, (active) => {
  if (!active) return;
  void sync_visible_anchor();
});

onBeforeUnmount(() => {
  cancel_scroll_animation();
  if (programmatic_scroll_reset_timer !== undefined) {
    window.clearTimeout(programmatic_scroll_reset_timer);
  }
});
</script>

<template>
  <section
    ref="lyrics_viewport"
    class="line_lyrics_renderer"
    aria-label="歌词"
    @scroll="handle_lyrics_scroll"
    @wheel.passive="mark_manual_scroll"
    @touchstart.passive="mark_manual_scroll"
  >
    <p v-if="loading" class="line_lyrics_state">正在读取歌词...</p>
    <template v-else>
      <p
        v-for="(line, index) in visible_lines"
        :id="lyric_anchor_id(index)"
        :key="line.key"
        class="line_lyrics_row"
        :class="{
          active: index === active_index,
          placeholder: !lyric_lines.length,
        }"
      >
        {{ line.text }}
      </p>
    </template>
  </section>
</template>

<style scoped>
.line_lyrics_renderer {
  height: min(44vh, 430px);
  box-sizing: border-box;
  padding: min(18vh, 180px) 0;
  overflow-y: auto;
  color: rgba(245, 246, 248, 0.48);
  font-size: clamp(1.2rem, 1.6vw, 1.7rem);
  font-weight: 900;
  line-height: 1.55;
  text-align: center;
  scrollbar-width: none;
}

.line_lyrics_renderer::-webkit-scrollbar {
  display: none;
}

.line_lyrics_row,
.line_lyrics_state {
  max-width: min(100%, 760px);
  margin: 0 auto 18px;
  overflow-wrap: anywhere;
  transition:
    color 180ms ease,
    opacity 180ms ease,
    transform 180ms ease;
}

.line_lyrics_row.active {
  color: var(--theme-highlight-color, #3bce82);
  opacity: 1;
  transform: scale(1.04);
}

.line_lyrics_row.placeholder,
.line_lyrics_state {
  color: rgba(245, 246, 248, 0.36);
}
</style>
