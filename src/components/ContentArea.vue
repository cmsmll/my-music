<script setup lang="ts">
import type { AlbumItem, ArtistItem, QueueSource, StatRankItem, StatsOverview, Track, ViewKey } from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration, is_missing_track } from "../utils/track";

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
  selected_playlist_id: string;
  playback_queue_source: QueueSource;
  artist_items: ArtistItem[];
  album_items: AlbumItem[];
  album_count: number;
  artist_count: number;
  total_duration: number;
  stats_overview: StatsOverview;
}>();

const emit = defineEmits<{
  play_track: [track: Track];
  open_track_menu: [track: Track, event: MouseEvent];
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
  if (props.active_view === "playlist_1") {
    return (
      props.playback_queue_source.type === "playlist" &&
      props.playback_queue_source.id === props.selected_playlist_id
    );
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

function play_track(track: Track) {
  if (is_missing_track(track)) return;
  emit("play_track", track);
}

function format_file_size(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const unit_index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** unit_index;
  return `${value.toFixed(value >= 10 || unit_index === 0 ? 0 : 1)} ${units[unit_index]}`;
}

function format_stat_duration(seconds: number) {
  if (!seconds) return "0分钟";
  const whole_seconds = Math.floor(seconds);
  const hours = Math.floor(whole_seconds / 3600);
  const minutes = Math.floor((whole_seconds % 3600) / 60);
  if (hours <= 0) return `${Math.max(minutes, 1)}分钟`;
  return minutes > 0 ? `${hours}小时${minutes}分钟` : `${hours}小时`;
}

function rank_percent(item: StatRankItem, items: StatRankItem[]) {
  const max = Math.max(...items.map((rank_item) => rank_item.value), 1);
  return `${Math.max((item.value / max) * 100, 4)}%`;
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
        :class="{ active: track.path === status_path, missing: is_missing_track(track) }"
        type="button"
        @click="play_track(track)"
        @contextmenu.prevent="emit('open_track_menu', track, $event)"
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
      <div class="stats_overview">
        <article class="stat_card primary">
          <span>音乐总数</span>
          <strong>{{ stats_overview.total_tracks }}</strong>
          <small>{{ stats_overview.directory_count }} 个目录</small>
        </article>
        <article class="stat_card">
          <span>艺术家</span>
          <strong>{{ stats_overview.total_artists }}</strong>
          <small>{{ stats_overview.known_genre_count }} 个流派</small>
        </article>
        <article class="stat_card">
          <span>专辑</span>
          <strong>{{ stats_overview.total_albums }}</strong>
          <small>{{ stats_overview.playlist_count }} 个歌单</small>
        </article>
        <article class="stat_card">
          <span>总时长</span>
          <strong>{{ format_stat_duration(stats_overview.total_duration) }}</strong>
          <small>平均 {{ format_duration(stats_overview.average_duration) }}</small>
        </article>
        <article class="stat_card">
          <span>总大小</span>
          <strong>{{ format_file_size(stats_overview.total_size) }}</strong>
          <small>{{ stats_overview.format_distribution.length }} 种格式</small>
        </article>
        <article class="stat_card">
          <span>年份信息</span>
          <strong>{{ stats_overview.known_year_count }}</strong>
          <small>来自音频元数据</small>
        </article>
      </div>

      <div class="stats_detail_grid">
        <article class="stats_panel">
          <header>
            <strong>歌手排行</strong>
            <span>按歌曲数量</span>
          </header>
          <div class="rank_list">
            <div v-for="artist in stats_overview.top_artists" :key="artist.name" class="rank_item">
              <span class="rank_name" :title="artist.name">{{ artist.name }}</span>
              <span class="rank_value">{{ artist.value }}</span>
              <span class="rank_bar"><i :style="{ width: rank_percent(artist, stats_overview.top_artists) }" /></span>
              <small>{{ artist.description }}</small>
            </div>
            <p v-if="!stats_overview.top_artists.length" class="empty_state">暂无歌手数据。</p>
          </div>
        </article>

        <article class="stats_panel">
          <header>
            <strong>专辑排行</strong>
            <span>按歌曲数量</span>
          </header>
          <div class="rank_list">
            <div v-for="album in stats_overview.top_albums" :key="album.name" class="rank_item">
              <span class="rank_name" :title="album.name">{{ album.name }}</span>
              <span class="rank_value">{{ album.value }}</span>
              <span class="rank_bar"><i :style="{ width: rank_percent(album, stats_overview.top_albums) }" /></span>
              <small>{{ album.description }}</small>
            </div>
            <p v-if="!stats_overview.top_albums.length" class="empty_state">暂无专辑数据。</p>
          </div>
        </article>

        <article class="stats_panel">
          <header>
            <strong>格式分布</strong>
            <span>按文件类型</span>
          </header>
          <div class="pill_grid">
            <span v-for="format in stats_overview.format_distribution" :key="format.name">
              <strong>{{ format.name }}</strong>
              {{ format.value }}
            </span>
          </div>
          <p v-if="!stats_overview.format_distribution.length" class="empty_state">暂无格式数据。</p>
        </article>

        <article class="stats_panel">
          <header>
            <strong>元数据覆盖</strong>
            <span>来自标签解析</span>
          </header>
          <div class="pill_grid">
            <span v-for="genre in stats_overview.top_genres" :key="genre.name">
              <strong>{{ genre.name }}</strong>
              {{ genre.value }}
            </span>
            <span v-for="year in stats_overview.top_years" :key="year.name">
              <strong>{{ year.name }}</strong>
              {{ year.value }}
            </span>
          </div>
          <p v-if="!stats_overview.top_genres.length && !stats_overview.top_years.length" class="empty_state">
            暂无流派或年份数据。
          </p>
        </article>

        <article class="stats_panel wide">
          <header>
            <strong>时长边界</strong>
            <span>最长与最短歌曲</span>
          </header>
          <div class="stats_track_pair">
            <div>
              <span>最长</span>
              <strong :title="stats_overview.longest_track?.title">{{ display_title(stats_overview.longest_track) }}</strong>
              <small>{{ display_artist(stats_overview.longest_track) }} {{ format_duration(stats_overview.longest_track?.duration) }}</small>
            </div>
            <div>
              <span>最短</span>
              <strong :title="stats_overview.shortest_track?.title">{{ display_title(stats_overview.shortest_track) }}</strong>
              <small>{{ display_artist(stats_overview.shortest_track) }} {{ format_duration(stats_overview.shortest_track?.duration) }}</small>
            </div>
          </div>
        </article>
      </div>
    </section>

    <section v-else class="placeholder_view">
      <p class="empty_state">这个播放列表界面已经预留，后续会接入播放记录和自定义歌单。</p>
    </section>
  </section>
</template>
