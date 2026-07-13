use std::{
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use image::{DynamicImage, GenericImageView, ImageDecoder, ImageReader, codecs::jpeg::JpegEncoder};
use thiserror::Error;

use crate::{feed::WallpaperEntry, paths::AppPaths};

pub const PREVIEW_WIDTH: u32 = 1920;
pub const PREVIEW_HEIGHT: u32 = 1080;
pub const PREVIEW_JPEG_QUALITY: u8 = 80;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("could not read cached feed: {0}")]
    ReadFeed(#[source] io::Error),
    #[error("could not decode cached feed: {0}")]
    DecodeFeed(#[source] serde_json::Error),
    #[error("could not encode cached feed: {0}")]
    EncodeFeed(#[source] serde_json::Error),
    #[error("could not write cached feed: {0}")]
    WriteFeed(#[source] io::Error),
}

/// Loads and deserializes the cached wallpaper feed.
pub fn load_feed(paths: &AppPaths) -> Result<Vec<WallpaperEntry>, CacheError> {
    let data = fs::read(paths.feed_file()).map_err(CacheError::ReadFeed)?;
    serde_json::from_slice(&data).map_err(CacheError::DecodeFeed)
}

/// Serializes the wallpaper feed and replaces the cache file atomically.
pub fn save_feed(paths: &AppPaths, entries: &[WallpaperEntry]) -> Result<(), CacheError> {
    fs::create_dir_all(&paths.cache_dir).map_err(CacheError::WriteFeed)?;
    let data = serde_json::to_vec(entries).map_err(CacheError::EncodeFeed)?;
    let destination = paths.feed_file();
    let temporary = destination.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(CacheError::WriteFeed)?;
    fs::rename(temporary, destination).map_err(CacheError::WriteFeed)
}

/// Builds the stable cache path for an entry's original image.
pub fn image_path(paths: &AppPaths, entry: &WallpaperEntry) -> PathBuf {
    hashed_image_path(paths.images_dir(), entry)
}

/// Builds the stable cache path for an entry's generated preview.
pub fn preview_path(paths: &AppPaths, entry: &WallpaperEntry) -> PathBuf {
    hashed_image_path(paths.previews_dir(), entry)
}

/// Derives a deterministic JPEG filename from an entry's image URL.
fn hashed_image_path(directory: PathBuf, entry: &WallpaperEntry) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    entry.image_url.hash(&mut hasher);
    directory.join(format!("{:016x}.jpg", hasher.finish()))
}

/// Returns the cached original only when it exists and can be decoded.
pub fn valid_image_path(paths: &AppPaths, entry: &WallpaperEntry) -> Option<PathBuf> {
    let path = image_path(paths, entry);
    (path.exists() && image::open(&path).is_ok()).then_some(path)
}

/// Returns a preview only when it has the expected dimensions, removing invalid files.
pub fn valid_preview_path(paths: &AppPaths, entry: &WallpaperEntry) -> Option<PathBuf> {
    let path = preview_path(paths, entry);
    let dimensions = ImageReader::open(&path)
        .and_then(|reader| reader.with_guessed_format())
        .ok()
        .and_then(|reader| reader.into_dimensions().ok());
    if dimensions == Some((PREVIEW_WIDTH, PREVIEW_HEIGHT)) {
        Some(path)
    } else {
        let _ = fs::remove_file(path);
        None
    }
}

/// Center-crops an original image to 16:9 and atomically writes a 1080p JPEG preview.
pub fn generate_preview(original: &Path, destination: &Path) -> Result<(), image::ImageError> {
    let mut decoder = ImageReader::open(original)?
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);

    let (width, height) = image.dimensions();
    let (crop_x, crop_y, crop_width, crop_height) =
        if width as u64 * PREVIEW_HEIGHT as u64 > height as u64 * PREVIEW_WIDTH as u64 {
            let crop_width = (height as u64 * PREVIEW_WIDTH as u64 / PREVIEW_HEIGHT as u64) as u32;
            ((width - crop_width) / 2, 0, crop_width, height)
        } else {
            let crop_height = (width as u64 * PREVIEW_HEIGHT as u64 / PREVIEW_WIDTH as u64) as u32;
            (0, (height - crop_height) / 2, width, crop_height)
        };
    let preview = image
        .crop_imm(crop_x, crop_y, crop_width, crop_height)
        .resize_exact(
            PREVIEW_WIDTH,
            PREVIEW_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );

    let temporary = prepare_atomic_write(destination)?;
    let result = (|| {
        let mut writer = BufWriter::new(File::create(&temporary)?);
        JpegEncoder::new_with_quality(&mut writer, PREVIEW_JPEG_QUALITY).encode_image(&preview)?;
        writer.flush()?;
        fs::rename(&temporary, destination)?;
        Ok::<_, image::ImageError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// Writes downloaded image bytes through a temporary file before replacing the destination.
pub fn write_image_atomically(destination: &Path, data: &[u8]) -> Result<(), io::Error> {
    let temporary = prepare_atomic_write(destination)?;
    fs::write(&temporary, data)?;
    fs::rename(temporary, destination)
}

/// Creates the parent directory and returns a process-unique temporary path.
fn prepare_atomic_write(destination: &Path) -> Result<PathBuf, io::Error> {
    let parent = destination.parent().expect("image path has a parent");
    fs::create_dir_all(parent)?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(destination.with_extension(format!("jpg.{}.{sequence}.tmp", std::process::id())))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// Creates isolated cache and configuration paths for a cache test.
    fn temporary_paths() -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "bingwall-cache-{}",
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
    /// Verifies feed persistence and deterministic, distinct image cache paths.
    fn feed_round_trips_and_image_names_are_stable() {
        let paths = temporary_paths();
        let entries = vec![WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Lake".into(),
            image_url: "https://cn.bing.com/lake.jpg".into(),
        }];

        save_feed(&paths, &entries).unwrap();
        assert_eq!(load_feed(&paths).unwrap(), entries);
        assert_eq!(
            image_path(&paths, &entries[0]),
            image_path(&paths, &entries[0])
        );
        assert_ne!(
            image_path(&paths, &entries[0]),
            preview_path(&paths, &entries[0])
        );

        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }

    #[test]
    /// Verifies previews are center-cropped and encoded at the required resolution.
    fn preview_is_center_cropped_and_encoded_at_1080p() {
        let paths = temporary_paths();
        let entry = WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Portrait".into(),
            image_url: "https://cn.bing.com/portrait.jpg".into(),
        };
        let original = image_path(&paths, &entry);
        let preview = preview_path(&paths, &entry);
        fs::create_dir_all(paths.images_dir()).unwrap();
        let mut source = image::RgbImage::new(160, 160);
        for (_x, y, pixel) in source.enumerate_pixels_mut() {
            *pixel = if y < 35 {
                image::Rgb([255, 0, 0])
            } else if y >= 125 {
                image::Rgb([0, 0, 255])
            } else {
                image::Rgb([0, 255, 0])
            };
        }
        source.save(&original).unwrap();

        generate_preview(&original, &preview).unwrap();

        assert_eq!(
            ImageReader::open(&preview)
                .unwrap()
                .into_dimensions()
                .unwrap(),
            (PREVIEW_WIDTH, PREVIEW_HEIGHT)
        );
        assert_eq!(valid_preview_path(&paths, &entry), Some(preview));
        let decoded = image::open(preview_path(&paths, &entry)).unwrap().to_rgb8();
        for y in [20, PREVIEW_HEIGHT / 2, PREVIEW_HEIGHT - 20] {
            let pixel = decoded.get_pixel(PREVIEW_WIDTH / 2, y);
            assert!(pixel[1] > pixel[0] && pixel[1] > pixel[2]);
        }
        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }

    #[test]
    /// Verifies incorrectly sized previews are rejected and deleted.
    fn invalid_preview_dimensions_are_rejected_and_removed() {
        let paths = temporary_paths();
        let entry = WallpaperEntry {
            date: "2026-01-01".into(),
            description: "Small".into(),
            image_url: "https://cn.bing.com/small.jpg".into(),
        };
        let preview = preview_path(&paths, &entry);
        fs::create_dir_all(paths.previews_dir()).unwrap();
        DynamicImage::new_rgb8(1, 1).save(&preview).unwrap();

        assert_eq!(valid_preview_path(&paths, &entry), None);
        assert!(!preview.exists());
        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }
}
