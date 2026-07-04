<script setup lang="ts">
import type { AlbumItem, ArtistItem, QueueSource, Track, ViewKey } from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration } from "../utils/track";

const props = defineProps<{
  active_view: ViewKey;
  query: string;
  loading: boolean;
  error_message: string;
  tracks: Track[];
  display_tracks: Track[];
  status_path?: string | null;
  is_playing: boolean;
  selected_artist: string;
  selected_album: string;
  playback_queue_source: QueueSource;
  artist_items: ArtistItem[];
  album_items: AlbumItem[];
  album_count: number;
  artist_count: number;
  total_duration: number;
}>();

const emit = defineEmits<{
  play_track: [track: Track];
  open_artist: [name: string];
  open_album: [name: string];
  close_detail: [];
}>();

function detail_total_duration() {
  return props.display_tracks.reduce((total, track) => total + (track.duration ?? 0), 0);
}

function visible_list_matches_playback_source() {
  const keyword = props.query.trim();
  if (keyword) {
    return props.playback_queue_source.type === "search" && props.playback_queue_source.id === keyword;
  }
  if (props.selected_artist) {
    return (
      props.playback_queue_source.type === "artist" &&
      props.playback_queue_source.id === props.selected_artist
    );
  }
  if (props.selected_album) {
    return props.playback_queue_source.type === "album" && props.playback_queue_source.id === props.selected_album;
  }
  return props.playback_queue_source.type === props.active_view;
}

function track_should_spin(track: Track) {
  return Boolean(
    props.is_playing &&
      track.path === props.status_path &&
      visible_list_matches_playback_source(),
  );
}

function artist_card_should_spin(name: string) {
  return Boolean(
    props.is_playing &&
      props.active_view === "artists" &&
      !props.selected_artist &&
      !props.query.trim() &&
      props.playback_queue_source.type === "artist" &&
      props.playback_queue_source.id === name,
  );
}

function album_card_should_spin(name: string) {
  return Boolean(
    props.is_playing &&
      props.active_view === "albums" &&
      !props.selected_album &&
      !props.query.trim() &&
      props.playback_queue_source.type === "album" &&
      props.playback_queue_source.id === name,
  );
}
</script>

<template>
  <section class="content_area">
    <p v-if="loading" class="status_line">正在加载曲库...</p>
    <p v-if="error_message" class="error_line">{{ error_message }}</p>

    <section
      v-if="
        active_view === 'all' ||
        active_view === 'recent' ||
        active_view === 'playlist_1' ||
        selected_artist ||
        selected_album ||
        query.trim()
      "
      class="track_table"
      aria-label="歌曲列表"
    >
      <header v-if="selected_artist || selected_album" class="detail_header">
        <button type="button" @click="emit('close_detail')">返回</button>
        <div class="detail_title">
          <strong>{{ selected_artist || selected_album }}</strong>
        </div>
        <span class="detail_meta">
          {{ display_tracks.length }} 首歌曲 {{ format_duration(detail_total_duration()) }}
        </span>
      </header>

      <div class="table_head">
        <span class="index_cell">#</span>
        <span>歌曲</span>
        <span>专辑</span>
        <span class="duration_cell">时长</span>
      </div>

      <button
        v-for="(track, index) in display_tracks"
        :key="track.id"
        class="table_row"
        :class="{ active: track.path === status_path }"
        type="button"
        @click="emit('play_track', track)"
      >
        <span class="index_cell">{{ index + 1 }}</span>
        <span class="song_cell">
          <span class="cover_thumb" :class="{ spinning_cover: track_should_spin(track) }">
            <img v-if="track.cover_cache_path" :src="cover_src(track)" alt="" />
            <span v-else>♪</span>
          </span>
          <span class="song_text">
            <strong>{{ display_title(track) }}</strong>
            <small>{{ display_artist(track) }}</small>
          </span>
        </span>
        <span class="album_cell">{{ display_album(track) }}</span>
        <span class="duration_cell">{{ format_duration(track.duration) }}</span>
      </button>

      <p v-if="!loading && !display_tracks.length" class="empty_state">
        没有找到歌曲，先添加音乐目录或调整搜索内容。
      </p>
    </section>

    <section v-else-if="active_view === 'albums'" class="placeholder_view">
      <div class="placeholder_grid">
        <button
          v-for="album in album_items"
          :key="album.name"
          class="album_tile media_tile"
          type="button"
          @click="emit('open_album', album.name)"
        >
          <span class="album_art" :class="{ spinning_cover: album_card_should_spin(album.name) }">
            <img v-if="album.cover_track?.cover_cache_path" :src="cover_src(album.cover_track)" alt="" />
            <span v-else>♪</span>
          </span>
          <strong :title="album.name">{{ album.name }}</strong>
          <small>{{ album.track_count }} 首歌曲 {{ format_duration(album.total_duration) }}</small>
        </button>
      </div>
      <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示专辑。</p>
    </section>

    <section v-else-if="active_view === 'artists'" class="placeholder_view">
      <div class="artist_grid">
        <button
          v-for="artist in artist_items"
          :key="artist.name"
          class="artist_tile media_tile"
          type="button"
          @click="emit('open_artist', artist.name)"
        >
          <span class="artist_art" :class="{ spinning_cover: artist_card_should_spin(artist.name) }">
            <img v-if="artist.cover_track?.cover_cache_path" :src="cover_src(artist.cover_track)" alt="" />
            <span v-else>♪</span>
          </span>
          <strong :title="artist.name">{{ artist.name }}</strong>
          <small>{{ artist.track_count }} 首歌曲 {{ format_duration(artist.total_duration) }}</small>
        </button>
      </div>
      <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示歌手。</p>
    </section>

    <section v-else-if="active_view === 'stats'" class="stats_view">
      <article>
        <strong>{{ tracks.length }}</strong>
        <span>歌曲</span>
      </article>
      <article>
        <strong>{{ album_count }}</strong>
        <span>专辑</span>
      </article>
      <article>
        <strong>{{ artist_count }}</strong>
        <span>歌手</span>
      </article>
      <article>
        <strong>{{ format_duration(total_duration) }}</strong>
        <span>总时长</span>
      </article>
    </section>

    <section v-else class="placeholder_view">
      <p class="empty_state">这个播放列表界面已经预留，后续会接入播放记录和自定义歌单。</p>
    </section>
  </section>
</template>
