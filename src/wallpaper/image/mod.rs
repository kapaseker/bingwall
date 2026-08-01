//! Owns original image and Wallpaper Preview acquisition and recovery policy.

use std::{io, path::PathBuf, time::Duration};

use thiserror::Error;

mod cache;

use crate::{feed::WallpaperEntry, paths::AppPaths};

/// Reports failures while acquiring an original image or Wallpaper Preview.
#[derive(Debug, Error)]
pub(crate) enum ImageAcquisitionError {
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("could not save the image: {0}")]
    SaveImage(#[source] io::Error),
    #[error("downloaded data is not a supported image: {0}")]
    DecodeImage(#[from] image::ImageError),
    #[error("background image task failed: {0}")]
    BackgroundTask(String),
    #[cfg(test)]
    #[error("{0}")]
    Operation(String),
}

/// Supplies external image-cache and download operations to acquisition policy.
trait ImageRuntime {
    /// Returns a cached original only when it is valid.
    async fn valid_original(
        &mut self,
        entry: &WallpaperEntry,
    ) -> Result<Option<PathBuf>, ImageAcquisitionError>;

    /// Returns a cached Wallpaper Preview only when it is valid.
    async fn valid_preview(
        &mut self,
        entry: &WallpaperEntry,
    ) -> Result<Option<PathBuf>, ImageAcquisitionError>;

    /// Reports whether the original destination currently exists.
    async fn original_exists(
        &mut self,
        path: &std::path::Path,
    ) -> Result<bool, ImageAcquisitionError>;

    /// Returns the destination path for an original image.
    fn original_path(&self, entry: &WallpaperEntry) -> PathBuf;

    /// Returns the destination path for a Wallpaper Preview.
    fn preview_path(&self, entry: &WallpaperEntry) -> PathBuf;

    /// Generates a Wallpaper Preview from an original image.
    async fn generate_preview(
        &mut self,
        original: &std::path::Path,
        preview: &std::path::Path,
    ) -> Result<(), ImageAcquisitionError>;

    /// Removes an unusable cached original before recovery download.
    async fn remove_original(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), ImageAcquisitionError>;

    /// Fetches one copy of the remote image bytes.
    async fn fetch(&mut self, url: &str) -> Result<Vec<u8>, ImageAcquisitionError>;

    /// Validates and atomically stores downloaded original bytes.
    async fn store_original(
        &mut self,
        destination: &std::path::Path,
        bytes: Vec<u8>,
    ) -> Result<(), ImageAcquisitionError>;

    /// Waits between bounded retry attempts.
    async fn delay(&mut self, duration: Duration);
}

/// Acquires an original image through the supplied runtime seam.
async fn acquire_original_with<R: ImageRuntime>(
    runtime: &mut R,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ImageAcquisitionError> {
    if let Some(cached) = runtime.valid_original(entry).await? {
        return Ok(cached);
    }
    download_original_with(runtime, entry).await
}

/// Downloads an original with bounded retries and stores it atomically.
async fn download_original_with<R: ImageRuntime>(
    runtime: &mut R,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ImageAcquisitionError> {
    let mut attempt = 0;
    let bytes = loop {
        match runtime.fetch(&entry.image_url).await {
            Ok(bytes) => break bytes,
            Err(_) if attempt < 2 => {
                attempt += 1;
                runtime
                    .delay(Duration::from_millis(200 * attempt as u64))
                    .await;
            }
            Err(error) => return Err(error),
        }
    };
    let destination = runtime.original_path(entry);
    runtime
        .store_original(&destination, bytes)
        .await
        .map(|()| destination)
}

/// Acquires a Wallpaper Preview through the supplied runtime seam.
async fn acquire_preview_with<R: ImageRuntime>(
    runtime: &mut R,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ImageAcquisitionError> {
    if let Some(cached) = runtime.valid_preview(entry).await? {
        return Ok(cached);
    }
    let original = runtime.original_path(entry);
    let original_exists = runtime.original_exists(&original).await?;
    let original = if original_exists {
        original
    } else {
        download_original_with(runtime, entry).await?
    };
    let preview = runtime.preview_path(entry);
    match generate_preview_with(runtime, &original, &preview).await {
        Ok(()) => Ok(preview),
        Err(_) if original_exists => {
            let _ = runtime.remove_original(&original).await;
            let original = download_original_with(runtime, entry).await?;
            generate_preview_with(runtime, &original, &preview).await?;
            Ok(preview)
        }
        Err(error) => Err(error),
    }
}

/// Generates a Wallpaper Preview with bounded retries for transient decode failures.
async fn generate_preview_with<R: ImageRuntime>(
    runtime: &mut R,
    original: &std::path::Path,
    preview: &std::path::Path,
) -> Result<(), ImageAcquisitionError> {
    let mut attempt = 0;
    loop {
        match runtime.generate_preview(original, preview).await {
            Ok(()) => return Ok(()),
            Err(_) if attempt < 2 => {
                attempt += 1;
                runtime
                    .delay(Duration::from_millis(200 * attempt as u64))
                    .await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Connects image-acquisition policy to reqwest and the persistent image cache.
struct SystemRuntime<'a> {
    client: &'a reqwest::Client,
    paths: &'a AppPaths,
}

impl ImageRuntime for SystemRuntime<'_> {
    /// Validates a cached original off the async executor.
    async fn valid_original(
        &mut self,
        entry: &WallpaperEntry,
    ) -> Result<Option<PathBuf>, ImageAcquisitionError> {
        let paths = self.paths.clone();
        let entry = entry.clone();
        tokio::task::spawn_blocking(move || cache::valid_image_path(&paths, &entry))
            .await
            .map_err(background_error)
    }

    /// Validates a cached Wallpaper Preview off the async executor.
    async fn valid_preview(
        &mut self,
        entry: &WallpaperEntry,
    ) -> Result<Option<PathBuf>, ImageAcquisitionError> {
        let paths = self.paths.clone();
        let entry = entry.clone();
        tokio::task::spawn_blocking(move || cache::valid_preview_path(&paths, &entry))
            .await
            .map_err(background_error)
    }

    /// Checks original-file existence off the async executor.
    async fn original_exists(
        &mut self,
        path: &std::path::Path,
    ) -> Result<bool, ImageAcquisitionError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || path.exists())
            .await
            .map_err(background_error)
    }

    /// Returns the stable cache path for an original image.
    fn original_path(&self, entry: &WallpaperEntry) -> PathBuf {
        cache::image_path(self.paths, entry)
    }

    /// Returns the stable cache path for a Wallpaper Preview.
    fn preview_path(&self, entry: &WallpaperEntry) -> PathBuf {
        cache::preview_path(self.paths, entry)
    }

    /// Generates a Wallpaper Preview off the async executor.
    async fn generate_preview(
        &mut self,
        original: &std::path::Path,
        preview: &std::path::Path,
    ) -> Result<(), ImageAcquisitionError> {
        let original = original.to_path_buf();
        let preview = preview.to_path_buf();
        tokio::task::spawn_blocking(move || cache::generate_preview(&original, &preview))
            .await
            .map_err(background_error)?
            .map_err(ImageAcquisitionError::DecodeImage)
    }

    /// Removes an unusable original off the async executor.
    async fn remove_original(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), ImageAcquisitionError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || std::fs::remove_file(path))
            .await
            .map_err(background_error)?
            .map_err(ImageAcquisitionError::SaveImage)
    }

    /// Fetches one remote image attempt.
    async fn fetch(&mut self, url: &str) -> Result<Vec<u8>, ImageAcquisitionError> {
        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(ImageAcquisitionError::Network)
    }

    /// Validates and atomically stores downloaded bytes off the async executor.
    async fn store_original(
        &mut self,
        destination: &std::path::Path,
        bytes: Vec<u8>,
    ) -> Result<(), ImageAcquisitionError> {
        let destination = destination.to_path_buf();
        tokio::task::spawn_blocking(move || {
            image::load_from_memory(&bytes)?;
            cache::write_image_atomically(&destination, &bytes)
                .map_err(ImageAcquisitionError::SaveImage)
        })
        .await
        .map_err(background_error)??;
        Ok(())
    }

    /// Waits between retry attempts without blocking the executor.
    async fn delay(&mut self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Converts a failed blocking task into an image-acquisition error.
fn background_error(error: tokio::task::JoinError) -> ImageAcquisitionError {
    ImageAcquisitionError::BackgroundTask(error.to_string())
}

/// Returns a valid original image, downloading and validating it when necessary.
pub(crate) async fn original(
    client: &reqwest::Client,
    paths: &AppPaths,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ImageAcquisitionError> {
    acquire_original_with(&mut SystemRuntime { client, paths }, entry).await
}

/// Returns a valid Wallpaper Preview generated from a cached or downloaded original.
pub(crate) async fn preview(
    client: &reqwest::Client,
    paths: &AppPaths,
    entry: &WallpaperEntry,
) -> Result<PathBuf, ImageAcquisitionError> {
    acquire_preview_with(&mut SystemRuntime { client, paths }, entry).await
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        ValidOriginal,
        ValidPreview,
        OriginalExists,
        GeneratePreview,
        RemoveOriginal,
        Fetch,
        StoreOriginal,
        Delay(Duration),
    }

    struct MemoryRuntime {
        cached_original: Option<PathBuf>,
        cached_preview: Option<PathBuf>,
        original: PathBuf,
        preview: PathBuf,
        original_exists: bool,
        fetch_results: VecDeque<Result<Vec<u8>, ImageAcquisitionError>>,
        generation_results: VecDeque<Result<(), ImageAcquisitionError>>,
        calls: Vec<Call>,
    }

    impl MemoryRuntime {
        /// Creates a deterministic runtime with empty caches and no operation outcomes.
        fn new() -> Self {
            Self {
                cached_original: None,
                cached_preview: None,
                original: PathBuf::from("/cache/original.jpg"),
                preview: PathBuf::from("/cache/preview.jpg"),
                original_exists: false,
                fetch_results: VecDeque::new(),
                generation_results: VecDeque::new(),
                calls: Vec::new(),
            }
        }
    }

    impl ImageRuntime for MemoryRuntime {
        /// Returns the configured in-memory original cache result.
        async fn valid_original(
            &mut self,
            _entry: &WallpaperEntry,
        ) -> Result<Option<PathBuf>, ImageAcquisitionError> {
            self.calls.push(Call::ValidOriginal);
            Ok(self.cached_original.clone())
        }

        /// Returns the configured in-memory Wallpaper Preview cache result.
        async fn valid_preview(
            &mut self,
            _entry: &WallpaperEntry,
        ) -> Result<Option<PathBuf>, ImageAcquisitionError> {
            self.calls.push(Call::ValidPreview);
            Ok(self.cached_preview.clone())
        }

        /// Returns whether the configured original exists.
        async fn original_exists(
            &mut self,
            _path: &std::path::Path,
        ) -> Result<bool, ImageAcquisitionError> {
            self.calls.push(Call::OriginalExists);
            Ok(self.original_exists)
        }

        /// Returns the configured original destination.
        fn original_path(&self, _entry: &WallpaperEntry) -> PathBuf {
            self.original.clone()
        }

        /// Returns the configured Wallpaper Preview destination.
        fn preview_path(&self, _entry: &WallpaperEntry) -> PathBuf {
            self.preview.clone()
        }

        /// Returns the next configured preview-generation outcome.
        async fn generate_preview(
            &mut self,
            _original: &std::path::Path,
            _preview: &std::path::Path,
        ) -> Result<(), ImageAcquisitionError> {
            self.calls.push(Call::GeneratePreview);
            self.generation_results
                .pop_front()
                .expect("generation result")
        }

        /// Records removal of an unusable cached original.
        async fn remove_original(
            &mut self,
            _path: &std::path::Path,
        ) -> Result<(), ImageAcquisitionError> {
            self.calls.push(Call::RemoveOriginal);
            Ok(())
        }

        /// Returns the next configured fetch outcome.
        async fn fetch(&mut self, _url: &str) -> Result<Vec<u8>, ImageAcquisitionError> {
            self.calls.push(Call::Fetch);
            self.fetch_results.pop_front().expect("fetch result")
        }

        /// Records storing downloaded original bytes.
        async fn store_original(
            &mut self,
            _destination: &std::path::Path,
            _bytes: Vec<u8>,
        ) -> Result<(), ImageAcquisitionError> {
            self.calls.push(Call::StoreOriginal);
            Ok(())
        }

        /// Records a retry delay without waiting in real time.
        async fn delay(&mut self, duration: Duration) {
            self.calls.push(Call::Delay(duration));
        }
    }

    /// Builds a deterministic Wallpaper Entry for acquisition tests.
    fn entry() -> WallpaperEntry {
        WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Wallpaper".into(),
            image_url: "https://cn.bing.com/wallpaper.jpg".into(),
        }
    }

    /// Creates isolated application paths for real cache-adapter tests.
    fn temporary_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "bingwall-image-{label}-{}",
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

    #[test]
    /// Verifies a valid cached original is returned without further acquisition work.
    fn valid_original_cache_is_used_directly() {
        let cached = PathBuf::from("/cache/original.jpg");
        let mut runtime = MemoryRuntime::new();
        runtime.cached_original = Some(cached.clone());

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_original_with(&mut runtime, &entry()))
            .unwrap();

        assert_eq!(result, cached);
        assert_eq!(runtime.calls, vec![Call::ValidOriginal]);
    }

    #[test]
    /// Verifies an uncached original retries twice before storing a successful download.
    fn original_download_is_bounded_and_retried() {
        let mut runtime = MemoryRuntime::new();
        runtime.fetch_results = VecDeque::from([
            Err(ImageAcquisitionError::Operation("offline".into())),
            Err(ImageAcquisitionError::Operation("timeout".into())),
            Ok(vec![1, 2, 3]),
        ]);

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_original_with(&mut runtime, &entry()))
            .unwrap();

        assert_eq!(result, PathBuf::from("/cache/original.jpg"));
        assert_eq!(
            runtime.calls,
            vec![
                Call::ValidOriginal,
                Call::Fetch,
                Call::Delay(Duration::from_millis(200)),
                Call::Fetch,
                Call::Delay(Duration::from_millis(400)),
                Call::Fetch,
                Call::StoreOriginal,
            ]
        );
    }

    #[test]
    /// Verifies an original download returns the third failure without attempting a cache write.
    fn original_download_stops_after_three_failures() {
        let mut runtime = MemoryRuntime::new();
        runtime.fetch_results = VecDeque::from([
            Err(ImageAcquisitionError::Operation("offline".into())),
            Err(ImageAcquisitionError::Operation("timeout".into())),
            Err(ImageAcquisitionError::Operation("final".into())),
        ]);

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_original_with(&mut runtime, &entry()))
            .unwrap_err();

        assert_eq!(error.to_string(), "final");
        assert_eq!(
            runtime.calls,
            vec![
                Call::ValidOriginal,
                Call::Fetch,
                Call::Delay(Duration::from_millis(200)),
                Call::Fetch,
                Call::Delay(Duration::from_millis(400)),
                Call::Fetch,
            ]
        );
    }

    #[test]
    /// Verifies a valid cached Wallpaper Preview is returned without original-image work.
    fn valid_preview_cache_is_used_directly() {
        let cached = PathBuf::from("/cache/preview.jpg");
        let mut runtime = MemoryRuntime::new();
        runtime.cached_preview = Some(cached.clone());

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_preview_with(&mut runtime, &entry()))
            .unwrap();

        assert_eq!(result, cached);
        assert_eq!(runtime.calls, vec![Call::ValidPreview]);
    }

    #[test]
    /// Verifies a cached original can generate a missing Wallpaper Preview.
    fn cached_original_generates_preview() {
        let mut runtime = MemoryRuntime::new();
        runtime.original_exists = true;
        runtime.generation_results.push_back(Ok(()));

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_preview_with(&mut runtime, &entry()))
            .unwrap();

        assert_eq!(result, PathBuf::from("/cache/preview.jpg"));
        assert_eq!(
            runtime.calls,
            vec![
                Call::ValidPreview,
                Call::OriginalExists,
                Call::GeneratePreview,
            ]
        );
    }

    #[test]
    /// Verifies a repeatedly invalid cached original is replaced before preview generation retries.
    fn invalid_cached_original_is_replaced() {
        let mut runtime = MemoryRuntime::new();
        runtime.original_exists = true;
        runtime.generation_results = VecDeque::from([
            Err(ImageAcquisitionError::Operation("decode 1".into())),
            Err(ImageAcquisitionError::Operation("decode 2".into())),
            Err(ImageAcquisitionError::Operation("decode 3".into())),
            Ok(()),
        ]);
        runtime.fetch_results.push_back(Ok(vec![1, 2, 3]));

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acquire_preview_with(&mut runtime, &entry()))
            .unwrap();

        assert_eq!(result, PathBuf::from("/cache/preview.jpg"));
        assert_eq!(
            runtime.calls,
            vec![
                Call::ValidPreview,
                Call::OriginalExists,
                Call::GeneratePreview,
                Call::Delay(Duration::from_millis(200)),
                Call::GeneratePreview,
                Call::Delay(Duration::from_millis(400)),
                Call::GeneratePreview,
                Call::RemoveOriginal,
                Call::Fetch,
                Call::StoreOriginal,
                Call::GeneratePreview,
            ]
        );
    }

    #[test]
    /// Verifies the real adapter never substitutes a Wallpaper Preview for a cached original.
    fn real_adapter_returns_original_for_wallpaper_application() {
        let paths = temporary_paths("original");
        let entry = entry();
        let original_path = cache::image_path(&paths, &entry);
        let preview_path = cache::preview_path(&paths, &entry);
        std::fs::create_dir_all(paths.images_dir()).unwrap();
        image::DynamicImage::new_rgb8(1, 1)
            .save(&original_path)
            .unwrap();
        std::fs::create_dir_all(paths.previews_dir()).unwrap();
        image::DynamicImage::new_rgb8(cache::PREVIEW_WIDTH, cache::PREVIEW_HEIGHT)
            .save(&preview_path)
            .unwrap();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(original(&reqwest::Client::new(), &paths, &entry))
            .unwrap();

        assert_eq!(result, original_path);
        assert_ne!(result, preview_path);
        std::fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }

    #[test]
    /// Verifies the real adapter generates a Wallpaper Preview from a cached original.
    fn real_adapter_generates_preview_without_network() {
        let paths = temporary_paths("preview");
        let entry = entry();
        let original_path = cache::image_path(&paths, &entry);
        std::fs::create_dir_all(paths.images_dir()).unwrap();
        image::DynamicImage::new_rgb8(3840, 2160)
            .save(&original_path)
            .unwrap();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(preview(&reqwest::Client::new(), &paths, &entry))
            .unwrap();

        assert_eq!(result, cache::preview_path(&paths, &entry));
        assert_eq!(cache::valid_preview_path(&paths, &entry), Some(result));
        std::fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }
}
