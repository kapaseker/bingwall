# Bingwall Agent Guide

## Project

- Rust 2024 single-crate Linux desktop application using Iced 0.14.
- Supports GNOME and Cinnamon. The GUI browses and applies Bing wallpapers; `bingwall update` performs the opt-in scheduled update.
- Use the domain terms defined in `CONTEXT.md`. Read the relevant source and nearby tests before editing, and follow an existing pattern when one exists.

## Project map

- `src/app.rs`: application state, Iced message/event mapping, subscriptions, and window setup.
- `src/pager.rs`, `src/preview.rs`, `src/wallpaper.rs`: pager behavior, preview residency, and wallpaper workflows.
- `src/image_acquisition.rs`, `src/cache.rs`: image acquisition/recovery policy and low-level persistent cache operations.
- `src/ui.rs`, `src/ui/`: view construction and custom Iced widgets; keep business and I/O logic out of views.
- `src/service.rs`: Wallpaper Feed network refresh and cached-feed fallback.
- `src/feed.rs`, `src/settings.rs`, `src/paths.rs`: feed parsing, settings persistence, and filesystem paths.
- `src/platform.rs`, `src/systemd.rs`: desktop integration and user-service management.
- `src/resources/`, `src/theme/`: typed resources and reusable Iced styles.
- `assets/resources/`: source UI resources consumed by `build.rs`.
- `packaging/`, `scripts/build-deb.sh`: Debian desktop, systemd, icon, and package files.

## Architecture and file organization

- Preserve the flow `UI -> app state/messages -> service/domain modules -> filesystem, network, or platform adapters`.
- Keep `ui` declarative: render from `State` and emit `Message`; state transitions belong in `app`, while reusable I/O or business workflows belong in `service` or a focused module.
- Give each module one clear responsibility. Add a sibling module when a new concern has its own state, errors, tests, or platform boundary; do not grow an unrelated existing file for convenience.
- Prefer private items. Expose an item from `lib.rs` only when another module or the binary genuinely needs it.
- Keep tests beside the implementation in `#[cfg(test)] mod tests`. Extract a separate test helper only when multiple modules share it.
- Add a meaningful `///` documentation comment to every new or changed Rust function, including private helpers, trait methods, and test functions.

## Resources

- Do not hard-code user-facing text, UI colors, fixed dimensions, or bundled image paths in Rust UI code.
- Add default text to `assets/resources/values/strings.properties` and Simplified Chinese overrides to `values-zh/strings.properties`; access text with `text!(key)` or a generated `TextResource`. Keep placeholders identical across locales.
- Add semantic colors to `values/colors.properties` and use `color!(key)`. Name colors by purpose, not by a raw color name.
- Add reusable fixed logical-pixel values, including text and icon sizes, to `values/dimensions.properties` and use `dimension!(key)`.
- Put embedded UI images in `assets/resources/images/` and use `image!(filename_stem)`. Use lowercase keys matching `[a-z][a-z0-9_]*`, with no repeated or trailing underscore.
- `build.rs` validates and generates resource accessors. Never edit generated files under `target/`.
- Keep distribution/window icons in their established locations: `app_icon.png` and `packaging/icons/`. When changing the application icon, update and verify every packaged size and the window icon path.

## Code conventions

- Run `cargo fmt`; keep code warning-free under Clippy and use structured errors rather than panics for recoverable runtime failures.
- Preserve local-first behavior: cached previews are for display, original images are used when applying wallpaper, and blocking filesystem/image work must not run on the UI thread.
- On unsupported desktops, stop before starting feed, cache, preview, or wallpaper work.
- Daily Change remains opt-in and disabled by default. Do not broaden the 08:00 systemd behavior without an explicit requirement.
- Do not add dependencies, change persistent formats, or alter packaging/install behavior unless the task requires it. Never install the generated Debian package without explicit user approval.

## Tests and verification

- Every added or modified Rust behavior must include or update focused unit tests in the same change. Cover success, failure, and boundary cases relevant to the change. Documentation-only or packaging-only edits do not require artificial unit tests.
- Resource changes must be covered by the generated-resource tests when introducing a new resource category or behavior; ordinary entries must at least compile through generated accessors.
- During development, run the narrowest relevant test first, for example `cargo test feed::tests`.
- After any code change, run all of the following from the repository root:

  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  cargo check --all-targets
  ```

- For dependency or release builds, also run `cargo build --release --locked`. For packaging changes, run `./scripts/build-deb.sh` and inspect the package contents; do not install it automatically.
- Do not report completion while a relevant check is failing. Report the exact failing command and error if an environmental blocker prevents verification.
