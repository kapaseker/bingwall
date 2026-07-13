use std::path::PathBuf;

use thiserror::Error;

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

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn feed_file(&self) -> PathBuf {
        self.cache_dir.join("feed.json")
    }

    pub fn images_dir(&self) -> PathBuf {
        self.cache_dir.join("images")
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.cache_dir.join("previews-v1")
    }
}
