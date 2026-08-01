# Spotlight Wallpaper Source Tasks

## Task 1: Establish Wallpaper Source persistence and cache identity

**Description:** Introduce the Bing/Spotlight domain enum, add backward-compatible Selected Wallpaper Source and Daily Change Source settings, and resolve a different Feed cache file for each source while preserving Bing's existing `feed.json`.

**Acceptance criteria:**

- [x] Missing source fields deserialize as Bing, including legacy `daily_change: true` settings.
- [x] Settings round-trip both source fields without changing the default opt-in behavior.
- [x] Bing and Spotlight resolve distinct cache paths and round-trip independently.

**Verification:**

- [x] Tests pass: `cargo test settings::`
- [x] Tests pass: `cargo test feed::cache::`
- [x] Build succeeds: `cargo check --all-targets`

**Dependencies:** None

**Files likely touched:**

- `src/feed/source.rs`
- `src/feed/mod.rs`
- `src/settings.rs`
- `src/paths.rs`
- `src/feed/cache.rs`

**Estimated scope:** Medium (5 files)

## Task 2: Add Spotlight RSS parsing and provider-aware Feed refresh

**Description:** Add the approved XML dependency and a focused Spotlight parser, then route cache loading, remote URL selection, parsing, and fallback through a Wallpaper Source parameter.

**Acceptance criteria:**

- [x] Valid RSS produces source-ordered entries with decoded titles, publication dates, and only original HTTPS landscape URLs.
- [x] Malformed, portrait-only, resized-thumbnail, and empty inputs cannot produce a valid Feed.
- [x] A refresh saves and falls back only to the requested source's cache; Bing behavior remains intact.

**Verification:**

- [x] Tests pass: `cargo test feed::`
- [x] Build succeeds: `cargo check --all-targets`

**Dependencies:** Task 1

**Files likely touched:**

- `Cargo.toml`
- `Cargo.lock`
- `src/feed/spotlight.rs`
- `src/feed/mod.rs`
- `src/feed/refresh.rs`

**Estimated scope:** Medium (5 files)

## Checkpoint: Feed Foundation

- [x] `cargo test feed::` passes.
- [x] `cargo test settings::` passes.
- [x] `cargo check --all-targets` passes.
- [x] Review parser assumptions against a fresh Spotlight RSS sample if network access is available.

## Task 3: Make Daily Change uniquely source-aware

**Description:** Pass Wallpaper Source through the Daily Change workflow and scheduled runtime so enabling on another source transfers the only assignment, failures retain the previous persisted assignment, and unattended updates use the saved Daily Change Source.

**Acceptance criteria:**

- [x] Successful enable/transfer persists exactly the requested source and applies that source's Current Wallpaper.
- [x] A failed transfer leaves the previous Daily Change settings unchanged; disable clears the active flag without changing unrelated source selection.
- [x] Scheduled updates refresh the persisted Daily Change Source and preserve unsupported-desktop early exit behavior.

**Verification:**

- [x] Tests pass: `cargo test wallpaper::tests`
- [x] Build succeeds: `cargo check --all-targets`

**Dependencies:** Task 2

**Files likely touched:**

- `src/wallpaper/mod.rs`
- `src/feed/mod.rs`
- `src/feed/refresh.rs`

**Estimated scope:** Medium (3 files)

## Task 4: Integrate source switching into application state

**Description:** Load the selected source at startup, add source-selection messages/tasks, display its cache before refresh, persist changes safely, and reject late Feed results tagged for a previously selected source.

**Acceptance criteria:**

- [x] Startup and switching load/refresh only the Selected Wallpaper Source and reset pager/preview state appropriately.
- [x] Source switching never applies a wallpaper or mutates Daily Change Source; a source with no available Feed remains selected on error.
- [x] Late cross-source results and stale async settings snapshots cannot overwrite the current entries, source selection, Daily Change Source, or locale.

**Verification:**

- [x] Tests pass: `cargo test app::`
- [x] Tests pass: `cargo test settings::`
- [x] Build succeeds: `cargo check --all-targets`

**Dependencies:** Tasks 1-3

**Files likely touched:**

- `src/app/mod.rs`
- `src/app/preview.rs`
- `src/feed/mod.rs`
- `src/settings.rs`

**Estimated scope:** Medium (4 files)

## Checkpoint: Source Behavior

- [x] `cargo test wallpaper::tests` passes.
- [x] `cargo test app::` passes.
- [x] Switching and transfer behavior match the approved examples.
- [x] No operation starts Feed, Preview, or Wallpaper work on an unsupported desktop.

## Task 5: Add the centered Wallpaper Source controls

**Description:** Add resource-backed Bing/Spotlight radio controls in a full-width center layer while retaining Daily Change in an independent right-aligned layer, deriving its visible toggle state from the Selected and Daily Change Sources.

**Acceptance criteria:**

- [x] Bing and Spotlight are mutually exclusive and disabled while the app is busy; selecting the current source is a no-op.
- [x] The radio group remains at the exact window center regardless of the Daily Change control's width.
- [x] Daily Change appears on only for its assigned source and transfers when enabled while browsing the other source.

**Verification:**

- [x] Tests pass: `cargo test app::`
- [x] Resource accessors compile: `cargo check --all-targets`
- [x] Manual check: confirm source center alignment and right toggle alignment in the live 2048×1152 window; locale independence and unique Daily Change behavior are covered structurally and by tests.

**Dependencies:** Task 4

**Files likely touched:**

- `src/ui.rs`
- `assets/resources/values/strings.properties`
- `assets/resources/values-zh/strings.properties`
- `assets/resources/values/dimensions.properties`

**Estimated scope:** Medium (4 files)

## Task 6: Complete repository verification and documentation sync

**Description:** Run the complete required quality gate, inspect the final diff for architectural and resource compliance, and adjust the specification/domain glossary only if the implemented behavior needs clarification.

**Acceptance criteria:**

- [x] All approved functional requirements and boundaries are represented by implementation and focused tests.
- [x] No user-facing text, reusable dimensions, or source-specific business logic is misplaced in the UI.
- [x] The final worktree contains only intentional Spotlight feature and planning/documentation changes.

**Verification:**

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test`
- [x] `cargo check --all-targets`
- [x] Manual diff review: `git diff --check` and `git diff --stat`

**Dependencies:** Tasks 1-5

**Files likely touched:**

- `CONTEXT.md`
- `docs/specs/spotlight-wallpaper-source.md`
- `tasks/plan.md`
- `tasks/todo.md`

**Estimated scope:** Small (verification plus documentation only)

## Checkpoint: Complete

- [x] Every task acceptance criterion is checked.
- [x] Every repository-required command passes without warnings.
- [x] Manual GUI verification is completed; automatic source clicking was unavailable because no X11 input automation utility is installed.
- [x] Changes are ready for user review; no commit or push is performed without a new explicit request.
