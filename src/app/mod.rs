//! Owns application state, messages, tasks, subscriptions, and window setup.

use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use iced::{Point, Size, Subscription, Task, event, keyboard, mouse, touch, widget::image, window};

pub(crate) mod pager;
mod preview;

use pager::Pager;
use preview::{PreviewCommand, PreviewEvent, PreviewFailure, PreviewResidency};

use crate::{
    feed::{self, FeedOrigin, WallpaperEntry, WallpaperSource},
    paths::AppPaths,
    platform::Desktop,
    resources::{Locale, TextResource, current_locale, generated_text as texts, set_locale},
    settings::Settings,
    ui, wallpaper,
};

const GPU_PRELOAD_LIMIT: usize = 4;
pub(crate) const BASE_WIDTH: f32 = 1280.0;
pub(crate) const BASE_HEIGHT: f32 = 720.0;
const ASPECT_RATIO: f32 = BASE_WIDTH / BASE_HEIGHT;
const SIZE_EPSILON: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatusText {
    Localized(TextResource),
    Raw(String),
}

impl StatusText {
    /// Creates a status that resolves a text resource using the current locale.
    fn localized(resource: TextResource) -> Self {
        Self::Localized(resource)
    }

    /// Resolves the status for display without translating external error text.
    pub(crate) fn resolve(&self) -> Cow<'_, str> {
        match self {
            Self::Localized(resource) => resource.resolve(&[]),
            Self::Raw(text) => Cow::Borrowed(text),
        }
    }
}

impl From<String> for StatusText {
    /// Preserves an externally produced status string without translating it.
    fn from(value: String) -> Self {
        Self::Raw(value)
    }
}

#[derive(Debug)]
pub(crate) struct State {
    pub initializing: bool,
    pub desktop: Option<Desktop>,
    pub paths: Option<AppPaths>,
    pub settings: Settings,
    pub entries: Vec<WallpaperEntry>,
    pub(crate) pager: Pager,
    client: Option<reqwest::Client>,
    previews: PreviewResidency,
    pub status: StatusText,
    pub busy: bool,
    settings_save_in_flight: bool,
    settings_save_pending: bool,
    window_size: Size,
    resize_target: Option<Size>,
    touch_finger: Option<touch::Finger>,
}

impl State {
    /// Returns the allocated GPU image handle for an entry when it is ready.
    pub(crate) fn preview_handle(&self, index: usize) -> Option<image::Handle> {
        let image_url = &self.entries.get(index)?.image_url;
        self.previews.handle_for(image_url)
    }

    /// Reports whether the selected wallpaper has an allocated preview.
    pub(crate) fn selected_preview_is_ready(&self) -> bool {
        self.preview_handle(self.pager.selected()).is_some()
    }

    /// Reports whether Daily Change belongs to the source currently being browsed.
    pub(crate) fn daily_change_enabled_for_selected_source(&self) -> bool {
        self.settings.daily_change
            && self.settings.selected_source == self.settings.daily_change_source
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Startup {
    Unsupported,
    Supported {
        desktop: Desktop,
        paths: AppPaths,
        client: reqwest::Client,
        settings: Settings,
        cached_entries: Vec<WallpaperEntry>,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Initialized(Result<Startup, String>),
    Refresh,
    SelectSource(WallpaperSource),
    SourceCacheLoaded(WallpaperSource, Vec<WallpaperEntry>),
    FeedLoaded(
        WallpaperSource,
        Result<(Vec<WallpaperEntry>, FeedOrigin), String>,
    ),
    Preview(PreviewEvent),
    Previous,
    Next,
    SetWallpaper,
    Applied(Result<Settings, String>),
    ToggleDaily(bool),
    ToggleFinished(bool, Result<Settings, String>),
    #[allow(dead_code)]
    SetLocale(Locale),
    SettingsSaved(Result<(), String>),
    RuntimeEvent(iced::Event),
    CapturedTouchEvent(iced::Event),
    WindowResized(window::Id, Size),
    PagerPointerMoved(Point),
    PagerPressed(f32),
    PagerReleased,
    AnimationTick(Instant),
}

/// Configures and launches the Bingwall graphical application.
pub fn run() -> iced::Result {
    iced::application(boot, update, ui::view)
        .title("Bingwall")
        .subscription(subscription)
        .window(window_settings())
        .centered()
        .antialiasing(true)
        .run()
}

/// Configures the window identity used by desktop launchers and task switchers.
fn window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(BASE_WIDTH, BASE_HEIGHT),
        min_size: Some(Size::new(BASE_WIDTH, BASE_HEIGHT)),
        icon: window::icon::from_file_data(
            include_bytes!("../../packaging/icons/bingwall-256.png"),
            None,
        )
        .ok(),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "bingwall".to_owned(),
            ..window::settings::PlatformSpecific::default()
        },
        ..window::Settings::default()
    }
}

