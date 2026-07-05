use crate::audio::{AudioCommand, AudioEngine};
use crate::config::ConfigManager;
use crate::decoder::{run_decoder as run_config_decoder, DecoderRunSummary};
use crate::library::{
    load_cached_all_directories, load_or_scan_all_directories, scan_tracks, write_library_cache,
};
use crate::lyrics::LyricsSearchService;
use crate::media_shortcuts::register_media_shortcuts as register_system_media_shortcuts;
use crate::models::{
    AppConfig, AppStartup, LibraryRefreshResult, LyricsSearchResult, PlayStatistics,
    PlayTrackResult, PlaybackStatus, PlaylistBundle, Track,
};
use crate::playlist::{
    empty_playlist, ensure_unique_playlist_name, load_my_playlist_caches, load_playlist_bundle,
    my_playlist_cache_path, next_user_playlist_index, playlist_cache_path, read_all_playlist_cache,
    read_playlist_cache, read_user_playlist_for_id, record_recent_track, unique_user_playlist_id,
    update_user_playlist_metadata, user_playlist_cache_path,
};
use crate::statistics::{read_play_statistics, record_track_listening_seconds, record_track_play};
use crate::utils::{safe_cache_name, short_hash, unix_timestamp, write_json_cache};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
};

/// 获取应用启动所需的配置、曲库缓存、歌单缓存和播放统计。
///
/// 启动阶段只读取已有缓存，不扫描音乐目录，也不刷新任何缓存文件。
#[tauri::command]
pub(crate) fn get_startup_state(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<AppStartup, String> {
    let config = config_manager.get()?;
    let tracks = load_cached_all_directories(&config_manager, &config)?;
    let playlists = load_playlist_bundle(&config).unwrap_or_else(|err| {
        eprintln!("读取启动歌单缓存失败: {err}");
        empty_playlist_bundle()
    });
    let play_statistics = read_play_statistics(&config).unwrap_or_else(|err| {
        eprintln!("读取启动播放统计失败: {err}");
        PlayStatistics::default()
    });

    Ok(AppStartup {
        config,
        default_config: config_manager.get_default(),
        tracks,
        playlists,
        play_statistics,
    })
}

/// 保存前端修改后的应用配置，并确保相关缓存、日志和解码输出目录存在。
#[tauri::command]
pub(crate) fn update_app_config(
    config_manager: tauri::State<'_, ConfigManager>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    config_manager.update_config(config)
}

/// 添加并扫描音乐目录，刷新对应目录的曲库缓存后返回完整歌曲列表。
#[tauri::command]
pub(crate) fn scan_music_dir(
    config_manager: tauri::State<'_, ConfigManager>,
    dirs: Vec<String>,
) -> Result<Vec<Track>, String> {
    let mut valid_dirs = Vec::new();
    for dir in dirs {
        let root = PathBuf::from(&dir);
        if !root.is_dir() {
            return Err(format!("请选择有效的音乐文件夹: {dir}"));
        }
        valid_dirs.push(root.to_string_lossy().to_string());
    }

    let config = config_manager.add_music_directories(valid_dirs.clone())?;
    for dir in &valid_dirs {
        let root = Path::new(dir);
        let tracks = scan_tracks(root, &config)?;
        let cache_path = config_manager.library_cache_path(dir)?;
        write_library_cache(&cache_path, dir, &config, &tracks)?;
    }

    load_or_scan_all_directories(&config_manager, &config)
}

/// 添加并扫描音乐目录，刷新曲库、歌单和播放统计后一次性返回前端需要的数据。
#[tauri::command]
pub(crate) fn reload_music_library(
    config_manager: tauri::State<'_, ConfigManager>,
    dirs: Vec<String>,
) -> Result<LibraryRefreshResult, String> {
    let tracks = scan_music_dir(config_manager.clone(), dirs)?;
    let config = config_manager.get()?;
    Ok(LibraryRefreshResult {
        tracks,
        playlists: load_playlist_bundle(&config)?,
        play_statistics: read_play_statistics(&config)?,
    })
}

/// 只保存新的音乐目录配置，不扫描曲库、不刷新歌曲缓存。
#[tauri::command]
pub(crate) fn add_music_dirs(
    config_manager: tauri::State<'_, ConfigManager>,
    dirs: Vec<String>,
) -> Result<AppConfig, String> {
    let mut valid_dirs = Vec::new();
    for dir in dirs {
        let root = PathBuf::from(&dir);
        if !root.is_dir() {
            return Err(format!("请选择有效的音乐文件夹: {dir}"));
        }
        valid_dirs.push(root.to_string_lossy().to_string());
    }

    config_manager.add_music_directories(valid_dirs)
}

/// 注册系统媒体热键，延迟到前端首屏显示后执行，避免阻塞应用启动。
#[tauri::command]
pub(crate) fn register_media_shortcuts(app: tauri::AppHandle) {
    register_system_media_shortcuts(&app);
}

/// 按配置中的解码器扫描目录和输出目录执行解码，并返回本次处理统计。
#[tauri::command]
pub(crate) fn run_decoder(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<DecoderRunSummary, String> {
    let config = config_manager.get()?;
    thread::Builder::new()
        .name("music-decoder".to_string())
        .spawn(move || run_config_decoder(&config))
        .map_err(|err| format!("解码线程启动失败: {err}"))?
        .join()
        .map_err(|_| "解码线程异常退出".to_string())
}

/// 读取曲库重载后生成的歌单缓存。
#[tauri::command]
pub(crate) fn get_playlist_bundle(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    load_playlist_bundle(&config)
}

/// 读取歌曲歌词缓存文本，文件不存在或路径为空时返回空值。
#[tauri::command]
pub(crate) fn read_lyrics_cache(path: String) -> Result<Option<String>, String> {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(trimmed_path);
    if !path.is_file() {
        return Ok(None);
    }

    fs::read_to_string(&path)
        .map(Some)
        .map_err(|err| format!("无法读取歌词缓存: {err}"))
}

/// 从 Lyrix 支持的公开歌词源搜索歌词候选，并使用内存缓存避免重复请求外部接口。
#[tauri::command]
pub(crate) async fn search_lyrics(
    lyrics_search: tauri::State<'_, LyricsSearchService>,
    title: String,
    artist: String,
    album: String,
    duration: Option<u64>,
) -> Result<Vec<LyricsSearchResult>, String> {
    lyrics_search.search(title, artist, album, duration).await
}

fn empty_playlist_bundle() -> PlaylistBundle {
    PlaylistBundle {
        recent: empty_playlist("recent", "最近播放", "recent"),
        my_playlist: empty_playlist("my_playlist", "我的歌单", "user"),
        my_playlists: Vec::new(),
        artists: empty_playlist("artists", "歌手", "artists"),
        albums: empty_playlist("albums", "专辑", "albums"),
    }
}

/// 将指定歌曲添加到用户歌单，并刷新返回所有歌单数据。
#[tauri::command]
pub(crate) fn add_track_to_playlist(
    config_manager: tauri::State<'_, ConfigManager>,
    playlist_id: String,
    track_id: String,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let all_playlist = read_all_playlist_cache(&config)?;

    if !all_playlist.tracks.contains_key(&track_id) {
        return Err("歌曲不存在于当前曲库缓存".to_string());
    }

    let playlist_path = user_playlist_cache_path(&config, &playlist_id)?;
    let mut playlist = read_user_playlist_for_id(&config, &playlist_id, &playlist_path)?;
    playlist.track_ids.retain(|current| current != &track_id);
    playlist.track_ids.push(track_id);
    update_user_playlist_metadata(&mut playlist, &all_playlist.tracks);

    write_json_cache(&playlist_path, &playlist, "我的歌单缓存")?;
    load_playlist_bundle(&config)
}

/// 从最近播放或用户歌单中移除指定歌曲记录，不删除音频文件。
#[tauri::command]
pub(crate) fn remove_track_from_playlist(
    config_manager: tauri::State<'_, ConfigManager>,
    playlist_id: String,
    track_id: String,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let all_playlist = read_all_playlist_cache(&config)?;

    let playlist_path = if playlist_id == "recent" {
        playlist_cache_path(&config, "recent_playlist.json")
    } else {
        user_playlist_cache_path(&config, &playlist_id)?
    };
    let mut playlist = read_playlist_cache(&playlist_path)?.unwrap_or_else(|| {
        if playlist_id == "recent" {
            empty_playlist("recent", "最近播放", "recent")
        } else {
            empty_playlist(&playlist_id, "我的歌单", "user")
        }
    });
    if playlist_id != "recent" {
        playlist = read_user_playlist_for_id(&config, &playlist_id, &playlist_path)?;
    }
    playlist.track_ids.retain(|current| current != &track_id);
    update_user_playlist_metadata(&mut playlist, &all_playlist.tracks);

    let label = if playlist_id == "recent" {
        "最近播放缓存"
    } else {
        "我的歌单缓存"
    };
    write_json_cache(&playlist_path, &playlist, label)?;
    load_playlist_bundle(&config)
}

/// 创建新的用户歌单缓存文件，并返回刷新后的歌单集合。
#[tauri::command]
pub(crate) fn create_user_playlist(
    config_manager: tauri::State<'_, ConfigManager>,
    name: String,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let name = name.trim();
    if name.is_empty() {
        return Err("歌单名称不能为空".to_string());
    }

    let playlists = load_my_playlist_caches(&config)?;
    ensure_unique_playlist_name(&playlists, name, None)?;

    let id = unique_user_playlist_id(name);
    let file_name = format!("{}_{}.json", safe_cache_name(name), short_hash(&id));
    let playlist_path = my_playlist_cache_path(&config, &file_name);
    let mut playlist = empty_playlist(&id, name, "user");
    playlist.metadata.index = next_user_playlist_index(&playlists);

    let all_tracks = read_all_playlist_cache(&config)
        .map(|cache| cache.tracks)
        .unwrap_or_default();
    update_user_playlist_metadata(&mut playlist, &all_tracks);

    write_json_cache(&playlist_path, &playlist, "我的歌单缓存")?;
    load_playlist_bundle(&config)
}

/// 重命名指定用户歌单，并同步更新该歌单的元数据。
#[tauri::command]
pub(crate) fn rename_user_playlist(
    config_manager: tauri::State<'_, ConfigManager>,
    playlist_id: String,
    name: String,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let name = name.trim();
    if name.is_empty() {
        return Err("歌单名称不能为空".to_string());
    }

    let playlists = load_my_playlist_caches(&config)?;
    ensure_unique_playlist_name(&playlists, name, Some(&playlist_id))?;

    let playlist_path = user_playlist_cache_path(&config, &playlist_id)?;
    let mut playlist = read_user_playlist_for_id(&config, &playlist_id, &playlist_path)?;
    playlist.name = name.to_string();
    playlist.kind = "user".to_string();

    let all_tracks = read_all_playlist_cache(&config)
        .map(|cache| cache.tracks)
        .unwrap_or_default();
    update_user_playlist_metadata(&mut playlist, &all_tracks);

    write_json_cache(&playlist_path, &playlist, "我的歌单缓存")?;
    load_playlist_bundle(&config)
}

/// 删除指定用户歌单缓存文件，保留曲库和音频文件不变。
#[tauri::command]
pub(crate) fn delete_user_playlist(
    config_manager: tauri::State<'_, ConfigManager>,
    playlist_id: String,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let playlist_path = user_playlist_cache_path(&config, &playlist_id)?;
    if playlist_path.exists() {
        fs::remove_file(&playlist_path).map_err(|err| format!("无法删除歌单缓存: {err}"))?;
    }

    if playlist_id == "my_playlist" {
        let fallback_path = playlist_cache_path(&config, "my_playlist.json");
        if fallback_path.exists() {
            fs::remove_file(&fallback_path)
                .map_err(|err| format!("无法删除旧版默认歌单缓存: {err}"))?;
        }
    }

    load_playlist_bundle(&config)
}

/// 按前端传入的顺序更新用户歌单的 index 元数据。
#[tauri::command]
pub(crate) fn reorder_user_playlists(
    config_manager: tauri::State<'_, ConfigManager>,
    playlist_ids: Vec<String>,
) -> Result<PlaylistBundle, String> {
    let config = config_manager.get()?;
    let mut playlists = load_my_playlist_caches(&config)?;
    let mut ordered_ids = Vec::new();

    for playlist_id in playlist_ids {
        if playlists.iter().any(|playlist| playlist.id == playlist_id)
            && !ordered_ids.iter().any(|current| current == &playlist_id)
        {
            ordered_ids.push(playlist_id);
        }
    }

    for playlist in &playlists {
        if !ordered_ids
            .iter()
            .any(|playlist_id| playlist_id == &playlist.id)
        {
            ordered_ids.push(playlist.id.clone());
        }
    }

    for playlist in &mut playlists {
        if let Some(index) = ordered_ids
            .iter()
            .position(|playlist_id| playlist_id == &playlist.id)
        {
            playlist.metadata.index = index;
            playlist.generated_at = unix_timestamp();
            let playlist_path = user_playlist_cache_path(&config, &playlist.id)?;
            write_json_cache(&playlist_path, playlist, "我的歌单缓存")?;
        }
    }

    load_playlist_bundle(&config)
}

/// 播放指定路径的音频文件，并记录到最近播放列表。
#[tauri::command]
pub(crate) fn play_track(
    engine: tauri::State<'_, AudioEngine>,
    config_manager: tauri::State<'_, ConfigManager>,
    path: String,
) -> Result<PlayTrackResult, String> {
    let requested_path = path.clone();
    engine.send(|reply| AudioCommand::Play { path, reply })?;
    let status = engine.status()?;
    let mut play_statistics = PlayStatistics::default();
    if let Ok(config) = config_manager.get() {
        play_statistics = read_play_statistics(&config).unwrap_or_default();
        let active_path = status.path.as_deref().unwrap_or(&requested_path);
        let _ = record_recent_track(&config, active_path);
        if let Ok(all_playlist) = read_all_playlist_cache(&config) {
            if let Some(track) = all_playlist
                .tracks
                .values()
                .find(|track| track.path == active_path || track.id == active_path)
            {
                if let Ok(next_statistics) = record_track_play(&config, track) {
                    play_statistics = next_statistics;
                }
            }
        }
    }
    Ok(PlayTrackResult {
        status,
        play_statistics,
    })
}

/// 暂停当前播放的音频并返回最新播放状态。
#[tauri::command]
pub(crate) fn pause_track(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Pause { reply })?;
    engine.status()
}

/// 恢复当前音频播放并返回最新播放状态。
#[tauri::command]
pub(crate) fn resume_track(
    engine: tauri::State<'_, AudioEngine>,
) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Resume { reply })?;
    engine.status()
}

