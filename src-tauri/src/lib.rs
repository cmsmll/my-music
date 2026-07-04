use lofty::{
    file::{AudioFile, TaggedFileExt},
    picture::MimeType,
    prelude::Accessor,
    probe::Probe,
    tag::ItemKey,
};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Track {
    id: String,
    title: String,
    artist: String,
    album: String,
    path: String,
    duration: Option<u64>,
    cover_cache_path: Option<String>,
    lyrics_cache_path: String,
    metadata: TrackMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackMetadata {
    title: String,
    artist: String,
    album: String,
    duration: Option<u64>,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
    year: Option<u16>,
    genre: Vec<String>,
    track_number: Option<u32>,
    disk_number: Option<u32>,
    cover_cache_path: Option<String>,
    lyrics_cache_path: String,
    metadata_source: MetadataSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MetadataSource {
    Embedded,
    EmbeddedWithFilenameFallback,
    Filename,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    music_directory: Vec<String>,
    library_cache_dir: String,
    cover_cache_dir: String,
    lyrics_cache_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyAppConfig {
    music_directory: Option<String>,
    library_cache_dir: String,
    cover_cache_dir: String,
    lyrics_cache_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryCache {
    music_directory: String,
    cover_cache_dir: String,
    lyrics_cache_dir: String,
    generated_at: u64,
    tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize)]
struct AppStartup {
    config: AppConfig,
    tracks: Vec<Track>,
}

struct ConfigManager {
    config_path: PathBuf,
    config: Mutex<AppConfig>,
}

impl ConfigManager {
    fn new() -> Self {
        let app_dir = current_app_dir();
        let config_path = app_dir.join("config.toml");
        let default_config = AppConfig {
            music_directory: Vec::new(),
            library_cache_dir: app_dir.join("library-cache").to_string_lossy().to_string(),
            cover_cache_dir: app_dir.join("cover-cache").to_string_lossy().to_string(),
            lyrics_cache_dir: app_dir.join("lyrics-cache").to_string_lossy().to_string(),
        };

        let config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| parse_config(&content))
            .unwrap_or(default_config);

        let manager = Self {
            config_path,
            config: Mutex::new(config),
        };

        let _ = manager.ensure_layout();
        let _ = manager.save();
        manager
    }

    fn get(&self) -> Result<AppConfig, String> {
        self.config
            .lock()
            .map_err(|_| "config state is unavailable".to_string())
            .map(|config| config.clone())
    }

    fn add_music_directories(&self, dirs: Vec<String>) -> Result<AppConfig, String> {
        {
            let mut config = self
                .config
                .lock()
                .map_err(|_| "config state is unavailable".to_string())?;
            for dir in dirs {
                if !config.music_directory.iter().any(|current| current == &dir) {
                    config.music_directory.push(dir);
                }
            }
        }
        self.ensure_layout()?;
        self.save()?;
        self.get()
    }

    fn save(&self) -> Result<(), String> {
        let config = self.get()?;
        let content = toml::to_string_pretty(&config)
            .map_err(|err| format!("无法序列化配置文件: {err}"))?;
        fs::write(&self.config_path, content).map_err(|err| format!("无法写入配置文件: {err}"))
    }

    fn ensure_layout(&self) -> Result<(), String> {
        let config = self.get()?;
        fs::create_dir_all(&config.library_cache_dir)
            .map_err(|err| format!("无法创建歌曲列表缓存目录: {err}"))?;
        fs::create_dir_all(&config.cover_cache_dir)
            .map_err(|err| format!("无法创建图标缓存目录: {err}"))?;
        fs::create_dir_all(&config.lyrics_cache_dir)
            .map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
        Ok(())
    }

    fn library_cache_path(&self, music_dir: &str) -> Result<PathBuf, String> {
        let config = self.get()?;
        let dir_path = Path::new(music_dir);
        let name = dir_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("music-library");
        let safe_name = safe_file_name(name);
        let hash = short_hash(music_dir);
        Ok(PathBuf::from(config.library_cache_dir).join(format!("{safe_name}-{hash}.json")))
    }
}

fn parse_config(content: &str) -> Option<AppConfig> {
    toml::from_str::<AppConfig>(content).ok().or_else(|| {
        toml::from_str::<LegacyAppConfig>(content)
            .ok()
            .map(|legacy| AppConfig {
                music_directory: legacy.music_directory.into_iter().collect(),
                library_cache_dir: legacy.library_cache_dir,
                cover_cache_dir: legacy.cover_cache_dir,
                lyrics_cache_dir: legacy.lyrics_cache_dir,
            })
    })
}

#[derive(Debug, Clone, Serialize)]
struct PlaybackStatus {
    path: Option<String>,
    playing: bool,
    volume: f32,
    elapsed: u64,
}

#[derive(Debug, Clone)]
struct PlaybackSnapshot {
    path: Option<String>,
    playing: bool,
    volume: f32,
    elapsed_offset: Duration,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            path: None,
            playing: false,
            volume: 1.0,
            elapsed_offset: Duration::ZERO,
            started_at: None,
            paused_at: None,
            paused_total: Duration::ZERO,
        }
    }
}

