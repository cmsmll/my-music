<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import CustomScrollbar from "./CustomScrollbar.vue";
import type { AlbumItem, ArtistItem, PlayStatistics, QueueSource, Track, TrackPlayStatistic, ViewKey } from "../types/music";
import { cover_src, display_album, display_artist, display_title, format_duration, format_file_size, is_missing_track } from "../utils/track";

const props = defineProps<{
  active_view: ViewKey;
  query: string;
  loading: boolean;
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

type CustomScrollbarExpose = {
  refresh: () => void;
  set_scroll_top: (value: number) => void;
  get_scroll_top: () => number;
  get_client_height: () => number;
  get_client_width: () => number;
  get_viewport: () => HTMLElement | null;
};

const track_table = ref<CustomScrollbarExpose | null>(null);
const artist_view = ref<CustomScrollbarExpose | null>(null);
const album_view = ref<CustomScrollbarExpose | null>(null);
const scroll_top = ref(0);
const viewport_height = ref(720);
const artist_scroll_top = ref(0);
const artist_viewport_height = ref(720);
const artist_viewport_width = ref(720);
const album_scroll_top = ref(0);
const album_viewport_height = ref(720);
const album_viewport_width = ref(720);
const track_row_height = 74;
const virtual_overscan = 8;
const media_tile_min_width = 150;
const media_grid_gap = 18;
const media_tile_text_height = 56;

const view_scroll_positions = new Map<string, number>();

const artist_scroll_key = computed(() => `artists:${props.query.trim()}`);
const album_scroll_key = computed(() => `albums:${props.query.trim()}`);
const artist_grid_columns = computed(() => media_grid_columns(artist_viewport_width.value));
const album_grid_columns = computed(() => media_grid_columns(album_viewport_width.value));
const artist_grid_row_height = computed(() => media_grid_row_height(artist_viewport_width.value, artist_grid_columns.value));
const album_grid_row_height = computed(() => media_grid_row_height(album_viewport_width.value, album_grid_columns.value));
const virtual_artist_start_row = computed(() =>
  Math.max(Math.floor(artist_scroll_top.value / artist_grid_row_height.value) - virtual_overscan, 0),
);
const virtual_album_start_row = computed(() =>
  Math.max(Math.floor(album_scroll_top.value / album_grid_row_height.value) - virtual_overscan, 0),
);
const virtual_artist_visible_rows = computed(() =>
  Math.ceil(artist_viewport_height.value / artist_grid_row_height.value) + virtual_overscan * 2,
);
const virtual_album_visible_rows = computed(() =>
  Math.ceil(album_viewport_height.value / album_grid_row_height.value) + virtual_overscan * 2,
);

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
const virtual_artist_items = computed(() => {
  const start = virtual_artist_start_row.value * artist_grid_columns.value;
  const count = virtual_artist_visible_rows.value * artist_grid_columns.value;
  return filtered_artist_items.value.slice(start, start + count);
});
const virtual_album_items = computed(() => {
  const start = virtual_album_start_row.value * album_grid_columns.value;
  const count = virtual_album_visible_rows.value * album_grid_columns.value;
  return filtered_album_items.value.slice(start, start + count);
});
const virtual_artist_top_padding = computed(() => virtual_artist_start_row.value * artist_grid_row_height.value);
const virtual_album_top_padding = computed(() => virtual_album_start_row.value * album_grid_row_height.value);
const virtual_artist_bottom_padding = computed(() =>
  virtual_media_bottom_padding(
    filtered_artist_items.value.length,
    virtual_artist_start_row.value,
    virtual_artist_items.value.length,
    artist_grid_columns.value,
    artist_grid_row_height.value,
  ),
);
const virtual_album_bottom_padding = computed(() =>
  virtual_media_bottom_padding(
    filtered_album_items.value.length,
    virtual_album_start_row.value,
    virtual_album_items.value.length,
    album_grid_columns.value,
    album_grid_row_height.value,
  ),
);

function update_virtual_viewport() {
  if (!track_table.value) return;
  viewport_height.value = track_table.value.get_client_height() || viewport_height.value;
  scroll_top.value = track_table.value.get_scroll_top();
}

function handle_track_table_scroll() {
  if (!track_table.value) return;
  scroll_top.value = track_table.value.get_scroll_top();
}

function media_grid_columns(viewport_width: number) {
  const available_width = Math.max(viewport_width - 16, media_tile_min_width);
  return Math.max(Math.floor((available_width + media_grid_gap) / (media_tile_min_width + media_grid_gap)), 1);
}

function media_grid_row_height(viewport_width: number, columns: number) {
  const available_width = Math.max(viewport_width - 16, media_tile_min_width);
  const tile_width = (available_width - media_grid_gap * (columns - 1)) / columns;
  return tile_width + media_tile_text_height + media_grid_gap;
}

function virtual_media_bottom_padding(
  total_items: number,
  start_row: number,
  rendered_items: number,
  columns: number,
  row_height: number,
) {
  const total_rows = Math.ceil(total_items / columns);
  const rendered_rows = Math.ceil(rendered_items / columns);
  return Math.max((total_rows - start_row - rendered_rows) * row_height, 0);
}

function update_artist_virtual_viewport() {
  if (!artist_view.value) return;
  artist_viewport_height.value = artist_view.value.get_client_height() || artist_viewport_height.value;
  artist_viewport_width.value = artist_view.value.get_client_width() || artist_viewport_width.value;
  artist_scroll_top.value = artist_view.value.get_scroll_top();
}

function update_album_virtual_viewport() {
  if (!album_view.value) return;
  album_viewport_height.value = album_view.value.get_client_height() || album_viewport_height.value;
  album_viewport_width.value = album_view.value.get_client_width() || album_viewport_width.value;
  album_scroll_top.value = album_view.value.get_scroll_top();
}

function handle_artist_view_scroll() {
  if (!artist_view.value) return;
  update_artist_virtual_viewport();
  view_scroll_positions.set(artist_scroll_key.value, artist_view.value.get_scroll_top());
}

function handle_album_view_scroll() {
  if (!album_view.value) return;
  update_album_virtual_viewport();
  view_scroll_positions.set(album_scroll_key.value, album_view.value.get_scroll_top());
}

async function restore_media_grid_scroll() {
  await nextTick();
  if (props.active_view === "artists" && !props.selected_artist && artist_view.value) {
    artist_view.value.set_scroll_top(view_scroll_positions.get(artist_scroll_key.value) ?? 0);
    update_artist_virtual_viewport();
  }
  if (props.active_view === "albums" && !props.selected_album && album_view.value) {
    album_view.value.set_scroll_top(view_scroll_positions.get(album_scroll_key.value) ?? 0);
    update_album_virtual_viewport();
  }
}

async function locate_status_track() {
  if (!props.status_path) return;

  await nextTick();

  const index = props.display_tracks.findIndex((track) => track.path === props.status_path);
  if (index < 0 || !track_table.value) return;

  const client_height = track_table.value.get_client_height();
  const top = Math.max(index * track_row_height - client_height / 2 + track_row_height / 2, 0);
  track_table.value.set_scroll_top(top);
  update_virtual_viewport();
}

function detail_total_duration() {
  return props.display_tracks.reduce((total, track) => total + (track.duration ?? 0), 0);
}

function visible_list_matches_playback_source() {
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

const filtered_artist_items = computed(() => {
  const keyword = props.query.trim().toLowerCase();
  if (!keyword) return props.artist_items;
  return props.artist_items.filter((artist) => artist.name.toLowerCase().includes(keyword));
});

const filtered_album_items = computed(() => {
  const keyword = props.query.trim().toLowerCase();
  if (!keyword) return props.album_items;
  return props.album_items.filter((album) => album.name.toLowerCase().includes(keyword));
});

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
      track_table.value.set_scroll_top(0);
    }
    update_virtual_viewport();
    update_artist_virtual_viewport();
    update_album_virtual_viewport();
    await restore_media_grid_scroll();
  },
);

