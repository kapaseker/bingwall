use std::env;

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
}

impl Locale {
    pub fn detect() -> Self {
        let value = env::var("LC_ALL")
            .or_else(|_| env::var("LC_MESSAGES"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        Self::from_name(&value)
    }

    pub fn from_name(value: &str) -> Self {
        if value.to_ascii_lowercase().starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    pub fn text(self, key: TextKey) -> &'static str {
        use TextKey::*;
        match (self, key) {
            (Self::English, Unsupported) => {
                "Bingwall supports only GNOME and Cinnamon on this platform."
            }
            (Self::SimplifiedChinese, Unsupported) => {
                "此平台不受支持。Bingwall 仅支持 GNOME 和 Cinnamon。"
            }
            (Self::English, DailyChange) => "Daily change",
            (Self::SimplifiedChinese, DailyChange) => "每日更换",
            (Self::English, Previous) => "Previous wallpaper",
            (Self::SimplifiedChinese, Previous) => "上一张壁纸",
            (Self::English, Next) => "Next wallpaper",
            (Self::SimplifiedChinese, Next) => "下一张壁纸",
            (Self::English, SetWallpaper) => "Set as wallpaper",
            (Self::SimplifiedChinese, SetWallpaper) => "设为壁纸",
            (Self::English, Refresh) => "Refresh feed",
            (Self::SimplifiedChinese, Refresh) => "刷新壁纸源",
            (Self::English, LoadingFeed) => "Loading wallpaper feed…",
            (Self::SimplifiedChinese, LoadingFeed) => "正在加载壁纸源…",
            (Self::English, LoadingPreview) => "Loading preview…",
            (Self::SimplifiedChinese, LoadingPreview) => "正在加载预览…",
            (Self::English, Ready) => "Ready",
            (Self::SimplifiedChinese, Ready) => "就绪",
            (Self::English, FeedRefreshed) => "Feed refreshed",
            (Self::SimplifiedChinese, FeedRefreshed) => "壁纸源已刷新",
            (Self::English, CachedFeed) => "Offline — showing cached feed",
            (Self::SimplifiedChinese, CachedFeed) => "离线 — 正在显示缓存的壁纸源",
            (Self::English, CachedFeedRefreshing) => "Showing cached feed while refreshing…",
            (Self::SimplifiedChinese, CachedFeedRefreshing) => "正在显示缓存并后台刷新…",
            (Self::English, Applied) => "Wallpaper applied",
            (Self::SimplifiedChinese, Applied) => "壁纸已应用",
            (Self::English, Enabled) => "Daily change enabled",
            (Self::SimplifiedChinese, Enabled) => "已启用每日更换",
            (Self::English, Disabled) => "Daily change disabled",
            (Self::SimplifiedChinese, Disabled) => "已关闭每日更换",
            (Self::English, Working) => "Working…",
            (Self::SimplifiedChinese, Working) => "处理中…",
            (Self::English, Retry) => "Retry",
            (Self::SimplifiedChinese, Retry) => "重试",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_chinese_only_for_zh_locales() {
        assert_eq!(Locale::from_name("zh_CN.UTF-8"), Locale::SimplifiedChinese);
        assert_eq!(Locale::from_name("en_US.UTF-8"), Locale::English);
    }
}
