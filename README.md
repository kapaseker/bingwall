# Bingwall

Bingwall is a small Rust/Iced desktop application for browsing and applying images from the daily Bing wallpaper feed. Automatic changes are opt-in and run independently of the application window at 08:00 local time.

## Supported systems

- Ubuntu 24.04 or newer with GNOME, on amd64
- Linux Mint 22 or newer with Cinnamon, on amd64
- X11 and Wayland sessions

On other desktops, Bingwall shows only an unsupported-platform message and performs no network or cache work.

## Features

- English or Simplified Chinese UI selected from the system locale
- Animated horizontal pager with buttons, keyboard arrows, wheel/touchpad scrolling, and touch swipes
- Ten-entry metadata batches with image downloads limited to the selected entry and its neighbors
- Manual **Set as wallpaper** action independent of automation
- **Daily change** toggle that is off on first launch
- Persistent systemd user timer at 08:00; a missed run executes after the next login or wake
- Cached feed shown immediately at startup while a refresh runs in the background
- Downloaded wallpapers are reused from local storage and kept permanently
- GNOME and Cinnamon wallpaper application in zoom/fill mode

## Run from source

Install a current Rust toolchain, then run:

```bash
cargo run
```

The standalone updater command used by the systemd service is:

```bash
cargo run -- update
```

It changes the wallpaper only when **Daily change** is enabled in saved settings.

## Build the Debian package

The build requires `cargo`, `dpkg-deb`, and standard native build tools:

```bash
./scripts/build-deb.sh
```

The resulting package is written to `dist/`. Installing the package does not enable automatic wallpaper changes; the user must open Bingwall and turn on **Daily change**.

## User data

- Settings: `${XDG_CONFIG_HOME:-~/.config}/bingwall/settings.json`
- Cached feed and images: `${XDG_CACHE_HOME:-~/.cache}/bingwall/`
- Logs for scheduled runs: `journalctl --user -u bingwall.service`

Downloaded wallpaper files are not evicted automatically. They remain in the cache until the cache directory is removed manually; an in-app cache clearing control is planned for a later release.

Wallpaper descriptions and attribution are displayed exactly as provided by the external feed. Bingwall does not collect analytics or telemetry.