/// Creates the initial application state and starts background initialization.
fn boot() -> (State, Task<Message>) {
    let locale = Locale::detect();
    set_locale(locale);
    let state = State {
        initializing: true,
        desktop: None,
        paths: None,
        settings: Settings::default(),
        entries: Vec::new(),
        pager: Pager::new(0),
        client: None,
        previews: PreviewResidency::new(),
        status: StatusText::localized(texts::loading_feed),
        busy: true,
        settings_save_in_flight: false,
        settings_save_pending: false,
        window_size: Size::new(BASE_WIDTH, BASE_HEIGHT),
        resize_target: None,
        touch_finger: None,
    };
    let task = Task::perform(
        async {
            tokio::task::spawn_blocking(load_startup)
                .await
                .map_err(|error| error.to_string())?
        },
        Message::Initialized,
    );
    (state, task)
}

/// Detects platform support and loads paths, settings, and any cached feed.
fn load_startup() -> Result<Startup, String> {
    let Ok(desktop) = Desktop::detect() else {
        return Ok(Startup::Unsupported);
    };
    let paths = AppPaths::discover().map_err(|error| error.to_string())?;
    let settings = Settings::load(&paths.settings_file()).map_err(|error| error.to_string())?;
    let cached_entries = feed::load_cached(&paths, settings.selected_source)
        .ok()
        .filter(|entries| !entries.is_empty())
        .unwrap_or_default();
    Ok(Startup::Supported {
        desktop,
        paths,
        client: reqwest::Client::new(),
        settings,
        cached_entries,
    })
}

