<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";

type LineLyricItem = {
  key: string;
  time: number | null;
  text: string;
};

const props = withDefaults(defineProps<{
  lyrics: string;
  elapsed: number;
  loading?: boolean;
  placeholder?: string[];
}>(), {
  loading: false,
  placeholder: () => ["暂未获取到歌词"],
});

const active_line = ref<HTMLElement | null>(null);

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

const active_index = computed(() => {
  if (!has_timed_lyrics.value) return -1;

  let index = -1;
  for (let current = 0; current < lyric_lines.value.length; current += 1) {
    const time = lyric_lines.value[current].time;
    if (time === null) continue;
    if (time <= props.elapsed + 0.08) {
      index = current;
    } else {
      break;
    }
  }
  return index;
});

watch(active_index, async () => {
  await nextTick();
  active_line.value?.scrollIntoView({
    block: "center",
    behavior: "smooth",
  });
});
</script>

<template>
  <section class="line_lyrics_renderer" aria-label="歌词">
    <p v-if="loading" class="line_lyrics_state">正在读取歌词...</p>
    <template v-else>
      <p
        v-for="(line, index) in visible_lines"
        :key="line.key"
        :ref="index === active_index ? (element) => { active_line = element as HTMLElement | null; } : undefined"
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
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 18px;
  height: min(44vh, 430px);
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
  margin: 0;
  overflow-wrap: anywhere;
  transition:
    color 180ms ease,
    opacity 180ms ease,
    transform 180ms ease;
}

.line_lyrics_row.active {
  color: #ffffff;
  opacity: 1;
  transform: scale(1.04);
}

.line_lyrics_row.placeholder,
.line_lyrics_state {
  color: rgba(245, 246, 248, 0.36);
}
</style>
