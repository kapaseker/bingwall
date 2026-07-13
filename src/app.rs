use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

use iced::{Point, Subscription, Task, keyboard, mouse, touch, widget::image};

use crate::{
    cache,
    feed::WallpaperEntry,
    locale::{Locale, TextKey},
    paths::AppPaths,
    platform::Desktop,
    service::{self, FeedOrigin},
    settings::Settings,
    systemd, ui,
};

const PAGE_BATCH: usize = 10;
const TRANSITION_DURATION: Duration = Duration::from_millis(180);
const WHEEL_DEBOUNCE: Duration = Duration::from_millis(240);
const MAX_IMAGE_TASKS: usize = 2;
const GPU_PRELOAD_LIMIT: usize = 4;

#[derive(Debug, Clone)]
pub(crate) struct Transition {
    pub from: usize,
    pub direction: f32,
    pub started: Instant,
}

#[derive(Debug)]
pub(crate) struct State {
    pub locale: Locale,
    pub initializing: bool,
    pub desktop: Option<Desktop>,
    pub paths: Option<AppPaths>,
    pub settings: Settings,
    pub entries: Vec<WallpaperEntry>,
    pub visible_count: usize,
    pub selected: usize,
    client: Option<reqwest::Client>,
    preview_paths: HashMap<String, PathBuf>,
    preview_allocations: HashMap<String, image::Allocation>,
    allocating_previews: HashSet<String>,
    queued_previews: VecDeque<WallpaperEntry>,
    active_previews: HashSet<String>,
    failed_previews: HashSet<String>,
    failed_allocations: HashSet<String>,
    invalidated_previews: HashSet<String>,
    gpu_preload_limit: usize,
    retried_current_allocation: HashSet<String>,
    pub status: String,
    pub busy: bool,
    pub transition: Option<Transition>,
    last_wheel: Option<Instant>,
    touch_start: Option<(touch::Finger, Point)>,
}

impl State {
    /// Returns the allocated GPU image handle for an entry when it is ready.
    pub(crate) fn preview_handle(&self, index: usize) -> Option<image::Handle> {
        let image_url = &self.entries.get(index)?.image_url;
        self.preview_allocations
            .get(image_url)
            .map(|allocation| allocation.handle().clone())
    }

