use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    feed::WallpaperEntry, image_acquisition, paths::AppPaths, platform::Desktop, service,
    settings::Settings, systemd,
};

/// Reports failures from applying wallpapers or configuring Daily Change.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WallpaperError {
    #[error("{0}")]
    Operation(String),
    #[error("daily change is disabled")]
    DailyChangeDisabled,
    #[error("the wallpaper feed is empty")]
    EmptyFeed,
}

/// Supplies external operations required by the wallpaper workflows.
trait WallpaperRuntime {
    async fn load_settings(&mut self) -> Result<Settings, WallpaperError>;
    async fn save_settings(&mut self, settings: &Settings) -> Result<(), WallpaperError>;
    async fn refresh_current(&mut self) -> Result<WallpaperEntry, WallpaperError>;
    async fn acquire_original(&mut self, entry: &WallpaperEntry)
    -> Result<PathBuf, WallpaperError>;
    async fn apply(&mut self, path: &Path) -> Result<(), WallpaperError>;
    async fn enable_daily_change(&mut self) -> Result<(), WallpaperError>;
    async fn disable_daily_change(&mut self) -> Result<(), WallpaperError>;
}

/// Applies a Selected Wallpaper through the supplied runtime seam.
async fn apply_selected_with<R: WallpaperRuntime>(
    runtime: &mut R,
    entry: WallpaperEntry,
) -> Result<Settings, WallpaperError> {
    let image = runtime.acquire_original(&entry).await?;
    runtime.apply(&image).await?;
    let mut settings = runtime.load_settings().await?;
    settings.applied_image = Some(image.to_string_lossy().into_owned());
    settings.last_update_status = Some(format!("Updated to {}", entry.date));
    runtime.save_settings(&settings).await?;
    Ok(settings)
}

/// Changes Daily Change through the supplied runtime seam.
async fn set_daily_change_with<R: WallpaperRuntime>(
    runtime: &mut R,
    enabled: bool,
    current: Option<WallpaperEntry>,
) -> Result<Settings, WallpaperError> {
    let mut settings = runtime.load_settings().await?;
    if enabled {
        let entry = current.ok_or(WallpaperError::EmptyFeed)?;
        let image = runtime.acquire_original(&entry).await?;
        runtime.apply(&image).await?;
        runtime.enable_daily_change().await?;
        settings.applied_image = Some(image.to_string_lossy().into_owned());
        settings.last_update_status = Some(format!("Updated to {}", entry.date));
    } else {
        runtime.disable_daily_change().await?;
    }
    settings.daily_change = enabled;
    runtime.save_settings(&settings).await?;
    Ok(settings)
}

/// Runs one unattended Wallpaper Update through the supplied runtime seam.
async fn run_scheduled_with<R: WallpaperRuntime>(
    runtime: &mut R,
) -> Result<PathBuf, WallpaperError> {
    let result = run_scheduled_inner(runtime).await;
    if let Err(error) = &result {
        record_failure(runtime, error).await;
    }
    result
}

/// Performs one unattended update before failure recording is applied.
async fn run_scheduled_inner<R: WallpaperRuntime>(
    runtime: &mut R,
) -> Result<PathBuf, WallpaperError> {
    let mut settings = runtime.load_settings().await?;
    if !settings.daily_change {
        return Err(WallpaperError::DailyChangeDisabled);
    }
    let current = runtime.refresh_current().await?;
    let image = runtime.acquire_original(&current).await?;
    runtime.apply(&image).await?;
    settings.applied_image = Some(image.to_string_lossy().into_owned());
    settings.last_update_status = Some(format!("Updated to {}", current.date));
    runtime.save_settings(&settings).await?;
    Ok(image)
}

/// Best-effort records an unattended update failure without replacing its original error.
async fn record_failure<R: WallpaperRuntime>(runtime: &mut R, error: &WallpaperError) {
    let Ok(mut settings) = runtime.load_settings().await else {
        return;
    };
    settings.last_update_status = Some(format!("Update failed: {error}"));
    let _ = runtime.save_settings(&settings).await;
}

/// Connects wallpaper workflows to the local filesystem, desktop, network, and systemd.
struct SystemRuntime {
    desktop: Desktop,
    paths: AppPaths,
    client: reqwest::Client,
}

impl WallpaperRuntime for SystemRuntime {
    /// Loads settings without blocking the async executor.
    async fn load_settings(&mut self) -> Result<Settings, WallpaperError> {
        let path = self.paths.settings_file();
        tokio::task::spawn_blocking(move || Settings::load(&path))
            .await
            .map_err(operation_error)?
            .map_err(operation_error)
    }

    /// Saves settings without blocking the async executor.
    async fn save_settings(&mut self, settings: &Settings) -> Result<(), WallpaperError> {
        let path = self.paths.settings_file();
        let settings = settings.clone();
        tokio::task::spawn_blocking(move || settings.save(&path))
            .await
            .map_err(operation_error)?
            .map_err(operation_error)
    }

