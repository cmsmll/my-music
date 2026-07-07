<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";

const props = defineProps<{
  content_class?: string;
}>();

const emit = defineEmits<{
  scroll: [event: Event];
  resize: [];
}>();

const viewport = ref<HTMLElement | null>(null);
const content = ref<HTMLElement | null>(null);
const scroll_top = ref(0);
const viewport_height = ref(0);
const scroll_height = ref(0);
const dragging = ref(false);
let last_viewport_height = 0;
let last_scroll_height = 0;

let resize_observer: ResizeObserver | null = null;
let drag_start_y = 0;
let drag_start_scroll_top = 0;
let frame = 0;
const track_vertical_padding = 8;

const max_scroll_top = computed(() => Math.max(scroll_height.value - viewport_height.value, 0));
const can_scroll = computed(() => max_scroll_top.value > 1);
const track_height = computed(() => Math.max(viewport_height.value - track_vertical_padding, 0));
const thumb_height = computed(() => {
  if (!can_scroll.value || scroll_height.value <= 0) return 0;
  return Math.min(Math.max((viewport_height.value / scroll_height.value) * track_height.value, 32), track_height.value);
});
const thumb_top = computed(() => {
  if (!can_scroll.value) return 0;
  const max_thumb_top = Math.max(track_height.value - thumb_height.value, 0);
  return max_scroll_top.value > 0 ? (scroll_top.value / max_scroll_top.value) * max_thumb_top : 0;
});
const thumb_style = computed(() => ({
  height: `${thumb_height.value}px`,
  transform: `translateY(${thumb_top.value}px)`,
}));

function update_metrics() {
  if (!viewport.value) return;
  scroll_top.value = viewport.value.scrollTop;
  viewport_height.value = viewport.value.clientHeight;
  scroll_height.value = viewport.value.scrollHeight;
  if (last_viewport_height !== viewport_height.value || last_scroll_height !== scroll_height.value) {
    last_viewport_height = viewport_height.value;
    last_scroll_height = scroll_height.value;
    emit("resize");
  }
}

function schedule_update() {
  if (frame) return;
  frame = window.requestAnimationFrame(() => {
    frame = 0;
    update_metrics();
  });
}

function handle_scroll(event: Event) {
  update_metrics();
  emit("scroll", event);
}

function set_scroll_top(value: number) {
  if (!viewport.value) return;
  viewport.value.scrollTop = value;
  update_metrics();
}

function get_scroll_top() {
  return viewport.value?.scrollTop ?? 0;
}

function get_client_height() {
  return viewport.value?.clientHeight ?? 0;
}

function get_viewport() {
  return viewport.value;
}

function query_selector<T extends Element = Element>(selector: string) {
  return viewport.value?.querySelector<T>(selector) ?? null;
}

function scroll_to(options: ScrollToOptions) {
  viewport.value?.scrollTo(options);
  schedule_update();
}

function begin_drag(event: PointerEvent) {
  if (!viewport.value) return;
  dragging.value = true;
  drag_start_y = event.clientY;
  drag_start_scroll_top = viewport.value.scrollTop;
  event.currentTarget instanceof HTMLElement && event.currentTarget.setPointerCapture(event.pointerId);
  window.addEventListener("pointermove", drag_thumb);
  window.addEventListener("pointerup", end_drag);
  window.addEventListener("pointercancel", end_drag);
}

function drag_thumb(event: PointerEvent) {
  if (!viewport.value || !can_scroll.value) return;
  const max_thumb_top = Math.max(track_height.value - thumb_height.value, 1);
  const scroll_ratio = max_scroll_top.value / max_thumb_top;
  set_scroll_top(drag_start_scroll_top + (event.clientY - drag_start_y) * scroll_ratio);
}

function end_drag() {
  dragging.value = false;
  window.removeEventListener("pointermove", drag_thumb);
  window.removeEventListener("pointerup", end_drag);
  window.removeEventListener("pointercancel", end_drag);
}

function jump_to(event: PointerEvent) {
  if (!viewport.value || !can_scroll.value) return;
  const target = event.target as HTMLElement | null;
  if (target?.classList.contains("custom_scrollbar_thumb")) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const next_thumb_top = event.clientY - rect.top - thumb_height.value / 2;
  const max_thumb_top = Math.max(track_height.value - thumb_height.value, 1);
  set_scroll_top((next_thumb_top / max_thumb_top) * max_scroll_top.value);
}

onMounted(async () => {
  await nextTick();
  update_metrics();
  if (viewport.value) {
    resize_observer = new ResizeObserver(schedule_update);
    resize_observer.observe(viewport.value);
    if (content.value) resize_observer.observe(content.value);
  }
});

onBeforeUnmount(() => {
  resize_observer?.disconnect();
  resize_observer = null;
  if (frame) window.cancelAnimationFrame(frame);
  end_drag();
});

defineExpose({
  viewport,
  refresh: update_metrics,
  set_scroll_top,
  get_scroll_top,
  get_client_height,
  get_viewport,
  query_selector,
  scroll_to,
});
</script>

<template>
  <div class="custom_scrollbar" :class="{ dragging, scrollable: can_scroll }">
    <div ref="viewport" class="custom_scrollbar_viewport" @scroll="handle_scroll">
      <div ref="content" class="custom_scrollbar_content" :class="props.content_class">
        <slot />
      </div>
    </div>
    <div v-if="can_scroll" class="custom_scrollbar_track" @pointerdown="jump_to">
      <div class="custom_scrollbar_thumb" :style="thumb_style" @pointerdown.stop="begin_drag" />
    </div>
  </div>
</template>

<style scoped>
.custom_scrollbar {
  position: relative;
  min-width: 0;
  min-height: 0;
  overflow: visible;
}

.custom_scrollbar_viewport {
  width: 100%;
  height: 100%;
  overflow: hidden auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.custom_scrollbar_viewport::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.custom_scrollbar_content {
  min-height: 100%;
}

.custom_scrollbar_track {
  position: absolute;
  top: 4px;
  right: -10px;
  bottom: 4px;
  z-index: 5;
  width: 10px;
  opacity: 0;
  cursor: pointer;
  transition: opacity 0.15s ease;
}

.custom_scrollbar:hover .custom_scrollbar_track,
.custom_scrollbar.dragging .custom_scrollbar_track {
  opacity: 1;
}

.custom_scrollbar_thumb {
  position: absolute;
  top: 0;
  left: 2px;
  width: 6px;
  border-radius: 999px;
  background: rgba(136, 150, 176, 0.42);
  cursor: pointer;
  transition: background 0.15s ease, width 0.15s ease, left 0.15s ease;
}

.custom_scrollbar_thumb:hover,
.custom_scrollbar.dragging .custom_scrollbar_thumb {
  left: 1px;
  width: 8px;
  background: var(--theme-highlight-color, #3bce82);
}
</style>
