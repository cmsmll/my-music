<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import file_icon from "../assets/icons/file.svg";
import x_icon from "../assets/icons/x.svg";
import type { AppConfig } from "../types/music";
import { icon_style } from "../utils/track";

const props = defineProps<{
  app_config?: AppConfig | null;
}>();

const emit = defineEmits<{
  close: [];
  choose_music_directory: [];
}>();

type SettingsSectionKey = "library" | "decoder" | "cache" | "state" | "about";
type CacheEntryKey =
  | "library_cache_dir"
  | "cover_cache_dir"
  | "lyrics_cache_dir"
  | "my_playlist_cache_dir"
  | "play_statistics_cache_path"
  | "log_dir";

type CacheEntry = {
  key: CacheEntryKey;
  title: string;
  value: string;
  directory: boolean;
};

const settings_sections: { key: SettingsSectionKey; title: string }[] = [
  { key: "library", title: "音乐库" },
  { key: "decoder", title: "解码器" },
  { key: "cache", title: "缓存" },
  { key: "state", title: "状态" },
  { key: "about", title: "关于" },
];

const active_section = ref<SettingsSectionKey>("library");

const active_section_title = computed(
  () => settings_sections.find((section) => section.key === active_section.value)?.title ?? "音乐库",
);

const cache_path_overrides = ref<Partial<Record<CacheEntryKey, string>>>({});

const cache_entries = computed<CacheEntry[]>(() => [
  {
    key: "library_cache_dir",
    title: "曲库缓存目录",
    value: props.app_config?.library_cache_dir ?? "",
    directory: true,
  },
  {
    key: "cover_cache_dir",
    title: "封面缓存目录",
    value: props.app_config?.cover_cache_dir ?? "",
    directory: true,
  },
  {
    key: "lyrics_cache_dir",
    title: "歌词缓存目录",
    value: props.app_config?.lyrics_cache_dir ?? "",
    directory: true,
  },
  {
    key: "my_playlist_cache_dir",
    title: "我的歌单缓存目录",
    value: props.app_config?.my_playlist_cache_dir ?? "",
    directory: true,
  },
  {
    key: "play_statistics_cache_path",
    title: "播放统计缓存文件",
    value: props.app_config?.play_statistics_cache_path ?? "",
    directory: false,
  },
  {
    key: "log_dir",
    title: "日志目录",
    value: props.app_config?.log_dir ?? "",
    directory: true,
  },
]);

function cache_entry_value(entry: CacheEntry) {
  return cache_path_overrides.value[entry.key] ?? entry.value;
}

function reset_cache_entry(entry: CacheEntry) {
  const next_overrides = { ...cache_path_overrides.value };
  delete next_overrides[entry.key];
  cache_path_overrides.value = next_overrides;
}

async function choose_cache_path(entry: CacheEntry) {
  const selected = await open({
    directory: entry.directory,
    multiple: false,
    title: `选择${entry.title}`,
  });

  const selected_path = Array.isArray(selected) ? selected[0] : selected;
  if (typeof selected_path !== "string" || !selected_path) return;

  cache_path_overrides.value = {
    ...cache_path_overrides.value,
    [entry.key]: selected_path,
  };
}
</script>

<template>
  <div class="settings_overlay" @click.self="emit('close')">
    <aside class="settings_panel" aria-label="设置">
      <header>
        <div>
          <h2>设置</h2>
          <p>配置文件内容</p>
        </div>
        <button class="tool_button" type="button" title="关闭设置" @click="emit('close')">
          <span class="svg_icon" :style="icon_style(x_icon)" />
        </button>
      </header>

      <div class="settings_body">
        <nav class="settings_nav" aria-label="设置分类">
          <button
            v-for="section in settings_sections"
            :key="section.key"
            class="settings_nav_item"
            :class="{ active: active_section === section.key }"
            type="button"
            @click="active_section = section.key"
          >
            {{ section.title }}
          </button>
        </nav>

        <section class="settings_content" :aria-label="active_section_title">
          <section v-if="active_section === 'library'" class="settings_section">
            <h3>音乐库</h3>
            <div class="settings_field_group">
              <div class="settings_row">
                <div>
                  <strong>扫描目录</strong>
                  <span>用于扫描本地音乐文件夹</span>
                </div>
                <button class="primary_button" type="button" @click="emit('choose_music_directory')">添加目录</button>
              </div>
              <div v-if="app_config?.music_directory.length" class="path_list">
                <p v-for="directory in app_config.music_directory" :key="directory">{{ directory }}</p>
              </div>
              <p v-else class="muted">尚未选择音乐目录。</p>
            </div>
          </section>

          <section v-else-if="active_section === 'decoder'" class="settings_section">
            <h3>解码器</h3>
            <div class="settings_field_group">
              <label>
                <span>音频后端</span>
                <input value="rodio 0.22.2" readonly />
              </label>
              <label>
                <span>解码特性</span>
                <input value="symphonia-all" readonly />
              </label>
              <div class="settings_placeholder">
                <strong>解码参数</strong>
                <span>预留配置区域</span>
              </div>
            </div>
          </section>

          <section v-else-if="active_section === 'cache'" class="settings_section">
            <h3>缓存</h3>
            <div class="settings_field_group">
              <label v-for="entry in cache_entries" :key="entry.key">
                <span>{{ entry.title }}</span>
                <div class="settings_input_row">
                  <input :value="cache_entry_value(entry)" readonly />
                  <button
                    class="settings_default_button"
                    type="button"
                    :title="`恢复默认${entry.title}`"
                    @click="reset_cache_entry(entry)"
                  >
                    默认
                  </button>
                  <button
                    class="settings_file_button"
                    type="button"
                    :title="`选择${entry.title}`"
                    @click="choose_cache_path(entry)"
                  >
                    <span class="svg_icon" :style="icon_style(file_icon)" />
                  </button>
                </div>
              </label>
            </div>
          </section>

          <section v-else-if="active_section === 'state'" class="settings_section">
            <h3>状态</h3>
            <div class="settings_field_group">
              <label>
                <span>音量等级</span>
                <input value="播放器状态缓存" readonly />
              </label>
              <label>
                <span>页面大小</span>
                <input value="系统窗口状态" readonly />
              </label>
              <label>
                <span>侧边栏宽度</span>
                <input value="本地界面状态" readonly />
              </label>
              <label>
                <span>背景颜色</span>
                <input value="#ffffff" readonly />
              </label>
            </div>
          </section>

          <section v-else class="settings_section">
            <h3>关于</h3>
            <div class="settings_field_group">
              <label>
                <span>软件名称</span>
                <input value="my-music" readonly />
              </label>
              <label>
                <span>运行环境</span>
                <input value="Tauri 2 + Vue 3 + Rust" readonly />
              </label>
              <label>
                <span>软件描述</span>
                <input value="本地音乐播放器" readonly />
              </label>
            </div>
          </section>
        </section>
      </div>
    </aside>
  </div>
</template>
