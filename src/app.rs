use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

use iced::{Point, Subscription, Task, keyboard, mouse, touch};

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

#[derive(Debug, Clone)]
pub(crate) struct Transition {
    pub from: usize,
    pub direction: f32,
    pub started: Instant,
}

#[derive(Debug)]
pub(crate) struct State {
    pub locale: Locale,
    pub desktop: Option<Desktop>,
    pub paths: Option<AppPaths>,
    pub settings: Settings,
    pub entries: Vec<WallpaperEntry>,
    pub visible_count: usize,
    pub selected: usize,
    pub images: HashMap<usize, PathBuf>,
    loading_images: HashSet<usize>,
    pub status: String,
    pub busy: bool,
    pub transition: Option<Transition>,
    last_wheel: Option<Instant>,
    touch_start: Option<(touch::Finger, Point)>,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Refresh,
    FeedLoaded(Result<(Vec<WallpaperEntry>, FeedOrigin), String>),
    ImageLoaded(usize, Result<PathBuf, String>),
    Previous,
    Next,
    SetWallpaper,
    Applied(Result<Settings, String>),
    ToggleDaily(bool),
    ToggleFinished(bool, Result<(Settings, Option<PathBuf>), String>),
    RuntimeEvent(iced::Event),
    AnimationTick(Instant),
}

pub fn run() -> iced::Result {
    iced::application(boot, update, ui::view)
        .title("Bingwall")
        .subscription(subscription)
        .window_size((1100.0, 760.0))
        .centered()
        .antialiasing(true)
        .run()
}

fn boot() -> (State, Task<Message>) {
    let locale = Locale::detect();
    let desktop = Desktop::detect().ok();
    if desktop.is_none() {
        return (
            State {
                locale,
                desktop,
                paths: None,
                settings: Settings::default(),
                entries: Vec::new(),
                visible_count: 0,
                selected: 0,
                images: HashMap::new(),
                loading_images: HashSet::new(),
                status: locale.text(TextKey::Unsupported).into(),
                busy: false,
                transition: None,
                last_wheel: None,
                touch_start: None,
            },
            Task::none(),
        );
    }

    let paths = match AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            return (
                State {
                    locale,
                    desktop,
                    paths: None,
                    settings: Settings::default(),
                    entries: Vec::new(),
                    visible_count: 0,
                    selected: 0,
                    images: HashMap::new(),
                    loading_images: HashSet::new(),
                    status: error.to_string(),
                    busy: false,
                    transition: None,
                    last_wheel: None,
                    touch_start: None,
                },
                Task::none(),
            );
        }
    };
    let settings = Settings::load(&paths.settings_file()).unwrap_or_default();
    let status = settings
        .last_update_status
        .clone()
        .unwrap_or_else(|| locale.text(TextKey::LoadingFeed).into());
    let mut state = State {
        locale,
        desktop,
        paths: Some(paths),
        settings,
        entries: Vec::new(),
        visible_count: 0,
        selected: 0,
        images: HashMap::new(),
        loading_images: HashSet::new(),
        status,
        busy: true,
        transition: None,
        last_wheel: None,
        touch_start: None,
    };
    let task = refresh_task(&mut state);
    (state, task)
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => refresh_task(state),
        Message::FeedLoaded(result) => {
            state.busy = false;
            match result {
                Ok((entries, origin)) => {
                    state.entries = entries;
                    state.selected = 0;
                    state.visible_count = state.entries.len().min(PAGE_BATCH);
                    state.images.clear();
                    state.loading_images.clear();
                    state.status = match origin {
                        FeedOrigin::Network => state.locale.text(TextKey::FeedRefreshed),
                        FeedOrigin::Cache => state.locale.text(TextKey::CachedFeed),
                    }
                    .into();
                    queue_neighbor_images(state)
                }
                Err(error) => {
                    state.status = error;
                    Task::none()
                }
            }
        }
        Message::ImageLoaded(index, result) => {
            state.loading_images.remove(&index);
            match result {
                Ok(path) => {
                    state.images.insert(index, path);
                }
                Err(error) if index == state.selected => state.status = error,
                Err(_) => {}
            }
            Task::none()
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
                Ok((settings, image)) => {
                    state.settings = settings;
                    if let Some(image) = image {
                        state.images.insert(0, image);
                    }
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

fn refresh_task(state: &mut State) -> Task<Message> {
    let Some(paths) = state.paths.clone() else {
        return Task::none();
    };
    state.busy = true;
    state.status = state.locale.text(TextKey::LoadingFeed).into();
    Task::perform(
        async move {
            service::refresh_feed(&reqwest::Client::new(), &paths)
                .await
                .map_err(|error| error.to_string())
        },
        Message::FeedLoaded,
    )
}

fn queue_neighbor_images(state: &mut State) -> Task<Message> {
    let Some(paths) = state.paths.clone() else {
        return Task::none();
    };
    let start = state.selected.saturating_sub(1);
    let end = (state.selected + 1).min(state.entries.len().saturating_sub(1));
    let mut tasks = Vec::new();
    for index in start..=end {
        if state.images.contains_key(&index) || !state.loading_images.insert(index) {
            continue;
        }
        let entry = state.entries[index].clone();
        let paths = paths.clone();
        tasks.push(Task::perform(
            async move {
                service::ensure_image(&reqwest::Client::new(), &paths, &entry)
                    .await
                    .map_err(|error| error.to_string())
            },
            move |result| Message::ImageLoaded(index, result),
        ));
    }
    Task::batch(tasks)
}

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
    if state.selected + 2 >= state.visible_count && state.visible_count < state.entries.len() {
        state.visible_count = (state.visible_count + PAGE_BATCH).min(state.entries.len());
    }
    state.transition = Some(Transition {
        from: previous,
        direction: direction.signum() as f32,
        started: Instant::now(),
    });
    queue_neighbor_images(state)
}

fn apply_selected_task(state: &mut State) -> Task<Message> {
    let (Some(desktop), Some(paths), Some(entry), Some(image)) = (
        state.desktop,
        state.paths.clone(),
        state.entries.get(state.selected).cloned(),
        state.images.get(&state.selected).cloned(),
    ) else {
        return Task::none();
    };
    state.busy = true;
    state.status = state.locale.text(TextKey::Working).into();
    Task::perform(
        async move { apply_wallpaper(desktop, paths, entry, image) },
        Message::Applied,
    )
}

fn toggle_daily_task(state: &mut State, enabled: bool) -> Task<Message> {
    let (Some(desktop), Some(paths)) = (state.desktop, state.paths.clone()) else {
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
        async move { set_daily_change(enabled, desktop, paths, current).await },
        move |result| Message::ToggleFinished(enabled, result),
    )
}

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
    settings.remember_image(&image);
    settings
        .save(&paths.settings_file())
        .map_err(|error| error.to_string())?;
    cache::prune_images(&paths, &settings).map_err(|error| error.to_string())?;
    Ok(settings)
}

