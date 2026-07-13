use std::{env, path::Path, process::Command};

use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Cinnamon,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Bingwall supports only GNOME and Cinnamon desktops")]
    UnsupportedDesktop,
    #[error("the wallpaper path is not an absolute local path")]
    InvalidWallpaperPath,
    #[error("gsettings could not apply the wallpaper: {0}")]
    ApplyFailed(String),
}

impl Desktop {
    pub fn detect() -> Result<Self, PlatformError> {
        let value = env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| env::var("XDG_SESSION_DESKTOP"))
            .unwrap_or_default();
        Self::detect_from(&value).ok_or(PlatformError::UnsupportedDesktop)
    }

    pub fn detect_from(value: &str) -> Option<Self> {
        let normalized = value.to_ascii_lowercase();
        if normalized.split(':').any(|part| part.contains("cinnamon")) {
            Some(Self::Cinnamon)
        } else if normalized.split(':').any(|part| part.contains("gnome")) {
            Some(Self::Gnome)
        } else {
            None
        }
    }

    pub fn apply(self, path: &Path) -> Result<(), PlatformError> {
        let uri = Url::from_file_path(path)
            .map_err(|_| PlatformError::InvalidWallpaperPath)?
            .to_string();
        let schema = match self {
            Self::Gnome => "org.gnome.desktop.background",
            Self::Cinnamon => "org.cinnamon.desktop.background",
        };

        set_gsettings(schema, "picture-uri", &uri)?;
        if self == Self::Gnome {
            set_gsettings(schema, "picture-uri-dark", &uri)?;
        }
        set_gsettings(schema, "picture-options", "zoom")
    }
}

fn set_gsettings(schema: &str, key: &str, value: &str) -> Result<(), PlatformError> {
    let output = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output()
        .map_err(|error| PlatformError::ApplyFailed(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PlatformError::ApplyFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_desktops_case_insensitively() {
        assert_eq!(Desktop::detect_from("ubuntu:GNOME"), Some(Desktop::Gnome));
        assert_eq!(Desktop::detect_from("X-Cinnamon"), Some(Desktop::Cinnamon));
    }

    #[test]
    fn rejects_other_desktops() {
        assert_eq!(Desktop::detect_from("KDE"), None);
        assert_eq!(Desktop::detect_from("XFCE"), None);
    }
}
