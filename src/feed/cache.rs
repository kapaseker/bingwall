use std::{fs, io};

use thiserror::Error;

use crate::paths::AppPaths;

use super::WallpaperEntry;

/// Reports failures while reading or writing the cached Wallpaper Feed.
#[derive(Debug, Error)]
pub(crate) enum CacheError {
    #[error("could not read cached feed: {0}")]
    Read(#[source] io::Error),
    #[error("could not decode cached feed: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("could not encode cached feed: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("could not write cached feed: {0}")]
    Write(#[source] io::Error),
}

/// Loads and deserializes the cached Wallpaper Feed.
pub(super) fn load_feed(paths: &AppPaths) -> Result<Vec<WallpaperEntry>, CacheError> {
    let data = fs::read(paths.feed_file()).map_err(CacheError::Read)?;
    serde_json::from_slice(&data).map_err(CacheError::Decode)
}

/// Serializes the Wallpaper Feed and replaces the cache file atomically.
pub(super) fn save_feed(paths: &AppPaths, entries: &[WallpaperEntry]) -> Result<(), CacheError> {
    fs::create_dir_all(&paths.cache_dir).map_err(CacheError::Write)?;
    let data = serde_json::to_vec(entries).map_err(CacheError::Encode)?;
    let destination = paths.feed_file();
    let temporary = destination.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(CacheError::Write)?;
    fs::rename(temporary, destination).map_err(CacheError::Write)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// Creates isolated cache and configuration paths for a Wallpaper Feed cache test.
    fn temporary_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "bingwall-feed-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        AppPaths {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
        }
    }

    #[test]
    /// Verifies Wallpaper Feed entries retain their values after cached persistence.
    fn feed_round_trips_through_cache() {
        let paths = temporary_paths();
        let entries = vec![WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Lake".into(),
            image_url: "https://cn.bing.com/lake.jpg".into(),
        }];

        save_feed(&paths, &entries).unwrap();

        assert_eq!(load_feed(&paths).unwrap(), entries);
        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }
}
