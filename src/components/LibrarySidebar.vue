<script setup lang="ts">
import album_icon from "../assets/icons/album.svg";
import artist_icon from "../assets/icons/artist.svg";
import clock_fill_icon from "../assets/icons/clock-fill.svg";
import music_note_icon from "../assets/icons/music-note.svg";
import playlist_grid_icon from "../assets/icons/playlist-grid.svg";
import plus_icon from "../assets/icons/plus.svg";
import statistics_icon from "../assets/icons/statistics.svg";
import type { PlaylistCache, ViewKey } from "../types/music";
import { icon_style } from "../utils/track";

defineProps<{
  active_view: ViewKey;
  has_query: boolean;
  track_count: number;
  artist_count: number;
  album_count: number;
  recent_count: number;
  playlist_items: PlaylistCache[];
  active_playlist_id: string;
}>();

const emit = defineEmits<{
  show_view: [view: ViewKey];
  show_playlist: [playlist_id: string];
  create_playlist: [];
  open_playlist_menu: [playlist: PlaylistCache, event: MouseEvent];
  begin_resize: [event: PointerEvent];
}>();
</script>

<template>
  <aside class="sidebar">
    <nav class="sidebar_nav" aria-label="主导航">
      <section class="nav_group">
        <h2>音乐曲库</h2>
        <button
          class="nav_item"
          :class="{ active: active_view === 'all' && !has_query }"
          type="button"
          :title="String(track_count)"
          @click="emit('show_view', 'all')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(music_note_icon)" />
          <span>全部</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'artists' }"
          type="button"
          :title="String(artist_count)"
          @click="emit('show_view', 'artists')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(artist_icon)" />
          <span>歌手</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'albums' }"
          type="button"
          :title="String(album_count)"
          @click="emit('show_view', 'albums')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(album_icon)" />
          <span>专辑</span>
        </button>
        <button
          class="nav_item"
          :class="{ active: active_view === 'stats' }"
          type="button"
          @click="emit('show_view', 'stats')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(statistics_icon)" />
          <span>统计</span>
        </button>
      </section>

      <section class="nav_group playlist_group">
        <h2>播放列表</h2>
        <button
          class="nav_item"
          :class="{ active: active_view === 'recent' }"
          type="button"
          :title="String(recent_count)"
          @click="emit('show_view', 'recent')"
        >
          <span class="nav_icon svg_icon" :style="icon_style(clock_fill_icon)" />
          <span>最近播放</span>
        </button>
        <button
          v-for="playlist in playlist_items"
          :key="playlist.id"
          class="nav_item"
          :class="{ active: active_view === 'playlist_1' && active_playlist_id === playlist.id }"
          type="button"
          :title="String(playlist.metadata.track_count)"
          @click="emit('show_playlist', playlist.id)"
          @contextmenu.prevent="emit('open_playlist_menu', playlist, $event)"
        >
          <span class="nav_icon svg_icon" :style="icon_style(playlist_grid_icon)" />
          <span>{{ playlist.name }}</span>
        </button>
        <button class="nav_item create_playlist" type="button" @click="emit('create_playlist')">
          <span class="nav_icon svg_icon" :style="icon_style(plus_icon)" />
          <span>新建歌单</span>
        </button>
      </section>
    </nav>
    <div
      class="sidebar_resize_handle"
      role="separator"
      aria-orientation="vertical"
      aria-label="调整侧边栏宽度"
      @pointerdown="emit('begin_resize', $event)"
    />
  </aside>
</template>
