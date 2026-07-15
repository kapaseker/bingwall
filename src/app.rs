use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

use iced::{Point, Size, Subscription, Task, event, keyboard, mouse, touch, widget::image, window};

use crate::{
    cache,
    feed::WallpaperEntry,
    paths::AppPaths,
    platform::Desktop,
    resources::{Locale, TextResource, current_locale, generated_text as texts, set_locale},
    service::{self, FeedOrigin},
    settings::Settings,
    systemd, ui,
};

const PAGE_BATCH: usize = 10;
const TRANSITION_DURATION: Duration = Duration::from_millis(360);
const MIN_TRANSITION_DURATION: Duration = Duration::from_millis(120);
const WHEEL_DEBOUNCE: Duration = Duration::from_millis(240);
const MAX_IMAGE_TASKS: usize = 2;
const GPU_PRELOAD_LIMIT: usize = 4;
pub(crate) const BASE_WIDTH: f32 = 1280.0;
pub(crate) const BASE_HEIGHT: f32 = 720.0;
const ASPECT_RATIO: f32 = BASE_WIDTH / BASE_HEIGHT;
const SNAP_DISTANCE_RATIO: f32 = 0.12;
const SNAP_VELOCITY: f32 = 650.0;
const SIZE_EPSILON: f32 = 0.5;

#[derive(Debug, Clone)]
pub(crate) struct Transition {
    pub from: usize,
    pub to: usize,
    pub start_offset: f32,
    pub end_offset: f32,
    pub started: Instant,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
struct PagerDrag {
    start_x: f32,
    last_x: f32,
    last_at: Instant,
    velocity: f32,
}

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
    pub status: StatusText,
    pub busy: bool,
    locale_save_in_flight: bool,
    locale_save_pending: bool,
    pub transition: Option<Transition>,
    pub pager_offset: f32,
    pager_drag: Option<PagerDrag>,
    pager_pointer_x: Option<f32>,
    pager_width: f32,
    window_size: Size,
    resize_target: Option<Size>,
    last_wheel: Option<Instant>,
    touch_finger: Option<touch::Finger>,
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
    #[allow(dead_code)]
    SetLocale(Locale),
    LocaleSaved(Result<(), String>),
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
            include_bytes!("../packaging/icons/bingwall-256.png"),
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
        status: StatusText::localized(texts::loading_feed),
        busy: true,
        locale_save_in_flight: false,
        locale_save_pending: false,
        transition: None,
        pager_offset: 0.0,
        pager_drag: None,
        pager_pointer_x: None,
        pager_width: BASE_WIDTH,
        window_size: Size::new(BASE_WIDTH, BASE_HEIGHT),
        resize_target: None,
        last_wheel: None,
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
                state.visible_count = state.entries.len().min(PAGE_BATCH);
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
                        state.status = error.into();
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
                            state.status = image::Error::OutOfMemory.to_string().into();
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
                        state.status = error.to_string().into();
                    }
                }
                Err(error) => {
                    state.failed_allocations.insert(image_url.clone());
                    if is_current_url(state, &image_url) {
                        state.status = error.to_string().into();
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
                        state.status = error.into();
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
                    let locale_changed = merge_settings_preserving_locale(state, settings);
                    state.status = StatusText::localized(texts::applied);
                    if locale_changed {
                        return persist_locale_task(state);
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
                    let locale_changed = merge_settings_preserving_locale(state, settings);
                    state.status = StatusText::localized(if enabled {
                        texts::enabled
                    } else {
                        texts::disabled
                    });
                    if locale_changed {
                        return persist_locale_task(state);
                    }
                }
                Err(error) => state.status = error.into(),
            }
            Task::none()
        }
        Message::SetLocale(locale) => {
            set_locale(locale);
            state.settings.locale = Some(locale);
            persist_locale_task(state)
        }
        Message::LocaleSaved(result) => {
            state.locale_save_in_flight = false;
            if state.locale_save_pending {
                state.locale_save_pending = false;
                return persist_locale_task(state);
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
            state.pager_pointer_x = Some(position.x);
            update_pager_drag(state, position.x, Instant::now());
            Task::none()
        }
        Message::PagerPressed(width) => {
            if state.transition.is_none() {
                state.pager_width = width.max(1.0);
                let x = state.pager_pointer_x.unwrap_or(width / 2.0);
                start_pager_drag(state, x, Instant::now());
            }
            Task::none()
        }
        Message::PagerReleased => finish_pager_drag(state, Instant::now()),
        Message::AnimationTick(now) => {
            if state.transition.as_ref().is_some_and(|transition| {
                now.duration_since(transition.started) >= transition.duration
            }) {
                state.transition = None;
                state.pager_offset = 0.0;
            }
            Task::none()
        }
    }
}

