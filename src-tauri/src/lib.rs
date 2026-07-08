mod audio;
mod commands;
mod config;
mod decoder;
mod kgm;
mod library;
mod lyrics;
mod media_shortcuts;
mod models;
mod ncm;
mod playlist;
mod scanner;
mod statistics;
mod utils;

use audio::AudioEngine;
use config::ConfigManager;
use lyrics::LyricsSearchService;
use media_shortcuts::media_shortcut_plugin;
use std::thread;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_manager = ConfigManager::new();
    let log_dir = config_manager
        .get()
        .map(|config| config.cache.log_cache_dir)
        .unwrap_or_else(|_| {
            utils::current_app_dir()
                .join("log-cache")
                .to_string_lossy()
                .to_string()
        });

    tauri::Builder::default()
        .plugin(media_shortcut_plugin())
        .manage(AudioEngine::new(log_dir))
        .manage(LyricsSearchService::new())
        .manage(config_manager)
        .setup(|app| {
            let handle = app.handle().clone();
            thread::spawn(move || {
                initialize_deferred_modules(handle);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_startup_state,
            commands::update_app_config,
            commands::add_music_dirs,
            commands::scan_music_dir,
            commands::reload_music_library,
            commands::register_media_shortcuts,
            commands::run_decoder,
            commands::get_playlist_bundle,
            commands::read_lyrics_cache,
            commands::search_lyrics,
            commands::use_lyrics_search_result,
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
            commands::get_playback_status,
            commands::get_play_statistics,
            commands::record_track_started,
            commands::record_listening_time
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn initialize_deferred_modules(handle: tauri::AppHandle) {
    if let Some(config_manager) = handle.try_state::<ConfigManager>() {
        if let Err(err) = config_manager.initialize_storage() {
            eprintln!("后台初始化配置存储失败: {err}");
        }
    }

    if let Err(err) = handle.plugin(tauri_plugin_dialog::init()) {
        eprintln!("后台初始化文件选择插件失败: {err}");
    }

    if let Err(err) = handle.plugin(tauri_plugin_opener::init()) {
        eprintln!("后台初始化文件打开插件失败: {err}");
    }
}
