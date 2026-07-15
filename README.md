# Bingwall

<p align="center">
  <img src="app_icon.png" alt="Bingwall app icon" width="256">
</p>

Bingwall is a small Rust/Iced desktop application for browsing and applying images from the daily Bing wallpaper feed. Automatic changes are opt-in and run independently of the application window at 08:00 local time.

## Install

1. Download the latest amd64 Debian package (`bingwall_<version>_amd64.deb`) from the [GitHub Releases page](https://github.com/kapaseker/bingwall/releases).
2. Open a terminal in the directory containing the downloaded package and install it with APT:

   ```bash
   sudo apt install ./bingwall_<version>_amd64.deb
   ```

3. Launch **Bingwall** from the application menu.

Installing Bingwall does not enable automatic wallpaper changes. To opt in, open the app and turn on **Daily change**.

## Features

- Browse and preview wallpapers from the daily Bing wallpaper feed
- Set any displayed image as your desktop wallpaper
- Optionally change your wallpaper automatically every day at 08:00 local time
- Use the app in English or Simplified Chinese

## Supported systems

- Ubuntu 24.04 or newer with GNOME, on amd64
- Linux Mint 22 or newer with Cinnamon, on amd64
- X11 and Wayland sessions

On other desktops, Bingwall shows an unsupported-platform message.

## Development

Install a current Rust toolchain and the native build tools required by Iced, then clone the repository:

```bash
git clone https://github.com/kapaseker/bingwall.git
cd bingwall
```

Run the app from source:

```bash
cargo run
```

Create an optimized application build:

```bash
cargo build --release --locked
```

The standalone updater command used by the systemd service is:

```bash
cargo run -- update
```

It changes the wallpaper only when **Daily change** is enabled in saved settings.

### Build the Debian package

The package build requires `cargo`, `dpkg-deb`, and standard native build tools:

```bash
./scripts/build-deb.sh
```

The resulting package is written to `dist/`.
