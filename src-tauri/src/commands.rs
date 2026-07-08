use crate::config::ConfigManager;
use crate::decoder::{run_decoder as run_config_decoder, DecoderRunSummary};
use crate::library::{
    load_cached_all_directories, reload_all_directories, update_track_lyrics_cache_hash,
};
use crate::lyrics::LyricsSearchService;
use crate::media_shortcuts::register_media_shortcuts as register_system_media_shortcuts;
use crate::models::{
    AppConfig, AppStartup, LibraryRefreshResult, LyricsSearchResponse, LyricsUseResult,
    PlayStatistics, PlaylistBundle, Track,
};
use crate::playlist::{
    empty_playlist, ensure_unique_playlist_name, load_my_playlist_caches, load_playlist_bundle,
    my_playlist_cache_path, next_user_playlist_index, playlist_cache_path, read_all_playlist_cache,
    read_playlist_cache, read_user_playlist_for_id, record_recent_track, unique_user_playlist_id,
    update_user_playlist_metadata, user_playlist_cache_path,
};
use crate::statistics::{read_play_statistics, record_track_listening_seconds, record_track_play};
use crate::utils::{safe_cache_name, short_hash, unix_timestamp, write_json_cache};
use std::{fs, fs::OpenOptions, io::Write, path::PathBuf};
use tauri::Manager;

/// 获取应用启动所需的配置、曲库缓存、歌单缓存和播放统计。
///
/// 启动阶段只读取已有缓存，不扫描音乐目录，也不刷新任何缓存文件。
#[tauri::command]
pub(crate) async fn get_startup_state(app: tauri::AppHandle) -> Result<AppStartup, String> {
    tokio::task::spawn_blocking(move || {
        let config_manager = app.state::<ConfigManager>();
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
    })
    .await
    .map_err(|_| "启动状态加载任务异常退出".to_string())?
}

/// 保存前端修改后的应用配置，并确保相关缓存、日志和解码输出目录存在。
#[tauri::command]
pub(crate) fn update_app_config(
    config_manager: tauri::State<'_, ConfigManager>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    config_manager.update_config(config)
}

/// 添加音乐目录并强制重载完整曲库，所有旧歌曲缓存都会被最新扫描结果替换。
#[tauri::command]
pub(crate) async fn scan_music_dir(
    app: tauri::AppHandle,
    dirs: Vec<String>,
) -> Result<Vec<Track>, String> {
    tokio::task::spawn_blocking(move || {
        let config_manager = app.state::<ConfigManager>();
        let valid_dirs = validate_music_dirs(dirs)?;
        let config = config_manager.add_music_directories(valid_dirs)?;
        reload_all_directories(&config_manager, &config)
    })
    .await
    .map_err(|_| "曲库扫描任务异常退出".to_string())?
}