/// Applies an application message to state and returns any resulting asynchronous work.
fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Initialized(result) => match result {
            Ok(Startup::Unsupported) => {
                state.initializing = false;
                state.busy = false;
                state.status = StatusText::localized(texts::unsupported);
                Task::none()
            }
            Ok(Startup::Supported {
                desktop,
                paths,
                client,
                settings,
                cached_entries,
            }) => {
                let has_cached_feed = !cached_entries.is_empty();
                set_locale(settings.locale.unwrap_or_else(current_locale));
                state.initializing = false;
                state.desktop = Some(desktop);
                state.paths = Some(paths);
                state.client = Some(client);
                state.settings = settings;
                state.entries = cached_entries;
                state.pager.reset(state.entries.len());
                state.busy = !has_cached_feed;
                state.status = if has_cached_feed {
                    StatusText::localized(texts::cached_feed_refreshing)
                } else {
                    state
                        .settings
                        .last_update_status
                        .clone()
                        .map(StatusText::from)
                        .unwrap_or_else(|| StatusText::localized(texts::loading_feed))
                };
                Task::batch([
                    schedule_previews(state),
                    refresh_task(state, !has_cached_feed),
                ])
            }
            Err(error) => {
                state.initializing = false;
                state.busy = false;
                state.status = error.into();
                Task::none()
            }
        },
        Message::Refresh => {
            state.previews.retry_acquisitions();
            refresh_task(state, true)
        }
        Message::SelectSource(source) => select_source(state, source),
        Message::SourceCacheLoaded(source, cached_entries) => {
            if source != state.settings.selected_source {
                return Task::none();
            }
            if cached_entries.is_empty() {
                return refresh_task(state, true);
            }
            state.entries = cached_entries;
            state.pager.reset(state.entries.len());
            state.busy = false;
            state.status = StatusText::localized(texts::cached_feed_refreshing);
            Task::batch([schedule_previews(state), refresh_task(state, false)])
        }
        Message::FeedLoaded(source, result) => {
            if source != state.settings.selected_source {
                return Task::none();
            }
            state.busy = false;
            match result {
                Ok((entries, origin)) => {
                    let feed_changed = state.entries != entries;
                    if feed_changed {
                        state.entries = entries;
                        state.pager.reset(state.entries.len());
                    }
                    state.status = StatusText::localized(match origin {
                        FeedOrigin::Network => texts::feed_refreshed,
                        FeedOrigin::Cache => texts::cached_feed,
                    });
                    schedule_previews(state)
                }
                Err(error) => {
                    state.status = error.into();
                    Task::none()
                }
            }
        }
        Message::Preview(event) => handle_preview_event(state, event),
        Message::Previous => navigate(state, -1),
        Message::Next => navigate(state, 1),
        Message::SetWallpaper => apply_selected_task(state),
        Message::Applied(result) => {
            state.busy = false;
            match result {
                Ok(settings) => {
                    let choices_changed = merge_settings_preserving_ui_choices(state, settings);
                    state.status = StatusText::localized(texts::applied);
                    if choices_changed {
                        return persist_settings_task(state);
                    }
                }
                Err(error) => state.status = error.into(),
            }
            Task::none()
        }
        Message::ToggleDaily(enabled) => toggle_daily_task(state, enabled),
        Message::ToggleFinished(enabled, result) => {
            state.busy = false;
            match result {
                Ok(settings) => {
                    let choices_changed = merge_settings_preserving_ui_choices(state, settings);
                    state.status = StatusText::localized(if enabled {
                        texts::enabled
                    } else {
                        texts::disabled
                    });
                    if choices_changed {
                        return persist_settings_task(state);
                    }
                }
                Err(error) => state.status = error.into(),
            }
            Task::none()
        }
        Message::SetLocale(locale) => {
            set_locale(locale);
            state.settings.locale = Some(locale);
            persist_settings_task(state)
        }
        Message::SettingsSaved(result) => {
            state.settings_save_in_flight = false;
            if state.settings_save_pending {
                state.settings_save_pending = false;
                return persist_settings_task(state);
            }
            if let Err(error) = result {
                state.status = error.into();
            }
            Task::none()
        }
        Message::RuntimeEvent(event) => handle_runtime_event(state, event),
        Message::CapturedTouchEvent(event) => handle_touch_event(state, event),
        Message::WindowResized(id, size) => handle_window_resize(state, id, size),
        Message::PagerPointerMoved(position) => {
            state.pager.pointer_moved(position.x, Instant::now());
            Task::none()
        }
        Message::PagerPressed(width) => {
            state.pager.press(width, Instant::now());
            Task::none()
        }
        Message::PagerReleased => finish_pager_drag(state, Instant::now()),
        Message::AnimationTick(now) => {
            state.pager.tick(now);
            Task::none()
        }
    }
}

/// Merges workflow settings without losing newer UI-owned source or locale choices.
fn merge_settings_preserving_ui_choices(state: &mut State, mut settings: Settings) -> bool {
    let locale = state.settings.locale;
    let selected_source = state.settings.selected_source;
    let choices_changed = settings.locale != locale || settings.selected_source != selected_source;
    settings.locale = locale;
    settings.selected_source = selected_source;
    state.settings = settings;
    choices_changed
}

/// Serializes persistence of UI-owned settings so the latest selection wins.
fn persist_settings_task(state: &mut State) -> Task<Message> {
    if state.settings_save_in_flight {
        state.settings_save_pending = true;
        return Task::none();
    }
    let Some(paths) = state.paths.clone() else {
        return Task::none();
    };
    let settings = state.settings.clone();
    state.settings_save_in_flight = true;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || settings.save(&paths.settings_file()))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        },
        Message::SettingsSaved,
    )
}

/// Switches browsing to another source without changing Applied Wallpaper or Daily Change.
fn select_source(state: &mut State, source: WallpaperSource) -> Task<Message> {
    if state.busy || source == state.settings.selected_source {
        return Task::none();
    }
    let Some(paths) = state.paths.clone() else {
        return Task::none();
    };
    state.settings.selected_source = source;
    state.entries.clear();
    state.pager.reset(0);
    state.previews.source_changed();
    state.busy = true;
    state.status = StatusText::localized(texts::loading_feed);
    Task::batch([
        persist_settings_task(state),
        load_source_cache_task(paths, source),
    ])
}

