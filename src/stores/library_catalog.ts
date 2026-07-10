import { defineStore } from "pinia";
import { computed, shallowRef } from "vue";
import type { AlbumItem, ArtistItem, Track } from "../types/music";
import { display_album, display_artist } from "../utils/track";

export const use_library_catalog_store = defineStore("library_catalog", () => {
  const tracks_by_id = shallowRef<Record<string, Track>>({});
  const track_ids = shallowRef<string[]>([]);
  const track_id_by_path = shallowRef<Record<string, string>>({});
  const track_ids_by_artist = shallowRef<Record<string, string[]>>({});
  const track_ids_by_album = shallowRef<Record<string, string[]>>({});
  const artist_items = shallowRef<ArtistItem[]>([]);
  const album_items = shallowRef<AlbumItem[]>([]);

  const tracks = computed(() => resolve_track_ids(track_ids.value));
  const artist_count = computed(() => artist_items.value.length);
  const album_count = computed(() => album_items.value.length);
  const total_duration = computed(() => tracks.value.reduce((sum, track) => sum + (track.duration ?? 0), 0));
  const total_size = computed(() => tracks.value.reduce((sum, track) => sum + (track.file_size ?? 0), 0));

  function set_tracks(next_tracks: Track[]) {
    tracks_by_id.value = Object.fromEntries(next_tracks.map((track) => [track.id, track]));
    track_ids.value = next_tracks.map((track) => track.id);
    rebuild_indexes();
  }

  function upsert_track(track: Track) {
    const exists = Boolean(tracks_by_id.value[track.id]);
    tracks_by_id.value = { ...tracks_by_id.value, [track.id]: track };
    if (!exists) track_ids.value = [...track_ids.value, track.id];
    rebuild_indexes();
  }

  function track_by_id(id?: string | null) { return id ? tracks_by_id.value[id] ?? null : null; }
  function track_by_path(path?: string | null) {
    const id = path ? track_id_by_path.value[path] : null;
    return id ? track_by_id(id) : null;
  }
  function resolve_track_ids(ids: string[]) {
    return ids.map((id) => tracks_by_id.value[id]).filter((track): track is Track => Boolean(track));
  }
  function tracks_for_artist(name: string) { return resolve_track_ids(track_ids_by_artist.value[name] ?? []); }
  function tracks_for_album(name: string) { return resolve_track_ids(track_ids_by_album.value[name] ?? []); }

  function rebuild_indexes() {
    const paths: Record<string, string> = {};
    const artists_index: Record<string, string[]> = {};
    const albums_index: Record<string, string[]> = {};
    const artists = new Map<string, ArtistItem>();
    const albums = new Map<string, AlbumItem>();
    for (const id of track_ids.value) {
      const track = tracks_by_id.value[id];
      if (!track) continue;
      paths[track.path] = id;
      const artist = display_artist(track);
      (artists_index[artist] ??= []).push(id);
      if (artist !== "未知歌手") {
        const item = artists.get(artist) ?? { name: artist, track_count: 0, total_duration: 0 };
        item.track_count += 1; item.total_duration += track.duration ?? 0;
        if (!item.cover_track || (!item.cover_track.cover_cache_path && track.cover_cache_path)) item.cover_track = track;
        artists.set(artist, item);
      }
      const album = display_album(track);
      (albums_index[album] ??= []).push(id);
      if (album !== "未知专辑") {
        const item = albums.get(album) ?? { name: album, artist, track_count: 0, total_duration: 0 };
        item.track_count += 1; item.total_duration += track.duration ?? 0;
        if (!item.cover_track || (!item.cover_track.cover_cache_path && track.cover_cache_path)) item.cover_track = track;
        albums.set(album, item);
      }
    }
    track_id_by_path.value = paths;
    track_ids_by_artist.value = artists_index;
    track_ids_by_album.value = albums_index;
    artist_items.value = [...artists.values()].sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
    album_items.value = [...albums.values()].sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
  }

  return { tracks_by_id, track_ids, tracks, artist_items, album_items, artist_count, album_count,
    total_duration, total_size, set_tracks, upsert_track, track_by_id, track_by_path,
    resolve_track_ids, tracks_for_artist, tracks_for_album };
});