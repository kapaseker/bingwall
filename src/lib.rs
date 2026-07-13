pub mod app;
pub mod cache;
pub mod feed;
pub mod locale;
pub mod paths;
pub mod platform;
pub mod service;
pub mod settings;
pub mod systemd;
mod ui;

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
