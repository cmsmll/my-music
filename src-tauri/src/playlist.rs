use crate::models::{
    AllPlaylistCache, AppConfig, PlaylistBundle, PlaylistCache, PlaylistMetadata, PlaylistSummary,
    Track,
};
use crate::utils::{short_hash, unix_timestamp};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub(crate) fn write_playlist_caches(config: &AppConfig, tracks: &[Track]) -> Result<(), String> {
    let all_tracks = track_map_from_tracks(tracks);
    let all_ids = track_ids_from_tracks(tracks);
    let generated_at = unix_timestamp();

    let recent_path = playlist_cache_path(config, "recent_playlist.json");
    let my_playlist_path = my_playlist_cache_path(config, "my_playlist.json");
    let artists_path = playlist_cache_path(config, "artists_playlist.json");
    let albums_path = playlist_cache_path(config, "albums_playlist.json");

    let recent = existing_playlist_or_default(
        &recent_path,
        "recent",
        "最近播放",
        "recent",
        generated_at,
        &all_tracks,
    );
    let my_playlist = load_my_playlist_cache(config)?
        .unwrap_or_else(|| empty_playlist("my_playlist", "我的歌单", "user"));

    let artists = write_group_playlists(
        config,
        "artists",
        "artists",
        "歌手",
        "artist",
        tracks,
        |track| normalized_group_name(&track.artist, "未知歌手"),
    )?;
    let albums = write_group_playlists(
        config,
        "albums",
        "albums",
        "专辑",
        "album",
        tracks,
        |track| normalized_group_name(&track.album, "未知专辑"),
    )?;

    let all_playlist_path = playlist_cache_path(config, "all_playlist.json");
    let all_playlist = AllPlaylistCache {
        id: "all".to_string(),
        name: "全部".to_string(),
        kind: "all".to_string(),
        music_directory: config.music_directory.clone(),
        cover_cache_dir: config.cover_cache_dir.clone(),
        lyrics_cache_dir: config.lyrics_cache_dir.clone(),
        generated_at,
        tracks: all_tracks.clone(),
        playlists: vec![
            playlist_summary_from_cache(&recent, &recent_path),
            playlist_summary_from_cache(&my_playlist, &my_playlist_path),
            playlist_summary_from_cache(&artists, &artists_path),
            playlist_summary_from_cache(&albums, &albums_path),
        ],
    };

    write_json_cache(&all_playlist_path, &all_playlist, "全部歌单缓存")?;
    write_json_cache(&recent_path, &recent, "最近播放缓存")?;

    let all_track_playlist = PlaylistCache {
        id: "all_tracks".to_string(),
        name: "全部歌曲".to_string(),
        kind: "all_tracks".to_string(),
        generated_at,
        metadata: playlist_metadata(&all_ids, &all_tracks, 0),
        track_ids: all_ids,
        children: Vec::new(),
    };
    write_json_cache(
        &playlist_cache_path(config, "all_tracks_playlist.json"),
        &all_track_playlist,
        "全部歌曲歌单缓存",
    )?;

    Ok(())
}

pub(crate) fn load_playlist_bundle(config: &AppConfig) -> Result<PlaylistBundle, String> {
    let my_playlists = load_my_playlist_caches(config)?;
    let my_playlist = my_playlists
        .iter()
        .find(|playlist| playlist.id == "my_playlist")
        .cloned()
        .unwrap_or_else(|| empty_playlist("my_playlist", "我的歌单", "user"));

    Ok(PlaylistBundle {
        recent: read_playlist_cache(&playlist_cache_path(config, "recent_playlist.json"))?
            .unwrap_or_else(|| empty_playlist("recent", "最近播放", "recent")),
        my_playlist,
        my_playlists,
        artists: read_playlist_cache(&playlist_cache_path(config, "artists_playlist.json"))?
            .unwrap_or_else(|| empty_playlist("artists", "歌手", "artists")),
        albums: read_playlist_cache(&playlist_cache_path(config, "albums_playlist.json"))?
            .unwrap_or_else(|| empty_playlist("albums", "专辑", "albums")),
    })
}

