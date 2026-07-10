import { defineStore } from "pinia";
import type { PlaylistCache, Track } from "../types/music";

type AppActions = {
  begin_progress_drag: (event: PointerEvent) => void;
  drag_progress: (event: PointerEvent) => void;
  end_progress_drag: (event: PointerEvent) => void;
  cancel_progress_drag: (event: PointerEvent) => void;
  previous_track: () => void | Promise<void>;
  toggle_playback: () => void | Promise<void>;
  next_track: () => void | Promise<void>;
  cycle_playback_mode: () => void;
  change_volume: (event: Event) => void | Promise<void>;
  play_track: (track: Track) => void | Promise<void>;
  play_queue_track: (track: Track) => void | Promise<void>;
  open_queue_source: () => void | Promise<void>;
  create_playlist: (name: string) => void | Promise<void>;
  reorder_playlists: (playlist_ids: string[]) => void | Promise<void>;
  open_playlist_menu: (playlist: PlaylistCache, event: MouseEvent) => void;
  open_track_menu: (track: Track, event: MouseEvent) => void;
  begin_sidebar_resize: (event: PointerEvent) => void;
  decode_music_files: () => void | Promise<void>;
  reload_library: () => void | Promise<void>;
  start_window_drag: (event: MouseEvent) => void;
  minimize_window: () => void;
  toggle_maximize_window: () => void;
  close_window: () => void | Promise<void>;
};

const noop = () => undefined;
const default_actions: AppActions = {
  begin_progress_drag: noop, drag_progress: noop, end_progress_drag: noop, cancel_progress_drag: noop,
  previous_track: noop, toggle_playback: noop, next_track: noop, cycle_playback_mode: noop,
  change_volume: noop, play_track: noop, play_queue_track: noop, open_queue_source: noop, create_playlist: noop,
  reorder_playlists: noop, open_playlist_menu: noop, open_track_menu: noop,
  begin_sidebar_resize: noop, decode_music_files: noop, reload_library: noop,
  start_window_drag: noop, minimize_window: noop, toggle_maximize_window: noop, close_window: noop,
};

export const use_app_actions_store = defineStore("app_actions", () => {
  let actions: AppActions = default_actions;
  function register(next_actions: AppActions) { actions = next_actions; }
  function reset() { actions = default_actions; }
  return {
    register, reset,
    begin_progress_drag: (event: PointerEvent) => actions.begin_progress_drag(event),
    drag_progress: (event: PointerEvent) => actions.drag_progress(event),
    end_progress_drag: (event: PointerEvent) => actions.end_progress_drag(event),
    cancel_progress_drag: (event: PointerEvent) => actions.cancel_progress_drag(event),
    previous_track: () => actions.previous_track(), toggle_playback: () => actions.toggle_playback(),
    next_track: () => actions.next_track(), cycle_playback_mode: () => actions.cycle_playback_mode(),
    change_volume: (event: Event) => actions.change_volume(event),
    play_track: (track: Track) => actions.play_track(track),
    play_queue_track: (track: Track) => actions.play_queue_track(track),
    open_queue_source: () => actions.open_queue_source(),
    create_playlist: (name: string) => actions.create_playlist(name),
    reorder_playlists: (ids: string[]) => actions.reorder_playlists(ids),
    open_playlist_menu: (playlist: PlaylistCache, event: MouseEvent) => actions.open_playlist_menu(playlist, event),
    open_track_menu: (track: Track, event: MouseEvent) => actions.open_track_menu(track, event),
    begin_sidebar_resize: (event: PointerEvent) => actions.begin_sidebar_resize(event),
    decode_music_files: () => actions.decode_music_files(), reload_library: () => actions.reload_library(),
    start_window_drag: (event: MouseEvent) => actions.start_window_drag(event),
    minimize_window: () => actions.minimize_window(), toggle_maximize_window: () => actions.toggle_maximize_window(),
    close_window: () => actions.close_window(),
  };
});