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

pub fn load_feed(paths: &AppPaths) -> Result<Vec<WallpaperEntry>, CacheError> {
    let data = fs::read(paths.feed_file()).map_err(CacheError::ReadFeed)?;
    serde_json::from_slice(&data).map_err(CacheError::DecodeFeed)
}

pub fn save_feed(paths: &AppPaths, entries: &[WallpaperEntry]) -> Result<(), CacheError> {
    fs::create_dir_all(&paths.cache_dir).map_err(CacheError::WriteFeed)?;
    let data = serde_json::to_vec(entries).map_err(CacheError::EncodeFeed)?;
    let destination = paths.feed_file();
    let temporary = destination.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(CacheError::WriteFeed)?;
    fs::rename(temporary, destination).map_err(CacheError::WriteFeed)
}

pub fn image_path(paths: &AppPaths, entry: &WallpaperEntry) -> PathBuf {
    hashed_image_path(paths.images_dir(), entry)
}

pub fn preview_path(paths: &AppPaths, entry: &WallpaperEntry) -> PathBuf {
    hashed_image_path(paths.previews_dir(), entry)
}

fn hashed_image_path(directory: PathBuf, entry: &WallpaperEntry) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    entry.image_url.hash(&mut hasher);
    directory.join(format!("{:016x}.jpg", hasher.finish()))
}

pub fn valid_image_path(paths: &AppPaths, entry: &WallpaperEntry) -> Option<PathBuf> {
    let path = image_path(paths, entry);
    (path.exists() && image::open(&path).is_ok()).then_some(path)
}

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

pub fn write_image_atomically(destination: &Path, data: &[u8]) -> Result<(), io::Error> {
    let temporary = prepare_atomic_write(destination)?;
    fs::write(&temporary, data)?;
    fs::rename(temporary, destination)
}

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
        DynamicImage::new_rgb8(1200, 1600).save(&original).unwrap();

        generate_preview(&original, &preview).unwrap();

        assert_eq!(
            ImageReader::open(&preview)
                .unwrap()
                .into_dimensions()
                .unwrap(),
            (PREVIEW_WIDTH, PREVIEW_HEIGHT)
        );
        assert_eq!(valid_preview_path(&paths, &entry), Some(preview));
        fs::remove_dir_all(paths.cache_dir.parent().unwrap()).unwrap();
    }

    #[test]
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
