# Limit version 1 desktop support to Cinnamon and GNOME

Version 1 will set wallpapers on Cinnamon and GNOME under both X11 and Wayland. Bingwall will detect the active desktop before performing network or cache work. On MATE, Xfce, KDE Plasma, and unrecognized environments, it will show only a localized unsupported-platform message—without fetching the feed, loading previews, or exposing wallpaper controls—rather than pretending that one fragile generic mechanism works everywhere.
