use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::resources::Locale;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub daily_change: bool,
    pub applied_image: Option<String>,
    pub last_update_status: Option<String>,
    pub locale: Option<Locale>,
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
    /// Loads settings from disk, returning safe defaults when the file does not exist.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).map_err(SettingsError::Parse),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(SettingsError::Read(error)),
        }
    }

    /// Serializes settings and atomically replaces the persisted settings file.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let parent = path.parent().expect("settings path has a parent");
        fs::create_dir_all(parent).map_err(SettingsError::Write)?;
        let data = serde_json::to_vec_pretty(self).map_err(SettingsError::Encode)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, data).map_err(SettingsError::Write)?;
        fs::rename(temporary, path).map_err(SettingsError::Write)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    /// Verifies a missing settings file produces disabled default settings.
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
    /// Verifies settings retain their values after being saved and loaded.
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
            locale: Some(Locale::SimplifiedChinese),
        };

        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains(r#""locale": "simplified_chinese""#)
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    /// Verifies obsolete fields do not prevent older settings files from loading.
    fn legacy_recent_image_list_is_ignored() {
        let value = br#"{
            "daily_change": true,
            "applied_image": "wall.jpg",
            "last_update_status": "ok",
            "recent_images": ["old.jpg"]
        }"#;

        let settings: Settings = serde_json::from_slice(value).unwrap();

        assert!(settings.daily_change);
        assert_eq!(settings.applied_image.as_deref(), Some("wall.jpg"));
        assert_eq!(settings.last_update_status.as_deref(), Some("ok"));
        assert_eq!(settings.locale, None);
    }
}
