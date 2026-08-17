// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::io::Write;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "simulator-settings.json";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DeviceFrame {
    #[default]
    None,
    AndroidPhone,
}

impl From<bool> for DeviceFrame {
    fn from(frame_enabled: bool) -> Self {
        if frame_enabled { Self::AndroidPhone } else { Self::None }
    }
}

impl DeviceFrame {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AndroidPhone => "android-phone",
        }
    }

    fn parse(value: &str) -> std::result::Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "android-phone" => Ok(Self::AndroidPhone),
            _ => Err(format!("unknown device frame '{value}'")),
        }
    }
}

pub fn load() -> DeviceFrame {
    let Some(path) = settings_path() else {
        tracing::warn!(
            "Cannot determine the platform configuration directory for simulator settings"
        );
        return DeviceFrame::None;
    };
    load_or_default_from_path(&path)
}

pub fn save(frame: DeviceFrame) -> std::io::Result<()> {
    let path = settings_path().ok_or_else(|| {
        std::io::Error::other("cannot determine the platform configuration directory")
    })?;
    save_to_path(&path, frame)
}

fn settings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "Slint", "slint-viewer")
        .map(|directories| directories.config_dir().join(SETTINGS_FILE))
}

fn load_or_default_from_path(path: &Path) -> DeviceFrame {
    match load_from_path(path) {
        Ok(frame) => frame,
        Err(error) => {
            tracing::warn!("Ignoring simulator settings at {}: {error}", path.display());
            DeviceFrame::None
        }
    }
}

fn load_from_path(path: &Path) -> std::result::Result<DeviceFrame, String> {
    let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: String = serde_json::from_str(&contents).map_err(|error| error.to_string())?;
    DeviceFrame::parse(&value)
}

fn save_to_path(path: &Path, frame: DeviceFrame) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::other(format!("simulator settings path has no parent: {}", path.display()))
    })?;
    std::fs::create_dir_all(parent)?;

    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, frame.as_str())?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_use_no_frame() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);

        assert_eq!(load_or_default_from_path(&path), DeviceFrame::None);
    }

    #[test]
    fn settings_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);

        save_to_path(&path, DeviceFrame::AndroidPhone).unwrap();

        assert_eq!(load_from_path(&path), Ok(DeviceFrame::AndroidPhone));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "\"android-phone\"\n");
    }

    #[test]
    fn malformed_settings_use_no_frame() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);
        std::fs::write(&path, "not json").unwrap();

        assert_eq!(load_or_default_from_path(&path), DeviceFrame::None);

        std::fs::write(&path, "\"tablet\"\n").unwrap();
        assert_eq!(load_or_default_from_path(&path), DeviceFrame::None);
    }

    #[test]
    fn save_replaces_the_settings_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(SETTINGS_FILE);

        save_to_path(&path, DeviceFrame::AndroidPhone).unwrap();
        save_to_path(&path, DeviceFrame::None).unwrap();

        assert_eq!(load_from_path(&path), Ok(DeviceFrame::None));
        let entries = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [path.file_name().unwrap()]);
    }
}
