use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub daily_change: bool,
    pub applied_image: Option<String>,
    pub last_update_status: Option<String>,
    pub recent_images: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not read settings: {0}")]
    Read(#[source] io::Error),
    #[error("could not parse settings: {0}")]
    Parse(#[source] serde_json::Error),
    #[error("could not write settings: {0}")]
    Write(#[source] io::Error),
    #[error("could not encode settings: {0}")]
    Encode(#[source] serde_json::Error),
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).map_err(SettingsError::Parse),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(SettingsError::Read(error)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let parent = path.parent().expect("settings path has a parent");
        fs::create_dir_all(parent).map_err(SettingsError::Write)?;
        let data = serde_json::to_vec_pretty(self).map_err(SettingsError::Encode)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, data).map_err(SettingsError::Write)?;
        fs::rename(temporary, path).map_err(SettingsError::Write)
    }

    pub fn remember_image(&mut self, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        self.recent_images.retain(|recent| recent != &path);
        self.recent_images.insert(0, path);
        self.recent_images.truncate(20);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn missing_settings_are_safe_and_disabled() {
        let path = std::env::temp_dir().join(format!(
            "bingwall-missing-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        assert!(!Settings::default().daily_change);
    }

    #[test]
    fn settings_round_trip() {
        let root = std::env::temp_dir().join(format!(
            "bingwall-settings-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("settings.json");
        let settings = Settings {
            daily_change: true,
            applied_image: Some("wall.jpg".into()),
            last_update_status: Some("ok".into()),
            recent_images: vec!["recent.jpg".into()],
        };

        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_twenty_unique_recent_images() {
        let mut settings = Settings::default();
        for index in 0..25 {
            settings.remember_image(Path::new(&format!("{index}.jpg")));
        }
        settings.remember_image(Path::new("20.jpg"));

        assert_eq!(settings.recent_images.len(), 20);
        assert_eq!(settings.recent_images[0], "20.jpg");
        assert_eq!(
            settings
                .recent_images
                .iter()
                .filter(|path| *path == "20.jpg")
                .count(),
            1
        );
    }
}
