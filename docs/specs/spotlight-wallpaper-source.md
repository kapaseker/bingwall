# Spec: Spotlight Wallpaper Source

## Objective

Add Spotlight as a second Wallpaper Source alongside Bing. Users can browse either source from a radio selector centered in the top overlay, while Daily Change remains assigned to at most one source.

Spotlight data comes from the site's RSS feed at <https://windows10spotlight.com/feed>. Only the recent entries exposed by RSS are in scope; historical page scraping is not.

## Functional Requirements

### Wallpaper Sources

- Define `WallpaperSource` with `Bing` and `Spotlight`; its default is `Bing`.
- Persist the Selected Wallpaper Source. Existing and missing settings default to Bing.
- Switching source changes the browsed Wallpaper Feed only. It never applies a wallpaper, enables or disables the timer, or changes the Daily Change Source.
- Each source has its own cached Wallpaper Feed. Keep the existing `feed.json` as Bing's cache and add a separate Spotlight cache.
- On startup and source switch, show that source's cache first when available, then refresh it from the network.
- Async Feed results carry their Wallpaper Source identity; stale results from a previously selected source are ignored.
- If the selected source has no cache and refresh fails, keep that source selected and show the existing error/retry interface. Do not fall back to the other source.

### Spotlight Feed

- Parse the WordPress RSS with `quick-xml 0.39.4` as a direct dependency.
- Preserve RSS item order so the first valid item is the Current Wallpaper.
- Map each valid item to its publication date, decoded title, and original 1920×1080 HTTPS image URL.
- Use only the first landscape original linked in `content:encoded`; never select the 1080×1920 portrait image or a resized thumbnail.
- Ignore malformed items and reject a Feed with no valid entries.
- Treat RSS and embedded HTML as untrusted data: do not render or execute its markup, scripts, or advertisements.

### Daily Change

- Persist `daily_change`, `daily_change_source`, and `selected_source` as separate settings. Missing source fields default to Bing, preserving existing enabled Bing installations.
- Daily Change is visibly enabled only when `daily_change` is true and the Selected Wallpaper Source equals the Daily Change Source.
- Enabling Daily Change on a different source transfers the single assignment to that source, immediately applies its Current Wallpaper, keeps/enables the timer, and persists the new source.
- If transfer fails, retain the previous Daily Change settings.
- Disabling Daily Change is available when browsing its assigned source and disables the timer as today.
- Scheduled Wallpaper Update refreshes and applies the Current Wallpaper from the persisted Daily Change Source, independently of the Selected Wallpaper Source.

### User Interface

- Place `Bing` and `Spotlight` radio buttons as one group at the strict horizontal center of the top overlay.
- Keep Daily Change right-aligned at the same vertical center; its width must not shift the radio group away from the window center.
- Disable source changes while the app is busy. Selecting the already selected source is a no-op.
- Add all labels and reusable dimensions through generated resources. Brand names may fall back to the English defaults in Simplified Chinese.

## Tech Stack

- Rust 2024, single crate
- Iced 0.14 radio widgets
- reqwest 0.13 for HTTP
- quick-xml 0.39.4 for RSS parsing
- serde/serde_json for backward-compatible settings

## Commands

- Focused tests: `cargo test feed::`, `cargo test settings::`, `cargo test app::`, `cargo test wallpaper::tests`
- Format: `cargo fmt`
- Format check: `cargo fmt --check`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Full tests: `cargo test`
- Compile: `cargo check --all-targets`

## Project Structure

- `src/feed/`: Wallpaper Source model, Bing/Spotlight parsing, network refresh, and per-source Feed cache
- `src/app/`: Selected source state, source-switch tasks, stale-result protection, and UI messages
- `src/wallpaper/`: manual application, unique Daily Change Source assignment, and scheduled updates
- `src/settings.rs`: backward-compatible persisted source selections
- `src/ui.rs`: centered source selector and right-aligned Daily Change control
- `assets/resources/`: localized labels and fixed dimensions
- Tests stay beside each implementation in `#[cfg(test)] mod tests`

## Code Style

Use explicit domain types and resource-backed UI values:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WallpaperSource {
    #[default]
    Bing,
    Spotlight,
}
```

Every new or changed Rust function receives a meaningful `///` documentation comment. UI remains declarative and emits messages; source selection, persistence, and network work remain in app/domain modules.

## Testing Strategy

- Parser tests cover valid RSS, XML entity decoding, portrait exclusion, malformed items, and an empty Feed.
- Cache tests prove Bing and Spotlight paths are distinct and round-trip independently.
- Settings tests prove default Bing migration, round-trip persistence, and legacy enabled Daily Change binding to Bing.
- App tests prove switching resets selection, uses source-specific cached entries, ignores stale Feed results, preserves Daily Change assignment, and retains a failed selected source.
- Wallpaper workflow tests prove source transfer is unique, transfer failure retains the old assignment, disabling works only for the assigned source flow, and scheduled updates use the Daily Change Source.
- Resource accessors must compile and existing generated-resource tests remain green.

## Boundaries

- Always: preserve local-first behavior, original-only wallpaper application, background image work, unsupported-desktop early stop, and opt-in Daily Change.
- Ask first: any additional dependency, historical Spotlight crawling, persistent format beyond the approved additive source fields, or UI behavior beyond the source selector.
- Never: execute RSS HTML, use portrait images as desktop wallpapers, enable two Daily Change Sources, apply a wallpaper merely because the source changed, install a generated Debian package, commit, or push without an explicit request.

## Success Criteria

- New and legacy installations start with Bing selected.
- Users can switch between Bing and the recent Spotlight RSS entries from the centered radio group.
- Source selection survives restart and each source retains an independent Feed cache.
- Switching source does not change the Applied Wallpaper or Daily Change assignment.
- Exactly one Daily Change Source can be assigned; enabling another source transfers it.
- Scheduled updates use the persisted Daily Change Source without starting Feed/Preview work on unsupported desktops.
- All focused and repository-wide verification commands pass without warnings.

## Sources

- Spotlight RSS: <https://windows10spotlight.com/feed>
- Spotlight site description and image provenance: <https://windows10spotlight.com/about>
- Iced 0.14 radio helper: <https://docs.rs/iced/0.14.0/iced/widget/fn.radio.html>
