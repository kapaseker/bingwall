pub mod app;
pub mod cache;
pub mod feed;
pub mod paths;
pub mod platform;
pub(crate) mod preview;
#[macro_use]
pub mod resources;
pub mod service;
pub mod settings;
pub mod systemd;
mod theme;
mod ui;

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/refs/heads/main/bing-wallpaper.md";

#[cfg(test)]
mod generated_resource_macro_tests {
    use crate::resources::{Locale, lock_locale_tests, set_locale};

    #[test]
    /// Verifies generated naked-key macros resolve every resource category.
    fn generated_macros_resolve_typed_resource_keys() {
        let _locale_guard = lock_locale_tests();
        set_locale(Locale::English);

        assert_eq!(text!(daily_change), "Daily change");
        assert_eq!(text!(page_counter, 3, 10), "3 / 10");
        assert_eq!(color!(surface_fallback), iced::Color::from_rgb8(18, 18, 18));
        assert_eq!(dimension!(toggle_size), 22.0);
        assert_eq!(dimension!(text_label), 16.0);
        assert_eq!(image!(ic_left).path(), "images/ic_left.svg");
    }
}
