import { defineStore } from "pinia";
import type { ViewKey } from "../types/music";

export const use_library_view_store = defineStore("library_view", {
  state: () => ({
    query: "",
    active_view: "all" as ViewKey,
    selected_artist: "",
    selected_album: "",
    selected_playlist_id: "my_playlist",
    locate_playing_track_request: 0,
  }),
  getters: {
    has_query(state) {
      return Boolean(state.query.trim());
    },
  },
  actions: {
    show_view(view: ViewKey) {
      this.active_view = view;
      this.selected_artist = "";
      this.selected_album = "";
    },
    show_playlist(playlist_id: string) {
      this.selected_playlist_id = playlist_id;
      this.show_view("user_playlist");
    },
    set_selected_playlist(playlist_id: string) {
      this.selected_playlist_id = playlist_id;
    },
    open_artist(name: string) {
      this.active_view = "artists";
      this.selected_artist = name;
      this.selected_album = "";
      this.query = "";
    },
    open_album(name: string) {
      this.active_view = "albums";
      this.selected_album = name;
      this.selected_artist = "";
      this.query = "";
    },
    close_detail() {
      this.selected_artist = "";
      this.selected_album = "";
    },
    update_query(value: string) {
      this.query = value;
      if (this.active_view === "stats") {
        this.active_view = "all";
        this.close_detail();
      }
    },
    focus_search() {
      if (this.active_view === "stats") {
        this.active_view = "all";
        this.close_detail();
      }
    },
    request_locate_playing_track() {
      this.locate_playing_track_request += 1;
    },
  },
});
