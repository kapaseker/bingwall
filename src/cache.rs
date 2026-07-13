use std::{
    collections::HashSet,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{feed::WallpaperEntry, paths::AppPaths, settings::Settings};

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("could not read cached feed: {0}")]
    ReadFeed(#[source] io::Error),
    #[error("could not decode cached feed: {0}")]
    DecodeFeed(#[source] serde_json::Error),
    #[error("could not encode cached feed: {0}")]
    EncodeFeed(#[source] serde_json::Error),
    #[error("could not write cached feed: {0}")]
    WriteFeed(#[source] io::Error),
    #[error("could not clean the image cache: {0}")]
    CleanImages(#[source] io::Error),
}

pub fn load_feed(paths: &AppPaths) -> Result<Vec<WallpaperEntry>, CacheError> {
    let data = fs::read(paths.feed_file()).map_err(CacheError::ReadFeed)?;
    serde_json::from_slice(&data).map_err(CacheError::DecodeFeed)
}

pub fn save_feed(paths: &AppPaths, entries: &[WallpaperEntry]) -> Result<(), CacheError> {
    fs::create_dir_all(&paths.cache_dir).map_err(CacheError::WriteFeed)?;
    let data = serde_json::to_vec(entries).map_err(CacheError::EncodeFeed)?;
    let destination = paths.feed_file();
    let temporary = destination.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(CacheError::WriteFeed)?;
    fs::rename(temporary, destination).map_err(CacheError::WriteFeed)
}

pub fn image_path(paths: &AppPaths, entry: &WallpaperEntry) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    entry.image_url.hash(&mut hasher);
    paths
        .images_dir()
        .join(format!("{:016x}.jpg", hasher.finish()))
}

pub fn prune_images(paths: &AppPaths, settings: &Settings) -> Result<(), CacheError> {
    let images_dir = paths.images_dir();
    let entries = match fs::read_dir(&images_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(CacheError::CleanImages(error)),
    };
    let mut keep = settings
        .recent_images
        .iter()
        .map(PathBuf::from)
        .collect::<HashSet<_>>();
    if let Some(applied) = &settings.applied_image {
        keep.insert(PathBuf::from(applied));
    }

    for entry in entries {
        let path = entry.map_err(CacheError::CleanImages)?.path();
        if path.is_file() && !keep.contains(&path) {
            fs::remove_file(path).map_err(CacheError::CleanImages)?;
        }
    }
    Ok(())
}

pub fn write_image_atomically(destination: &Path, data: &[u8]) -> Result<(), io::Error> {
    let parent = destination.parent().expect("image path has a parent");
    fs::create_dir_all(parent)?;
    let temporary = destination.with_extension("jpg.tmp");
    fs::write(&temporary, data)?;
    fs::rename(temporary, destination)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temporary_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "bingwall-cache-{}",
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
    fn feed_round_trips_and_image_names_are_stable() {
        let paths = temporary_paths();
        let entries = vec![WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Lake".into(),
            image_url: "https://cn.bing.com/lake.jpg".into(),
        }];

        save_feed(&paths, &entries).unwrap();
        assert_eq!(load_feed(&paths).unwrap(), entries);
        assert_eq!(
            image_path(&paths, &entries[0]),
            image_path(&paths, &entries[0])
        );

        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn prune_preserves_applied_and_recent_images() {
        let paths = temporary_paths();
        fs::create_dir_all(paths.images_dir()).unwrap();
        let applied = paths.images_dir().join("applied.jpg");
        let recent = paths.images_dir().join("recent.jpg");
        let old = paths.images_dir().join("old.jpg");
        for path in [&applied, &recent, &old] {
            fs::write(path, b"image").unwrap();
        }
        let settings = Settings {
            applied_image: Some(applied.to_string_lossy().into_owned()),
            recent_images: vec![recent.to_string_lossy().into_owned()],
            ..Settings::default()
        };

        prune_images(&paths, &settings).unwrap();

        assert!(applied.exists());
        assert!(recent.exists());
        assert!(!old.exists());
        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }
}