/// Merges asynchronously returned settings without losing the latest language choice.
fn merge_settings_preserving_locale(state: &mut State, mut settings: Settings) -> bool {
    let locale = state.settings.locale;
    let locale_changed = settings.locale != locale;
    settings.locale = locale;
    state.settings = settings;
    locale_changed
}

/// Serializes persistence of the current language choice so the latest selection wins.
fn persist_locale_task(state: &mut State) -> Task<Message> {
    if state.locale_save_in_flight {
        state.locale_save_pending = true;
        return Task::none();
    }
    let Some(paths) = state.paths.clone() else {
        return Task::none();
    };
    let settings = state.settings.clone();
    state.locale_save_in_flight = true;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || settings.save(&paths.settings_file()))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        },
        Message::LocaleSaved,
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
    navigate_from_offset(state, direction, 0.0)
}

/// Starts a horizontal snap from the supplied normalized viewport offset.
fn navigate_from_offset(state: &mut State, direction: isize, start_offset: f32) -> Task<Message> {
    if state.entries.is_empty() || state.transition.is_some() || state.pager_drag.is_some() {
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
    let end_offset = -(direction.signum() as f32);
    state.transition = Some(Transition {
        from: previous,
        to: next,
        start_offset,
        end_offset,
        started: Instant::now(),
        duration: snap_duration(start_offset, end_offset),
    });
    state.pager_offset = start_offset;
    schedule_previews(state)
}

/// Starts a pointer-driven pager gesture.
fn start_pager_drag(state: &mut State, x: f32, now: Instant) {
    if state.entries.is_empty() || state.transition.is_some() {
        return;
    }
    state.pager_offset = 0.0;
    state.pager_drag = Some(PagerDrag {
        start_x: x,
        last_x: x,
        last_at: now,
        velocity: 0.0,
    });
}

/// Updates the normalized horizontal pager offset and release velocity.
fn update_pager_drag(state: &mut State, x: f32, now: Instant) {
    let Some(drag) = state.pager_drag.as_mut() else {
        return;
    };
    let elapsed = now.duration_since(drag.last_at).as_secs_f32();
    if elapsed > f32::EPSILON {
        let instantaneous = (x - drag.last_x) / elapsed;
        drag.velocity = drag.velocity * 0.65 + instantaneous * 0.35;
    }
    drag.last_x = x;
    drag.last_at = now;

    let mut offset = (x - drag.start_x) / state.pager_width.max(1.0);
    let has_previous = state.selected > 0;
    let has_next = state.selected + 1 < state.entries.len();
    if !has_previous {
        offset = offset.min(0.0);
    }
    if !has_next {
        offset = offset.max(0.0);
    }
    state.pager_offset = offset.clamp(-1.0, 1.0);
}

/// Chooses the adjacent page or snaps the pager back to its current page.
fn finish_pager_drag(state: &mut State, now: Instant) -> Task<Message> {
    let Some(drag) = state.pager_drag.take() else {
        return Task::none();
    };
    let direction = snap_direction(state.pager_offset, drag.velocity);
    if direction != 0 {
        return navigate_from_offset(state, direction, state.pager_offset);
    }
    if state.pager_offset.abs() > f32::EPSILON {
        state.transition = Some(Transition {
            from: state.selected,
            to: state.selected,
            start_offset: state.pager_offset,
            end_offset: 0.0,
            started: now,
            duration: snap_duration(state.pager_offset, 0.0),
        });
    }
    Task::none()
}

/// Returns the requested page direction from drag distance and velocity.
fn snap_direction(offset: f32, velocity: f32) -> isize {
    if offset <= -SNAP_DISTANCE_RATIO || velocity <= -SNAP_VELOCITY {
        1
    } else if offset >= SNAP_DISTANCE_RATIO || velocity >= SNAP_VELOCITY {
        -1
    } else {
        0
    }
}

/// Keeps snap velocity consistent by scaling duration with remaining distance.
fn snap_duration(start_offset: f32, end_offset: f32) -> Duration {
    let distance = (end_offset - start_offset).abs().min(1.0);
    let millis = (TRANSITION_DURATION.as_millis() as f32 * distance)
        .round()
        .max(MIN_TRANSITION_DURATION.as_millis() as f32);
    Duration::from_millis(millis as u64)
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
    state.status = StatusText::localized(texts::working);
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
        state.status = StatusText::localized(texts::loading_feed);
        return Task::none();
    }
    state.busy = true;
    state.status = StatusText::localized(texts::working);
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
            state.pager_width = state.window_size.width.max(1.0);
            start_pager_drag(state, position.x, now);
            Task::none()
        }
        iced::Event::Touch(touch::Event::FingerMoved { id, position }) => {
            if state.touch_finger.is_some_and(|finger| finger == id) {
                update_pager_drag(state, position.x, Instant::now());
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
                state.pager_drag = None;
                state.pager_offset = 0.0;
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
    if state.transition.is_some() {
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

/// Returns the selected-image transition's normalized elapsed progress.
pub(crate) fn transition_progress(state: &State) -> f32 {
    state
        .transition
        .as_ref()
        .map(|transition| {
            (Instant::now()
                .duration_since(transition.started)
                .as_secs_f32()
                / transition.duration.as_secs_f32())
            .clamp(0.0, 1.0)
        })
        .unwrap_or(1.0)
}

/// Returns the current normalized horizontal pager offset.
pub(crate) fn transition_offset(state: &State) -> f32 {
    state
        .transition
        .as_ref()
        .map(|transition| {
            let progress = transition_progress(state);
            transition_offset_at(transition, progress)
        })
        .unwrap_or(state.pager_offset)
}

/// Interpolates a pager transition without changing image scale or opacity.
fn transition_offset_at(transition: &Transition, progress: f32) -> f32 {
    transition.start_offset
        + (transition.end_offset - transition.start_offset) * progress.clamp(0.0, 1.0)
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
            status: StatusText::Raw(String::new()),
            busy: false,
            locale_save_in_flight: false,
            locale_save_pending: false,
            transition: None,
            pager_offset: 0.0,
            pager_drag: None,
            pager_pointer_x: None,
            pager_width: BASE_WIDTH,
            window_size: Size::new(BASE_WIDTH, BASE_HEIGHT),
            resize_target: None,
            last_wheel: None,
            touch_finger: None,
        }
    }

    /// Completes a pager animation so a test can issue another navigation input.
    fn complete_transition(state: &mut State) {
        state.transition = None;
        state.pager_offset = 0.0;
    }

    #[test]
    /// Verifies navigation expands visible metadata in ten-entry batches.
    fn pager_loads_metadata_in_batches_of_ten() {
        let mut state = state_with_entries(25);
        for _ in 0..8 {
            let _ = navigate(&mut state, 1);
            complete_transition(&mut state);
        }
        assert_eq!(state.visible_count, 20);
        for _ in 0..10 {
            let _ = navigate(&mut state, 1);
            complete_transition(&mut state);
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
        complete_transition(&mut state);
        let _ = navigate(&mut state, 1);
        assert_eq!(state.selected, 1);
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
    /// Verifies pager snap decisions honor distance, velocity, and direction.
    fn pager_snap_uses_distance_or_release_velocity() {
        assert_eq!(snap_direction(-SNAP_DISTANCE_RATIO, 0.0), 1);
        assert_eq!(snap_direction(SNAP_DISTANCE_RATIO, 0.0), -1);
        assert_eq!(snap_direction(-0.02, -SNAP_VELOCITY), 1);
        assert_eq!(snap_direction(0.02, SNAP_VELOCITY), -1);
        assert_eq!(snap_direction(0.05, 100.0), 0);
    }

    #[test]
    /// Verifies a full-page snap is slow enough to remain visually trackable.
    fn pager_snap_duration_is_deliberate() {
        assert!(TRANSITION_DURATION >= Duration::from_millis(320));
    }

    #[test]
    /// Verifies left and right transitions are linear mirror images.
    fn pager_motion_is_linear_and_mirrored() {
        let started = Instant::now();
        let left = Transition {
            from: 0,
            to: 1,
            start_offset: 0.0,
            end_offset: -1.0,
            started,
            duration: TRANSITION_DURATION,
        };
        let right = Transition {
            from: 1,
            to: 0,
            start_offset: 0.0,
            end_offset: 1.0,
            started,
            duration: TRANSITION_DURATION,
        };

        assert_eq!(transition_offset_at(&left, 0.5), -0.5);
        assert_eq!(transition_offset_at(&right, 0.5), 0.5);
        assert_eq!(left.duration, right.duration);
        assert_eq!(snap_duration(-0.25, -1.0), Duration::from_millis(270));
    }

    #[test]
    /// Verifies navigation creates a translation-only snap between adjacent pages.
    fn pager_navigation_uses_normalized_horizontal_offsets() {
        let mut state = state_with_entries(3);

        let _ = navigate_from_offset(&mut state, 1, -0.25);

        assert_eq!(state.selected, 1);
        let transition = state.transition.expect("navigation starts a snap");
        assert_eq!(transition.from, 0);
        assert_eq!(transition.to, 1);
        assert_eq!(transition.start_offset, -0.25);
        assert_eq!(transition.end_offset, -1.0);
    }

    #[test]
    /// Verifies a live pointer drag follows the cursor and snaps to the adjacent page.
    fn pointer_drag_tracks_and_snaps_horizontally() {
        let mut state = state_with_entries(3);
        state.pager_width = 1000.0;
        let started = Instant::now();

        start_pager_drag(&mut state, 500.0, started);
        update_pager_drag(&mut state, 300.0, started + Duration::from_millis(200));
        assert_eq!(state.pager_offset, -0.2);

        let _ = finish_pager_drag(&mut state, started + Duration::from_millis(210));

        assert_eq!(state.selected, 1);
        let transition = state.transition.expect("release starts a snap");
        assert_eq!(transition.start_offset, -0.2);
        assert_eq!(transition.end_offset, -1.0);
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
        assert_eq!(state.visible_count, PAGE_BATCH);
        assert_eq!(state.selected, 0);
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
    /// Verifies rapid language changes coalesce into a final persistence task.
    fn locale_persistence_keeps_the_latest_selection() {
        let _locale_guard = crate::resources::lock_locale_tests();
        let mut state = state_with_entries(0);
        state.paths = Some(temporary_paths("locale-save"));

        let first = update(&mut state, Message::SetLocale(Locale::SimplifiedChinese));
        assert!(state.locale_save_in_flight);
        let second = update(&mut state, Message::SetLocale(Locale::English));
        assert!(state.locale_save_pending);
        drop((first, second));

        let final_save = update(&mut state, Message::LocaleSaved(Ok(())));
        assert!(state.locale_save_in_flight);
        assert!(!state.locale_save_pending);
        assert_eq!(state.settings.locale, Some(Locale::English));
        drop(final_save);
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
