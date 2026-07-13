use std::{io, path::PathBuf};

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
    if let Some(cached) = cache::valid_image_path(paths, entry) {
        return Ok(cached);
    }
    let destination = cache::image_path(paths, entry);

    let bytes = client
        .get(&entry.image_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    image::load_from_memory(&bytes)?;
    cache::write_image_atomically(&destination, &bytes).map_err(ServiceError::SaveImage)?;
    Ok(destination)
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
        std::fs::create_dir_all(paths.images_dir()).unwrap();
        image::DynamicImage::new_rgb8(1, 1).save(&image).unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime
            .block_on(ensure_image(&reqwest::Client::new(), &paths, &entry))
            .unwrap();

        assert_eq!(result, image);
        assert!(image.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
