use thiserror::Error;

use crate::paths::AppPaths;

use super::{FeedError, WallpaperEntry, cache, parse};

const FEED_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/refs/heads/main/bing-wallpaper.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedOrigin {
    Network,
    Cache,
}

#[derive(Debug, Error)]
pub(crate) enum RefreshError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("wallpaper feed is invalid: {0}")]
    Feed(#[from] FeedError),
    #[error("cached data is unavailable: {0}")]
    Cache(#[from] cache::CacheError),
}

/// Fetches and caches the remote feed, falling back to the cached feed when refresh fails.
pub(crate) async fn refresh_feed(
    client: &reqwest::Client,
    paths: &AppPaths,
) -> Result<(Vec<WallpaperEntry>, FeedOrigin), RefreshError> {
    let remote = async {
        let markdown = client
            .get(FEED_URL)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let entries = parse(&markdown)?;
        cache::save_feed(paths, &entries)?;
        Ok::<_, RefreshError>(entries)
    }
    .await;

    match remote {
        Ok(entries) => Ok((entries, FeedOrigin::Network)),
        Err(_) => cache::load_feed(paths)
            .map(|entries| (entries, FeedOrigin::Cache))
            .map_err(RefreshError::Cache),
    }
}
