use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod cache;
mod refresh;
mod source;
mod spotlight;

pub(crate) use refresh::{FeedOrigin, refresh_feed};
pub(crate) use source::WallpaperSource;

use crate::paths::AppPaths;

/// Loads one source's cached Wallpaper Feed for local-first application startup.
pub(crate) fn load_cached(
    paths: &AppPaths,
    source: WallpaperSource,
) -> Result<Vec<WallpaperEntry>, cache::CacheError> {
    cache::load_feed(paths, source)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WallpaperEntry {
    pub date: String,
    pub description: String,
    pub image_url: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum FeedError {
    #[error("the wallpaper feed contains no valid HTTPS entries")]
    NoEntries,
}

/// Extracts dated HTTPS wallpaper entries from the Markdown feed in source order.
pub(crate) fn parse(markdown: &str) -> Result<Vec<WallpaperEntry>, FeedError> {
    static ENTRY: OnceLock<Regex> = OnceLock::new();
    let entry = ENTRY.get_or_init(|| {
        Regex::new(r"(?s)(\d{4}-\d{2}-\d{2})\s*\|\s*\[(.*?)\]\((https://[^)]+)\)")
            .expect("the feed regex is valid")
    });

    let entries = entry
        .captures_iter(markdown)
        .map(|capture| WallpaperEntry {
            date: capture[1].to_owned(),
            description: capture[2].trim().to_owned(),
            image_url: capture[3].to_owned(),
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        Err(FeedError::NoEntries)
    } else {
        Ok(entries)
    }
}

/// Parses a provider response using the representation owned by that source.
fn parse_source(source: WallpaperSource, contents: &str) -> Result<Vec<WallpaperEntry>, FeedError> {
    match source {
        WallpaperSource::Bing => parse(contents),
        WallpaperSource::Spotlight => spotlight::parse(contents),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies valid entries and descriptions are parsed without reordering.
    fn parses_entries_in_feed_order() {
        let markdown = "## Bing Wallpaper 2026-01-02 | [Lake (© Person)](https://cn.bing.com/a.jpg) 2026-01-01 | [Hill](https://cn.bing.com/b.jpg)";

        let entries = parse(markdown).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].date, "2026-01-02");
        assert_eq!(entries[0].description, "Lake (© Person)");
        assert_eq!(entries[1].image_url, "https://cn.bing.com/b.jpg");
    }

    #[test]
    /// Verifies a feed without HTTPS entries is rejected.
    fn rejects_non_https_and_empty_feeds() {
        assert_eq!(
            parse("2026-01-02 | [Lake](http://example.com/a.jpg)"),
            Err(FeedError::NoEntries)
        );
    }

    #[test]
    /// Verifies each Wallpaper Source uses its own Feed representation.
    fn parses_feed_according_to_source() {
        let bing = "2026-01-02 | [Lake](https://cn.bing.com/a.jpg)";
        let spotlight = r#"<rss xmlns:content="urn:content"><channel><item>
            <title>Cliffs</title><pubDate>Sat, 01 Aug 2026 12:00:00 +0000</pubDate>
            <content:encoded><![CDATA[<a href="https://windows10spotlight.com/cliffs.jpg"><img width="1920" height="1080" /></a>]]></content:encoded>
            </item></channel></rss>"#;

        assert_eq!(
            parse_source(WallpaperSource::Bing, bing).unwrap()[0].description,
            "Lake"
        );
        assert_eq!(
            parse_source(WallpaperSource::Spotlight, spotlight).unwrap()[0].description,
            "Cliffs"
        );
        assert_eq!(
            parse_source(WallpaperSource::Spotlight, bing),
            Err(FeedError::NoEntries)
        );
    }
}