/// Loads a selected source's cache away from the UI thread.
fn load_source_cache_task(paths: AppPaths, source: WallpaperSource) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                feed::load_cached(&paths, source)
                    .ok()
                    .filter(|entries| !entries.is_empty())
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        },
        move |entries| Message::SourceCacheLoaded(source, entries),
    )
}

/// Starts a feed refresh and optionally places the interface in a blocking state.
fn refresh_task(state: &mut State, blocking: bool) -> Task<Message> {
    let (Some(paths), Some(client)) = (state.paths.clone(), state.client.clone()) else {
        return Task::none();
    };
    if blocking {
        state.busy = true;
        state.status = StatusText::localized(texts::loading_feed);
    }
    let source = state.settings.selected_source;
    Task::perform(
        async move {
            feed::refresh_feed(&client, &paths, source)
                .await
                .map_err(|error| error.to_string())
        },
        move |result| Message::FeedLoaded(source, result),
    )
}

/// Prioritizes preview generation and GPU allocation around the current selection.
fn schedule_previews(state: &mut State) -> Task<Message> {
    let (Some(paths), Some(client)) = (state.paths.clone(), state.client.clone()) else {
        return Task::none();
    };
    if state.entries.is_empty() {
        return Task::none();
    }

    let desired_entries = desired_preview_entries(state);
    let commands = state.previews.reconcile(&desired_entries);
    execute_preview_commands(commands, paths, client)
}

/// Routes a completed preview event through the residency module.
fn handle_preview_event(state: &mut State, event: PreviewEvent) -> Task<Message> {
    let (Some(paths), Some(client)) = (state.paths.clone(), state.client.clone()) else {
        return Task::none();
    };
    let desired_entries = desired_preview_entries(state);
    let preview_update = state.previews.handle(event, &desired_entries);
    if let Some(failure) = preview_update.selected_failure {
        let message = match failure {
            PreviewFailure::Acquisition(error) | PreviewFailure::Invalidation(error) => error,
            PreviewFailure::Allocation(error) => error.to_string(),
        };
        state.status = message.into();
    }
    execute_preview_commands(preview_update.commands, paths, client)
}

/// Converts preview commands into Iced tasks without owning preview policy.
fn execute_preview_commands(
    commands: Vec<PreviewCommand>,
    paths: AppPaths,
    client: reqwest::Client,
) -> Task<Message> {
    Task::batch(commands.into_iter().map(|command| match command {
        PreviewCommand::Acquire(entry) => {
            let image_url = entry.image_url.clone();
            let paths = paths.clone();
            let client = client.clone();
            Task::perform(
                async move {
                    wallpaper::image::preview(&client, &paths, &entry)
                        .await
                        .map_err(|error| error.to_string())
                },
                move |result| {
                    Message::Preview(PreviewEvent::Acquired {
                        image_url: image_url.clone(),
                        result,
                    })
                },
            )
        }
        PreviewCommand::Allocate { image_url, path } => {
            image::allocate(image::Handle::from_path(path)).map(move |result| {
                Message::Preview(match result {
                    Ok(allocation) => PreviewEvent::Allocated {
                        image_url: image_url.clone(),
                        allocation,
                    },
                    Err(error) => PreviewEvent::AllocationFailed {
                        image_url: image_url.clone(),
                        error,
                    },
                })
            })
        }
        PreviewCommand::RemoveInvalid { image_url, path } => Task::perform(
            async move {
                let Some(path) = path else {
                    return Ok(());
                };
                tokio::task::spawn_blocking(move || std::fs::remove_file(path))
                    .await
                    .map_err(|error| error.to_string())?
                    .or_else(|error| {
                        (error.kind() == std::io::ErrorKind::NotFound)
                            .then_some(())
                            .ok_or(error)
                    })
                    .map_err(|error| error.to_string())
            },
            move |result| {
                Message::Preview(PreviewEvent::Invalidated {
                    image_url: image_url.clone(),
                    result,
                })
            },
        ),
    }))
}

