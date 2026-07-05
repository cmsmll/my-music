<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { AlbumItem, ArtistItem, PlayStatistics, QueueSource, Track, TrackPlayStatistic, ViewKey } from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration, format_file_size, is_missing_track } from "../utils/track";

const props = defineProps<{
  active_view: ViewKey;
  query: string;
  loading: boolean;
  error_message: string;
  tracks: Track[];
  display_tracks: Track[];
  status_path?: string | null;
  locate_track_request: number;
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
  total_size: number;
  play_statistics: PlayStatistics;
}>();

const emit = defineEmits<{
  play_track: [track: Track];
  open_track_menu: [track: Track, event: MouseEvent];
  open_artist: [name: string];
  open_album: [name: string];
  close_detail: [];
}>();

const track_table = ref<HTMLElement | null>(null);
const scroll_top = ref(0);
const viewport_height = ref(720);
const track_row_height = 74;
const virtual_overscan = 8;

let resize_observer: ResizeObserver | null = null;

const virtual_start_index = computed(() =>
  Math.max(Math.floor(scroll_top.value / track_row_height) - virtual_overscan, 0),
);
const virtual_visible_count = computed(() =>
  Math.ceil(viewport_height.value / track_row_height) + virtual_overscan * 2,
);
const virtual_track_rows = computed(() => {
  const start = virtual_start_index.value;
  return props.display_tracks
    .slice(start, start + virtual_visible_count.value)
    .map((track, offset) => ({
      track,
      index: start + offset,
    }));
});
const virtual_top_padding = computed(() => virtual_start_index.value * track_row_height);
const virtual_bottom_padding = computed(() =>
  Math.max(
    (props.display_tracks.length - virtual_start_index.value - virtual_track_rows.value.length) *
      track_row_height,
    0,
  ),
);

function update_virtual_viewport() {
  if (!track_table.value) return;
  viewport_height.value = track_table.value.clientHeight || viewport_height.value;
  scroll_top.value = track_table.value.scrollTop;
}

function handle_track_table_scroll() {
  if (!track_table.value) return;
  scroll_top.value = track_table.value.scrollTop;
}

async function locate_status_track() {
  if (!props.status_path) return;

  await nextTick();

  const index = props.display_tracks.findIndex((track) => track.path === props.status_path);
  if (index < 0 || !track_table.value) return;

  const top = Math.max(index * track_row_height - track_table.value.clientHeight / 2 + track_row_height / 2, 0);
  track_table.value.scrollTop = top;
  update_virtual_viewport();
}

function detail_total_duration() {
  return props.display_tracks.reduce((total, track) => total + (track.duration ?? 0), 0);
}

