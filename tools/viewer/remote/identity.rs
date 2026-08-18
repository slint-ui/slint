// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Persistent identity for one installed remote-viewer application.

use std::io;
use std::path::{Path, PathBuf};

use i_slint_live_preview::remote::ViewerIdentity;
use uuid::Uuid;

const IDENTITY_FILE: &str = "installation-id";

pub(super) fn load_or_create_viewer_identity() -> ViewerIdentity {
    let device_id = identity_path()
        .ok_or_else(|| io::Error::other("the viewer data directory is unavailable"))
        .and_then(|path| InstallationIdentityStore::new(path).load_or_create())
        .unwrap_or_else(|error| {
            let fallback = Uuid::new_v4();
            tracing::warn!(
                "Failed to persist the viewer installation ID; using {fallback} for this process: {error}"
            );
            fallback
        });
    ViewerIdentity::new(device_id.to_string(), std::env::consts::OS)
}

fn identity_path() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        return super::ANDROID_DATA_PATH
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|path| path.join(IDENTITY_FILE));
    }
    #[cfg(not(target_os = "android"))]
    {
        directories::ProjectDirs::from("dev", "Slint", "slint-viewer")
            .map(|directories| directories.data_dir().join(IDENTITY_FILE))
    }
}

struct InstallationIdentityStore {
    path: PathBuf,
}

impl InstallationIdentityStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load_or_create(&self) -> io::Result<Uuid> {
        match read_identity(&self.path) {
            Ok(identity) => return Ok(identity),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let identity = Uuid::new_v4();
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("the installation ID path has no parent"))?;
        std::fs::create_dir_all(parent)?;
        let temporary = temporary_path(&self.path, identity);
        std::fs::write(&temporary, format!("{identity}\n"))?;
        match std::fs::rename(&temporary, &self.path) {
            Ok(()) => Ok(identity),
            Err(error) => {
                // A second process may have won the first-run race. Prefer its
                // completed identity before returning the rename failure.
                let raced_identity = read_identity(&self.path);
                std::fs::remove_file(temporary).ok();
                raced_identity.map_err(|_| error)
            }
        }
    }
}

fn read_identity(path: &Path) -> io::Result<Uuid> {
    let contents = std::fs::read_to_string(path)?;
    Uuid::parse_str(contents.trim()).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("invalid installation ID: {error}"))
    })
}

fn temporary_path(path: &Path, identity: Uuid) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{identity}.tmp"));
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_survives_reopening_the_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(IDENTITY_FILE);

        let first = InstallationIdentityStore::new(path.clone()).load_or_create().unwrap();
        let second = InstallationIdentityStore::new(path).load_or_create().unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn deleting_app_data_creates_a_new_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(IDENTITY_FILE);
        let store = InstallationIdentityStore::new(path.clone());
        let first = store.load_or_create().unwrap();

        std::fs::remove_file(path).unwrap();
        let second = store.load_or_create().unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn malformed_identity_is_not_silently_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(IDENTITY_FILE);
        std::fs::write(&path, "not-a-uuid\n").unwrap();

        let error = InstallationIdentityStore::new(path.clone()).load_or_create().unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "not-a-uuid\n");
    }
}