/// Returns unique entries to preload in navigation-priority order.
fn desired_preview_entries(state: &State) -> Vec<WallpaperEntry> {
    let mut indices = Vec::with_capacity(GPU_PRELOAD_LIMIT);
    let selected = state.pager.selected();
    for index in [
        Some(selected),
        selected.checked_add(1),
        selected.checked_sub(1),
        selected.checked_add(2),
    ]
    .into_iter()
    .flatten()
    {
        if index < state.entries.len() && !indices.contains(&index) {
            indices.push(index);
        }
    }
    indices
        .into_iter()
        .map(|index| state.entries[index].clone())
        .collect()
}

/// Moves the selection within bounds, extends the visible page, and starts a transition.
fn navigate(state: &mut State, direction: isize) -> Task<Message> {
    let changed = state.pager.navigate(direction, Instant::now());
    finish_pager_selection_change(state, changed)
}

/// Finishes a pointer drag and schedules previews when selection changes.
fn finish_pager_drag(state: &mut State, now: Instant) -> Task<Message> {
    let changed = state.pager.release(now);
    finish_pager_selection_change(state, changed)
}

/// Retries the newly selected preview and reconciles nearby preview residency.
fn finish_pager_selection_change(state: &mut State, changed: bool) -> Task<Message> {
    if !changed {
        return Task::none();
    }
    if let Some(entry) = state.entries.get(state.pager.selected()) {
        state.previews.retry(&entry.image_url);
    }
    schedule_previews(state)
}

/// Starts the work needed to download and apply the selected wallpaper.
fn apply_selected_task(state: &mut State) -> Task<Message> {
    let (Some(desktop), Some(paths), Some(entry)) = (
        state.desktop,
        state.paths.clone(),
        state.entries.get(state.pager.selected()).cloned(),
    ) else {
        return Task::none();
    };
    let Some(client) = state.client.clone() else {
        return Task::none();
    };
    state.busy = true;
    state.status = StatusText::localized(texts::working);
    Task::perform(
        async move {
            wallpaper::apply_selected(desktop, paths, client, entry)
                .await
                .map_err(|error| error.to_string())
        },
        Message::Applied,
    )
}

/// Starts the work needed to enable or disable automatic daily changes.
fn toggle_daily_task(state: &mut State, enabled: bool) -> Task<Message> {
    let (Some(desktop), Some(paths)) = (state.desktop, state.paths.clone()) else {
        return Task::none();
    };
    let Some(client) = state.client.clone() else {
        return Task::none();
    };
    let current = state.entries.first().cloned();
    let source = state.settings.selected_source;
    if enabled && current.is_none() {
        state.status = StatusText::localized(texts::loading_feed);
        return Task::none();
    }
    state.busy = true;
    state.status = StatusText::localized(texts::working);
    Task::perform(
        async move {
            wallpaper::set_daily_change(enabled, source, desktop, paths, client, current)
                .await
                .map_err(|error| error.to_string())
        },
        move |result| Message::ToggleFinished(enabled, result),
    )
}

/// Maps keyboard, wheel, and touch gestures to wallpaper navigation.
fn handle_runtime_event(state: &mut State, event: iced::Event) -> Task<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => navigate(state, -1),
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => navigate(state, 1),
            _ => Task::none(),
        },
        iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let now = Instant::now();
            let (x, y) = match delta {
                mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => (x, y),
            };
            let movement = if x.abs() > y.abs() { x } else { y };
            if movement.abs() < f32::EPSILON {
                Task::none()
            } else {
                let changed = state.pager.wheel(if movement < 0.0 { 1 } else { -1 }, now);
                finish_pager_selection_change(state, changed)
            }
        }
        iced::Event::Touch(_) => handle_touch_event(state, event),
        _ => Task::none(),
    }
}

