use crate::models::{AppConfig, CacheConfig, CacheConfigFile, ConfigFile, MusicDirectoryConfig};
use crate::utils::{current_app_dir, safe_file_name, short_hash};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
pub(crate) struct ConfigManager {
    config_path: PathBuf,
    default_config: AppConfig,
    config: Mutex<AppConfig>,
}

impl ConfigManager {
    pub(crate) fn new() -> Self {
        let app_dir = current_app_dir();
        let config_path = app_dir.join("config.toml");
        let default_config = AppConfig {
            music_directory: Vec::new(),
            decoder: crate::models::DecoderConfig {
                output_dir: String::new(),
                process_formats: "mp3,flac,kgm,kgma,ncm".to_string(),
                scan_directory: Vec::new(),
            },
            cache: CacheConfig {
                library_cache_dir: app_dir.join("library-cache").to_string_lossy().to_string(),
                cover_cache_dir: app_dir.join("cover-cache").to_string_lossy().to_string(),
                lyrics_cache_dir: app_dir.join("lyrics-cache").to_string_lossy().to_string(),
                my_playlist_cache_dir: app_dir
                    .join("my-playlist-cache")
                    .to_string_lossy()
                    .to_string(),
                log_dir: app_dir.join("logs").to_string_lossy().to_string(),
                play_statistics_cache_path: app_dir
                    .join("library-cache")
                    .join("play-statistics.json")
                    .to_string_lossy()
                    .to_string(),
            },
            style: crate::models::StyleConfig {
                background_color: "#ffffff".to_string(),
                background_image: String::new(),
                background_image_opacity: 1.0,
                title_color: "#1e2026".to_string(),
                subtitle_color: "#8b919c".to_string(),
                highlight_color: "#22a05a".to_string(),
                show_border: true,
            },
            state: crate::models::AppStateConfig {
                width: 1280,
                height: 820,
                volume: 1.0,
                sidebar_width: 250,
            },
        };

        let config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| parse_config(&content, &default_config))
            .unwrap_or_else(|| default_config.clone());

        Self {
            config_path,
            default_config,
            config: Mutex::new(config),
        }
    }

    pub(crate) fn get(&self) -> Result<AppConfig, String> {
        self.config
            .lock()
            .map_err(|_| "config state is unavailable".to_string())
            .map(|config| config.clone())
    }

    pub(crate) fn get_default(&self) -> AppConfig {
        self.default_config.clone()
    }

    pub(crate) fn initialize_storage(&self) -> Result<(), String> {
        self.ensure_layout()?;
        self.save()
    }

    pub(crate) fn update_config(&self, next_config: AppConfig) -> Result<AppConfig, String> {
        {
            let mut config = self
                .config
                .lock()
                .map_err(|_| "config state is unavailable".to_string())?;
            *config = sanitize_config(next_config);
        }
        self.ensure_layout()?;
        self.save()?;
        self.get()
    }

    pub(crate) fn add_music_directories(&self, dirs: Vec<String>) -> Result<AppConfig, String> {
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
        let content =
            toml::to_string_pretty(&config).map_err(|err| format!("无法序列化配置文件: {err}"))?;
        fs::write(&self.config_path, content).map_err(|err| format!("无法写入配置文件: {err}"))
    }

    fn ensure_layout(&self) -> Result<(), String> {
        let config = self.get()?;
        fs::create_dir_all(&config.cache.library_cache_dir)
            .map_err(|err| format!("无法创建歌曲列表缓存目录: {err}"))?;
        fs::create_dir_all(&config.cache.cover_cache_dir)
            .map_err(|err| format!("无法创建图标缓存目录: {err}"))?;
        fs::create_dir_all(&config.cache.lyrics_cache_dir)
            .map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
        fs::create_dir_all(&config.cache.my_playlist_cache_dir)
            .map_err(|err| format!("无法创建我的歌单缓存目录: {err}"))?;
        fs::create_dir_all(&config.cache.log_dir)
            .map_err(|err| format!("无法创建日志目录: {err}"))?;
        if !config.decoder.output_dir.trim().is_empty() {
            fs::create_dir_all(&config.decoder.output_dir)
                .map_err(|err| format!("无法创建解码输出目录: {err}"))?;
        }
        if let Some(parent) = Path::new(&config.cache.play_statistics_cache_path).parent() {
            fs::create_dir_all(parent).map_err(|err| format!("无法创建播放统计缓存目录: {err}"))?;
        }
        Ok(())
    }

    pub(crate) fn library_cache_path(&self, music_dir: &str) -> Result<PathBuf, String> {
        let config = self.get()?;
        let dir_path = Path::new(music_dir);
        let name = dir_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("music-library");
        let safe_name = safe_file_name(name);
        let hash = short_hash(music_dir);
        Ok(PathBuf::from(config.cache.library_cache_dir).join(format!("{safe_name}-{hash}.json")))
    }
}