/// 停止当前音频播放并清空后端播放状态。
#[tauri::command]
pub(crate) fn stop_track(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Stop { reply })?;
    engine.status()
}

/// 设置播放器音量并返回最新播放状态。
#[tauri::command]
pub(crate) fn set_volume(
    engine: tauri::State<'_, AudioEngine>,
    volume: f32,
) -> Result<PlaybackStatus, String> {
    engine.set_volume(volume)?;
    engine.status()
}

/// 跳转当前音频播放进度到指定秒数并返回最新播放状态。
#[tauri::command]
pub(crate) fn seek_track(
    engine: tauri::State<'_, AudioEngine>,
    seconds: u64,
) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Seek { seconds, reply })?;
    engine.status()
}

/// 获取当前后端播放器状态，用于前端同步播放进度和按钮状态。
#[tauri::command]
pub(crate) fn get_playback_status(
    engine: tauri::State<'_, AudioEngine>,
) -> Result<PlaybackStatus, String> {
    engine.status()
}

/// 获取播放统计缓存，用于统计页展示累计播放、聆听时长和常听歌曲。
#[tauri::command]
pub(crate) fn get_play_statistics(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<PlayStatistics, String> {
    let config = config_manager.get()?;
    read_play_statistics(&config)
}

/// 记录指定歌曲本次聆听的秒数，并写入播放统计缓存。
#[tauri::command]
pub(crate) fn record_listening_time(
    config_manager: tauri::State<'_, ConfigManager>,
    track_id: String,
    seconds: u64,
) -> Result<PlayStatistics, String> {
    let config = config_manager.get()?;
    let all_playlist = read_all_playlist_cache(&config).ok();
    let track = all_playlist
        .as_ref()
        .and_then(|playlist| playlist.tracks.get(&track_id));
    record_track_listening_seconds(&config, track, &track_id, seconds)
}