struct Playback {
    sink: Sink,
}

struct AudioEngine {
    tx: Sender<AudioCommand>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
}

enum AudioCommand {
    Play {
        path: String,
        reply: Sender<Result<(), String>>,
    },
    Pause {
        reply: Sender<Result<(), String>>,
    },
    Resume {
        reply: Sender<Result<(), String>>,
    },
    Stop {
        reply: Sender<Result<(), String>>,
    },
    SetVolume {
        volume: f32,
        reply: Sender<Result<(), String>>,
    },
    Seek {
        seconds: u64,
        reply: Sender<Result<(), String>>,
    },
}

impl AudioEngine {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot::default()));
        let thread_snapshot = Arc::clone(&snapshot);

        thread::spawn(move || {
            let stream = OutputStream::try_default();
            let Ok((_stream, handle)) = stream else {
                while let Ok(command) = rx.recv() {
                    respond(command, Err("无法打开默认音频输出设备".to_string()));
                }
                return;
            };

            let mut playback: Option<Playback> = None;

            while let Ok(command) = rx.recv() {
                match command {
                    AudioCommand::Play { path, reply } => {
                        let result = (|| {
                            let file =
                                File::open(&path).map_err(|err| format!("无法打开音频文件: {err}"))?;
                            let source = Decoder::new(BufReader::new(file))
                                .map_err(|err| format!("无法解码音频文件: {err}"))?;
                            let sink = Sink::try_new(&handle)
                                .map_err(|err| format!("无法创建播放通道: {err}"))?;

                            if let Some(current) = playback.take() {
                                current.sink.stop();
                            }

                            sink.append(source);
                            sink.set_volume(1.0);
                            sink.play();
                            playback = Some(Playback { sink });

                            update_snapshot(&thread_snapshot, |state| {
                                state.path = Some(path);
                                state.playing = true;
                                state.volume = 1.0;
                                state.elapsed_offset = Duration::ZERO;
                                state.started_at = Some(Instant::now());
                                state.paused_at = None;
                                state.paused_total = Duration::ZERO;
                            })
                        })();

                        let _ = reply.send(result);
                    }
                    AudioCommand::Pause { reply } => {
                        if let Some(current) = playback.as_ref() {
                            current.sink.pause();
                            let _ = update_snapshot(&thread_snapshot, |state| {
                                if state.paused_at.is_none() {
                                    state.paused_at = Some(Instant::now());
                                }
                                state.playing = false;
                            });
                        }

                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Resume { reply } => {
                        if let Some(current) = playback.as_ref() {
                            current.sink.play();
                            let _ = update_snapshot(&thread_snapshot, |state| {
                                if let Some(paused_at) = state.paused_at.take() {
                                    state.paused_total += paused_at.elapsed();
                                }
                                state.playing = true;
                            });
                        }

                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Stop { reply } => {
                        if let Some(current) = playback.take() {
                            current.sink.stop();
                        }
                        let _ = update_snapshot(&thread_snapshot, |state| {
                            *state = PlaybackSnapshot::default()
                        });
                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::SetVolume { volume, reply } => {
                        let next_volume = volume.clamp(0.0, 1.5);
                        if let Some(current) = playback.as_ref() {
                            current.sink.set_volume(next_volume);
                        }
                        let _ = update_snapshot(&thread_snapshot, |state| state.volume = next_volume);
                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Seek { seconds, reply } => {
                        let result = (|| {
                            let Some(current) = playback.as_ref() else {
                                return Ok(());
                            };

                            if current.sink.try_seek(Duration::from_secs(seconds)).is_err() {
                                let snapshot = thread_snapshot
                                    .lock()
                                    .map_err(|_| "audio engine state is unavailable".to_string())?
                                    .clone();
                                let Some(path) = snapshot.path.clone() else {
                                    return Ok(());
                                };

                                let file = File::open(&path)
                                    .map_err(|err| format!("无法打开音频文件: {err}"))?;
                                let source = Decoder::new(BufReader::new(file))
                                    .map_err(|err| format!("无法解码音频文件: {err}"))?
                                    .skip_duration(Duration::from_secs(seconds));
                                let sink = Sink::try_new(&handle)
                                    .map_err(|err| format!("无法创建播放通道: {err}"))?;

                                sink.append(source);
                                sink.set_volume(snapshot.volume);
                                if snapshot.playing {
                                    sink.play();
                                } else {
                                    sink.pause();
                                }

                                if let Some(current) = playback.take() {
                                    current.sink.stop();
                                }
                                playback = Some(Playback { sink });
                            }

                            update_snapshot(&thread_snapshot, |state| {
                                state.elapsed_offset = Duration::from_secs(seconds);
                                state.started_at = Some(Instant::now());
                                state.paused_total = Duration::ZERO;
                                state.paused_at = if state.playing {
                                    None
                                } else {
                                    Some(Instant::now())
                                };
                            })
                        })();

                        let _ = reply.send(result);
                    }
                }
            }
        });

        Self { tx, snapshot }
    }

    fn send(&self, command: impl FnOnce(Sender<Result<(), String>>) -> AudioCommand) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(command(reply_tx))
            .map_err(|_| "音频线程已停止".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "音频线程没有返回结果".to_string())?
    }

    fn status(&self) -> Result<PlaybackStatus, String> {
        let snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "audio engine state is unavailable".to_string())?
            .clone();

        let elapsed = elapsed_seconds(&snapshot);

        Ok(PlaybackStatus {
            path: snapshot.path,
            playing: snapshot.playing,
            volume: snapshot.volume,
            elapsed,
        })
    }
}

