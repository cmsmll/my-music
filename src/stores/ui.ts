import { defineStore } from "pinia";
import { ref } from "vue";

export const use_ui_store = defineStore("ui", () => {
  const settings_open = ref(false);
  const playback_queue_open = ref(false);
  const now_playing_open = ref(false);

  function open_settings() {
    settings_open.value = true;
  }

  function close_settings() {
    settings_open.value = false;
  }

  function open_playback_queue() {
    playback_queue_open.value = true;
  }

  function close_playback_queue() {
    playback_queue_open.value = false;
  }

  function open_now_playing() {
    now_playing_open.value = true;
  }

  function close_now_playing() {
    now_playing_open.value = false;
  }

  return {
    settings_open,
    playback_queue_open,
    now_playing_open,
    open_settings,
    close_settings,
    open_playback_queue,
    close_playback_queue,
    open_now_playing,
    close_now_playing,
  };
});
