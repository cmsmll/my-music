<script setup lang="ts">
import x_icon from "../assets/icons/x.svg";
import type { AppConfig } from "../types/music";
import { icon_style } from "../utils/track";

defineProps<{
  app_config?: AppConfig | null;
}>();

const emit = defineEmits<{
  close: [];
  choose_music_directory: [];
}>();
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

      <section class="settings_section">
        <h3>音乐目录</h3>
        <div v-if="app_config?.music_directory.length" class="path_list">
          <p v-for="directory in app_config.music_directory" :key="directory">{{ directory }}</p>
        </div>
        <p v-else class="muted">尚未选择音乐目录。</p>
        <button class="primary_button" type="button" @click="emit('choose_music_directory')">添加音乐目录</button>
      </section>

      <section class="settings_section">
        <h3>缓存位置</h3>
        <label>
          <span>library_cache_dir</span>
          <input :value="app_config?.library_cache_dir ?? ''" readonly />
        </label>
        <label>
          <span>cover_cache_dir</span>
          <input :value="app_config?.cover_cache_dir ?? ''" readonly />
        </label>
        <label>
          <span>lyrics_cache_dir</span>
          <input :value="app_config?.lyrics_cache_dir ?? ''" readonly />
        </label>
        <label>
          <span>my_playlist_cache_dir</span>
          <input :value="app_config?.my_playlist_cache_dir ?? ''" readonly />
        </label>
      </section>
    </aside>
  </div>
</template>
