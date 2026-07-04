mod audio;
mod commands;
mod config;
mod library;
mod media_shortcuts;
mod models;
mod playlist;
mod utils;

use audio::AudioEngine;
use config::ConfigManager;
use media_shortcuts::{media_shortcut_plugin, register_media_shortcuts};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_manager = ConfigManager::new();
    let log_dir = config_manager
        .get()
        .map(|config| config.log_dir)
        .unwrap_or_else(|_| {
            utils::current_app_dir()
                .join("logs")
                .to_string_lossy()
                .to_string()
        });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(media_shortcut_plugin())
        .plugin(tauri_plugin_opener::init())
        .manage(AudioEngine::new(log_dir))
        .manage(config_manager)
        .setup(|app| {
            register_media_shortcuts(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_startup_state,
            commands::scan_music_dir,
            commands::add_track_to_playlist,
            commands::remove_track_from_playlist,
            commands::create_user_playlist,
            commands::rename_user_playlist,
            commands::delete_user_playlist,
            commands::reorder_user_playlists,
            commands::play_track,
            commands::pause_track,
            commands::resume_track,
            commands::stop_track,
            commands::set_volume,
            commands::seek_track,
            commands::get_playback_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