    /// Reports whether the selected wallpaper has an allocated preview.
    pub(crate) fn selected_preview_is_ready(&self) -> bool {
        self.preview_handle(self.selected).is_some()
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
    FeedLoaded(Result<(Vec<WallpaperEntry>, FeedOrigin), String>),
    PreviewReady(String, Result<PathBuf, String>),
    PreviewAllocated(String, Result<image::Allocation, image::Error>),
    PreviewInvalidated(String, Result<(), String>),
    Previous,
    Next,
    SetWallpaper,
    Applied(Result<Settings, String>),
    ToggleDaily(bool),
    ToggleFinished(bool, Result<Settings, String>),
    RuntimeEvent(iced::Event),
    AnimationTick(Instant),
}

/// Configures and launches the Bingwall graphical application.
pub fn run() -> iced::Result {
    iced::application(boot, update, ui::view)
        .title("Bingwall")
        .subscription(subscription)
        .window_size((1100.0, 760.0))
        .centered()
        .antialiasing(true)
        .run()
}

/// Creates the initial application state and starts background initialization.
fn boot() -> (State, Task<Message>) {
    let locale = Locale::detect();
    let state = State {
        locale,
        initializing: true,
        desktop: None,
        paths: None,
        settings: Settings::default(),
        entries: Vec::new(),
        visible_count: 0,
        selected: 0,
        client: None,
        preview_paths: HashMap::new(),
        preview_allocations: HashMap::new(),
        allocating_previews: HashSet::new(),
        queued_previews: VecDeque::new(),
        active_previews: HashSet::new(),
        failed_previews: HashSet::new(),
        failed_allocations: HashSet::new(),
        invalidated_previews: HashSet::new(),
        gpu_preload_limit: GPU_PRELOAD_LIMIT,
        retried_current_allocation: HashSet::new(),
        status: locale.text(TextKey::LoadingFeed).into(),
        busy: true,
        transition: None,
        last_wheel: None,
        touch_start: None,
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
    let cached_entries = cache::load_feed(&paths)
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
                state.status = state.locale.text(TextKey::Unsupported).into();
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
                state.initializing = false;
                state.desktop = Some(desktop);
                state.paths = Some(paths);
                state.client = Some(client);
                state.settings = settings;
                state.entries = cached_entries;
                state.visible_count = state.entries.len().min(PAGE_BATCH);
                state.busy = !has_cached_feed;
                state.status = if has_cached_feed {
                    state.locale.text(TextKey::CachedFeedRefreshing).into()
                } else {
                    state
                        .settings
                        .last_update_status
                        .clone()
                        .unwrap_or_else(|| state.locale.text(TextKey::LoadingFeed).into())
                };
                Task::batch([
                    schedule_previews(state),
                    refresh_task(state, !has_cached_feed),
                ])
            }
            Err(error) => {
                state.initializing = false;
                state.busy = false;
                state.status = error;
                Task::none()
            }
        },
        Message::Refresh => {
            state.failed_previews.clear();
            refresh_task(state, true)
        }
        Message::FeedLoaded(result) => {
            state.busy = false;
            match result {
                Ok((entries, origin)) => {
                    let feed_changed = state.entries != entries;
                    if feed_changed {
                        state.entries = entries;
                        state.selected = 0;
                        state.visible_count = state.entries.len().min(PAGE_BATCH);
                    }
                    state.status = match origin {
                        FeedOrigin::Network => state.locale.text(TextKey::FeedRefreshed),
                        FeedOrigin::Cache => state.locale.text(TextKey::CachedFeed),
                    }
                    .into();
                    schedule_previews(state)
                }
                Err(error) => {
                    state.status = error;
                    Task::none()
                }
            }
        }
        Message::PreviewReady(image_url, result) => {
            state.active_previews.remove(&image_url);
            match result {
                Ok(path) => {
                    state.failed_previews.remove(&image_url);
                    state.failed_allocations.remove(&image_url);
                    state.preview_paths.insert(image_url.clone(), path);
                }
                Err(error) => {
                    state.failed_previews.insert(image_url.clone());
                    if is_current_url(state, &image_url) {
                        state.status = error;
                    }
                }
            }
            schedule_previews(state)
        }
        Message::PreviewAllocated(image_url, result) => {
            state.allocating_previews.remove(&image_url);
            match result {
                Ok(allocation) if desired_preview_urls(state).contains(&image_url) => {
                    state.preview_allocations.insert(image_url, allocation);
                }
                Ok(_) => {}
                Err(image::Error::OutOfMemory) => {
                    state.gpu_preload_limit = state.gpu_preload_limit.saturating_sub(1).max(1);
                    if !is_current_url(state, &image_url)
                        || !state.retried_current_allocation.insert(image_url.clone())
                    {
                        state.failed_allocations.insert(image_url.clone());
                        if is_current_url(state, &image_url) {
                            state.status = image::Error::OutOfMemory.to_string();
                        }
                    }
                }
                Err(
                    error @ (image::Error::Invalid(_)
                    | image::Error::Inaccessible(_)
                    | image::Error::Empty),
                ) => {
                    if state.invalidated_previews.insert(image_url.clone()) {
                        let path = state.preview_paths.remove(&image_url);
                        return Task::perform(
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
                            move |result| Message::PreviewInvalidated(image_url.clone(), result),
                        );
                    }
                    state.failed_allocations.insert(image_url.clone());
                    if is_current_url(state, &image_url) {
                        state.status = error.to_string();
                    }
                }
                Err(error) => {
                    state.failed_allocations.insert(image_url.clone());
                    if is_current_url(state, &image_url) {
                        state.status = error.to_string();
                    }
                }
            }
            schedule_previews(state)
        }
        Message::PreviewInvalidated(image_url, result) => {
            state.failed_allocations.remove(&image_url);
            match result {
                Ok(()) => {
                    state.failed_previews.remove(&image_url);
                }
                Err(error) => {
                    state.failed_previews.insert(image_url.clone());
                    if is_current_url(state, &image_url) {
                        state.status = error;
                    }
                }
            }
            schedule_previews(state)
        }
        Message::Previous => navigate(state, -1),
        Message::Next => navigate(state, 1),
        Message::SetWallpaper => apply_selected_task(state),
        Message::Applied(result) => {
            state.busy = false;
            match result {
                Ok(settings) => {
                    state.settings = settings;
                    state.status = state.locale.text(TextKey::Applied).into();
                }
                Err(error) => state.status = error,
            }
            Task::none()
        }
        Message::ToggleDaily(enabled) => toggle_daily_task(state, enabled),
        Message::ToggleFinished(enabled, result) => {
            state.busy = false;
            match result {
                Ok(settings) => {
                    state.settings = settings;
                    state.status = state
                        .locale
                        .text(if enabled {
                            TextKey::Enabled
                        } else {
                            TextKey::Disabled
                        })
                        .into();
                }
                Err(error) => state.status = error,
            }
            Task::none()
        }
        Message::RuntimeEvent(event) => handle_runtime_event(state, event),
        Message::AnimationTick(now) => {
            if state.transition.as_ref().is_some_and(|transition| {
                now.duration_since(transition.started) >= TRANSITION_DURATION
            }) {
                state.transition = None;
            }
            Task::none()
        }
    }
}