pub(crate) fn write_group_playlists(
    config: &AppConfig,
    group_dir: &str,
    aggregate_id: &str,
    aggregate_name: &str,
    child_kind: &str,
    tracks: &[Track],
    group_name: impl Fn(&Track) -> String,
) -> Result<PlaylistCache, String> {
    let generated_at = unix_timestamp();
    let all_tracks = track_map_from_tracks(tracks);
    let mut grouped: BTreeMap<String, Vec<Track>> = BTreeMap::new();

    for track in tracks {
        grouped
            .entry(group_name(track))
            .or_default()
            .push(track.clone());
    }

    let mut children = Vec::new();
    let aggregate_path = playlist_cache_path(config, &format!("{aggregate_id}_playlist.json"));
    let group_root = PathBuf::from(&config.library_cache_dir).join(group_dir);
    if group_root.is_dir() {
        fs::remove_dir_all(&group_root)
            .map_err(|err| format!("无法清理旧版{aggregate_name}细分缓存目录: {err}"))?;
    } else if group_root.exists() {
        fs::remove_file(&group_root)
            .map_err(|err| format!("无法清理旧版{aggregate_name}细分缓存文件: {err}"))?;
    }

    for (name, mut group_tracks) in grouped {
        group_tracks.sort_by(|a, b| a.title.cmp(&b.title).then(a.path.cmp(&b.path)));
        let track_ids = track_ids_from_tracks(&group_tracks);
        let id = format!("{child_kind}_{}", short_hash(&name));
        let playlist = PlaylistCache {
            id,
            name: name.clone(),
            kind: child_kind.to_string(),
            generated_at,
            metadata: playlist_metadata(&track_ids, &all_tracks, 0),
            track_ids,
            children: Vec::new(),
        };
        children.push(playlist_summary_from_cache(&playlist, &aggregate_path));
    }

    let all_ids = track_ids_from_tracks(tracks);
    let aggregate = PlaylistCache {
        id: aggregate_id.to_string(),
        name: aggregate_name.to_string(),
        kind: aggregate_id.to_string(),
        generated_at,
        metadata: playlist_metadata(&all_ids, &all_tracks, children.len()),
        track_ids: all_ids,
        children,
    };

    write_json_cache(
        &aggregate_path,
        &aggregate,
        &format!("{aggregate_name}汇总歌单缓存"),
    )?;

    Ok(aggregate)
}

pub(crate) fn existing_playlist_or_default(
    path: &Path,
    id: &str,
    name: &str,
    kind: &str,
    generated_at: u64,
    all_tracks: &BTreeMap<String, Track>,
) -> PlaylistCache {
    let mut playlist = read_playlist_cache(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| empty_playlist(id, name, kind));

    playlist.id = id.to_string();
    playlist.name = name.to_string();
    playlist.kind = kind.to_string();
    playlist.generated_at = generated_at;
    playlist
        .track_ids
        .retain(|track_id| all_tracks.contains_key(track_id));
    playlist.children.clear();
    playlist.metadata = playlist_metadata(&playlist.track_ids, all_tracks, 0);
    playlist
}

pub(crate) fn empty_playlist(id: &str, name: &str, kind: &str) -> PlaylistCache {
    PlaylistCache {
        id: id.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        generated_at: unix_timestamp(),
        metadata: PlaylistMetadata {
            track_count: 0,
            total_duration: 0,
            item_count: 0,
            cover_cache_path: None,
            index: 0,
        },
        track_ids: Vec::new(),
        children: Vec::new(),
    }
}

pub(crate) fn playlist_metadata(
    track_ids: &[String],
    all_tracks: &BTreeMap<String, Track>,
    item_count: usize,
) -> PlaylistMetadata {
    let mut total_duration = 0;
    let mut cover_cache_path = None;

    for track_id in track_ids {
        let Some(track) = all_tracks.get(track_id) else {
            continue;
        };
        total_duration += track.duration.unwrap_or(0);
        if cover_cache_path.is_none() && track.cover_cache_path.is_some() {
            cover_cache_path = track.cover_cache_path.clone();
        }
    }

    PlaylistMetadata {
        track_count: track_ids.len(),
        total_duration,
        item_count,
        cover_cache_path,
        index: 0,
    }
}

