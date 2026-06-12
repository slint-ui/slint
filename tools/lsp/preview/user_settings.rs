// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common;
use i_slint_live_preview::protocol::PreviewUserSettings;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const SETTINGS_FILE_NAME: &str = "preview-user-settings.json";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
fn preview_user_settings_path() -> Option<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("dev", "Slint", "slint-lsp")?;
    Some(preview_user_settings_path_from_config_dir(project_dirs.config_dir()))
}

pub fn load_preview_user_settings() -> PreviewUserSettings {
    preview_user_settings_path()
        .as_deref()
        .map_or_else(PreviewUserSettings::default, load_preview_user_settings_from_path)
}

pub fn save_preview_user_settings(settings: &PreviewUserSettings) -> common::Result<()> {
    preview_user_settings_path().as_deref().map_or_else(
        || {
            Err(std::io::Error::other("cannot determine OS config directory for preview settings")
                .into())
        },
        |path| save_preview_user_settings_to_path(path, settings),
    )
}

#[cfg(target_arch = "wasm32")]
fn preview_user_settings_path() -> Option<PathBuf> {
    None
}

fn preview_user_settings_path_from_config_dir(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}

fn load_preview_user_settings_from_path(path: &Path) -> PreviewUserSettings {
    let Ok(file) = File::open(path) else {
        if path.exists() {
            tracing::warn!("Ignoring unreadable preview user settings at {}", path.display());
        } else {
            tracing::debug!("No preview user settings at {}, using defaults", path.display());
        }
        return PreviewUserSettings::default();
    };

    let reader = BufReader::new(file);
    match serde_json::from_reader(reader) {
        Ok(settings) => settings,
        Err(err) => {
            tracing::warn!("Ignoring malformed preview user settings at {}: {err}", path.display());
            PreviewUserSettings::default()
        }
    }
}

fn save_preview_user_settings_to_path(
    path: &Path,
    settings: &PreviewUserSettings,
) -> common::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::other(format!("preview settings path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent)?;

    let mut temp_path_counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temp_path = temp_path_for(path, temp_path_counter);
    let write_result = (|| -> common::Result<()> {
        let mut file = loop {
            match OpenOptions::new().write(true).create_new(true).open(&temp_path) {
                Ok(file) => break file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    temp_path_counter = temp_path_counter.wrapping_add(1);
                    temp_path = temp_path_for(path, temp_path_counter);
                }
                Err(err) => return Err(err.into()),
            }
        };
        serde_json::to_writer_pretty(&mut file, settings)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn temp_path_for(path: &Path, counter: u64) -> PathBuf {
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("settings");
    path.with_file_name(format!(".{}.{}.{}.tmp", file_name, std::process::id(), counter))
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    // `fs::rename()` does not reliably replace an existing destination on Windows.
    // We write to a temporary file first, then use `MoveFileExW` so repeated saves
    // replace the existing settings file without a delete-then-rename gap.
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(
            lpExistingFileName: *const u16,
            lpNewFileName: *const u16,
            dwFlags: u32,
        ) -> i32;
    }

    let from: Vec<u16> = from.as_os_str().encode_wide().chain([0]).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain([0]).collect();
    let ok = unsafe {
        MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
    };
    if ok != 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_config_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir()
            .join("preview-user-settings-tests")
            .join(format!("{test_name}-{}-{nanos}", std::process::id()))
    }

    fn clean_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn settings_path_is_inside_config_dir() {
        let config_dir = unique_config_dir("path");
        assert_eq!(
            preview_user_settings_path_from_config_dir(&config_dir),
            config_dir.join(SETTINGS_FILE_NAME)
        );
    }

    #[test]
    fn load_defaults_when_file_is_missing() {
        let config_dir = unique_config_dir("missing");
        clean_dir(&config_dir);
        let settings_path = preview_user_settings_path_from_config_dir(&config_dir);

        assert_eq!(
            load_preview_user_settings_from_path(&settings_path),
            PreviewUserSettings::default()
        );
        assert!(!settings_path.exists());
    }

    #[test]
    fn load_defaults_when_file_is_malformed() {
        let config_dir = unique_config_dir("malformed");
        let settings_path = preview_user_settings_path_from_config_dir(&config_dir);
        fs::create_dir_all(config_dir).unwrap();
        fs::write(&settings_path, b"{ this is not json").unwrap();

        assert_eq!(
            load_preview_user_settings_from_path(&settings_path),
            PreviewUserSettings::default()
        );
    }

    #[test]
    fn load_preserves_older_subset_fields() {
        let config_dir = unique_config_dir("subset");
        clean_dir(&config_dir);
        let settings_path = preview_user_settings_path_from_config_dir(&config_dir);
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            &settings_path,
            br#"{
  "version": 1,
  "always_on_top": true
}"#,
        )
        .unwrap();

        assert_eq!(
            load_preview_user_settings_from_path(&settings_path),
            PreviewUserSettings {
                version: PreviewUserSettings::CURRENT_VERSION,
                always_on_top: true,
                show_library: false,
                show_properties: false,
                show_outline: false,
                show_simulation_data: false,
                show_console: false,
            }
        );
    }

    #[test]
    fn save_overwrites_existing_file_atomically() {
        let config_dir = unique_config_dir("roundtrip");
        clean_dir(&config_dir);
        let settings_path = preview_user_settings_path_from_config_dir(&config_dir);
        let settings = PreviewUserSettings {
            version: PreviewUserSettings::CURRENT_VERSION,
            always_on_top: true,
            show_library: true,
            show_properties: false,
            show_outline: true,
            show_simulation_data: false,
            show_console: true,
        };
        let updated = PreviewUserSettings {
            version: PreviewUserSettings::CURRENT_VERSION,
            always_on_top: false,
            show_library: false,
            show_properties: true,
            show_outline: false,
            show_simulation_data: true,
            show_console: false,
        };

        save_preview_user_settings_to_path(&settings_path, &settings).unwrap();
        save_preview_user_settings_to_path(&settings_path, &updated).unwrap();

        assert!(settings_path.exists());
        assert_eq!(load_preview_user_settings_from_path(&settings_path), updated);

        let temp_files = fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(temp_files, 0);
    }
}
