// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Generic, settings-agnostic persistence for preview user settings.
//!
//! The LSP owns disk access (the preview may be a child process, a browser tab
//! or a remote viewer), so it acts as a dumb keyed blob store: it reads and
//! writes named files verbatim and never interprets their contents. Each
//! preview owns the (de)serialization of its own settings.

use crate::common;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Load the raw contents of the settings file `name`, or `None` if it is
/// missing, unreadable, or no config directory can be determined.
pub fn load(name: &str) -> Option<String> {
    let path = settings_path(name)?;
    load_from_path(&path)
}

/// Persist `contents` verbatim to the settings file `name`.
pub fn save(name: &str, contents: &str) -> common::Result<()> {
    let path = settings_path(name).ok_or_else(|| {
        std::io::Error::other("cannot determine OS config directory for preview settings")
    })?;
    save_to_path(&path, contents)
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_path(name: &str) -> Option<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("dev", "Slint", "slint-lsp")?;
    settings_path_from_config_dir(project_dirs.config_dir(), name)
}

#[cfg(target_arch = "wasm32")]
fn settings_path(_name: &str) -> Option<PathBuf> {
    None
}

/// Resolve `name` against `config_dir`, using only its final path component so
/// a malicious or malformed name cannot escape the config directory.
fn settings_path_from_config_dir(config_dir: &Path, name: &str) -> Option<PathBuf> {
    let file_name = Path::new(name).file_name()?;
    Some(config_dir.join(file_name))
}

fn load_from_path(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(contents) => Some(contents),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!("No user settings at {}, using defaults", path.display());
            None
        }
        Err(err) => {
            tracing::warn!("Ignoring unreadable user settings at {}: {err}", path.display());
            None
        }
    }
}

fn save_to_path(path: &Path, contents: &str) -> common::Result<()> {
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
        file.write_all(contents.as_bytes())?;
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
    unsafe extern "system" {
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_config_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir()
            .join("preview-settings-store-tests")
            .join(format!("{test_name}-{}-{nanos}", std::process::id()))
    }

    fn clean_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn settings_path_uses_only_the_final_component() {
        let config_dir = unique_config_dir("path");
        assert_eq!(
            settings_path_from_config_dir(&config_dir, "preview-user-settings.json"),
            Some(config_dir.join("preview-user-settings.json"))
        );
        // A name with directory components cannot escape the config dir.
        assert_eq!(
            settings_path_from_config_dir(&config_dir, "../../etc/passwd"),
            Some(config_dir.join("passwd"))
        );
        assert_eq!(settings_path_from_config_dir(&config_dir, ".."), None);
    }

    #[test]
    fn load_returns_none_when_file_is_missing() {
        let config_dir = unique_config_dir("missing");
        clean_dir(&config_dir);
        let path = settings_path_from_config_dir(&config_dir, "settings.json").unwrap();

        assert_eq!(load_from_path(&path), None);
        assert!(!path.exists());
    }

    #[test]
    fn save_then_load_overwrites_atomically() {
        let config_dir = unique_config_dir("roundtrip");
        clean_dir(&config_dir);
        let path = settings_path_from_config_dir(&config_dir, "settings.json").unwrap();

        save_to_path(&path, "first").unwrap();
        save_to_path(&path, "second").unwrap();

        assert!(path.exists());
        assert_eq!(load_from_path(&path).as_deref(), Some("second"));

        let temp_files = fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(temp_files, 0);
    }
}
