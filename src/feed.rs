use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WallpaperEntry {
    pub date: String,
    pub description: String,
    pub image_url: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FeedError {
    #[error("the wallpaper feed contains no valid HTTPS entries")]
    NoEntries,
}

pub fn parse(markdown: &str) -> Result<Vec<WallpaperEntry>, FeedError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_in_feed_order() {
        let markdown = "## Bing Wallpaper 2026-01-02 | [Lake (© Person)](https://cn.bing.com/a.jpg) 2026-01-01 | [Hill](https://cn.bing.com/b.jpg)";

        let entries = parse(markdown).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].date, "2026-01-02");
        assert_eq!(entries[0].description, "Lake (© Person)");
        assert_eq!(entries[1].image_url, "https://cn.bing.com/b.jpg");
    }

    #[test]
    fn rejects_non_https_and_empty_feeds() {
        assert_eq!(
            parse("2026-01-02 | [Lake](http://example.com/a.jpg)"),
            Err(FeedError::NoEntries)
        );
    }
}