    /// Refreshes the feed and returns its Current Wallpaper.
    async fn refresh_current(&mut self) -> Result<WallpaperEntry, WallpaperError> {
        service::refresh_feed(&self.client, &self.paths)
            .await
            .map_err(operation_error)?
            .0
            .into_iter()
            .next()
            .ok_or(WallpaperError::EmptyFeed)
    }

    /// Acquires the original image for a wallpaper entry.
    async fn acquire_original(
        &mut self,
        entry: &WallpaperEntry,
    ) -> Result<PathBuf, WallpaperError> {
        image_acquisition::original(&self.client, &self.paths, entry)
            .await
            .map_err(operation_error)
    }

    /// Applies an original image without blocking the async executor.
    async fn apply(&mut self, path: &Path) -> Result<(), WallpaperError> {
        let desktop = self.desktop;
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || desktop.apply(&path))
            .await
            .map_err(operation_error)?
            .map_err(operation_error)
    }

    /// Installs and enables the Daily Change timer without blocking the async executor.
    async fn enable_daily_change(&mut self) -> Result<(), WallpaperError> {
        let paths = self.paths.clone();
        tokio::task::spawn_blocking(move || systemd::enable(&paths))
            .await
            .map_err(operation_error)?
            .map_err(operation_error)
    }

    /// Disables the Daily Change timer without blocking the async executor.
    async fn disable_daily_change(&mut self) -> Result<(), WallpaperError> {
        tokio::task::spawn_blocking(systemd::disable)
            .await
            .map_err(operation_error)?
            .map_err(operation_error)
    }
}

/// Converts an external operation failure into the workflow's stable error surface.
fn operation_error(error: impl ToString) -> WallpaperError {
    WallpaperError::Operation(error.to_string())
}

/// Applies a user-selected wallpaper and persists its metadata.
pub(crate) async fn apply_selected(
    desktop: Desktop,
    paths: AppPaths,
    client: reqwest::Client,
    entry: WallpaperEntry,
) -> Result<Settings, WallpaperError> {
    let mut runtime = SystemRuntime {
        desktop,
        paths,
        client,
    };
    apply_selected_with(&mut runtime, entry).await
}

/// Enables or disables Daily Change while preserving the current application behavior.
pub(crate) async fn set_daily_change(
    enabled: bool,
    desktop: Desktop,
    paths: AppPaths,
    client: reqwest::Client,
    current: Option<WallpaperEntry>,
) -> Result<Settings, WallpaperError> {
    let mut runtime = SystemRuntime {
        desktop,
        paths,
        client,
    };
    set_daily_change_with(&mut runtime, enabled, current).await
}

/// Runs one systemd-compatible unattended Wallpaper Update.
pub async fn run_scheduled_update() -> Result<PathBuf, WallpaperError> {
    let paths = AppPaths::discover().map_err(operation_error)?;
    let desktop = match Desktop::detect() {
        Ok(desktop) => desktop,
        Err(error) => {
            let error = operation_error(error);
            record_system_failure(&paths, &error).await;
            return Err(error);
        }
    };
    let mut runtime = SystemRuntime {
        desktop,
        paths,
        client: reqwest::Client::new(),
    };
    run_scheduled_with(&mut runtime).await
}