function visible_list_matches_playback_source() {
  const keyword = props.query.trim();
  if (keyword) {
    return props.playback_queue_source.type === "all";
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
  if (props.active_view === "user_playlist") {
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

const most_played_tracks = computed(() =>
  Object.values(props.play_statistics.tracks)
    .filter((track) => track.play_count > 0)
    .sort((left, right) => {
      if (right.play_count !== left.play_count) return right.play_count - left.play_count;
      return right.listening_seconds - left.listening_seconds;
    })
    .slice(0, 20),
);

const favorite_artist = computed(() => favorite_group("artist"));
const favorite_album = computed(() => favorite_group("album"));

function favorite_group(field: "artist" | "album") {
  const groups = new Map<string, number>();

  for (const track of Object.values(props.play_statistics.tracks)) {
    const name = (field === "artist" ? track.artist : track.album).trim();
    const fallback = field === "artist" ? "未知歌手" : "未知专辑";
    groups.set(name || fallback, (groups.get(name || fallback) ?? 0) + track.play_count);
  }

  return (
    Array.from(groups.entries()).sort((left, right) => {
      if (right[1] !== left[1]) return right[1] - left[1];
      return left[0].localeCompare(right[0], "zh-Hans-CN");
    })[0]?.[0] ?? "--"
  );
}

function statistic_track_title(track: TrackPlayStatistic) {
  return track.title.trim() || track.track_id;
}

function statistic_track_artist(track: TrackPlayStatistic) {
  return track.artist.trim() || "未知歌手";
}

function format_stat_duration(seconds: number) {
  return seconds > 0 ? format_duration(seconds) : "0:00";
}

watch(
  () => [props.active_view, props.query, props.selected_artist, props.selected_album, props.selected_playlist_id],
  async () => {
    await nextTick();
    if (track_table.value) {
      track_table.value.scrollTop = 0;
    }
    update_virtual_viewport();
  },
);

watch(
  () => props.display_tracks.length,
  () => {
    void nextTick(update_virtual_viewport);
  },
);

watch(
  () => props.locate_track_request,
  () => {
    void locate_status_track();
  },
);

onMounted(() => {
  update_virtual_viewport();
  if (track_table.value) {
    resize_observer = new ResizeObserver(update_virtual_viewport);
    resize_observer.observe(track_table.value);
  }
});

onBeforeUnmount(() => {
  resize_observer?.disconnect();
  resize_observer = null;
});
</script>

<template>
  <section class="content_area">
    <p v-if="loading" class="status_line">正在加载曲库...</p>
    <p v-if="error_message" class="error_line">{{ error_message }}</p>

    <section
      v-if="
        active_view === 'all' ||
        active_view === 'recent' ||
        active_view === 'user_playlist' ||
        selected_artist ||
        selected_album ||
        query.trim()
      "
      class="track_table_view"
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

      <section
        class="track_table"
        ref="track_table"
        @scroll="handle_track_table_scroll"
      >
        <div class="virtual_track_spacer" :style="{ height: `${virtual_top_padding}px` }" />
        <button
          v-for="row in virtual_track_rows"
          :key="row.track.id"
          class="table_row"
          :class="{ active: row.track.path === status_path, missing: is_missing_track(row.track) }"
          type="button"
          @click="play_track(row.track)"
          @contextmenu.prevent="emit('open_track_menu', row.track, $event)"
        >
          <span class="index_cell">{{ row.index + 1 }}</span>
          <span class="song_cell">
            <span class="cover_thumb" :class="{ spinning_cover: track_should_spin(row.track) }">
              <img v-if="row.track.cover_cache_path" :src="cover_src(row.track)" alt="" />
              <span v-else>♪</span>
            </span>
            <span class="song_text">
              <strong>{{ display_title(row.track) }}</strong>
              <small>{{ display_artist(row.track) }}</small>
            </span>
          </span>
          <span class="album_cell">{{ display_album(row.track) }}</span>
          <span class="duration_cell">{{ format_duration(row.track.duration) }}</span>
        </button>
        <div class="virtual_track_spacer" :style="{ height: `${virtual_bottom_padding}px` }" />

        <p v-if="!loading && !display_tracks.length" class="empty_state">
          没有找到歌曲，先添加音乐目录或调整搜索内容。
        </p>
      </section>
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
      <section class="stats_section">
        <h2>音乐统计</h2>
        <div class="stats_card_grid music_stats_grid">
          <article>
            <strong>{{ tracks.length }}</strong>
            <span>歌曲</span>
          </article>
          <article>
            <strong>{{ artist_count }}</strong>
            <span>歌手</span>
          </article>
          <article>
            <strong>{{ album_count }}</strong>
            <span>专辑</span>
          </article>
          <article>
            <strong>{{ format_stat_duration(total_duration) }}</strong>
            <span>总时长</span>
          </article>
          <article>
            <strong>{{ format_file_size(total_size) }}</strong>
            <span>总大小</span>
          </article>
        </div>
      </section>

      <section class="stats_section">
        <h2>播放统计</h2>
        <div class="stats_card_grid">
          <article>
            <strong>{{ play_statistics.total_play_count }}</strong>
            <span>累计播放</span>
          </article>
          <article>
            <strong>{{ format_stat_duration(play_statistics.total_listening_seconds) }}</strong>
            <span>聆听时长</span>
          </article>
          <article>
            <strong :title="favorite_artist">{{ favorite_artist }}</strong>
            <span>最爱歌手</span>
          </article>
          <article>
            <strong :title="favorite_album">{{ favorite_album }}</strong>
            <span>最爱专辑</span>
          </article>
        </div>
      </section>

      <section class="stats_section most_played_section">
        <h2>最常播放</h2>
        <div v-if="most_played_tracks.length" class="most_played_list">
          <div v-for="(track, index) in most_played_tracks" :key="track.track_id" class="most_played_row">
            <span class="index_cell">{{ index + 1 }}</span>
            <span class="most_played_song">
              <strong :title="statistic_track_title(track)">{{ statistic_track_title(track) }}</strong>
              <small :title="statistic_track_artist(track)">{{ statistic_track_artist(track) }}</small>
            </span>
            <span class="album_cell">{{ track.album || "未知专辑" }}</span>
            <span class="play_count_cell">{{ track.play_count }} 次</span>
          </div>
        </div>
        <p v-else class="empty_state">播放歌曲后会在这里展示最常播放。</p>
      </section>
    </section>

    <section v-else class="placeholder_view">
      <p class="empty_state">这个播放列表界面已经预留，后续会接入播放记录和自定义歌单。</p>
    </section>
  </section>
</template>
