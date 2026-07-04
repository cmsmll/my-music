<script setup lang="ts">
import maximize_icon from "../assets/icons/maximize.svg";
import minimize_icon from "../assets/icons/minimize.svg";
import refresh_icon from "../assets/icons/refresh.svg";
import search_icon from "../assets/icons/search.svg";
import settings_icon from "../assets/icons/settings.svg";
import tools_icon from "../assets/icons/tools.svg";
import x_icon from "../assets/icons/x.svg";
import { icon_style } from "../utils/track";

defineProps<{
  query: string;
}>();

const emit = defineEmits<{
  "update:query": [value: string];
  focus_search: [];
  open_tools: [];
  reload_library: [];
  open_settings: [];
  minimize_window: [];
  toggle_maximize_window: [];
  close_window: [];
  start_window_drag: [event: MouseEvent];
}>();
</script>

<template>
  <header class="top_bar" @mousedown="emit('start_window_drag', $event)">
    <label class="search_box">
      <span class="svg_icon" :style="icon_style(search_icon)" />
      <input
        :value="query"
        type="search"
        placeholder="搜索歌曲、歌手、专辑"
        @input="emit('update:query', ($event.target as HTMLInputElement).value)"
        @focus="emit('focus_search')"
      />
    </label>

    <div class="toolbar">
      <button class="tool_button" type="button" title="解码" @click="emit('open_tools')">
        <span class="svg_icon" :style="icon_style(tools_icon)" />
      </button>
      <button class="tool_button" type="button" title="重载" @click="emit('reload_library')">
        <span class="svg_icon" :style="icon_style(refresh_icon)" />
      </button>
      <button class="tool_button" type="button" title="设置" @click="emit('open_settings')">
        <span class="svg_icon" :style="icon_style(settings_icon)" />
      </button>
      <button class="window_button" type="button" title="最小化" @click="emit('minimize_window')">
        <span class="svg_icon" :style="icon_style(minimize_icon)" />
      </button>
      <button
        class="window_button"
        type="button"
        title="最大化"
        @click="emit('toggle_maximize_window')"
      >
        <span class="svg_icon" :style="icon_style(maximize_icon)" />
      </button>
      <button class="window_button close" type="button" title="关闭" @click="emit('close_window')">
        <span class="svg_icon" :style="icon_style(x_icon)" />
      </button>
    </div>
  </header>
</template>
