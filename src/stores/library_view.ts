import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { ViewKey } from "../types/music";

export const use_library_view_store = defineStore("library_view", () => {
  const query = ref("");
  const active_view = ref<ViewKey>("all");
  const selected_artist = ref("");
  const selected_album = ref("");
  const selected_playlist_id = ref("my_playlist");
  const locate_playing_track_request = ref(0);

  const has_query = computed(() => Boolean(query.value.trim()));

  function show_view(view: ViewKey) {
    active_view.value = view;
    selected_artist.value = "";
    selected_album.value = "";
  }

  function show_playlist(playlist_id: string) {
    selected_playlist_id.value = playlist_id;
    show_view("user_playlist");
  }

  function set_selected_playlist(playlist_id: string) {
    selected_playlist_id.value = playlist_id;
  }

  function open_artist(name: string) {
    active_view.value = "artists";
    selected_artist.value = name;
    selected_album.value = "";
    query.value = "";
  }

  function open_album(name: string) {
    active_view.value = "albums";
    selected_album.value = name;
    selected_artist.value = "";
    query.value = "";
  }

  function close_detail() {
    selected_artist.value = "";
    selected_album.value = "";
  }

  function update_query(value: string) {
    query.value = value;
    if (active_view.value === "stats") {
      active_view.value = "all";
      close_detail();
    }
  }

  function focus_search() {
    if (active_view.value === "stats") {
      active_view.value = "all";
      close_detail();
    }
  }

  function request_locate_playing_track() {
    locate_playing_track_request.value += 1;
  }

  return {
    query,
    active_view,
    selected_artist,
    selected_album,
    selected_playlist_id,
    locate_playing_track_request,
    has_query,
    show_view,
    show_playlist,
    set_selected_playlist,
    open_artist,
    open_album,
    close_detail,
    update_query,
    focus_search,
    request_locate_playing_track,
  };
});
