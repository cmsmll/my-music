use crate::config::ConfigManager;
use crate::lyrics::{current_lyrics_hash, lyrics_hash};
use crate::models::{
    AppConfig, LibraryCache, MetadataSource, Track, TrackCacheEntries, TrackMetadata,
};
use crate::playlist::{track_map_from_tracks, write_playlist_caches};
use crate::utils::unix_timestamp;
use lofty::{
    file::{AudioFile, TaggedFileExt},
    picture::MimeType,
    prelude::Accessor,
    probe::Probe,
    tag::ItemKey,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use symphonia::core::{
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];

pub(crate) fn reload_all_directories(
    config_manager: &ConfigManager,
    config: &AppConfig,
) -> Result<Vec<Track>, String> {
    let mut all_tracks = Vec::new();

    for dir in &config.music_directory {
        let root = Path::new(dir);
        if !root.is_dir() {
            continue;
        }

        let cache_path = config_manager.library_cache_path(dir)?;
        clear_existing_cache(&cache_path, "旧歌曲缓存")?;
        let tracks = scan_tracks(root, config)?;
        write_library_cache(&cache_path, dir, config, &tracks)?;
        all_tracks.extend(tracks);
    }

    all_tracks.sort_by(|a, b| {
        a.artist
            .cmp(&b.artist)
            .then(a.title.cmp(&b.title))
            .then(a.path.cmp(&b.path))
    });

    write_playlist_caches(config, &all_tracks)?;

    Ok(all_tracks)
}

pub(crate) fn load_cached_all_directories(
    config_manager: &ConfigManager,
    config: &AppConfig,
) -> Result<Vec<Track>, String> {
    let mut all_tracks = Vec::new();

    for dir in &config.music_directory {
        let cache_path = config_manager.library_cache_path(dir)?;
        if !cache_path.exists() {
            continue;
        }

        match read_library_cache(&cache_path) {
            Ok(mut tracks) => {
                if fill_missing_track_cache_info(&mut tracks) {
                    let _ = write_library_cache(&cache_path, dir, config, &tracks);
                }
                all_tracks.append(&mut tracks);
            }
            Err(err) => eprintln!("读取启动曲库缓存失败: {err}"),
        }
    }

    all_tracks.sort_by(|a, b| {
        a.artist
            .cmp(&b.artist)
            .then(a.title.cmp(&b.title))
            .then(a.path.cmp(&b.path))
    });

    Ok(all_tracks)
}

pub(crate) fn scan_tracks(root: &Path, config: &AppConfig) -> Result<Vec<Track>, String> {
    let mut tracks = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_audio(path) {
            tracks.push(track_from_path(path, config));
        }
    }

    tracks.sort_by(|a, b| a.artist.cmp(&b.artist).then(a.title.cmp(&b.title)));

    Ok(tracks)
}

pub(crate) fn read_library_cache(cache_path: &Path) -> Result<Vec<Track>, String> {
    let content =
        fs::read_to_string(cache_path).map_err(|err| format!("无法读取歌曲缓存: {err}"))?;
    let cache: LibraryCache =
        serde_json::from_str(&content).map_err(|err| format!("无法解析歌曲缓存: {err}"))?;
    Ok(cache.tracks.into_tracks())
}

pub(crate) fn write_library_cache(
    cache_path: &Path,
    music_directory: &str,
    config: &AppConfig,
    tracks: &[Track],
) -> Result<(), String> {
    let cache = LibraryCache {
        music_directory: music_directory.to_string(),
        cover_cache_dir: config.cache.cover_cache_dir.clone(),
        lyrics_cache_dir: config.cache.lyrics_cache_dir.clone(),
        generated_at: unix_timestamp(),
        tracks: TrackCacheEntries::ById(track_map_from_tracks(tracks)),
    };
    let content =
        serde_json::to_string_pretty(&cache).map_err(|err| format!("无法序列化歌曲缓存: {err}"))?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建歌曲缓存目录: {err}"))?;
    }
    fs::write(cache_path, content).map_err(|err| format!("无法写入歌曲缓存: {err}"))
}

fn clear_existing_cache(cache_path: &Path, label: &str) -> Result<(), String> {
    if cache_path.is_dir() {
        fs::remove_dir_all(cache_path).map_err(|err| format!("无法删除{label}: {err}"))?;
    } else if cache_path.exists() {
        fs::remove_file(cache_path).map_err(|err| format!("无法删除{label}: {err}"))?;
    }
    Ok(())
}

pub(crate) fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| supported.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

