use std::{borrow::Cow, env};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    English,
    SimplifiedChinese,
}

#[derive(Debug, Clone, Copy)]
pub enum TextKey {
    Unsupported,
    DailyChange,
    Previous,
    Next,
    SetWallpaper,
    Refresh,
    LoadingFeed,
    LoadingPreview,
    Ready,
    FeedRefreshed,
    CachedFeed,
    CachedFeedRefreshing,
    Applied,
    Enabled,
    Disabled,
    Working,
    Retry,
    PageCounter { current: usize, total: usize },
}

impl Locale {
    /// Detects the active locale from the standard locale environment variables.
    pub fn detect() -> Self {
        let value = env::var("LC_ALL")
            .or_else(|_| env::var("LC_MESSAGES"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        Self::from_name(&value)
    }

    /// Maps a locale name to one of the translations supported by the application.
    pub fn from_name(value: &str) -> Self {
        if value.to_ascii_lowercase().starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    /// Resolves a localized user-interface message for this locale.
    pub fn text(self, key: TextKey) -> Cow<'static, str> {
        resolve_text(self, key)
    }
}

/// Resolves static and parameterized user-interface messages for a locale.
pub fn resolve_text(locale: Locale, key: TextKey) -> Cow<'static, str> {
    use TextKey::*;

    match (locale, key) {
        (Locale::English, Unsupported) => {
            "Bingwall supports only GNOME and Cinnamon on this platform.".into()
        }
        (Locale::SimplifiedChinese, Unsupported) => {
            "此平台不受支持。Bingwall 仅支持 GNOME 和 Cinnamon。".into()
        }
        (Locale::English, DailyChange) => "Daily change".into(),
        (Locale::SimplifiedChinese, DailyChange) => "每日更换".into(),
        (Locale::English, Previous) => "Previous wallpaper".into(),
        (Locale::SimplifiedChinese, Previous) => "上一张壁纸".into(),
        (Locale::English, Next) => "Next wallpaper".into(),
        (Locale::SimplifiedChinese, Next) => "下一张壁纸".into(),
        (Locale::English, SetWallpaper) => "Set as wallpaper".into(),
        (Locale::SimplifiedChinese, SetWallpaper) => "设为壁纸".into(),
        (Locale::English, Refresh) => "Refresh feed".into(),
        (Locale::SimplifiedChinese, Refresh) => "刷新壁纸源".into(),
        (Locale::English, LoadingFeed) => "Loading wallpaper feed…".into(),
        (Locale::SimplifiedChinese, LoadingFeed) => "正在加载壁纸源…".into(),
        (Locale::English, LoadingPreview) => "Loading preview…".into(),
        (Locale::SimplifiedChinese, LoadingPreview) => "正在加载预览…".into(),
        (Locale::English, Ready) => "Ready".into(),
        (Locale::SimplifiedChinese, Ready) => "就绪".into(),
        (Locale::English, FeedRefreshed) => "Feed refreshed".into(),
        (Locale::SimplifiedChinese, FeedRefreshed) => "壁纸源已刷新".into(),
        (Locale::English, CachedFeed) => "Offline — showing cached feed".into(),
        (Locale::SimplifiedChinese, CachedFeed) => "离线 — 正在显示缓存的壁纸源".into(),
        (Locale::English, CachedFeedRefreshing) => "Showing cached feed while refreshing…".into(),
        (Locale::SimplifiedChinese, CachedFeedRefreshing) => "正在显示缓存并后台刷新…".into(),
        (Locale::English, Applied) => "Wallpaper applied".into(),
        (Locale::SimplifiedChinese, Applied) => "壁纸已应用".into(),
        (Locale::English, Enabled) => "Daily change enabled".into(),
        (Locale::SimplifiedChinese, Enabled) => "已启用每日更换".into(),
        (Locale::English, Disabled) => "Daily change disabled".into(),
        (Locale::SimplifiedChinese, Disabled) => "已关闭每日更换".into(),
        (Locale::English, Working) => "Working…".into(),
        (Locale::SimplifiedChinese, Working) => "处理中…".into(),
        (Locale::English, Retry) => "Retry".into(),
        (Locale::SimplifiedChinese, Retry) => "重试".into(),
        (_, PageCounter { current, total }) => format!("{current} / {total}").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies only locale names beginning with `zh` select Chinese text.
    fn selects_chinese_only_for_zh_locales() {
        assert_eq!(Locale::from_name("zh_CN.UTF-8"), Locale::SimplifiedChinese);
        assert_eq!(Locale::from_name("en_US.UTF-8"), Locale::English);
    }

    #[test]
    /// Keeps parameter formatting inside the localization boundary.
    fn formats_page_counter_from_runtime_values() {
        let key = TextKey::PageCounter {
            current: 3,
            total: 10,
        };
        assert_eq!(resolve_text(Locale::English, key), "3 / 10");
        assert_eq!(resolve_text(Locale::SimplifiedChinese, key), "3 / 10");
    }
}
