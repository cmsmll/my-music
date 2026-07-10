<script setup lang="ts">
import { storeToRefs } from "pinia";
import maximize_icon from "../assets/icons/maximize.svg";
import minimize_icon from "../assets/icons/minimize.svg";
import refresh_icon from "../assets/icons/refresh.svg";
import search_icon from "../assets/icons/search.svg";
import settings_icon from "../assets/icons/settings.svg";
import tools_icon from "../assets/icons/tools.svg";
import x_icon from "../assets/icons/x.svg";
import { use_library_view_store } from "../stores/library_view";
import { use_ui_store } from "../stores/ui";
import { use_app_actions_store } from "../stores/app_actions";
import { icon_style } from "../utils/track";

const library_view = use_library_view_store();
const ui_store = use_ui_store();
const app_actions = use_app_actions_store();
const { query } = storeToRefs(library_view);
</script>

<template>
  <header class="top_bar" @mousedown="app_actions.start_window_drag($event)">
    <label class="search_box">
      <span class="svg_icon" :style="icon_style(search_icon)" />
      <input
        :value="query"
        type="search"
        placeholder="搜索歌曲、歌手、专辑"
        @input="library_view.update_query(($event.target as HTMLInputElement).value)"
        @focus="library_view.focus_search()"
      />
    </label>

    <div class="toolbar">
      <button class="tool_button hover_border_control" type="button" title="解码" @click="app_actions.decode_music_files()">
        <span class="svg_icon" :style="icon_style(tools_icon)" />
      </button>
      <button class="tool_button hover_border_control" type="button" title="重载" @click="app_actions.reload_library()">
        <span class="svg_icon" :style="icon_style(refresh_icon)" />
      </button>
      <button class="tool_button hover_border_control" type="button" title="设置" @click="ui_store.open_settings()">
        <span class="svg_icon" :style="icon_style(settings_icon)" />
      </button>
      <button class="window_button hover_border_control" type="button" title="最小化" @click="app_actions.minimize_window()">
        <span class="svg_icon" :style="icon_style(minimize_icon)" />
      </button>
      <button
        class="window_button hover_border_control"
        type="button"
        title="最大化"
        @click="app_actions.toggle_maximize_window()"
      >
        <span class="svg_icon" :style="icon_style(maximize_icon)" />
      </button>
      <button class="window_button close hover_border_control" type="button" title="关闭" @click="app_actions.close_window()">
        <span class="svg_icon" :style="icon_style(x_icon)" />
      </button>
    </div>
  </header>
</template>

<style>
.top_bar {
  display: grid;
  justify-content: center;
  align-items: center;
  grid-template-columns: 1fr 260px;
  border-bottom: var(--app_border_width, 2px) solid var(--theme-border-color, #e8e8e8);
  padding-right: 28px;
  cursor: move;
  user-select: none;
}

.search_box {
  display: flex;
  align-items: center;
  justify-content: center;
  justify-self: center;
  gap: 12px;
  flex: 1;
  width: 60%;
  min-width: 300px;
  height: 52px;
  border: var(--app_border_width, 2px) solid var(--theme-border-color, #e8e8e8);
  border-radius: 8px;
  padding: 0 18px;
  color: #858b96;
  background: transparent;
  cursor: text;
  user-select: auto;
}

.search_box input {
  width: 100%;
  border: 0;
  outline: 0;
  color: var(--theme-title-color, #1e2026);
  background: transparent;
  font-size: 1rem;
  user-select: none;
  -webkit-user-select: none;
}

.search_box .svg_icon {
  width: 18px;
  height: 18px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-shrink: 0;
  cursor: default;
}

.window_button.close {
  font-weight: 900;
}
</style>
