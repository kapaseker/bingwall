# Implementation Plan: Spotlight Wallpaper Source

## Overview

Add Spotlight as a source-aware vertical feature path: a persisted source model selects an independent cached feed, the feed layer parses and refreshes the selected provider, the app ignores stale cross-source results, Daily Change owns exactly one source, and the UI exposes a strictly centered Bing/Spotlight radio group without moving the existing right-aligned toggle.

## Architecture Decisions

- Model `WallpaperSource` as a small serialized domain enum shared by settings, feed refresh, application state, and scheduled updates. Missing persisted fields default to Bing.
- Keep `feed.json` as the Bing cache and introduce a distinct Spotlight cache path so upgrades retain existing local data.
- Give feed parsing a provider boundary: retain the existing Bing Markdown parser and add a focused Spotlight RSS parser using the approved direct `quick-xml` dependency.
- Tag every asynchronous feed result with its source. The app accepts the result only when it still matches the Selected Wallpaper Source.
- Keep `selected_source` and `daily_change_source` independent. UI state derives Daily Change visibility from both fields; enabling on another source transfers the sole assignment only after its workflow succeeds.
- Preserve UI-owned settings fields when asynchronous wallpaper workflows return a settings snapshot, preventing source or locale choices made later from being overwritten.
- Use an Iced overlay/stack layout for the top controls so the source group is centered against the window, not against the remaining width beside Daily Change.

## Dependency Graph

```text
WallpaperSource + persisted defaults + source cache paths
    |
    +-- Spotlight RSS parser + provider-aware feed refresh
    |       |
    |       +-- source-aware scheduled Daily Change
    |       |
    |       +-- source-switch application state and stale-result guard
    |               |
    |               +-- centered source selector and derived Daily Change toggle
    |
    +-- backward-compatible settings tests
```

## Task List

### Phase 1: Source Foundation

- [x] Task 1: Add the source domain model, backward-compatible settings fields, and independent cache paths.
- [x] Task 2: Parse Spotlight RSS and route Feed loading/refreshing by source.

### Checkpoint: Feed Foundation

- [x] `cargo test feed::` and `cargo test settings::` pass.
- [x] Bing cache compatibility and Spotlight cache isolation are proven by tests.
- [x] Invalid or portrait-only Spotlight items cannot become a Current Wallpaper.

### Phase 2: Behavioral Integration

- [x] Task 3: Make Daily Change assignment and scheduled updates source-aware.
- [x] Task 4: Integrate persisted source switching into application state with stale-result protection.

### Checkpoint: Source Behavior

- [x] `cargo test wallpaper::tests` and `cargo test app::` pass.
- [x] Switching sources never applies a wallpaper or changes Daily Change ownership.
- [x] A failed transfer preserves the previous Daily Change settings.
- [x] Scheduled updates refresh the persisted Daily Change Source.

### Phase 3: User Interface and Completion

- [x] Task 5: Add localized source controls with strict-center layout and source-aware Daily Change state.
- [x] Task 6: Run end-to-end verification and update feature documentation if implementation details require clarification.

### Checkpoint: Complete

- [x] Bing remains the default for new and legacy settings.
- [x] Both sources browse from independent local-first feeds.
- [x] Exactly one source can own Daily Change.
- [x] `cargo fmt --check` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] `cargo test` passes.
- [x] `cargo check --all-targets` passes.
- [x] The GUI behavior is manually checked when a graphical session is available.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| WordPress RSS markup changes | High | Parse only RSS item boundaries and validated HTTPS original links; keep fixture tests for ordering, entities, malformed items, portraits, and thumbnails. |
| A slow Bing request completes after switching to Spotlight | High | Carry `WallpaperSource` in task results and ignore results that no longer match application state. |
| Async settings snapshots overwrite a newer source selection | High | Merge returned settings while preserving the current UI-owned source and locale fields; serialize source persistence. |
| Daily Change transfer partially completes | High | Do not mutate in-memory ownership until the workflow succeeds; persist the new owner only after original acquisition, apply, and timer enable succeed. |
| Top control widths move the radio group | Medium | Center the source group in a full-width overlay layer independent of the right-aligned Daily Change layer. |
| Spotlight is unavailable and has no cache | Medium | Keep Spotlight selected, retain its empty/error view, and expose the existing retry action without falling back to Bing. |

## Open Questions

None. The approved specification resolves source scope, cache behavior, transfer semantics, failure behavior, and first-version RSS limits.
