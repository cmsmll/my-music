<script setup lang="ts">
import { computed, ref } from "vue";
import CustomScrollbar from "./CustomScrollbar.vue";
import { open } from "@tauri-apps/plugin-dialog";
import delete_icon from "../assets/icons/delete.svg";
import folder_open_icon from "../assets/icons/folder-open.svg";
import system_icon from "../assets/icons/system.svg";
import x_icon from "../assets/icons/x.svg";
import { use_app_config_store } from "../stores/app_config";
import type { AppConfig } from "../types/music";
import { icon_style } from "../utils/track";

const props = defineProps<{
  app_config?: AppConfig | null;
}>();

const emit = defineEmits<{
  close: [];
  choose_music_directory: [];
}>();

type SettingsSectionKey = "library" | "decoder" | "cache" | "style" | "about";
type CacheEntryKey =
  | "library_cache_dir"
  | "playlist_cache_dir"
  | "lyrics_cache_dir"
  | "cover_cache_dir"
  | "log_cache_dir";

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
  { key: "style", title: "样式" },
  { key: "about", title: "关于" },
];

const app_config_store = use_app_config_store();
const active_section = ref<SettingsSectionKey>("library");
const app_version = __APP_VERSION__;
const github_url = "https://github.com/cmsmll/my-music";

const current_config = computed(
  () => app_config_store.config ?? props.app_config ?? null,
);

const default_config = computed(() => app_config_store.default_config);

const decoder_output_directory = computed(
  () => current_config.value?.decoder.output_dir ?? "",
);

const decoder_process_formats = computed({
  get: () => current_config.value?.decoder.process_formats ?? "mp3,flac,kgm,kgma,ncm",
  set: (value: string) => {
    app_config_store.update_decoder({ process_formats: value });
  },
});

const decoder_directories = computed(
  () => current_config.value?.decoder.scan_directory ?? [],
);

const background_color = computed({
  get: () => current_config.value?.style.background_color ?? "#ffffff",
  set: (value: string) => {
    app_config_store.update_style({ background_color: value });
  },
});

const title_color = computed({
  get: () => current_config.value?.style.title_color ?? "#1e2026",
  set: (value: string) => {
    app_config_store.update_style({ title_color: value });
  },
});

const subtitle_color = computed({
  get: () => current_config.value?.style.subtitle_color ?? "#8b919c",
  set: (value: string) => {
    app_config_store.update_style({ subtitle_color: value });
  },
});

const highlight_color = computed({
  get: () => current_config.value?.style.highlight_color ?? "#3bce82",
  set: (value: string) => {
    app_config_store.update_style({ highlight_color: value });
  },
});

const border_color = computed({
  get: () => current_config.value?.style.border_color ?? "#e8e8e8",
  set: (value: string) => {
    app_config_store.update_style({ border_color: value });
  },
});

const show_border = computed({
  get: () => current_config.value?.style.show_border ?? true,
  set: (value: boolean) => {
    app_config_store.update_style({ show_border: value });
  },
});

const background_image = computed(
  () => current_config.value?.style.background_image ?? "",
);

const background_image_opacity = computed({
  get: () => current_config.value?.style.background_image_opacity ?? 1,
  set: (value: number | string) => {
    const opacity = Number(value);
    app_config_store.update_style({
      background_image_opacity: Number.isFinite(opacity)
        ? Math.min(Math.max(opacity, 0), 1)
        : 1,
    });
  },
});

const background_image_opacity_percent = computed(() =>
  Math.round(background_image_opacity.value * 100),
);

const active_section_title = computed(
  () =>
    settings_sections.find((section) => section.key === active_section.value)
      ?.title ?? "音乐库",
);

const music_directories = computed(
  () => current_config.value?.music_directory ?? [],
);