pub(crate) fn playlist_summary_from_cache(
    playlist: &PlaylistCache,
    cache_path: &Path,
) -> PlaylistSummary {
    PlaylistSummary {
        id: playlist.id.clone(),
        name: playlist.name.clone(),
        kind: playlist.kind.clone(),
        cache_path: cache_path.to_string_lossy().to_string(),
        track_count: playlist.metadata.track_count,
        total_duration: playlist.metadata.total_duration,
        cover_cache_path: playlist.metadata.cover_cache_path.clone(),
        track_ids: playlist.track_ids.clone(),
    }
}

pub(crate) fn read_playlist_cache(path: &Path) -> Result<Option<PlaylistCache>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|err| format!("无法读取歌单缓存: {err}"))?;
    let playlist =
        serde_json::from_str(&content).map_err(|err| format!("无法解析歌单缓存: {err}"))?;
    Ok(Some(playlist))
}

pub(crate) fn load_my_playlist_cache(config: &AppConfig) -> Result<Option<PlaylistCache>, String> {
    let primary_path = my_playlist_cache_path(config, "my_playlist.json");
    let fallback_path = playlist_cache_path(config, "my_playlist.json");
    let mut playlist = match read_playlist_cache(&primary_path)? {
        Some(playlist) => Some(playlist),
        None => read_playlist_cache(&fallback_path)?,
    };

    if let Some(playlist) = playlist.as_mut() {
        playlist.id = "my_playlist".to_string();
        if playlist.name.trim().is_empty() {
            playlist.name = "我的歌单".to_string();
        }
        playlist.kind = "user".to_string();
        playlist.metadata.track_count = playlist.track_ids.len();
        playlist.metadata.item_count = 0;
    }

    Ok(playlist)
}

pub(crate) fn load_my_playlist_caches(config: &AppConfig) -> Result<Vec<PlaylistCache>, String> {
    let mut playlists = Vec::new();
    if let Some(playlist) = load_my_playlist_cache(config)? {
        playlists.push(playlist);
    }

    let root = PathBuf::from(&config.my_playlist_cache_dir);
    if root.is_dir() {
        let entries =
            fs::read_dir(&root).map_err(|err| format!("无法读取我的歌单缓存目录: {err}"))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("无法读取我的歌单缓存文件: {err}"))?;
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) == Some("my_playlist.json") {
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            if let Some(mut playlist) = read_playlist_cache(&path)? {
                if playlist.kind.is_empty() {
                    playlist.kind = "user".to_string();
                }
                playlists.push(playlist);
            }
        }
    }

    playlists.sort_by(|left, right| {
        left.metadata
            .index
            .cmp(&right.metadata.index)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(playlists)
}

pub(crate) fn user_playlist_cache_path(
    config: &AppConfig,
    playlist_id: &str,
) -> Result<PathBuf, String> {
    if playlist_id == "my_playlist" {
        return Ok(my_playlist_cache_path(config, "my_playlist.json"));
    }

    let root = PathBuf::from(&config.my_playlist_cache_dir);
    if root.is_dir() {
        let entries =
            fs::read_dir(&root).map_err(|err| format!("无法读取我的歌单缓存目录: {err}"))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("无法读取我的歌单缓存文件: {err}"))?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let Some(playlist) = read_playlist_cache(&path)? else {
                continue;
            };
            if playlist.id == playlist_id {
                return Ok(path);
            }
        }
    }

    Err(format!("找不到歌单: {playlist_id}"))
}

pub(crate) fn read_user_playlist_for_id(
    config: &AppConfig,
    playlist_id: &str,
    playlist_path: &Path,
) -> Result<PlaylistCache, String> {
    if playlist_id == "my_playlist" {
        return Ok(load_my_playlist_cache(config)?
            .unwrap_or_else(|| empty_playlist("my_playlist", "我的歌单", "user")));
    }

    Ok(read_playlist_cache(playlist_path)?
        .unwrap_or_else(|| empty_playlist(playlist_id, "我的歌单", "user")))
}