pub(crate) fn track_from_path(path: &Path, config: &AppConfig) -> Track {
    let metadata = parse_track_metadata(path, config);

    Track {
        id: path.to_string_lossy().to_string(),
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: metadata.album.clone(),
        path: path.to_string_lossy().to_string(),
        duration: metadata.duration,
        file_size: fs::metadata(path).ok().map(|metadata| metadata.len()),
        bitrate: metadata.bitrate,
        sample_rate: metadata.sample_rate,
        year: metadata.year,
        genre: metadata.genre,
        track_number: metadata.track_number,
        disk_number: metadata.disk_number,
        cover_cache_path: metadata.cover_cache_path.clone(),
        lyrics_cache_path: metadata.lyrics_cache_path.clone(),
        lyrics_cache_hash: metadata.lyrics_cache_hash,
        metadata_source: metadata.metadata_source,
        legacy_metadata: None,
    }
}

fn fill_missing_track_cache_info(tracks: &mut [Track]) -> bool {
    let mut changed = false;
    for track in tracks {
        if let Some(metadata) = track.legacy_metadata.take() {
            promote_legacy_metadata(track, metadata);
            changed = true;
        }

        if track.file_size.is_none() {
            track.file_size = fs::metadata(&track.path)
                .ok()
                .map(|metadata| metadata.len());
            changed = true;
        }

        if track.lyrics_cache_hash.trim().is_empty() {
            if let Ok(Some(hash)) = current_lyrics_hash(&track.lyrics_cache_path) {
                track.lyrics_cache_hash = hash;
                changed = true;
            }
        }
    }
    changed
}

fn promote_legacy_metadata(track: &mut Track, metadata: TrackMetadata) {
    if track.bitrate.is_none() {
        track.bitrate = metadata.bitrate;
    }
    if track.sample_rate.is_none() {
        track.sample_rate = metadata.sample_rate;
    }
    if track.year.is_none() {
        track.year = metadata.year;
    }
    if track.genre.is_empty() {
        track.genre = metadata.genre;
    }
    if track.track_number.is_none() {
        track.track_number = metadata.track_number;
    }
    if track.disk_number.is_none() {
        track.disk_number = metadata.disk_number;
    }
    if track.cover_cache_path.is_none() {
        track.cover_cache_path = metadata.cover_cache_path;
    }
    if track.lyrics_cache_path.trim().is_empty() {
        track.lyrics_cache_path = metadata.lyrics_cache_path;
    }
    if track.lyrics_cache_hash.trim().is_empty() {
        track.lyrics_cache_hash = metadata.lyrics_cache_hash;
    }
    if matches!(track.metadata_source, MetadataSource::Filename) {
        track.metadata_source = metadata.metadata_source;
    }
}

pub(crate) fn parse_track_metadata(path: &Path, config: &AppConfig) -> TrackMetadata {
    let file_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("未知歌曲")
        .trim();

    let fallback_album = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("未知专辑")
        .to_string();
    let (fallback_artist, fallback_title) = parse_artist_and_title(file_name);
    let lyrics_cache_path = lyrics_cache_path(path, config);
    let lyrics_cache_hash = current_lyrics_hash(&lyrics_cache_path)
        .ok()
        .flatten()
        .unwrap_or_default();

    let Ok(tagged_file) = Probe::open(path).and_then(|probe| probe.read()) else {
        return TrackMetadata {
            title: fallback_title,
            artist: fallback_artist,
            album: fallback_album,
            duration: duration_seconds(path),
            bitrate: None,
            sample_rate: None,
            year: None,
            genre: Vec::new(),
            track_number: None,
            disk_number: None,
            cover_cache_path: None,
            lyrics_cache_path,
            lyrics_cache_hash,
            metadata_source: MetadataSource::Filename,
        };
    };

    let properties = tagged_file.properties();
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let embedded_title =
        tag.and_then(|tag| non_empty_owned(tag.title().map(|value| value.into_owned())));
    let embedded_artist = tag.and_then(|tag| {
        non_empty_owned(tag.artist().map(|value| value.into_owned()))
            .or_else(|| non_empty_owned(tag.get_string(ItemKey::AlbumArtist).map(String::from)))
    });
    let embedded_album =
        tag.and_then(|tag| non_empty_owned(tag.album().map(|value| value.into_owned())));

    let used_fallback =
        embedded_title.is_none() || embedded_artist.is_none() || embedded_album.is_none();
    let metadata_source = if used_fallback {
        MetadataSource::EmbeddedWithFilenameFallback
    } else {
        MetadataSource::Embedded
    };

    let cover_cache_path = tag.and_then(|tag| cache_cover(tag, path, config));
    let cached_lyrics = tag
        .and_then(extract_embedded_lyrics)
        .and_then(|lyrics| cache_lyrics(&lyrics, &lyrics_cache_path).ok());
    let (lyrics_cache_path, lyrics_cache_hash) = cached_lyrics
        .map(|cached| (cached.path, cached.hash))
        .unwrap_or((lyrics_cache_path, lyrics_cache_hash));

    TrackMetadata {
        title: embedded_title.unwrap_or(fallback_title),
        artist: embedded_artist.unwrap_or(fallback_artist),
        album: embedded_album.unwrap_or(fallback_album),
        duration: Some(properties.duration().as_secs()).filter(|duration| *duration > 0),
        bitrate: properties
            .audio_bitrate()
            .or_else(|| properties.overall_bitrate()),
        sample_rate: properties.sample_rate(),
        year: tag.and_then(|tag| tag.date().map(|date| date.year)),
        genre: tag
            .and_then(|tag| tag.genre().map(|genre| vec![genre.into_owned()]))
            .unwrap_or_default(),
        track_number: tag.and_then(|tag| tag.track()),
        disk_number: tag.and_then(|tag| tag.disk()),
        cover_cache_path,
        lyrics_cache_path,
        lyrics_cache_hash,
        metadata_source,
    }
}

