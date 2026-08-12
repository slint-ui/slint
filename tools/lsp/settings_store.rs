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
    fs,
    io::Write,
    path::{Path, PathBuf},
};

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

    // Write next to the settings file and rename over it, so an interrupted
    // write cannot leave truncated settings behind.
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A not-yet-existing directory, so `save_to_path()` has to create it.
    fn config_dir(parent: &tempfile::TempDir) -> PathBuf {
        parent.path().join("slint-lsp")
    }

    #[test]
    fn settings_path_uses_only_the_final_component() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_dir = config_dir(&temp_dir);
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
        let temp_dir = tempfile::tempdir().unwrap();
        let config_dir = config_dir(&temp_dir);
        let path = settings_path_from_config_dir(&config_dir, "settings.json").unwrap();

        assert_eq!(load_from_path(&path), None);
        assert!(!path.exists());
    }

    #[test]
    fn save_then_load_overwrites_atomically() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_dir = config_dir(&temp_dir);
        let path = settings_path_from_config_dir(&config_dir, "settings.json").unwrap();

        save_to_path(&path, "first").unwrap();
        save_to_path(&path, "second").unwrap();

        assert_eq!(load_from_path(&path).as_deref(), Some("second"));

        let entries = fs::read_dir(&config_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [path.file_name().unwrap()]);
    }
}