async fn set_daily_change(
    enabled: bool,
    desktop: Desktop,
    paths: AppPaths,
    current: Option<WallpaperEntry>,
) -> Result<(Settings, Option<PathBuf>), String> {
    let mut settings = Settings::load(&paths.settings_file()).map_err(|error| error.to_string())?;
    let image = if enabled {
        let entry = current.ok_or_else(|| "the wallpaper feed is empty".to_owned())?;
        let image = service::ensure_image(&reqwest::Client::new(), &paths, &entry)
            .await
            .map_err(|error| error.to_string())?;
        desktop.apply(&image).map_err(|error| error.to_string())?;
        systemd::enable(&paths).map_err(|error| error.to_string())?;
        settings.applied_image = Some(image.to_string_lossy().into_owned());
        settings.last_update_status = Some(format!("Updated to {}", entry.date));
        settings.remember_image(&image);
        Some(image)
    } else {
        systemd::disable().map_err(|error| error.to_string())?;
        None
    };
    settings.daily_change = enabled;
    settings
        .save(&paths.settings_file())
        .map_err(|error| error.to_string())?;
    cache::prune_images(&paths, &settings).map_err(|error| error.to_string())?;
    Ok((settings, image))
}

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
    use super::*;

    fn state_with_entries(count: usize) -> State {
        State {
            locale: Locale::English,
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
            images: HashMap::new(),
            loading_images: HashSet::new(),
            status: String::new(),
            busy: false,
            transition: None,
            last_wheel: None,
            touch_start: None,
        }
    }

    #[test]
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
    fn pager_never_moves_outside_the_feed() {
        let mut state = state_with_entries(2);
        let _ = navigate(&mut state, -1);
        assert_eq!(state.selected, 0);
        let _ = navigate(&mut state, 1);
        let _ = navigate(&mut state, 1);
        assert_eq!(state.selected, 1);
    }
}
