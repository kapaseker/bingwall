pub mod app;
pub mod cache;
pub mod feed;
pub mod paths;
pub mod platform;
pub mod resources;
pub mod service;
pub mod settings;
pub mod systemd;
mod theme;
mod ui;

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