const cache_entries = computed<CacheEntry[]>(() => [
  {
    key: "library_cache_dir",
    title: "曲库缓存目录",
    value: current_config.value?.cache.library_cache_dir ?? "",
    directory: true,
  },
  {
    key: "playlist_cache_dir",
    title: "歌单缓存目录",
    value: current_config.value?.cache.playlist_cache_dir ?? "",
    directory: true,
  },
  {
    key: "lyrics_cache_dir",
    title: "歌词缓存目录",
    value: current_config.value?.cache.lyrics_cache_dir ?? "",
    directory: true,
  },
  {
    key: "cover_cache_dir",
    title: "封面缓存目录",
    value: current_config.value?.cache.cover_cache_dir ?? "",
    directory: true,
  },
  {
    key: "log_cache_dir",
    title: "日志缓存目录",
    value: current_config.value?.cache.log_cache_dir ?? "",
    directory: true,
  },
]);

function cache_entry_value(entry: CacheEntry) {
  return entry.value;
}

function reset_cache_entry(entry: CacheEntry) {
  const default_value = default_config.value?.cache[entry.key] ?? "";
  app_config_store.update_config((config) => {
    return {
      ...config,
      cache: {
        ...config.cache,
        [entry.key]: default_value,
      },
    };
  });
}

function remove_music_directory(directory: string) {
  app_config_store.update_config({
    music_directory: music_directories.value.filter(
      (current) => current !== directory,
    ),
  });
}

function reset_decoder_output_directory() {
  app_config_store.update_decoder({
    output_dir: default_config.value?.decoder.output_dir ?? "",
  });
}

function remove_decoder_directory(directory: string) {
  app_config_store.update_decoder({
    scan_directory: decoder_directories.value.filter(
      (current) => current !== directory,
    ),
  });
}

async function choose_decoder_output_directory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择解码输出目录",
  });

  const selected_path = Array.isArray(selected) ? selected[0] : selected;
  if (typeof selected_path !== "string" || !selected_path) return;

  app_config_store.update_decoder({ output_dir: selected_path });
}

async function choose_decoder_directory() {
  const selected = await open({
    directory: true,
    multiple: true,
    title: "选择加密音频目录",
  });

  const selected_paths = Array.isArray(selected) ? selected : [selected];
  const directories = selected_paths.filter(
    (selected_path): selected_path is string =>
      typeof selected_path === "string" && Boolean(selected_path),
  );
  if (!directories.length) return;

  app_config_store.update_decoder({
    scan_directory: Array.from(
      new Set([...decoder_directories.value, ...directories]),
    ),
  });
}

function reset_background_color() {
  app_config_store.update_style({
    background_color: default_config.value?.style.background_color ?? "#ffffff",
  });
}

function reset_title_color() {
  app_config_store.update_style({
    title_color: default_config.value?.style.title_color ?? "#1e2026",
  });
}

function reset_subtitle_color() {
  app_config_store.update_style({
    subtitle_color: default_config.value?.style.subtitle_color ?? "#8b919c",
  });
}

function reset_highlight_color() {
  app_config_store.update_style({
    highlight_color: default_config.value?.style.highlight_color ?? "#3bce82",
  });
}

function reset_border_color() {
  app_config_store.update_style({
    border_color: default_config.value?.style.border_color ?? "#e8e8e8",
  });
}

function reset_background_image() {
  app_config_store.update_style({
    background_image: default_config.value?.style.background_image ?? "",
    background_image_opacity:
      default_config.value?.style.background_image_opacity ?? 1,
  });
}

function reset_background_image_opacity() {
  app_config_store.update_style({
    background_image_opacity:
      default_config.value?.style.background_image_opacity ?? 1,
  });
}

async function choose_background_image() {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "选择背景图片",
    filters: [
      {
        name: "图片",
        extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"],
      },
    ],
  });

  const selected_path = Array.isArray(selected) ? selected[0] : selected;
  if (typeof selected_path !== "string" || !selected_path) return;

  app_config_store.update_style({ background_image: selected_path });
}

