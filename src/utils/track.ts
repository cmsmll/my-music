import { convertFileSrc } from "@tauri-apps/api/core";
import type { Track } from "../types/music";

export function display_title(track?: Track | null) {
  return track?.title?.trim() || "未知歌曲";
}

export function display_artist(track?: Track | null) {
  return track?.artist?.trim() || "未知歌手";
}

export function display_album(track?: Track | null) {
  return track?.album?.trim() || "未知专辑";
}

export function format_duration(seconds?: number | null) {
  if (!seconds) return "--:--";
  const whole_seconds = Math.floor(seconds);
  const hours = Math.floor(whole_seconds / 3600);
  const minutes = Math.floor((whole_seconds % 3600) / 60);
  const rest = whole_seconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(rest).padStart(2, "0")}`;
  }

  return `${minutes}:${String(rest).padStart(2, "0")}`;
}

export function format_file_size(bytes?: number | null) {
  if (!bytes) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit_index = 0;

  while (value >= 1024 && unit_index < units.length - 1) {
    value /= 1024;
    unit_index += 1;
  }

  const precision = value >= 10 || unit_index === 0 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unit_index]}`;
}

export function cover_src(track?: Track | null) {
  return track?.cover_cache_path ? convertFileSrc(track.cover_cache_path) : "";
}

export function is_missing_track(track?: Track | null) {
  return Boolean(track?.missing);
}

export function icon_style(icon: string) {
  return { "--icon": `url("${icon}")` };
}