pub(crate) fn parse_config(content: &str, default_config: &AppConfig) -> Option<AppConfig> {
    toml::from_str::<ConfigFile>(content).ok().map(|config| {
        let decoder = config.decoder.unwrap_or(crate::models::DecoderConfigFile {
            output_dir: None,
            process_formats: None,
            scan_directory: None,
        });
        let cache = config.cache.unwrap_or(CacheConfigFile {
            library_cache_dir: None,
            cover_cache_dir: None,
            lyrics_cache_dir: None,
            my_playlist_cache_dir: None,
            log_dir: None,
            play_statistics_cache_path: None,
        });
        let style = config.style.unwrap_or(crate::models::StyleConfigFile {
            background_color: None,
            background_image: None,
            background_image_opacity: None,
            title_color: None,
            subtitle_color: None,
            highlight_color: None,
            control_color: None,
            show_border: None,
        });
        let state = config.state.unwrap_or(crate::models::AppStateConfigFile {
            width: None,
            height: None,
            volume: None,
            sidebar_width: None,
        });

        let legacy_highlight_color = style.control_color.clone();

        sanitize_config(AppConfig {
            music_directory: config
                .music_directory
                .map(MusicDirectoryConfig::into_vec)
                .unwrap_or_else(|| default_config.music_directory.clone()),
            decoder: crate::models::DecoderConfig {
                output_dir: decoder
                    .output_dir
                    .unwrap_or_else(|| default_config.decoder.output_dir.clone()),
                process_formats: decoder
                    .process_formats
                    .unwrap_or_else(|| default_config.decoder.process_formats.clone()),
                scan_directory: decoder
                    .scan_directory
                    .map(MusicDirectoryConfig::into_vec)
                    .unwrap_or_else(|| default_config.decoder.scan_directory.clone()),
            },
            cache: CacheConfig {
                library_cache_dir: cache
                    .library_cache_dir
                    .or(config.library_cache_dir)
                    .unwrap_or_else(|| default_config.cache.library_cache_dir.clone()),
                cover_cache_dir: cache
                    .cover_cache_dir
                    .or(config.cover_cache_dir)
                    .unwrap_or_else(|| default_config.cache.cover_cache_dir.clone()),
                lyrics_cache_dir: cache
                    .lyrics_cache_dir
                    .or(config.lyrics_cache_dir)
                    .unwrap_or_else(|| default_config.cache.lyrics_cache_dir.clone()),
                my_playlist_cache_dir: cache
                    .my_playlist_cache_dir
                    .or(config.my_playlist_cache_dir)
                    .unwrap_or_else(|| default_config.cache.my_playlist_cache_dir.clone()),
                log_dir: cache
                    .log_dir
                    .or(config.log_dir)
                    .unwrap_or_else(|| default_config.cache.log_dir.clone()),
                play_statistics_cache_path: cache
                    .play_statistics_cache_path
                    .or(config.play_statistics_cache_path)
                    .unwrap_or_else(|| default_config.cache.play_statistics_cache_path.clone()),
            },
            style: crate::models::StyleConfig {
                background_color: style
                    .background_color
                    .unwrap_or_else(|| default_config.style.background_color.clone()),
                background_image: style
                    .background_image
                    .unwrap_or_else(|| default_config.style.background_image.clone()),
                background_image_opacity: style
                    .background_image_opacity
                    .unwrap_or(default_config.style.background_image_opacity),
                title_color: style
                    .title_color
                    .unwrap_or_else(|| default_config.style.title_color.clone()),
                subtitle_color: style
                    .subtitle_color
                    .unwrap_or_else(|| default_config.style.subtitle_color.clone()),
                highlight_color: style
                    .highlight_color
                    .or(legacy_highlight_color)
                    .unwrap_or_else(|| default_config.style.highlight_color.clone()),
                show_border: style.show_border.unwrap_or(default_config.style.show_border),
            },
            state: crate::models::AppStateConfig {
                width: state.width.unwrap_or(default_config.state.width),
                height: state.height.unwrap_or(default_config.state.height),
                volume: state.volume.unwrap_or(default_config.state.volume),
                sidebar_width: state
                    .sidebar_width
                    .unwrap_or(default_config.state.sidebar_width),
            },
        })
    })
}

fn sanitize_config(mut config: AppConfig) -> AppConfig {
    config
        .music_directory
        .retain(|directory| !directory.trim().is_empty());
    dedup_strings(&mut config.music_directory);
    config
        .decoder
        .scan_directory
        .retain(|directory| !directory.trim().is_empty());
    dedup_strings(&mut config.decoder.scan_directory);
    config.decoder.process_formats = sanitize_process_formats(&config.decoder.process_formats);
    config.state.volume = config.state.volume.clamp(0.0, 1.5);
    config.style.background_image_opacity = config.style.background_image_opacity.clamp(0.0, 1.0);
    config.state.width = config.state.width.max(600);
    config.state.height = config.state.height.max(700);
    config.state.sidebar_width = config.state.sidebar_width.clamp(72, 420);
    config
}

fn sanitize_process_formats(value: &str) -> String {
    let mut formats: Vec<String> = value
        .split(',')
        .map(|format| format.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|format| !format.is_empty())
        .collect();
    dedup_strings(&mut formats);
    if formats.is_empty() {
        "mp3,flac,kgm,kgma,ncm".to_string()
    } else {
        formats.join(",")
    }
}

fn dedup_strings(values: &mut Vec<String>) {
    let mut unique_values = Vec::new();
    values.retain(|value| {
        if unique_values.iter().any(|current| current == value) {
            return false;
        }
        unique_values.push(value.clone());
        true
    });
}
