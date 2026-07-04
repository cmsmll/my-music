use crate::models::{AppConfig, PlayStatistics, Track, TrackPlayStatistic};
use crate::utils::{unix_timestamp, write_json_cache};
use std::{fs, path::Path};

pub(crate) fn read_play_statistics(config: &AppConfig) -> Result<PlayStatistics, String> {
    let path = Path::new(&config.play_statistics_cache_path);
    if !path.exists() {
        return Ok(PlayStatistics::default());
    }

    let content = fs::read_to_string(path).map_err(|err| format!("无法读取播放统计缓存: {err}"))?;
    serde_json::from_str(&content).map_err(|err| format!("无法解析播放统计缓存: {err}"))
}

pub(crate) fn write_play_statistics(
    config: &AppConfig,
    statistics: &PlayStatistics,
) -> Result<(), String> {
    write_json_cache(
        Path::new(&config.play_statistics_cache_path),
        statistics,
        "播放统计缓存",
    )
}

pub(crate) fn record_track_play(
    config: &AppConfig,
    track: &Track,
) -> Result<PlayStatistics, String> {
    let mut statistics = read_play_statistics(config)?;
    let now = unix_timestamp();
    let entry = statistics
        .tracks
        .entry(track.id.clone())
        .or_insert_with(|| statistic_from_track(track));

    entry.title = track.title.clone();
    entry.artist = track.artist.clone();
    entry.album = track.album.clone();
    entry.path = track.path.clone();
    entry.play_count = entry.play_count.saturating_add(1);
    entry.last_played_at = now;
    statistics.total_play_count = statistics.total_play_count.saturating_add(1);

    write_play_statistics(config, &statistics)?;
    Ok(statistics)
}

pub(crate) fn record_track_listening_seconds(
    config: &AppConfig,
    track: Option<&Track>,
    track_id: &str,
    seconds: u64,
) -> Result<PlayStatistics, String> {
    let mut statistics = read_play_statistics(config)?;
    if seconds == 0 || track_id.trim().is_empty() {
        return Ok(statistics);
    }

    let entry = statistics
        .tracks
        .entry(track_id.to_string())
        .or_insert_with(|| match track {
            Some(track) => statistic_from_track(track),
            None => TrackPlayStatistic {
                track_id: track_id.to_string(),
                title: track_id.to_string(),
                artist: "未知歌手".to_string(),
                album: "未知专辑".to_string(),
                path: String::new(),
                play_count: 0,
                listening_seconds: 0,
                last_played_at: 0,
            },
        });

    if let Some(track) = track {
        entry.title = track.title.clone();
        entry.artist = track.artist.clone();
        entry.album = track.album.clone();
        entry.path = track.path.clone();
    }

    entry.listening_seconds = entry.listening_seconds.saturating_add(seconds);
    statistics.total_listening_seconds = statistics.total_listening_seconds.saturating_add(seconds);

    write_play_statistics(config, &statistics)?;
    Ok(statistics)
}

fn statistic_from_track(track: &Track) -> TrackPlayStatistic {
    TrackPlayStatistic {
        track_id: track.id.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        path: track.path.clone(),
        play_count: 0,
        listening_seconds: 0,
        last_played_at: 0,
    }
}
