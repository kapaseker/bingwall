use std::path::PathBuf;

use thiserror::Error;

use crate::feed::WallpaperSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("the user configuration directory is unavailable")]
    MissingConfigDirectory,
    #[error("the user cache directory is unavailable")]
    MissingCacheDirectory,
}

impl AppPaths {
    /// Resolves the per-user configuration and cache directories used by Bingwall.
    pub fn discover() -> Result<Self, PathError> {
        let config_dir = dirs::config_dir()
            .ok_or(PathError::MissingConfigDirectory)?
            .join("bingwall");
        let cache_dir = dirs::cache_dir()
            .ok_or(PathError::MissingCacheDirectory)?
            .join("bingwall");
        Ok(Self {
            config_dir,
            cache_dir,
        })
    }

    /// Returns the path to the persisted user settings file.
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    /// Returns the independent cached Wallpaper Feed path for a source.
    pub(crate) fn feed_file(&self, source: WallpaperSource) -> PathBuf {
        let filename = match source {
            WallpaperSource::Bing => "feed.json",
            WallpaperSource::Spotlight => "spotlight-feed.json",
        };
        self.cache_dir.join(filename)
    }

    /// Returns the directory that stores full-resolution wallpaper images.
    pub fn images_dir(&self) -> PathBuf {
        self.cache_dir.join("images")
    }

    /// Returns the versioned directory that stores generated previews.
    pub fn previews_dir(&self) -> PathBuf {
        self.cache_dir.join("previews-v1")
    }
}
