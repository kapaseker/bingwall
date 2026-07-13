use std::{fs, io, path::Path, process::Command};

use thiserror::Error;

use crate::paths::AppPaths;

const TIMER: &str = r#"[Unit]
Description=Change the Bingwall wallpaper every day at 08:00

[Timer]
OnCalendar=*-*-* 08:00:00
Persistent=true
Unit=bingwall.service

[Install]
WantedBy=timers.target
"#;

#[derive(Debug, Error)]
pub enum SystemdError {
    #[error("could not locate the Bingwall executable: {0}")]
    Executable(#[source] io::Error),
    #[error("could not install the systemd user units: {0}")]
    Install(#[source] io::Error),
    #[error("systemctl failed: {0}")]
    Command(String),
}

pub fn enable(paths: &AppPaths) -> Result<(), SystemdError> {
    if !Path::new("/usr/lib/systemd/user/bingwall.timer").exists() {
        install_units(paths)?;
    }
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", "bingwall.timer"])
}

pub fn disable() -> Result<(), SystemdError> {
    systemctl(&["disable", "--now", "bingwall.timer"])
}

fn install_units(paths: &AppPaths) -> Result<(), SystemdError> {
    let executable = std::env::current_exe().map_err(SystemdError::Executable)?;
    let unit_dir = paths.config_dir.join("systemd-user");
    fs::create_dir_all(&unit_dir).map_err(SystemdError::Install)?;
    let service = format!(
        "[Unit]\nDescription=Apply the current Bingwall wallpaper\nAfter=network-online.target\n\n[Service]\nType=oneshot\nExecStart={} update\n",
        escape_unit_path(&executable)
    );
    fs::write(unit_dir.join("bingwall.service"), service).map_err(SystemdError::Install)?;
    fs::write(unit_dir.join("bingwall.timer"), TIMER).map_err(SystemdError::Install)?;

    let systemd_dir = dirs::config_dir()
        .ok_or_else(|| SystemdError::Install(io::Error::other("missing config directory")))?
        .join("systemd/user");
    fs::create_dir_all(&systemd_dir).map_err(SystemdError::Install)?;
    copy_unit(&unit_dir.join("bingwall.service"), &systemd_dir)?;
    copy_unit(&unit_dir.join("bingwall.timer"), &systemd_dir)
}

fn copy_unit(source: &Path, destination_dir: &Path) -> Result<(), SystemdError> {
    let destination = destination_dir.join(source.file_name().expect("unit has a file name"));
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(SystemdError::Install)
}

fn escape_unit_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\x20")
}

fn systemctl(arguments: &[&str]) -> Result<(), SystemdError> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .output()
        .map_err(|error| SystemdError::Command(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SystemdError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_spaces_in_service_executable_paths() {
        assert_eq!(
            escape_unit_path(Path::new("/home/me/Bing Wall/bingwall")),
            "/home/me/Bing\\x20Wall/bingwall"
        );
    }
}
