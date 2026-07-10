//! Tauri 应用入口。
//!
//! 这里负责创建全局状态、注册插件和暴露前端可调用的 command。

mod decoder;
mod interaction;
mod logger;
mod lyrics_search;
mod utils;

use interaction::media_shortcuts::media_shortcut_plugin;
use interaction::{config::ConfigManager, LyricsSearchService};
use logger::LogKind;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 启动 Tauri 应用并挂载全局状态。
pub fn run() {
    let config_manager = ConfigManager::new();
    let log_dir = config_manager
        .get()
        .map(|config| config.cache.log_cache_dir)
        .unwrap_or_else(|_| config_manager.get_default().cache.log_cache_dir);
    logger::set_log_dir(log_dir);

    tauri::Builder::default()
        .plugin(media_shortcut_plugin())
        .manage(LyricsSearchService::new())
        .manage(config_manager)
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                initialize_deferred_modules(handle);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            interaction::commands::get_startup_state,
            interaction::commands::update_app_config,
            interaction::commands::add_music_dirs,
            interaction::commands::scan_music_dir,
            interaction::commands::reload_music_library,
            interaction::commands::register_media_shortcuts,
            interaction::commands::run_decoder,
            interaction::commands::get_playlist_bundle,
            interaction::commands::save_playback_record,
            interaction::commands::read_lyrics_cache,
            interaction::commands::search_lyrics,
            interaction::commands::use_lyrics_search_result,
            interaction::commands::add_track_to_playlist,
            interaction::commands::remove_track_from_playlist,
            interaction::commands::create_user_playlist,
            interaction::commands::rename_user_playlist,
            interaction::commands::delete_user_playlist,
            interaction::commands::reorder_user_playlists,
            interaction::commands::get_play_statistics,
            interaction::commands::record_track_started,
            interaction::commands::record_audio_error,
            interaction::commands::record_listening_time
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 后台初始化非首屏必需的插件和存储目录。
///
/// 注意：该函数运行在独立 OS 线程中，避免文件选择器等插件初始化影响窗口显示速度。
fn initialize_deferred_modules(handle: tauri::AppHandle) {
    if let Some(config_manager) = handle.try_state::<ConfigManager>() {
        if let Err(err) = config_manager.initialize_storage() {
            logger::error(
                LogKind::App,
                format!("后台初始化配置存储失败 | 原因=\"{err}\""),
            );
        }
    }

    if let Err(err) = handle.plugin(tauri_plugin_dialog::init()) {
        logger::error(
            LogKind::App,
            format!("后台初始化文件选择插件失败 | 原因=\"{err}\""),
        );
    }

    if let Err(err) = handle.plugin(tauri_plugin_opener::init()) {
        logger::error(
            LogKind::App,
            format!("后台初始化文件打开插件失败 | 原因=\"{err}\""),
        );
    }
}