/// Maps touch positions to the same live drag and snap behavior as the mouse.
fn handle_touch_event(state: &mut State, event: iced::Event) -> Task<Message> {
    match event {
        iced::Event::Touch(touch::Event::FingerPressed { id, position }) => {
            let now = Instant::now();
            state.touch_finger = Some(id);
            state.pager.pointer_moved(position.x, now);
            state.pager.press(state.window_size.width, now);
            Task::none()
        }
        iced::Event::Touch(touch::Event::FingerMoved { id, position }) => {
            if state.touch_finger.is_some_and(|finger| finger == id) {
                state.pager.pointer_moved(position.x, Instant::now());
            }
            Task::none()
        }
        iced::Event::Touch(touch::Event::FingerLifted { id, .. }) => {
            if state.touch_finger.take().is_some_and(|finger| finger == id) {
                finish_pager_drag(state, Instant::now())
            } else {
                Task::none()
            }
        }
        iced::Event::Touch(touch::Event::FingerLost { id, .. }) => {
            if state.touch_finger.take().is_some_and(|finger| finger == id) {
                state.pager.cancel_drag();
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

/// Enforces the 16:9 client-area ratio while honoring the 1280x720 minimum.
fn handle_window_resize(state: &mut State, id: window::Id, size: Size) -> Task<Message> {
    if state
        .resize_target
        .is_some_and(|target| sizes_are_close(target, size))
    {
        state.resize_target = None;
        state.window_size = size;
        return Task::none();
    }

    let corrected = proportional_size(state.window_size, size);
    state.window_size = corrected;
    if sizes_are_close(corrected, size) {
        Task::none()
    } else {
        state.resize_target = Some(corrected);
        window::resize(id, corrected)
    }
}

/// Corrects a requested window size using the dimension changed most by the user.
fn proportional_size(previous: Size, requested: Size) -> Size {
    let width_change = ((requested.width - previous.width) / previous.width.max(1.0)).abs();
    let height_change = ((requested.height - previous.height) / previous.height.max(1.0)).abs();
    let mut corrected = if width_change >= height_change {
        Size::new(requested.width, requested.width / ASPECT_RATIO)
    } else {
        Size::new(requested.height * ASPECT_RATIO, requested.height)
    };
    if corrected.width < BASE_WIDTH || corrected.height < BASE_HEIGHT {
        corrected = Size::new(BASE_WIDTH, BASE_HEIGHT);
    }
    corrected
}

/// Reports whether two logical window sizes differ by less than one resize step.
fn sizes_are_close(left: Size, right: Size) -> bool {
    (left.width - right.width).abs() <= SIZE_EPSILON
        && (left.height - right.height).abs() <= SIZE_EPSILON
}

/// Subscribes to runtime events and animation ticks required by the current state.
fn subscription(state: &State) -> Subscription<Message> {
    let events = iced::event::listen().map(Message::RuntimeEvent);
    let captured_touches = event::listen_with(|event, status, _| {
        matches!(status, event::Status::Captured)
            .then_some(event)
            .filter(|event| matches!(event, iced::Event::Touch(_)))
    })
    .map(Message::CapturedTouchEvent);
    let resize_events = window::resize_events().map(|(id, size)| Message::WindowResized(id, size));
    if state.pager.is_animating() {
        Subscription::batch([
            events,
            captured_touches,
            resize_events,
            iced::time::every(Duration::from_millis(16)).map(Message::AnimationTick),
        ])
    } else {
        Subscription::batch([events, captured_touches, resize_events])
    }
}

/// Returns the current normalized horizontal pager offset.
pub(crate) fn transition_offset(state: &State) -> f32 {
    state.pager.offset_at(Instant::now())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    /// Verifies Cinnamon can associate the running window with its desktop entry and icon.
    fn window_has_packaged_application_identity() {
        let settings = window_settings();
        assert!(settings.icon.is_some());
        assert_eq!(settings.platform_specific.application_id, "bingwall");
    }

    /// Creates isolated cache and configuration paths for an application test.
    fn temporary_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "bingwall-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        AppPaths {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
        }
    }

    /// Builds a deterministic application state containing the requested number of entries.
    fn state_with_entries(count: usize) -> State {
        State {
            initializing: false,
            desktop: Some(Desktop::Gnome),
            paths: None,
            settings: Settings::default(),
            entries: (0..count)
                .map(|index| WallpaperEntry {
                    date: format!("2026-01-{index:02}"),
                    description: format!("Wallpaper {index}"),
                    image_url: format!("https://cn.bing.com/{index}.jpg"),
                })
                .collect(),
            pager: Pager::new(count),
            client: Some(reqwest::Client::new()),
            previews: PreviewResidency::new(),
            status: StatusText::Raw(String::new()),
            busy: false,
            settings_save_in_flight: false,
            settings_save_pending: false,
            window_size: Size::new(BASE_WIDTH, BASE_HEIGHT),
            resize_target: None,
            touch_finger: None,
        }
    }

    #[test]
    /// Verifies width-led and height-led resizes remain 16:9 and above the minimum.
    fn window_resize_preserves_the_base_aspect_ratio() {
        let base = Size::new(BASE_WIDTH, BASE_HEIGHT);
        assert_eq!(
            proportional_size(base, Size::new(1600.0, 720.0)),
            Size::new(1600.0, 900.0)
        );
        assert_eq!(
            proportional_size(base, Size::new(1280.0, 900.0)),
            Size::new(1600.0, 900.0)
        );
        assert_eq!(proportional_size(base, Size::new(900.0, 600.0)), base);
    }

    #[test]
    /// Verifies startup exposes cached entries while a background refresh runs.
    fn initialization_populates_the_ui_from_cached_feed_before_refresh() {
        let _locale_guard = crate::resources::lock_locale_tests();
        let paths = temporary_paths("local-first");
        let entries = state_with_entries(12).entries;
        let mut state = state_with_entries(0);
        state.initializing = true;
        let settings = Settings {
            locale: Some(Locale::SimplifiedChinese),
            ..Settings::default()
        };

        let _ = update(
            &mut state,
            Message::Initialized(Ok(Startup::Supported {
                desktop: Desktop::Gnome,
                paths,
                client: reqwest::Client::new(),
                settings,
                cached_entries: entries.clone(),
            })),
        );

        assert_eq!(state.entries, entries);
        assert_eq!(state.pager.selected(), 0);
        assert!(!state.busy);
        assert_eq!(
            state.status,
            StatusText::Localized(texts::cached_feed_refreshing)
        );
        assert_eq!(current_locale(), Locale::SimplifiedChinese);
    }

    #[test]
    /// Verifies a locale message changes both runtime and in-memory settings immediately.
    fn locale_message_switches_runtime_language_immediately() {
        let _locale_guard = crate::resources::lock_locale_tests();
        let mut state = state_with_entries(0);
        state.status = StatusText::localized(texts::loading_feed);

        let _ = update(&mut state, Message::SetLocale(Locale::SimplifiedChinese));

        assert_eq!(current_locale(), Locale::SimplifiedChinese);
        assert_eq!(state.settings.locale, Some(Locale::SimplifiedChinese));
        assert_eq!(state.status.resolve(), "正在加载壁纸源…");
    }

    #[test]
    /// Verifies rapid settings changes coalesce into a final persistence task.
    fn settings_persistence_keeps_the_latest_selection() {
        let _locale_guard = crate::resources::lock_locale_tests();
        let mut state = state_with_entries(0);
        state.paths = Some(temporary_paths("locale-save"));

        let first = update(&mut state, Message::SetLocale(Locale::SimplifiedChinese));
        assert!(state.settings_save_in_flight);
        let second = update(&mut state, Message::SetLocale(Locale::English));
        assert!(state.settings_save_pending);
        drop((first, second));

        let final_save = update(&mut state, Message::SettingsSaved(Ok(())));
        assert!(state.settings_save_in_flight);
        assert!(!state.settings_save_pending);
        assert_eq!(state.settings.locale, Some(Locale::English));
        drop(final_save);
    }

    #[test]
    /// Verifies an unchanged refresh keeps the current selection.
    fn unchanged_background_refresh_preserves_selection() {
        let mut state = state_with_entries(3);
        assert!(state.pager.navigate(1, Instant::now()));
        let entries = state.entries.clone();

        let _ = update(
            &mut state,
            Message::FeedLoaded(WallpaperSource::Bing, Ok((entries, FeedOrigin::Network))),
        );

        assert_eq!(state.pager.selected(), 1);
    }

    #[test]
    /// Verifies selecting another source changes browsing state without moving Daily Change.
    fn selecting_source_preserves_daily_change_assignment() {
        let mut state = state_with_entries(3);
        state.paths = Some(temporary_paths("select-source"));
        state.settings.daily_change = true;
        state.settings.daily_change_source = WallpaperSource::Bing;

        let task = update(
            &mut state,
            Message::SelectSource(WallpaperSource::Spotlight),
        );

        assert_eq!(state.settings.selected_source, WallpaperSource::Spotlight);
        assert_eq!(state.settings.daily_change_source, WallpaperSource::Bing);
        assert!(state.settings.daily_change);
        assert!(state.entries.is_empty());
        assert!(state.busy);
        drop(task);
    }

    #[test]
    /// Verifies cached entries appear before the selected source refresh completes.
    fn source_cache_is_displayed_before_network_refresh() {
        let mut state = state_with_entries(0);
        state.paths = Some(temporary_paths("source-cache"));
        state.settings.selected_source = WallpaperSource::Spotlight;
        state.busy = true;
        let cached = vec![WallpaperEntry {
            date: "2026-08-01".into(),
            description: "Cliffs".into(),
            image_url: "https://windows10spotlight.com/cliffs.jpg".into(),
        }];

        let task = update(
            &mut state,
            Message::SourceCacheLoaded(WallpaperSource::Spotlight, cached.clone()),
        );

        assert_eq!(state.entries, cached);
        assert!(!state.busy);
        assert_eq!(
            state.status,
            StatusText::Localized(texts::cached_feed_refreshing)
        );
        drop(task);
    }

    #[test]
    /// Verifies a late Feed result cannot replace another source's current entries.
    fn stale_cross_source_feed_result_is_ignored() {
        let mut state = state_with_entries(1);
        state.settings.selected_source = WallpaperSource::Spotlight;
        state.busy = true;
        let spotlight_entries = state.entries.clone();
        let stale_bing = vec![WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Bing".into(),
            image_url: "https://cn.bing.com/stale.jpg".into(),
        }];

        let _ = update(
            &mut state,
            Message::FeedLoaded(WallpaperSource::Bing, Ok((stale_bing, FeedOrigin::Network))),
        );

        assert_eq!(state.entries, spotlight_entries);
        assert!(state.busy);
    }

    #[test]
    /// Verifies a refresh failure retains the selected source instead of falling back.
    fn selected_source_is_retained_when_feed_refresh_fails() {
        let mut state = state_with_entries(0);
        state.settings.selected_source = WallpaperSource::Spotlight;
        state.busy = true;

        let _ = update(
            &mut state,
            Message::FeedLoaded(WallpaperSource::Spotlight, Err("offline".into())),
        );

        assert_eq!(state.settings.selected_source, WallpaperSource::Spotlight);
        assert!(state.entries.is_empty());
        assert!(!state.busy);
        assert_eq!(state.status, StatusText::Raw("offline".into()));
    }

    #[test]
    /// Verifies Daily Change appears enabled only while browsing its assigned source.
    fn daily_change_visibility_follows_its_unique_source() {
        let mut state = state_with_entries(0);
        state.settings.daily_change = true;
        state.settings.daily_change_source = WallpaperSource::Bing;
        state.settings.selected_source = WallpaperSource::Bing;
        assert!(state.daily_change_enabled_for_selected_source());

        state.settings.selected_source = WallpaperSource::Spotlight;
        assert!(!state.daily_change_enabled_for_selected_source());

        state.settings.daily_change_source = WallpaperSource::Spotlight;
        assert!(state.daily_change_enabled_for_selected_source());

        state.settings.daily_change = false;
        assert!(!state.daily_change_enabled_for_selected_source());
    }

    #[test]
    /// Verifies workflow snapshots cannot replace newer UI-owned choices.
    fn workflow_settings_merge_preserves_current_source_and_locale() {
        let mut state = state_with_entries(0);
        state.settings.selected_source = WallpaperSource::Spotlight;
        state.settings.locale = Some(Locale::SimplifiedChinese);
        let workflow_settings = Settings {
            daily_change: true,
            daily_change_source: WallpaperSource::Spotlight,
            ..Settings::default()
        };

        let needs_persistence = merge_settings_preserving_ui_choices(&mut state, workflow_settings);

        assert!(needs_persistence);
        assert_eq!(state.settings.selected_source, WallpaperSource::Spotlight);
        assert_eq!(state.settings.locale, Some(Locale::SimplifiedChinese));
        assert!(state.settings.daily_change);
        assert_eq!(
            state.settings.daily_change_source,
            WallpaperSource::Spotlight
        );
    }
}
