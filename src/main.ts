import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.vue";
import DesktopLyricsWindow from "./components/DesktopLyricsWindow.vue";
import "./styles/app.css";

const url_params = new URLSearchParams(window.location.search);

function current_window_label() {
  try {
    return getCurrentWindow().label;
  } catch {
    return "";
  }
}

const root_component =
  current_window_label() === "desktop_lyrics" ||
  url_params.get("window") === "desktop-lyrics" || window.location.hash === "#desktop-lyrics"
    ? DesktopLyricsWindow
    : App;

createApp(root_component).use(createPinia()).mount("#app");