pub(crate) fn cache_cover(
    tag: &lofty::tag::Tag,
    audio_path: &Path,
    config: &AppConfig,
) -> Option<String> {
    let picture = tag.pictures().first()?;
    let mime = picture
        .mime_type()
        .map(mime_type_to_string)
        .unwrap_or("image/jpeg");
    let extension = extension_for_mime(mime);
    let cache_path = PathBuf::from(&config.cache.cover_cache_dir).join(format!(
        "{}.{}",
        cache_file_stem(audio_path),
        extension
    ));

    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cache_path, picture.data());

    Some(cache_path.to_string_lossy().to_string())
}

pub(crate) fn extract_embedded_lyrics(tag: &lofty::tag::Tag) -> Option<String> {
    [ItemKey::Lyrics, ItemKey::UnsyncLyrics]
        .into_iter()
        .find_map(|key| tag.get_string(key))
        .map(str::trim)
        .filter(|lyrics| !lyrics.is_empty())
        .map(String::from)
}

pub(crate) struct CachedLyrics {
    path: String,
    hash: String,
}

pub(crate) fn cache_lyrics(lyrics: &str, lyrics_cache_path: &str) -> Result<CachedLyrics, String> {
    let path = PathBuf::from(lyrics_cache_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
    }
    fs::write(&path, lyrics).map_err(|err| format!("无法写入歌词缓存: {err}"))?;
    let hash = lyrics_hash(lyrics);
    Ok(CachedLyrics {
        path: path.to_string_lossy().to_string(),
        hash,
    })
}

pub(crate) fn update_track_lyrics_cache_hash(
    config_manager: &ConfigManager,
    config: &AppConfig,
    track_id: &str,
    lyrics_cache_path: &str,
    lyrics_cache_hash: &str,
) -> Result<Option<Track>, String> {
    let mut updated_track = None;
    for dir in &config.music_directory {
        let cache_path = config_manager.library_cache_path(dir)?;
        if !cache_path.exists() {
            continue;
        }

        let mut tracks = read_library_cache(&cache_path)?;
        let mut changed = false;
        for track in &mut tracks {
            if track.id == track_id {
                track.lyrics_cache_path = lyrics_cache_path.to_string();
                track.lyrics_cache_hash = lyrics_cache_hash.to_string();
                track.legacy_metadata = None;
                updated_track = Some(track.clone());
                changed = true;
                break;
            }
        }

        if changed {
            write_library_cache(&cache_path, dir, config, &tracks)?;
            let all_tracks = load_cached_all_directories(config_manager, config)?;
            write_playlist_caches(config, &all_tracks)?;
            break;
        }
    }

    Ok(updated_track)
}

pub(crate) fn non_empty_owned(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn mime_type_to_string(mime_type: &MimeType) -> &'static str {
    match mime_type {
        MimeType::Png => "image/png",
        MimeType::Jpeg => "image/jpeg",
        MimeType::Tiff => "image/tiff",
        MimeType::Bmp => "image/bmp",
        MimeType::Gif => "image/gif",
        _ => "image/jpeg",
    }
}

pub(crate) fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/tiff" => "tiff",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        _ => "jpg",
    }
}

pub(crate) fn lyrics_cache_path(path: &Path, config: &AppConfig) -> String {
    PathBuf::from(&config.cache.lyrics_cache_dir)
        .join(format!("{}.lrc", cache_file_stem(path)))
        .to_string_lossy()
        .to_string()
}

fn cache_file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown-track".to_string())
}

pub(crate) fn parse_artist_and_title(file_name: &str) -> (String, String) {
    let Some((artist, title)) = file_name.split_once('-') else {
        return ("未知歌手".to_string(), file_name.to_string());
    };

    let artist = artist.trim();
    let title = title.trim();

    if artist.is_empty() || title.is_empty() {
        return ("未知歌手".to_string(), file_name.to_string());
    }

    (artist.to_string(), title.to_string())
}

pub(crate) fn duration_seconds(path: &Path) -> Option<u64> {
    let file = fs::File::open(path).ok()?;
    let media_stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            media_stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let track = probed
        .format
        .default_track()
        .or_else(|| probed.format.tracks().first())?;
    let duration = track
        .codec_params
        .time_base?
        .calc_time(track.codec_params.n_frames?);

    Some(duration.seconds)
}
