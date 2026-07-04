use crate::models::{AppConfig, ConfigFile, MusicDirectoryConfig};
use crate::utils::{current_app_dir, safe_file_name, short_hash};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
pub(crate) struct ConfigManager {
    config_path: PathBuf,
    config: Mutex<AppConfig>,
}

impl ConfigManager {
    pub(crate) fn new() -> Self {
        let app_dir = current_app_dir();
        let config_path = app_dir.join("config.toml");
        let default_config = AppConfig {
            music_directory: Vec::new(),
            library_cache_dir: app_dir.join("library-cache").to_string_lossy().to_string(),
            cover_cache_dir: app_dir.join("cover-cache").to_string_lossy().to_string(),
            lyrics_cache_dir: app_dir.join("lyrics-cache").to_string_lossy().to_string(),
            my_playlist_cache_dir: app_dir
                .join("my-playlist-cache")
                .to_string_lossy()
                .to_string(),
            log_dir: app_dir.join("logs").to_string_lossy().to_string(),
        };

        let config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|content| parse_config(&content, &default_config))
            .unwrap_or(default_config);

        let manager = Self {
            config_path,
            config: Mutex::new(config),
        };

        let _ = manager.ensure_layout();
        let _ = manager.save();
        manager
    }

    pub(crate) fn get(&self) -> Result<AppConfig, String> {
        self.config
            .lock()
            .map_err(|_| "config state is unavailable".to_string())
            .map(|config| config.clone())
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
        fs::create_dir_all(&config.library_cache_dir)
            .map_err(|err| format!("无法创建歌曲列表缓存目录: {err}"))?;
        fs::create_dir_all(&config.cover_cache_dir)
            .map_err(|err| format!("无法创建图标缓存目录: {err}"))?;
        fs::create_dir_all(&config.lyrics_cache_dir)
            .map_err(|err| format!("无法创建歌词缓存目录: {err}"))?;
        fs::create_dir_all(&config.my_playlist_cache_dir)
            .map_err(|err| format!("无法创建我的歌单缓存目录: {err}"))?;
        fs::create_dir_all(&config.log_dir).map_err(|err| format!("无法创建日志目录: {err}"))?;
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
        Ok(PathBuf::from(config.library_cache_dir).join(format!("{safe_name}-{hash}.json")))
    }
}

pub(crate) fn parse_config(content: &str, default_config: &AppConfig) -> Option<AppConfig> {
    toml::from_str::<ConfigFile>(content)
        .ok()
        .map(|config| AppConfig {
            music_directory: config
                .music_directory
                .map(MusicDirectoryConfig::into_vec)
                .unwrap_or_else(|| default_config.music_directory.clone()),
            library_cache_dir: config
                .library_cache_dir
                .unwrap_or_else(|| default_config.library_cache_dir.clone()),
            cover_cache_dir: config
                .cover_cache_dir
                .unwrap_or_else(|| default_config.cover_cache_dir.clone()),
            lyrics_cache_dir: config
                .lyrics_cache_dir
                .unwrap_or_else(|| default_config.lyrics_cache_dir.clone()),
            my_playlist_cache_dir: config
                .my_playlist_cache_dir
                .unwrap_or_else(|| default_config.my_playlist_cache_dir.clone()),
            log_dir: config
                .log_dir
                .unwrap_or_else(|| default_config.log_dir.clone()),
        })
}