watch(
  () => props.display_tracks.length,
  () => {
    void nextTick(update_virtual_viewport);
  },
);

watch(
  () => [filtered_artist_items.value.length, filtered_album_items.value.length],
  () => {
    void nextTick(() => {
      update_artist_virtual_viewport();
      update_album_virtual_viewport();
    });
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
  update_artist_virtual_viewport();
  update_album_virtual_viewport();
});

</script>

<template>
  <section class="content_area">
    <section v-if="
      active_view === 'all' ||
      active_view === 'recent' ||
      active_view === 'user_playlist' ||
      selected_artist ||
      selected_album
    " class="track_table_view" aria-label="歌曲列表">
      <header v-if="selected_artist || selected_album" class="detail_header">
        <button class="detail_title" type="button" @click="emit('close_detail')">
          <strong>{{ selected_artist || selected_album }}</strong>
        </button>
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

      <CustomScrollbar ref="track_table" class="track_table" content_class="track_table_content"
        @scroll="handle_track_table_scroll" @resize="update_virtual_viewport">
        <div class="virtual_track_spacer" :style="{ height: `${virtual_top_padding}px` }" />
        <button v-for="row in virtual_track_rows" :key="row.track.id" class="table_row"
          :class="{ active: row.track.path === status_path, missing: is_missing_track(row.track) }" type="button"
          @click="play_track(row.track)" @contextmenu.stop.prevent="emit('open_track_menu', row.track, $event)">
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
      </CustomScrollbar>
    </section>

    <CustomScrollbar v-else-if="active_view === 'albums'" ref="album_view" class="placeholder_view"
      content_class="placeholder_view_content" @scroll="handle_album_view_scroll"
      @resize="update_album_virtual_viewport">
      <div class="virtual_track_spacer" :style="{ height: `${virtual_album_top_padding}px` }" />
      <div class="placeholder_grid">
        <button v-for="album in virtual_album_items" :key="album.name" class="album_tile media_tile" type="button"
          @click="emit('open_album', album.name)">
          <span class="album_art" :class="{ spinning_cover: album_card_should_spin(album.name) }">
            <img v-if="album.cover_track?.cover_cache_path" :src="cover_src(album.cover_track)" alt="" />
            <span v-else>♪</span>
          </span>
          <strong :title="album.name">{{ album.name }}</strong>
          <small>{{ album.track_count }} 首歌曲 {{ format_duration(album.total_duration) }}</small>
        </button>
      </div>
      <div class="virtual_track_spacer" :style="{ height: `${virtual_album_bottom_padding}px` }" />
      <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示专辑。</p>
      <p v-else-if="!filtered_album_items.length" class="empty_state">没有找到匹配的专辑。</p>
    </CustomScrollbar>

    <CustomScrollbar v-else-if="active_view === 'artists'" ref="artist_view" class="placeholder_view"
      content_class="placeholder_view_content" @scroll="handle_artist_view_scroll"
      @resize="update_artist_virtual_viewport">
      <div class="virtual_track_spacer" :style="{ height: `${virtual_artist_top_padding}px` }" />
      <div class="artist_grid">
        <button v-for="artist in virtual_artist_items" :key="artist.name" class="artist_tile media_tile" type="button"
          @click="emit('open_artist', artist.name)">
          <span class="artist_art" :class="{ spinning_cover: artist_card_should_spin(artist.name) }">
            <img v-if="artist.cover_track?.cover_cache_path" :src="cover_src(artist.cover_track)" alt="" />
            <span v-else>♪</span>
          </span>
          <strong :title="artist.name">{{ artist.name }}</strong>
          <small>{{ artist.track_count }} 首歌曲 {{ format_duration(artist.total_duration) }}</small>
        </button>
      </div>
      <div class="virtual_track_spacer" :style="{ height: `${virtual_artist_bottom_padding}px` }" />
      <p v-if="!tracks.length" class="empty_state">添加音乐目录后会在这里展示歌手。</p>
      <p v-else-if="!filtered_artist_items.length" class="empty_state">没有找到匹配的歌手。</p>
    </CustomScrollbar>

    <CustomScrollbar v-else-if="active_view === 'stats'" class="stats_view" content_class="stats_view_content">
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
    </CustomScrollbar>

    <CustomScrollbar v-else class="placeholder_view" content_class="placeholder_view_content">
      <p class="empty_state">这个播放列表界面已经预留，后续会接入播放记录和自定义歌单。</p>
    </CustomScrollbar>
  </section>
</template>