#[tauri::command]
fn get_startup_state(config_manager: tauri::State<'_, ConfigManager>) -> Result<AppStartup, String> {
    let config = config_manager.get()?;
    let tracks = load_or_scan_all_directories(&config_manager, &config)?;

    Ok(AppStartup { config, tracks })
}

#[tauri::command]
fn scan_music_dir(
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

fn load_or_scan_all_directories(
    config_manager: &ConfigManager,
    config: &AppConfig,
) -> Result<Vec<Track>, String> {
    let mut all_tracks = Vec::new();

    for dir in &config.music_directory {
        if !Path::new(dir).is_dir() {
            continue;
        }

        let cache_path = config_manager.library_cache_path(dir)?;
        let tracks = if cache_path.exists() {
            read_library_cache(&cache_path).unwrap_or_else(|_| {
                let tracks = scan_tracks(Path::new(dir), config).unwrap_or_default();
                let _ = write_library_cache(&cache_path, dir, config, &tracks);
                tracks
            })
        } else {
            let tracks = scan_tracks(Path::new(dir), config)?;
            write_library_cache(&cache_path, dir, config, &tracks)?;
            tracks
        };

        all_tracks.extend(tracks);
    }

    all_tracks.sort_by(|a, b| {
        a.artist
            .cmp(&b.artist)
            .then(a.title.cmp(&b.title))
            .then(a.path.cmp(&b.path))
    });

    Ok(all_tracks)
}

fn scan_tracks(root: &Path, config: &AppConfig) -> Result<Vec<Track>, String> {
    let mut tracks = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_audio(path) {
            tracks.push(track_from_path(path, config));
        }
    }

    tracks.sort_by(|a, b| {
        a.artist
            .cmp(&b.artist)
            .then(a.title.cmp(&b.title))
    });

    Ok(tracks)
}

