use std::{io, path::PathBuf, time::Duration};

use thiserror::Error;

use crate::{
    FEED_URL, cache,
    feed::{self, WallpaperEntry},
    paths::AppPaths,
    platform::{Desktop, PlatformError},
    settings::{Settings, SettingsError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedOrigin {
    Network,
    Cache,
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("wallpaper feed is invalid: {0}")]
    Feed(#[from] feed::FeedError),
    #[error("cached data is unavailable: {0}")]
    Cache(#[from] cache::CacheError),
    #[error("could not save the image: {0}")]
    SaveImage(#[source] io::Error),
    #[error("could not resolve the user data directories: {0}")]
    Paths(String),
    #[error("downloaded data is not a supported image: {0}")]
    DecodeImage(#[from] image::ImageError),
    #[error("background image task failed: {0}")]
    BackgroundTask(String),
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error("the wallpaper feed is empty")]
    EmptyFeed,
    #[error("daily change is disabled")]
    DailyChangeDisabled,
}

pub async fn refresh_feed(
    client: &reqwest::Client,
    paths: &AppPaths,
) -> Result<(Vec<WallpaperEntry>, FeedOrigin), ServiceError> {
    let remote = async {
        let markdown = client
            .get(FEED_URL)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let entries = feed::parse(&markdown)?;
        cache::save_feed(paths, &entries)?;
        Ok::<_, ServiceError>(entries)
    }
    .await;

    match remote {
        Ok(entries) => Ok((entries, FeedOrigin::Network)),
        Err(_) => cache::load_feed(paths)
            .map(|entries| (entries, FeedOrigin::Cache))
            .map_err(ServiceError::Cache),
    }
}

pub async fn ensure_image(
    client: &reqwest::Client,
    paths: &AppPaths,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ServiceError> {
    let cached_paths = paths.clone();
    let cached_entry = entry.clone();
    if let Some(cached) =
        tokio::task::spawn_blocking(move || cache::valid_image_path(&cached_paths, &cached_entry))
            .await
            .map_err(|error| ServiceError::BackgroundTask(error.to_string()))?
    {
        return Ok(cached);
    }
    download_original(client, paths, entry).await
}

pub async fn ensure_preview(
    client: &reqwest::Client,
    paths: &AppPaths,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ServiceError> {
    let cached_paths = paths.clone();
    let cached_entry = entry.clone();
    if let Some(cached) =
        tokio::task::spawn_blocking(move || cache::valid_preview_path(&cached_paths, &cached_entry))
            .await
            .map_err(|error| ServiceError::BackgroundTask(error.to_string()))?
    {
        return Ok(cached);
    }

    let original = cache::image_path(paths, entry);
    let original_exists = {
        let original = original.clone();
        tokio::task::spawn_blocking(move || original.exists())
            .await
            .map_err(|error| ServiceError::BackgroundTask(error.to_string()))?
    };
    let original = if original_exists {
        original
    } else {
        download_original(client, paths, entry).await?
    };
    let preview = cache::preview_path(paths, entry);

    match generate_preview(original.clone(), preview.clone()).await {
        Ok(()) => Ok(preview),
        Err(_) if original_exists => {
            tokio::task::spawn_blocking(move || std::fs::remove_file(original))
                .await
                .map_err(|error| ServiceError::BackgroundTask(error.to_string()))?
                .ok();
            let original = download_original(client, paths, entry).await?;
            generate_preview(original, preview.clone()).await?;
            Ok(preview)
        }
        Err(error) => Err(error),
    }
}

async fn generate_preview(original: PathBuf, preview: PathBuf) -> Result<(), ServiceError> {
    let mut attempt = 0;
    loop {
        let original = original.clone();
        let preview = preview.clone();
        let result =
            tokio::task::spawn_blocking(move || cache::generate_preview(&original, &preview))
                .await
                .map_err(|error| ServiceError::BackgroundTask(error.to_string()))?;
        match result {
            Ok(()) => return Ok(()),
            Err(_) if attempt < 2 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200 * attempt)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

async fn download_original(
    client: &reqwest::Client,
    paths: &AppPaths,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ServiceError> {
    let destination = cache::image_path(paths, entry);
    let bytes = download_with_retry(client, &entry.image_url).await?;
    let write_destination = destination.clone();
    tokio::task::spawn_blocking(move || {
        image::load_from_memory(&bytes)?;
        cache::write_image_atomically(&write_destination, &bytes)
            .map_err(ServiceError::SaveImage)?;
        Ok::<_, ServiceError>(())
    })
    .await
    .map_err(|error| ServiceError::BackgroundTask(error.to_string()))??;
    Ok(destination)
}

async fn download_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<u8>, reqwest::Error> {
    let mut attempt = 0;
    loop {
        let result = async {
            client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await
        }
        .await;
        match result {
            Ok(bytes) => return Ok(bytes.to_vec()),
            Err(_) if attempt < 2 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200 * attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

pub async fn run_scheduled_update() -> Result<PathBuf, ServiceError> {
    let paths = AppPaths::discover().map_err(|error| ServiceError::Paths(error.to_string()))?;
    let mut settings = Settings::load(&paths.settings_file())?;
    if !settings.daily_change {
        return Err(ServiceError::DailyChangeDisabled);
    }
    let desktop = Desktop::detect()?;
    let client = reqwest::Client::new();
    let (entries, _) = refresh_feed(&client, &paths).await?;
    let current = entries.first().ok_or(ServiceError::EmptyFeed)?;
    let image = ensure_image(&client, &paths, current).await?;
    desktop.apply(&image)?;

    settings.applied_image = Some(image.to_string_lossy().into_owned());
    settings.last_update_status = Some(format!("Updated to {}", current.date));
    settings.save(&paths.settings_file())?;
    Ok(image)
}

pub fn mark_failed_update(error: &ServiceError) {
    let Ok(paths) = AppPaths::discover() else {
        return;
    };
    let Ok(mut settings) = Settings::load(&paths.settings_file()) else {
        return;
    };
    settings.last_update_status = Some(format!("Update failed: {error}"));
    let _ = settings.save(&paths.settings_file());
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn valid_cached_image_is_used_without_downloading_again() {
        let root = std::env::temp_dir().join(format!(
            "bingwall-service-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = AppPaths {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
        };
        let entry = WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Cached image".into(),
            image_url: "https://127.0.0.1:9/cached.jpg".into(),
        };
        let image = cache::image_path(&paths, &entry);
        let preview = cache::preview_path(&paths, &entry);
        std::fs::create_dir_all(paths.images_dir()).unwrap();
        image::DynamicImage::new_rgb8(1, 1).save(&image).unwrap();
        std::fs::create_dir_all(paths.previews_dir()).unwrap();
        image::DynamicImage::new_rgb8(cache::PREVIEW_WIDTH, cache::PREVIEW_HEIGHT)
            .save(&preview)
            .unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime
            .block_on(ensure_image(&reqwest::Client::new(), &paths, &entry))
            .unwrap();

        assert_eq!(result, image);
        assert_ne!(result, preview);
        assert!(image.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cached_original_generates_preview_without_network() {
        let root = std::env::temp_dir().join(format!(
            "bingwall-service-preview-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = AppPaths {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
        };
        let entry = WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Cached original".into(),
            image_url: "https://127.0.0.1:9/cached.jpg".into(),
        };
        let original = cache::image_path(&paths, &entry);
        std::fs::create_dir_all(paths.images_dir()).unwrap();
        image::DynamicImage::new_rgb8(3840, 2160)
            .save(&original)
            .unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let preview = runtime
            .block_on(ensure_preview(&reqwest::Client::new(), &paths, &entry))
            .unwrap();

        assert_eq!(preview, cache::preview_path(&paths, &entry));
        assert_eq!(cache::valid_preview_path(&paths, &entry), Some(preview));
        std::fs::remove_dir_all(root).unwrap();
    }
}
