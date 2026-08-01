use thiserror::Error;

use crate::paths::AppPaths;

use super::{FeedError, WallpaperEntry, WallpaperSource, cache, parse_source};

const BING_FEED_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
const SPOTLIGHT_FEED_URL: &str = "https://windows10spotlight.com/feed";

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
    source: WallpaperSource,
) -> Result<(Vec<WallpaperEntry>, FeedOrigin), RefreshError> {
    let remote = async {
        let contents = client
            .get(feed_url(source))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let entries = parse_source(source, &contents)?;
        cache::save_feed(paths, source, &entries)?;
        Ok::<_, RefreshError>(entries)
    }
    .await;

    match remote {
        Ok(entries) => Ok((entries, FeedOrigin::Network)),
        Err(_) => cache::load_feed(paths, source)
            .map(|entries| (entries, FeedOrigin::Cache))
            .map_err(RefreshError::Cache),
    }
}

/// Returns the remote Feed endpoint owned by a Wallpaper Source.
fn feed_url(source: WallpaperSource) -> &'static str {
    match source {
        WallpaperSource::Bing => BING_FEED_URL,
        WallpaperSource::Spotlight => SPOTLIGHT_FEED_URL,
    }
}