/// Starts a feed refresh and optionally places the interface in a blocking state.
fn refresh_task(state: &mut State, blocking: bool) -> Task<Message> {
    let (Some(paths), Some(client)) = (state.paths.clone(), state.client.clone()) else {
        return Task::none();
    };
    if blocking {
        state.busy = true;
        state.status = state.locale.text(TextKey::LoadingFeed).into();
    }
    Task::perform(
        async move {
            service::refresh_feed(&client, &paths)
                .await
                .map_err(|error| error.to_string())
        },
        Message::FeedLoaded,
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
    let desired_urls = desired_entries
        .iter()
        .map(|entry| entry.image_url.clone())
        .collect::<Vec<_>>();
    let gpu_urls = desired_urls
        .iter()
        .take(state.gpu_preload_limit)
        .cloned()
        .collect::<HashSet<_>>();
    state
        .preview_allocations
        .retain(|url, _| gpu_urls.contains(url));

    for entry in &desired_entries {
        let url = &entry.image_url;
        if state.preview_paths.contains_key(url)
            || state.active_previews.contains(url)
            || state.failed_previews.contains(url)
            || state
                .queued_previews
                .iter()
                .any(|queued| queued.image_url == *url)
        {
            continue;
        }
        state.queued_previews.push_back(entry.clone());
    }

    let priority = desired_urls
        .iter()
        .enumerate()
        .map(|(rank, url)| (url.as_str(), rank))
        .collect::<HashMap<_, _>>();
    state
        .queued_previews
        .make_contiguous()
        .sort_by_key(|entry| {
            priority
                .get(entry.image_url.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });

    let mut tasks = Vec::new();
    while state.active_previews.len() < MAX_IMAGE_TASKS {
        let Some(entry) = state.queued_previews.pop_front() else {
            break;
        };
        let image_url = entry.image_url.clone();
        state.active_previews.insert(image_url.clone());
        let paths = paths.clone();
        let client = client.clone();
        tasks.push(Task::perform(
            async move {
                service::ensure_preview(&client, &paths, &entry)
                    .await
                    .map_err(|error| error.to_string())
            },
            move |result| Message::PreviewReady(image_url.clone(), result),
        ));
    }

    for image_url in desired_urls.into_iter().take(state.gpu_preload_limit) {
        if state.preview_allocations.contains_key(&image_url)
            || state.failed_allocations.contains(&image_url)
            || !state.allocating_previews.insert(image_url.clone())
        {
            continue;
        }
        let Some(path) = state.preview_paths.get(&image_url).cloned() else {
            state.allocating_previews.remove(&image_url);
            continue;
        };
        tasks.push(
            image::allocate(image::Handle::from_path(path))
                .map(move |result| Message::PreviewAllocated(image_url.clone(), result)),
        );
    }
    Task::batch(tasks)
}

/// Returns unique entries to preload in navigation-priority order.
fn desired_preview_entries(state: &State) -> Vec<WallpaperEntry> {
    let mut indices = Vec::with_capacity(GPU_PRELOAD_LIMIT);
    for index in [
        Some(state.selected),
        state.selected.checked_add(1),
        state.selected.checked_sub(1),
        state.selected.checked_add(2),
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

/// Returns the image URLs currently eligible for preview allocation.
fn desired_preview_urls(state: &State) -> HashSet<String> {
    desired_preview_entries(state)
        .into_iter()
        .map(|entry| entry.image_url)
        .collect()
}

/// Reports whether an image URL belongs to the selected entry.
fn is_current_url(state: &State, image_url: &str) -> bool {
    state
        .entries
        .get(state.selected)
        .is_some_and(|entry| entry.image_url == image_url)
}

/// Moves the selection within bounds, extends the visible page, and starts a transition.
fn navigate(state: &mut State, direction: isize) -> Task<Message> {
    if state.entries.is_empty() {
        return Task::none();
    }
    let next = state.selected.saturating_add_signed(direction);
    if next >= state.entries.len() || next == state.selected {
        return Task::none();
    }
    let previous = state.selected;
    state.selected = next;
    if let Some(entry) = state.entries.get(state.selected) {
        state.failed_previews.remove(&entry.image_url);
        state.failed_allocations.remove(&entry.image_url);
    }
    if state.selected + 2 >= state.visible_count && state.visible_count < state.entries.len() {
        state.visible_count = (state.visible_count + PAGE_BATCH).min(state.entries.len());
    }
    state.transition = Some(Transition {
        from: previous,
        direction: direction.signum() as f32,
        started: Instant::now(),
    });
    schedule_previews(state)
}

/// Starts the work needed to download and apply the selected wallpaper.
fn apply_selected_task(state: &mut State) -> Task<Message> {
    let (Some(desktop), Some(paths), Some(entry)) = (
        state.desktop,
        state.paths.clone(),
        state.entries.get(state.selected).cloned(),
    ) else {
        return Task::none();
    };
    let Some(client) = state.client.clone() else {
        return Task::none();
    };
    state.busy = true;
    state.status = state.locale.text(TextKey::Working).into();
    Task::perform(
        async move {
            let image = service::ensure_image(&client, &paths, &entry)
                .await
                .map_err(|error| error.to_string())?;
            tokio::task::spawn_blocking(move || apply_wallpaper(desktop, paths, entry, image))
                .await
                .map_err(|error| error.to_string())?
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
    if enabled && current.is_none() {
        state.status = state.locale.text(TextKey::LoadingFeed).into();
        return Task::none();
    }
    state.busy = true;
    state.status = state.locale.text(TextKey::Working).into();
    Task::perform(
        async move { set_daily_change(enabled, desktop, paths, current, client).await },
        move |result| Message::ToggleFinished(enabled, result),
    )
}

/// Applies an image and persists the selected wallpaper metadata.
fn apply_wallpaper(
    desktop: Desktop,
    paths: AppPaths,
    entry: WallpaperEntry,
    image: PathBuf,
) -> Result<Settings, String> {
    desktop.apply(&image).map_err(|error| error.to_string())?;
    let mut settings = Settings::load(&paths.settings_file()).map_err(|error| error.to_string())?;
    settings.applied_image = Some(image.to_string_lossy().into_owned());
    settings.last_update_status = Some(format!("Updated to {}", entry.date));
    settings
        .save(&paths.settings_file())
        .map_err(|error| error.to_string())?;
    Ok(settings)
}

/// Updates the systemd timer, optionally applies today's image, and persists the setting.
async fn set_daily_change(
    enabled: bool,
    desktop: Desktop,
    paths: AppPaths,
    current: Option<WallpaperEntry>,
    client: reqwest::Client,
) -> Result<Settings, String> {
    let settings_path = paths.settings_file();
    let mut settings = tokio::task::spawn_blocking(move || Settings::load(&settings_path))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let (image, applied_date) = if enabled {
        let entry = current.ok_or_else(|| "the wallpaper feed is empty".to_owned())?;
        let date = entry.date.clone();
        (
            Some(
                service::ensure_image(&client, &paths, &entry)
                    .await
                    .map_err(|error| error.to_string())?,
            ),
            Some(date),
        )
    } else {
        (None, None)
    };
    tokio::task::spawn_blocking(move || {
        if let Some(image) = image {
            desktop.apply(&image).map_err(|error| error.to_string())?;
            systemd::enable(&paths).map_err(|error| error.to_string())?;
            settings.applied_image = Some(image.to_string_lossy().into_owned());
            let date = applied_date.expect("enabled daily change has a current entry");
            settings.last_update_status = Some(format!("Updated to {date}"));
        } else {
            systemd::disable().map_err(|error| error.to_string())?;
        }
        settings.daily_change = enabled;
        settings
            .save(&paths.settings_file())
            .map_err(|error| error.to_string())?;
        Ok(settings)
    })
    .await
    .map_err(|error| error.to_string())?
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
            if state
                .last_wheel
                .is_some_and(|last| now.duration_since(last) < WHEEL_DEBOUNCE)
            {
                return Task::none();
            }
            let (x, y) = match delta {
                mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => (x, y),
            };
            let movement = if x.abs() > y.abs() { x } else { y };
            if movement.abs() < f32::EPSILON {
                Task::none()
            } else {
                state.last_wheel = Some(now);
                navigate(state, if movement < 0.0 { 1 } else { -1 })
            }
        }
        iced::Event::Touch(touch::Event::FingerPressed { id, position }) => {
            state.touch_start = Some((id, position));
            Task::none()
        }
        iced::Event::Touch(touch::Event::FingerLifted { id, position }) => {
            let start = state
                .touch_start
                .take()
                .filter(|(finger, _)| *finger == id)
                .map(|(_, point)| point);
            match start.map(|point| position.x - point.x) {
                Some(distance) if distance > 50.0 => navigate(state, -1),
                Some(distance) if distance < -50.0 => navigate(state, 1),
                _ => Task::none(),
            }
        }
        iced::Event::Touch(touch::Event::FingerLost { .. }) => {
            state.touch_start = None;
            Task::none()
        }
        _ => Task::none(),
    }
}

/// Subscribes to runtime events and animation ticks required by the current state.
fn subscription(state: &State) -> Subscription<Message> {
    let events = iced::event::listen().map(Message::RuntimeEvent);
    if state.transition.is_some() {
        Subscription::batch([
            events,
            iced::time::every(Duration::from_millis(16)).map(Message::AnimationTick),
        ])
    } else {
        events
    }
}

/// Returns the selected-image transition's normalized elapsed progress.
pub(crate) fn transition_progress(state: &State) -> f32 {
    state
        .transition
        .as_ref()
        .map(|transition| {
            (Instant::now()
                .duration_since(transition.started)
                .as_secs_f32()
                / TRANSITION_DURATION.as_secs_f32())
            .clamp(0.0, 1.0)
        })
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

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
            locale: Locale::English,
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
            visible_count: count.min(PAGE_BATCH),
            selected: 0,
            client: Some(reqwest::Client::new()),
            preview_paths: HashMap::new(),
            preview_allocations: HashMap::new(),
            allocating_previews: HashSet::new(),
            queued_previews: VecDeque::new(),
            active_previews: HashSet::new(),
            failed_previews: HashSet::new(),
            failed_allocations: HashSet::new(),
            invalidated_previews: HashSet::new(),
            gpu_preload_limit: GPU_PRELOAD_LIMIT,
            retried_current_allocation: HashSet::new(),
            status: String::new(),
            busy: false,
            transition: None,
            last_wheel: None,
            touch_start: None,
        }
    }

    #[test]
    /// Verifies navigation expands visible metadata in ten-entry batches.
    fn pager_loads_metadata_in_batches_of_ten() {
        let mut state = state_with_entries(25);
        for _ in 0..8 {
            let _ = navigate(&mut state, 1);
        }
        assert_eq!(state.visible_count, 20);
        for _ in 0..10 {
            let _ = navigate(&mut state, 1);
        }
        assert_eq!(state.visible_count, 25);
    }

    #[test]
    /// Verifies navigation cannot move before the first or after the last entry.
    fn pager_never_moves_outside_the_feed() {
        let mut state = state_with_entries(2);
        let _ = navigate(&mut state, -1);
        assert_eq!(state.selected, 0);
        let _ = navigate(&mut state, 1);
        let _ = navigate(&mut state, 1);
        assert_eq!(state.selected, 1);
    }

    #[test]
    /// Verifies startup exposes cached entries while a background refresh runs.
    fn initialization_populates_the_ui_from_cached_feed_before_refresh() {
        let paths = temporary_paths("local-first");
        let entries = state_with_entries(12).entries;
        let mut state = state_with_entries(0);
        state.initializing = true;
        state.locale = Locale::SimplifiedChinese;

        let _ = update(
            &mut state,
            Message::Initialized(Ok(Startup::Supported {
                desktop: Desktop::Gnome,
                paths,
                client: reqwest::Client::new(),
                settings: Settings::default(),
                cached_entries: entries.clone(),
            })),
        );

        assert_eq!(state.entries, entries);
        assert_eq!(state.visible_count, PAGE_BATCH);
        assert_eq!(state.selected, 0);
        assert!(!state.busy);
        assert_eq!(
            state.status,
            Locale::SimplifiedChinese.text(TextKey::CachedFeedRefreshing)
        );
    }

    #[test]
    /// Verifies an unchanged refresh keeps the current selection and loaded previews.
    fn unchanged_background_refresh_preserves_loaded_images_and_selection() {
        let mut state = state_with_entries(3);
        state.selected = 1;
        let selected_url = state.entries[1].image_url.clone();
        state
            .preview_paths
            .insert(selected_url.clone(), PathBuf::from("cached.jpg"));
        let entries = state.entries.clone();

        let _ = update(
            &mut state,
            Message::FeedLoaded(Ok((entries, FeedOrigin::Network))),
        );

        assert_eq!(state.selected, 1);
        assert_eq!(
            state.preview_paths.get(&selected_url),
            Some(&PathBuf::from("cached.jpg"))
        );
    }

    #[test]
    /// Verifies completed off-window previews remain cached without consuming GPU preload space.
    fn completed_old_feed_task_is_kept_without_entering_the_gpu_window() {
        let mut state = state_with_entries(1);
        let stale_url = "https://cn.bing.com/old.jpg".to_owned();
        state.active_previews.insert(stale_url.clone());

        let _ = update(
            &mut state,
            Message::PreviewReady(stale_url.clone(), Ok(PathBuf::from("old-preview.jpg"))),
        );

        assert_eq!(
            state.preview_paths.get(&stale_url),
            Some(&PathBuf::from("old-preview.jpg"))
        );
        assert!(state.preview_allocations.is_empty());
        assert!(state.active_previews.is_empty());
    }

    #[test]
    /// Verifies queued preview work follows a changed selection without discarding prior work.
    fn queued_previews_are_reprioritized_without_being_cancelled() {
        let mut state = state_with_entries(6);
        state.selected = 2;
        state.paths = Some(temporary_paths("queue"));

        let _ = schedule_previews(&mut state);
        assert_eq!(
            state.active_previews,
            HashSet::from([
                "https://cn.bing.com/2.jpg".to_owned(),
                "https://cn.bing.com/3.jpg".to_owned(),
            ])
        );

        let _ = navigate(&mut state, -1);
        let queued = state
            .queued_previews
            .iter()
            .map(|entry| entry.image_url.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            queued,
            vec![
                "https://cn.bing.com/1.jpg",
                "https://cn.bing.com/0.jpg",
                "https://cn.bing.com/4.jpg",
            ]
        );
    }

    #[test]
    /// Verifies GPU exhaustion reduces preloading and retries the selected preview only once.
    fn gpu_out_of_memory_reduces_preload_window_and_retries_current_once() {
        let mut state = state_with_entries(4);
        let current_url = state.entries[0].image_url.clone();
        state.allocating_previews.insert(current_url.clone());

        let _ = update(
            &mut state,
            Message::PreviewAllocated(current_url.clone(), Err(image::Error::OutOfMemory)),
        );

        assert_eq!(state.gpu_preload_limit, 3);
        assert!(state.retried_current_allocation.contains(&current_url));
        assert!(!state.failed_allocations.contains(&current_url));
    }
}