async function choose_cache_path(entry: CacheEntry) {
  const selected = await open({
    directory: entry.directory,
    multiple: false,
    title: `选择${entry.title}`,
  });

  const selected_path = Array.isArray(selected) ? selected[0] : selected;
  if (typeof selected_path !== "string" || !selected_path) return;

  app_config_store.update_config((config) => {
    return {
      ...config,
      cache: {
        ...config.cache,
        [entry.key]: selected_path,
      },
    };
  });
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
        <button
          class="tool_button hover_border_control"
          type="button"
          title="关闭设置"
          @click="emit('close')"
        >
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

        <CustomScrollbar
          class="settings_content"
          content_class="settings_content_inner"
          :aria-label="active_section_title"
        >
          <section v-if="active_section === 'library'" class="settings_section">
            <h3>音乐库</h3>
            <div class="settings_field_group">
              <div class="settings_row">
                <div>
                  <strong>扫描目录</strong>
                  <span>用于扫描本地音乐文件夹</span>
                </div>
                <button
                  class="primary_button"
                  type="button"
                  @click="emit('choose_music_directory')"
                >
                  添加目录
                </button>
              </div>
              <div v-if="music_directories.length" class="path_list">
                <div
                  v-for="directory in music_directories"
                  :key="directory"
                  class="path_list_row"
                >
                  <p>{{ directory }}</p>
                  <button
                    class="settings_delete_button hover_border_control"
                    type="button"
                    title="删除音乐目录"
                    @click="remove_music_directory(directory)"
                  >
                    <span class="svg_icon" :style="icon_style(delete_icon)" />
                  </button>
                </div>
              </div>
              <p v-else class="muted">尚未选择音乐目录。</p>
            </div>
          </section>

          <section
            v-else-if="active_section === 'decoder'"
            class="settings_section"
          >
            <h3>解码器</h3>
            <div class="settings_field_group">
              <label>
                <span>输出目录</span>
                <div class="settings_input_row">
                  <input
                    :value="decoder_output_directory"
                    placeholder="默认输出目录"
                    readonly
                  />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认输出目录"
                    @click="reset_decoder_output_directory"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <button
                    class="settings_file_button hover_border_control"
                    type="button"
                    title="选择输出目录"
                    @click="choose_decoder_output_directory"
                  >
                    <span
                      class="svg_icon"
                      :style="icon_style(folder_open_icon)"
                    />
                  </button>
                </div>
              </label>
              <label>
                <span>处理格式</span>
                <input
                  v-model="decoder_process_formats"
                  placeholder="mp3,flac,kgm,kgma,ncm"
                />
              </label>
              <div class="settings_row">
                <div>
                  <strong>扫描目录</strong>
                  <span>用于扫描并解锁加密格式的音频文件夹</span>
                </div>
                <button
                  class="primary_button"
                  type="button"
                  @click="choose_decoder_directory"
                >
                  添加目录
                </button>
              </div>
              <div v-if="decoder_directories.length" class="path_list">
                <div
                  v-for="directory in decoder_directories"
                  :key="directory"
                  class="path_list_row"
                >
                  <p>{{ directory }}</p>
                  <button
                    class="settings_delete_button hover_border_control"
                    type="button"
                    title="删除加密音频目录"
                    @click="remove_decoder_directory(directory)"
                  >
                    <span class="svg_icon" :style="icon_style(delete_icon)" />
                  </button>
                </div>
              </div>
              <p v-else class="muted">尚未选择加密音频目录。</p>
            </div>
          </section>

          <section
            v-else-if="active_section === 'cache'"
            class="settings_section"
          >
            <h3>缓存</h3>
            <div class="settings_field_group">
              <label v-for="entry in cache_entries" :key="entry.key">
                <span>{{ entry.title }}</span>
                <div class="settings_input_row">
                  <input :value="cache_entry_value(entry)" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    :title="`恢复默认${entry.title}`"
                    @click="reset_cache_entry(entry)"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <button
                    class="settings_file_button hover_border_control"
                    type="button"
                    :title="`选择${entry.title}`"
                    @click="choose_cache_path(entry)"
                  >
                    <span
                      class="svg_icon"
                      :style="icon_style(folder_open_icon)"
                    />
                  </button>
                </div>
              </label>
            </div>
          </section>

          <section
            v-else-if="active_section === 'style'"
            class="settings_section"
          >
            <h3>样式</h3>
            <div class="settings_field_group">
              <label>
                <span>背景颜色</span>
                <div class="settings_input_row">
                  <input :value="background_color" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认背景颜色"
                    @click="reset_background_color"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <input
                    v-model="background_color"
                    class="settings_color_picker"
                    type="color"
                    title="选择背景颜色"
                  />
                </div>
              </label>
              <label>
                <span>标题颜色</span>
                <div class="settings_input_row">
                  <input :value="title_color" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认标题颜色"
                    @click="reset_title_color"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <input
                    v-model="title_color"
                    class="settings_color_picker"
                    type="color"
                    title="选择标题颜色"
                  />
                </div>
              </label>
              <label>
                <span>副标题颜色</span>
                <div class="settings_input_row">
                  <input :value="subtitle_color" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认副标题颜色"
                    @click="reset_subtitle_color"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <input
                    v-model="subtitle_color"
                    class="settings_color_picker"
                    type="color"
                    title="选择副标题颜色"
                  />
                </div>
              </label>
              <label>
                <span>高亮色</span>
                <div class="settings_input_row">
                  <input :value="highlight_color" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认高亮色"
                    @click="reset_highlight_color"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <input
                    v-model="highlight_color"
                    class="settings_color_picker"
                    type="color"
                    title="选择高亮色"
                  />
                </div>
              </label>
              <label>
                <span>边框色</span>
                <div class="settings_input_row">
                  <input :value="border_color" readonly />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认边框色"
                    @click="reset_border_color"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <input
                    v-model="border_color"
                    class="settings_color_picker"
                    type="color"
                    title="选择边框色"
                  />
                </div>
              </label>
              <div class="settings_radio_field">
                <span>是否显示边框</span>
                <div class="settings_radio_group">
                  <label
                    class="settings_radio_option"
                    :class="{ active: show_border }"
                  >
                    <input v-model="show_border" type="radio" :value="true" />
                    <span>显示</span>
                  </label>
                  <label
                    class="settings_radio_option"
                    :class="{ active: !show_border }"
                  >
                    <input v-model="show_border" type="radio" :value="false" />
                    <span>不显示</span>
                  </label>
                </div>
              </div>
              <label>
                <span>背景图片</span>
                <div class="settings_input_row">
                  <input
                    :value="background_image"
                    placeholder="默认空"
                    readonly
                  />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认背景图片"
                    @click="reset_background_image"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                  <button
                    class="settings_file_button hover_border_control"
                    type="button"
                    title="选择背景图片"
                    @click="choose_background_image"
                  >
                    <span
                      class="svg_icon"
                      :style="icon_style(folder_open_icon)"
                    />
                  </button>
                </div>
                <div class="settings_opacity_control">
                  <span>透明度 {{ background_image_opacity_percent }}%</span>
                  <input
                    v-model.number="background_image_opacity"
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    title="调节背景图片透明度"
                  />
                  <button
                    class="settings_default_button hover_border_control"
                    type="button"
                    title="恢复默认背景图片透明度"
                    @click="reset_background_image_opacity"
                  >
                    <span class="svg_icon" :style="icon_style(system_icon)" />
                  </button>
                </div>
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
                <span>版本信息</span>
                <input :value="app_version" readonly />
              </label>
              <label>
                <span>GitHub 地址</span>
                <input :value="github_url" readonly />
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
        </CustomScrollbar>
      </div>
    </aside>
  </div>
</template>
