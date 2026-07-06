import { defineStore } from "pinia";

export const use_ui_store = defineStore("ui", {
  state: () => ({
    settings_open: false,
    playback_queue_open: false,
    now_playing_open: false,
    now_playing_mounted: false,
  }),
  actions: {
    open_settings() {
      this.settings_open = true;
    },
    close_settings() {
      this.settings_open = false;
    },
    open_playback_queue() {
      this.playback_queue_open = true;
    },
    close_playback_queue() {
      this.playback_queue_open = false;
    },
    open_now_playing() {
      this.now_playing_mounted = true;
      this.now_playing_open = true;
    },
    close_now_playing() {
      this.now_playing_open = false;
    },
  },
});