pub(crate) fn update_user_playlist_metadata(
    playlist: &mut PlaylistCache,
    all_tracks: &BTreeMap<String, Track>,
) {
    let index = playlist.metadata.index;
    playlist.generated_at = unix_timestamp();
    playlist.metadata = playlist_metadata(&playlist.track_ids, all_tracks, 0);
    playlist.metadata.index = index;
}

pub(crate) fn read_all_playlist_cache(config: &AppConfig) -> Result<AllPlaylistCache, String> {
    let all_playlist_path = playlist_cache_path(config, "all_playlist.json");
    let content = fs::read_to_string(&all_playlist_path)
        .map_err(|err| format!("无法读取全部歌单缓存: {err}"))?;
    serde_json::from_str(&content).map_err(|err| format!("无法解析全部歌单缓存: {err}"))
}

pub(crate) fn unique_user_playlist_id(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("playlist_{}", short_hash(&format!("{name}-{nanos}")))
}

pub(crate) fn ensure_unique_playlist_name(
    playlists: &[PlaylistCache],
    name: &str,
    current_id: Option<&str>,
) -> Result<(), String> {
    let exists = playlists
        .iter()
        .any(|playlist| Some(playlist.id.as_str()) != current_id && playlist.name.trim() == name);
    if exists {
        return Err("歌单名称已存在".to_string());
    }
    Ok(())
}

pub(crate) fn next_user_playlist_index(playlists: &[PlaylistCache]) -> usize {
    playlists
        .iter()
        .map(|playlist| playlist.metadata.index)
        .max()
        .map(|index| index + 1)
        .unwrap_or(0)
}

pub(crate) fn record_recent_track(config: &AppConfig, path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }

    if !playlist_cache_path(config, "all_playlist.json").exists() {
        return Ok(());
    }

    let all_playlist = read_all_playlist_cache(config)?;

    let track_id = if all_playlist.tracks.contains_key(path) {
        path.to_string()
    } else {
        all_playlist
            .tracks
            .values()
            .find(|track| track.path == path)
            .map(|track| track.id.clone())
            .unwrap_or_default()
    };

    if track_id.is_empty() {
        return Ok(());
    }

    let recent_path = playlist_cache_path(config, "recent_playlist.json");
    let mut recent = read_playlist_cache(&recent_path)?
        .unwrap_or_else(|| empty_playlist("recent", "最近播放", "recent"));
    recent.track_ids.retain(|current| current != &track_id);
    recent.track_ids.insert(0, track_id);
    recent
        .track_ids
        .retain(|track_id| all_playlist.tracks.contains_key(track_id));
    recent.track_ids.truncate(100);
    recent.generated_at = unix_timestamp();
    recent.metadata = playlist_metadata(&recent.track_ids, &all_playlist.tracks, 0);

    write_json_cache(&recent_path, &recent, "最近播放缓存")
}

pub(crate) fn write_json_cache<T: Serialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(value).map_err(|err| format!("无法序列化{label}: {err}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建{label}目录: {err}"))?;
    }
    fs::write(path, content).map_err(|err| format!("无法写入{label}: {err}"))
}

pub(crate) fn track_map_from_tracks(tracks: &[Track]) -> BTreeMap<String, Track> {
    tracks
        .iter()
        .map(|track| (track.id.clone(), track.clone()))
        .collect()
}

pub(crate) fn track_ids_from_tracks(tracks: &[Track]) -> Vec<String> {
    tracks.iter().map(|track| track.id.clone()).collect()
}

pub(crate) fn playlist_cache_path(config: &AppConfig, file_name: &str) -> PathBuf {
    PathBuf::from(&config.library_cache_dir).join(file_name)
}

pub(crate) fn my_playlist_cache_path(config: &AppConfig, file_name: &str) -> PathBuf {
    PathBuf::from(&config.my_playlist_cache_dir).join(file_name)
}

pub(crate) fn normalized_group_name(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}