fn read_library_cache(cache_path: &Path) -> Result<Vec<Track>, String> {
    let content = fs::read_to_string(cache_path).map_err(|err| format!("无法读取歌曲缓存: {err}"))?;
    let cache: LibraryCache =
        serde_json::from_str(&content).map_err(|err| format!("无法解析歌曲缓存: {err}"))?;
    Ok(cache.tracks)
}

fn write_library_cache(
    cache_path: &Path,
    music_directory: &str,
    config: &AppConfig,
    tracks: &[Track],
) -> Result<(), String> {
    let cache = LibraryCache {
        music_directory: music_directory.to_string(),
        cover_cache_dir: config.cover_cache_dir.clone(),
        lyrics_cache_dir: config.lyrics_cache_dir.clone(),
        generated_at: unix_timestamp(),
        tracks: tracks.to_vec(),
    };
    let content = serde_json::to_string_pretty(&cache)
        .map_err(|err| format!("无法序列化歌曲缓存: {err}"))?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建歌曲缓存目录: {err}"))?;
    }
    fs::write(cache_path, content).map_err(|err| format!("无法写入歌曲缓存: {err}"))
}

#[tauri::command]
fn play_track(engine: tauri::State<'_, AudioEngine>, path: String) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Play { path, reply })?;
    engine.status()
}

#[tauri::command]
fn pause_track(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Pause { reply })?;
    engine.status()
}

#[tauri::command]
fn resume_track(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Resume { reply })?;
    engine.status()
}

#[tauri::command]
fn stop_track(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Stop { reply })?;
    engine.status()
}

#[tauri::command]
fn set_volume(engine: tauri::State<'_, AudioEngine>, volume: f32) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::SetVolume { volume, reply })?;
    engine.status()
}

#[tauri::command]
fn seek_track(engine: tauri::State<'_, AudioEngine>, seconds: u64) -> Result<PlaybackStatus, String> {
    engine.send(|reply| AudioCommand::Seek { seconds, reply })?;
    engine.status()
}

#[tauri::command]
fn get_playback_status(engine: tauri::State<'_, AudioEngine>) -> Result<PlaybackStatus, String> {
    engine.status()
}

fn respond(command: AudioCommand, result: Result<(), String>) {
    match command {
        AudioCommand::Play { reply, .. }
        | AudioCommand::Pause { reply }
        | AudioCommand::Resume { reply }
        | AudioCommand::Stop { reply }
        | AudioCommand::SetVolume { reply, .. }
        | AudioCommand::Seek { reply, .. } => {
            let _ = reply.send(result);
        }
    }
}

fn update_snapshot(
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    change: impl FnOnce(&mut PlaybackSnapshot),
) -> Result<(), String> {
    let mut state = snapshot
        .lock()
        .map_err(|_| "audio engine state is unavailable".to_string())?;
    change(&mut state);
    Ok(())
}

fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| supported.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

fn track_from_path(path: &Path, config: &AppConfig) -> Track {
    let metadata = parse_track_metadata(path, config);

    Track {
        id: path.to_string_lossy().to_string(),
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: metadata.album.clone(),
        path: path.to_string_lossy().to_string(),
        duration: metadata.duration,
        cover_cache_path: metadata.cover_cache_path.clone(),
        lyrics_cache_path: metadata.lyrics_cache_path.clone(),
        metadata,
    }
}