/// 添加并扫描音乐目录，刷新曲库、歌单和播放统计后一次性返回前端需要的数据。
#[tauri::command]
pub(crate) async fn reload_music_library(
    app: tauri::AppHandle,
    dirs: Vec<String>,
) -> Result<LibraryRefreshResult, String> {
    tokio::task::spawn_blocking(move || {
        let config_manager = app.state::<ConfigManager>();
        let valid_dirs = validate_music_dirs(dirs)?;
        let config = config_manager.add_music_directories(valid_dirs)?;
        let tracks = reload_all_directories(&config_manager, &config)?;
        let config = config_manager.get()?;
        Ok(LibraryRefreshResult {
            tracks,
            playlists: load_playlist_bundle(&config)?,
            play_statistics: read_play_statistics(&config)?,
        })
    })
    .await
    .map_err(|_| "曲库重载任务异常退出".to_string())?
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

/// 按配置中的解码器扫描目录和输出目录执行解码，并异步返回本次处理统计。
#[tauri::command]
pub(crate) async fn run_decoder(
    config_manager: tauri::State<'_, ConfigManager>,
) -> Result<DecoderRunSummary, String> {
    let config = config_manager.get()?;
    tokio::task::spawn_blocking(move || run_config_decoder(&config))
        .await
        .map_err(|_| "解码任务异常退出".to_string())
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

/// 从 Lyrix 支持的公开歌词源搜索歌词候选，并返回当前缓存歌词哈希供前端判断使用状态。
#[tauri::command]
pub(crate) async fn search_lyrics(
    lyrics_search: tauri::State<'_, LyricsSearchService>,
    track_id: String,
    title: String,
    artist: String,
    album: String,
    duration: Option<u64>,
    lyrics_cache_path: String,
    lyrics_cache_hash: Option<String>,
    force_refresh: bool,
) -> Result<LyricsSearchResponse, String> {
    lyrics_search
        .search(
            track_id,
            title,
            artist,
            album,
            duration,
            lyrics_cache_path,
            lyrics_cache_hash,
            force_refresh,
        )
        .await
}

/// 使用指定搜索结果的歌词内容，写入当前歌曲固定歌词缓存路径，并同步歌曲缓存中的歌词哈希。
#[tauri::command]
pub(crate) async fn use_lyrics_search_result(
    app: tauri::AppHandle,
    lyrics_search: tauri::State<'_, LyricsSearchService>,
    track_id: String,
    lyrics_cache_path: String,
    lyrics: String,
) -> Result<LyricsUseResult, String> {
    let lyrics_search = lyrics_search.inner().clone();
    tokio::task::spawn_blocking(move || {
        let config_manager = app.state::<ConfigManager>();
        let mut result = lyrics_search.use_lyrics(lyrics_cache_path, lyrics)?;
        let config = config_manager.get()?;
        result.track = update_track_lyrics_cache_hash(
            &config_manager,
            &config,
            &track_id,
            &result.lyrics_cache_path,
            &result.lyrics_hash,
        )?;
        Ok(result)
    })
    .await
    .map_err(|_| "歌词使用任务异常退出".to_string())?
}

fn validate_music_dirs(dirs: Vec<String>) -> Result<Vec<String>, String> {
    let mut valid_dirs = Vec::new();
    for dir in dirs {
        let root = PathBuf::from(&dir);
        if !root.is_dir() {
            return Err(format!("请选择有效的音乐文件夹: {dir}"));
        }
        valid_dirs.push(root.to_string_lossy().to_string());
    }
    Ok(valid_dirs)
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

/// 记录前端播放器开始播放的歌曲，用于最近播放和播放统计。
#[tauri::command]
pub(crate) fn record_track_started(
    config_manager: tauri::State<'_, ConfigManager>,
    path: String,
) -> Result<PlayStatistics, String> {
    let config = config_manager.get()?;
    let _ = record_recent_track(&config, &path);
    let mut play_statistics = read_play_statistics(&config).unwrap_or_default();
    if let Ok(all_playlist) = read_all_playlist_cache(&config) {
        if let Some(track) = all_playlist
            .tracks
            .values()
            .find(|track| track.path == path || track.id == path)
        {
            play_statistics = record_track_play(&config, track)?;
        }
    }
    Ok(play_statistics)
}

/// 记录前端 audio 标签播放错误，便于排查 WebView 媒体解码和本地资源加载问题。
#[tauri::command]
pub(crate) fn record_frontend_audio_error(
    config_manager: tauri::State<'_, ConfigManager>,
    path: Option<String>,
    source: String,
    code: Option<u16>,
    message: String,
    elapsed: u64,
    ready_state: u16,
    network_state: u16,
) -> Result<(), String> {
    let config = config_manager.get()?;
    write_frontend_audio_error_log(
        &config,
        path.as_deref(),
        &source,
        code,
        &message,
        elapsed,
        ready_state,
        network_state,
    );
    Ok(())
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

fn write_frontend_audio_error_log(
    config: &AppConfig,
    path: Option<&str>,
    source: &str,
    code: Option<u16>,
    message: &str,
    elapsed: u64,
    ready_state: u16,
    network_state: u16,
) {
    let log_dir = PathBuf::from(&config.cache.log_cache_dir);
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("frontend-audio.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let file_path = path.map(log_value).unwrap_or_else(|| "无".to_string());
    let code = code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "无".to_string());
    let _ = writeln!(
        file,
        "[{}] 前端音频播放失败 | 文件=\"{}\" | 地址=\"{}\" | 错误码={} | 秒数={} | ready_state={} | network_state={} | 原因=\"{}\"",
        unix_timestamp(),
        file_path,
        log_value(source),
        code,
        elapsed,
        ready_state,
        network_state,
        log_value(message),
    );
}

fn log_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
