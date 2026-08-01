use serde::{Deserialize, Serialize};

/// Identifies a provider that supplies a Wallpaper Feed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WallpaperSource {
    #[default]
    Bing,
    Spotlight,
}