fn parse_track_metadata(path: &Path, config: &AppConfig) -> TrackMetadata {
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
            metadata_source: MetadataSource::Filename,
        };
    };

    let properties = tagged_file.properties();
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let embedded_title = tag.and_then(|tag| non_empty_owned(tag.title().map(|value| value.into_owned())));
    let embedded_artist = tag.and_then(|tag| {
        non_empty_owned(tag.artist().map(|value| value.into_owned()))
            .or_else(|| non_empty_owned(tag.get_string(ItemKey::AlbumArtist).map(String::from)))
    });
    let embedded_album = tag.and_then(|tag| non_empty_owned(tag.album().map(|value| value.into_owned())));

    let used_fallback =
        embedded_title.is_none() || embedded_artist.is_none() || embedded_album.is_none();
    let metadata_source = if used_fallback {
        MetadataSource::EmbeddedWithFilenameFallback
    } else {
        MetadataSource::Embedded
    };

    let cover_cache_path = tag.and_then(|tag| cache_cover(tag, path, config));
    let lyrics_cache_path = tag
        .and_then(extract_embedded_lyrics)
        .and_then(|lyrics| cache_lyrics(&lyrics, &lyrics_cache_path).ok())
        .unwrap_or(lyrics_cache_path);

    TrackMetadata {
        title: embedded_title.unwrap_or(fallback_title),
        artist: embedded_artist.unwrap_or(fallback_artist),
        album: embedded_album.unwrap_or(fallback_album),
        duration: Some(properties.duration().as_secs()).filter(|duration| *duration > 0),
        bitrate: properties.audio_bitrate().or_else(|| properties.overall_bitrate()),
        sample_rate: properties.sample_rate(),
        year: tag.and_then(|tag| tag.date().map(|date| date.year)),
        genre: tag
            .and_then(|tag| tag.genre().map(|genre| vec![genre.into_owned()]))
            .unwrap_or_default(),
        track_number: tag.and_then(|tag| tag.track()),
        disk_number: tag.and_then(|tag| tag.disk()),
        cover_cache_path,
        lyrics_cache_path,
        metadata_source,
    }
}

fn cache_cover(tag: &lofty::tag::Tag, audio_path: &Path, config: &AppConfig) -> Option<String> {
    let picture = tag.pictures().first()?;
    let mime = picture
        .mime_type()
        .map(mime_type_to_string)
        .unwrap_or("image/jpeg");
    let extension = extension_for_mime(mime);
    let cache_path = PathBuf::from(&config.cover_cache_dir)
        .join(format!("{}.{}", short_hash(&audio_path.to_string_lossy()), extension));

    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cache_path, picture.data());

    Some(cache_path.to_string_lossy().to_string())
}

fn extract_embedded_lyrics(tag: &lofty::tag::Tag) -> Option<String> {
    [ItemKey::Lyrics, ItemKey::UnsyncLyrics]
        .into_iter()
        .find_map(|key| tag.get_string(key))
        .map(str::trim)
        .filter(|lyrics| !lyrics.is_empty())
        .map(String::from)
}

fn cache_lyrics(lyrics: &str, lyrics_cache_path: &str) -> Result<String, String> {
    let path = PathBuf::from(lyrics_cache_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
    }
    fs::write(&path, lyrics).map_err(|err| format!("无法写入歌词缓存: {err}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn non_empty_owned(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn mime_type_to_string(mime_type: &MimeType) -> &'static str {
    match mime_type {
        MimeType::Png => "image/png",
        MimeType::Jpeg => "image/jpeg",
        MimeType::Tiff => "image/tiff",
        MimeType::Bmp => "image/bmp",
        MimeType::Gif => "image/gif",
        _ => "image/jpeg",
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/tiff" => "tiff",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        _ => "jpg",
    }
}

fn lyrics_cache_path(path: &Path, config: &AppConfig) -> String {
    PathBuf::from(&config.lyrics_cache_dir)
        .join(format!("{}.lrc", short_hash(&path.to_string_lossy())))
        .to_string_lossy()
        .to_string()
}

fn parse_artist_and_title(file_name: &str) -> (String, String) {
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

fn duration_seconds(path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    decoder.total_duration().map(|duration| duration.as_secs())
}

fn current_app_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn safe_file_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if safe.trim_matches('_').is_empty() {
        "music-library".to_string()
    } else {
        safe
    }
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn elapsed_seconds(snapshot: &PlaybackSnapshot) -> u64 {
    let Some(started_at) = snapshot.started_at else {
        return 0;
    };

    let active_elapsed = match snapshot.paused_at {
        Some(paused_at) => paused_at.duration_since(started_at),
        None => started_at.elapsed(),
    };

    (snapshot.elapsed_offset + active_elapsed.saturating_sub(snapshot.paused_total)).as_secs()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AudioEngine::new())
        .manage(ConfigManager::new())
        .invoke_handler(tauri::generate_handler![
            get_startup_state,
            scan_music_dir,
            play_track,
            pause_track,
            resume_track,
            stop_track,
            set_volume,
            seek_track,
            get_playback_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
