use crate::audio::{AudioCommand, AudioEngine};
use crate::config::ConfigManager;
use crate::library::{load_or_scan_all_directories, scan_tracks, write_library_cache};
use crate::models::{AppStartup, PlayStatistics, PlaybackStatus, PlaylistBundle, Track};
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
};

/// 获取应用启动所需的配置、曲库歌曲和歌单缓存数据。
#[tauri::command]
pub(crate) fn get_startup_state(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<AppStartup, String> {
    let config = config_manager.get()?;
    let tracks = load_or_scan_all_directories(&config_manager, &config)?;
    let playlists = load_playlist_bundle(&config)?;

    Ok(AppStartup {
        config,
        tracks,
        playlists,
    })
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
) -> Result<PlaybackStatus, String> {
    let requested_path = path.clone();
    engine.send(|reply| AudioCommand::Play { path, reply })?;
    let status = engine.status()?;
    if let Ok(config) = config_manager.get() {
        let active_path = status.path.as_deref().unwrap_or(&requested_path);
        let _ = record_recent_track(&config, active_path);
        if let Ok(all_playlist) = read_all_playlist_cache(&config) {
            if let Some(track) = all_playlist
                .tracks
                .values()
                .find(|track| track.path == active_path || track.id == active_path)
            {
                let _ = record_track_play(&config, track);
            }
        }
    }
    Ok(status)
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
    engine.send(|reply| AudioCommand::SetVolume { volume, reply })?;
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