/// Best-effort records a startup failure that occurs before a system runtime can be built.
async fn record_system_failure(paths: &AppPaths, error: &WallpaperError) {
    let path = paths.settings_file();
    let status = format!("Update failed: {error}");
    let _ = tokio::task::spawn_blocking(move || {
        let mut settings = Settings::load(&path)?;
        settings.last_update_status = Some(status);
        settings.save(&path)
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        LoadSettings,
        SaveSettings,
        RefreshCurrent,
        Acquire(String),
        Apply(PathBuf),
        EnableDailyChange,
        DisableDailyChange,
    }

    struct MemoryRuntime {
        settings: Settings,
        current: WallpaperEntry,
        original: PathBuf,
        calls: Vec<Call>,
        refresh_error: Option<WallpaperError>,
    }

    impl MemoryRuntime {
        /// Creates an in-memory runtime with deterministic wallpaper data.
        fn new() -> Self {
            Self {
                settings: Settings::default(),
                current: entry("2026-01-02", "current"),
                original: PathBuf::from("/cache/original.jpg"),
                calls: Vec::new(),
                refresh_error: None,
            }
        }
    }

    impl WallpaperRuntime for MemoryRuntime {
        /// Returns the current in-memory settings.
        async fn load_settings(&mut self) -> Result<Settings, WallpaperError> {
            self.calls.push(Call::LoadSettings);
            Ok(self.settings.clone())
        }

        /// Persists settings in memory.
        async fn save_settings(&mut self, settings: &Settings) -> Result<(), WallpaperError> {
            self.calls.push(Call::SaveSettings);
            self.settings = settings.clone();
            Ok(())
        }

        /// Returns the configured Current Wallpaper.
        async fn refresh_current(&mut self) -> Result<WallpaperEntry, WallpaperError> {
            self.calls.push(Call::RefreshCurrent);
            if let Some(error) = self.refresh_error.clone() {
                return Err(error);
            }
            Ok(self.current.clone())
        }

        /// Returns the configured original image path.
        async fn acquire_original(
            &mut self,
            entry: &WallpaperEntry,
        ) -> Result<PathBuf, WallpaperError> {
            self.calls.push(Call::Acquire(entry.image_url.clone()));
            Ok(self.original.clone())
        }

        /// Records a desktop wallpaper application.
        async fn apply(&mut self, path: &Path) -> Result<(), WallpaperError> {
            self.calls.push(Call::Apply(path.to_path_buf()));
            Ok(())
        }

        /// Records enabling Daily Change.
        async fn enable_daily_change(&mut self) -> Result<(), WallpaperError> {
            self.calls.push(Call::EnableDailyChange);
            Ok(())
        }

        /// Records disabling Daily Change.
        async fn disable_daily_change(&mut self) -> Result<(), WallpaperError> {
            self.calls.push(Call::DisableDailyChange);
            Ok(())
        }
    }

    /// Creates a Wallpaper Entry with deterministic metadata.
    fn entry(date: &str, id: &str) -> WallpaperEntry {
        WallpaperEntry {
            date: date.to_owned(),
            description: id.to_owned(),
            image_url: format!("https://cn.bing.com/{id}.jpg"),
        }
    }

    #[test]
    /// Verifies manual application uses the Selected Wallpaper original and persists it.
    fn manual_application_uses_selected_wallpaper() {
        let mut runtime = MemoryRuntime::new();
        let selected = entry("2026-01-01", "selected");

        let settings = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(apply_selected_with(&mut runtime, selected.clone()))
            .unwrap();

        assert_eq!(
            runtime.calls,
            vec![
                Call::Acquire(selected.image_url),
                Call::Apply(PathBuf::from("/cache/original.jpg")),
                Call::LoadSettings,
                Call::SaveSettings,
            ]
        );
        assert_eq!(
            settings.applied_image.as_deref(),
            Some("/cache/original.jpg")
        );
        assert_eq!(
            settings.last_update_status.as_deref(),
            Some("Updated to 2026-01-01")
        );
    }

    #[test]
    /// Verifies enabling Daily Change applies the Current Wallpaper before enabling the timer.
    fn enabling_daily_change_applies_current_wallpaper() {
        let mut runtime = MemoryRuntime::new();
        let current = runtime.current.clone();

        let settings = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_daily_change_with(
                &mut runtime,
                true,
                Some(current.clone()),
            ))
            .unwrap();

        assert_eq!(
            runtime.calls,
            vec![
                Call::LoadSettings,
                Call::Acquire(current.image_url),
                Call::Apply(PathBuf::from("/cache/original.jpg")),
                Call::EnableDailyChange,
                Call::SaveSettings,
            ]
        );
        assert!(settings.daily_change);
        assert_eq!(
            settings.applied_image.as_deref(),
            Some("/cache/original.jpg")
        );
        assert_eq!(
            settings.last_update_status.as_deref(),
            Some("Updated to 2026-01-02")
        );
    }

    #[test]
    /// Verifies disabling Daily Change stops the timer without applying a wallpaper.
    fn disabling_daily_change_only_stops_the_timer() {
        let mut runtime = MemoryRuntime::new();
        runtime.settings.daily_change = true;

        let settings = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(set_daily_change_with(&mut runtime, false, None))
            .unwrap();

        assert_eq!(
            runtime.calls,
            vec![
                Call::LoadSettings,
                Call::DisableDailyChange,
                Call::SaveSettings,
            ]
        );
        assert!(!settings.daily_change);
    }

    #[test]
    /// Verifies an unattended Wallpaper Update applies the refreshed Current Wallpaper.
    fn scheduled_update_uses_refreshed_current_wallpaper() {
        let mut runtime = MemoryRuntime::new();
        runtime.settings.daily_change = true;
        let current = runtime.current.clone();

        let applied = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(run_scheduled_with(&mut runtime))
            .unwrap();

        assert_eq!(applied, PathBuf::from("/cache/original.jpg"));
        assert_eq!(
            runtime.calls,
            vec![
                Call::LoadSettings,
                Call::RefreshCurrent,
                Call::Acquire(current.image_url),
                Call::Apply(PathBuf::from("/cache/original.jpg")),
                Call::SaveSettings,
            ]
        );
        assert_eq!(
            runtime.settings.last_update_status.as_deref(),
            Some("Updated to 2026-01-02")
        );
    }

    #[test]
    /// Verifies an unattended failure is persisted before the original error is returned.
    fn scheduled_update_records_failure() {
        let mut runtime = MemoryRuntime::new();
        runtime.settings.daily_change = true;
        runtime.refresh_error = Some(WallpaperError::Operation("offline".into()));

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(run_scheduled_with(&mut runtime))
            .unwrap_err();

        assert_eq!(error, WallpaperError::Operation("offline".into()));
        assert_eq!(
            runtime.calls,
            vec![
                Call::LoadSettings,
                Call::RefreshCurrent,
                Call::LoadSettings,
                Call::SaveSettings,
            ]
        );
        assert_eq!(
            runtime.settings.last_update_status.as_deref(),
            Some("Update failed: offline")
        );
    }
}
