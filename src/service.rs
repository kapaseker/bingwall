use thiserror::Error;

use crate::{
    FEED_URL, cache,
    feed::{self, WallpaperEntry},
    paths::AppPaths,
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
}

/// Fetches and caches the remote feed, falling back to the cached feed when refresh fails.
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
