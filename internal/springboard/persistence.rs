// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStatus};

const DEVICE_STATE_FILE: &str = "devices.json";

/// The current on-disk Springboard device-state schema.
pub const DEVICE_STATE_SCHEMA_VERSION: u32 = 1;

/// A remote or manually configured device remembered across sessions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RememberedDevice {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default)]
    pub manual: bool,
}

impl RememberedDevice {
    /// Convert a remembered profile into an unavailable runtime device.
    pub fn to_device(&self) -> Device {
        Device {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            origin: if self.manual { DeviceOrigin::Manual } else { DeviceOrigin::Remembered },
            status: DeviceStatus::Unavailable,
            capabilities: DeviceCapabilities {
                launch: true,
                stop: true,
                refresh: true,
                reconnect: true,
            },
            version: self.version.clone(),
            platform: self.platform.clone(),
        }
    }
}

/// Device choices shared by all Springboard projects for one user.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GlobalDeviceState {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_device: Option<DeviceId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub remembered_devices: BTreeMap<DeviceId, RememberedDevice>,
}

impl Default for GlobalDeviceState {
    fn default() -> Self {
        Self {
            schema_version: DEVICE_STATE_SCHEMA_VERSION,
            last_used_device: None,
            remembered_devices: BTreeMap::new(),
        }
    }
}

impl GlobalDeviceState {
    /// Remember a remote or manually configured device.
    ///
    /// Built-in devices remain runtime definitions and are never serialized as profiles.
    pub fn remember_device(&mut self, device: &Device, addresses: Vec<String>) -> bool {
        if device.origin == DeviceOrigin::BuiltIn {
            return false;
        }
        let profile = RememberedDevice {
            id: device.id.clone(),
            name: device.name.clone(),
            kind: device.kind,
            addresses,
            version: device.version.clone(),
            platform: device.platform.clone(),
            manual: device.origin == DeviceOrigin::Manual,
        };
        self.remembered_devices.insert(profile.id.clone(), profile);
        true
    }

    /// Merge persisted profiles with built-in and currently discovered runtime devices.
    pub fn merge_runtime_devices(
        &self,
        runtime_devices: impl IntoIterator<Item = Device>,
    ) -> BTreeMap<DeviceId, Device> {
        let mut devices = self
            .remembered_devices
            .values()
            .map(|profile| (profile.id.clone(), profile.to_device()))
            .collect::<BTreeMap<_, _>>();
        for device in runtime_devices {
            devices.insert(device.id.clone(), device);
        }
        devices
    }
}

/// A loaded device state and any non-fatal fallback warning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadDeviceState {
    pub state: GlobalDeviceState,
    pub warning: Option<String>,
}

/// The result of an atomic device-state save.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveDeviceState {
    /// The backup path used to preserve a malformed or unsupported previous file.
    pub preserved_invalid_path: Option<PathBuf>,
}

/// Persistent storage for Springboard's user-level device state.
#[derive(Clone, Debug)]
pub struct DeviceStateStore {
    path: PathBuf,
}

impl DeviceStateStore {
    /// Locate the device state in Springboard's OS-specific configuration directory.
    pub fn from_platform_config() -> Option<Self> {
        directories::ProjectDirs::from("dev", "Slint", "slint-springboard")
            .map(|directories| Self::new(directories.config_dir().join(DEVICE_STATE_FILE)))
    }

    /// Use an explicit device-state path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Return the device-state path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load device state, falling back to defaults without modifying invalid input.
    pub fn load(&self) -> LoadDeviceState {
        let contents = match std::fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return LoadDeviceState { state: GlobalDeviceState::default(), warning: None };
            }
            Err(error) => {
                return LoadDeviceState {
                    state: GlobalDeviceState::default(),
                    warning: Some(format!(
                        "Failed to read Springboard device state {}: {error}",
                        self.path.display()
                    )),
                };
            }
        };

        match parse_state(&contents) {
            Ok(state) => LoadDeviceState { state, warning: None },
            Err(error) => LoadDeviceState {
                state: GlobalDeviceState::default(),
                warning: Some(format!(
                    "Ignoring Springboard device state {}: {error}",
                    self.path.display()
                )),
            },
        }
    }

    /// Save device state atomically and preserve invalid previous input beside it.
    pub fn save(&self, state: &GlobalDeviceState) -> std::io::Result<SaveDeviceState> {
        if state.schema_version != DEVICE_STATE_SCHEMA_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Cannot save device-state schema {}; expected {}",
                    state.schema_version, DEVICE_STATE_SCHEMA_VERSION
                ),
            ));
        }
        let parent = self.path.parent().ok_or_else(|| {
            std::io::Error::other(format!(
                "Springboard device-state path has no parent: {}",
                self.path.display()
            ))
        })?;
        std::fs::create_dir_all(parent)?;

        let preserved_invalid_path = self.preserve_invalid_file()?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, state)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(|error| error.error)?;

        Ok(SaveDeviceState { preserved_invalid_path })
    }

    fn preserve_invalid_file(&self) -> std::io::Result<Option<PathBuf>> {
        let contents = match std::fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if parse_state(&contents).is_ok() {
            return Ok(None);
        }

        let backup = next_invalid_path(&self.path);
        std::fs::rename(&self.path, &backup)?;
        Ok(Some(backup))
    }
}

fn parse_state(contents: &str) -> Result<GlobalDeviceState, String> {
    let state: GlobalDeviceState =
        serde_json::from_str(contents).map_err(|error| error.to_string())?;
    if state.schema_version != DEVICE_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema version {}; expected {}",
            state.schema_version, DEVICE_STATE_SCHEMA_VERSION
        ));
    }
    Ok(state)
}

fn next_invalid_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("devices");
    let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or("json");
    for suffix in 0.. {
        let suffix = if suffix == 0 { String::new() } else { format!("-{suffix}") };
        let candidate = parent.join(format!("{stem}.invalid{suffix}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(directory: &tempfile::TempDir) -> DeviceStateStore {
        DeviceStateStore::new(directory.path().join("config").join(DEVICE_STATE_FILE))
    }

    fn remote_device(id: &str, origin: DeviceOrigin) -> Device {
        Device {
            id: DeviceId::new(id).unwrap(),
            name: "Phone".into(),
            kind: DeviceKind::RemoteViewer,
            origin,
            status: DeviceStatus::Available,
            capabilities: DeviceCapabilities::launchable(),
            version: Some("1.18.0".into()),
            platform: Some("ios".into()),
        }
    }

    #[test]
    fn missing_state_uses_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);

        assert_eq!(
            store.load(),
            LoadDeviceState { state: GlobalDeviceState::default(), warning: None }
        );
        assert!(!store.path().exists());
    }

    #[test]
    fn state_round_trips_and_replaces_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        let mut state = GlobalDeviceState::default();
        let phone = remote_device("remote:phone", DeviceOrigin::Discovered);
        assert!(state.remember_device(&phone, vec!["192.0.2.1:8080".into()]));
        state.last_used_device = Some(phone.id.clone());

        store.save(&state).unwrap();
        assert_eq!(store.load().state, state);

        state.last_used_device = None;
        store.save(&state).unwrap();
        assert_eq!(store.load().state, state);
        let entries = std::fs::read_dir(store.path().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [store.path().file_name().unwrap()]);
    }

    #[test]
    fn malformed_state_is_preserved_while_load_uses_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::fs::write(store.path(), "not json\n").unwrap();

        let loaded = store.load();

        assert_eq!(loaded.state, GlobalDeviceState::default());
        assert!(loaded.warning.unwrap().contains("Ignoring Springboard device state"));
        assert_eq!(std::fs::read_to_string(store.path()).unwrap(), "not json\n");

        let saved = store.save(&GlobalDeviceState::default()).unwrap();
        let invalid_path = saved.preserved_invalid_path.unwrap();
        assert_eq!(std::fs::read_to_string(invalid_path).unwrap(), "not json\n");
        assert_eq!(store.load().state, GlobalDeviceState::default());
    }

    #[test]
    fn unsupported_schema_is_rejected_and_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::fs::write(store.path(), r#"{"schema_version":999,"remembered_devices":{}}"#).unwrap();

        let loaded = store.load();

        assert_eq!(loaded.state, GlobalDeviceState::default());
        assert!(loaded.warning.unwrap().contains("unsupported schema version"));
        assert!(
            store.save(&GlobalDeviceState::default()).unwrap().preserved_invalid_path.is_some()
        );
    }

    #[test]
    fn built_ins_are_merged_at_runtime_but_not_remembered() {
        let mut state = GlobalDeviceState::default();
        let built_in = Device {
            id: DeviceId::new("builtin:local-viewer").unwrap(),
            name: "Local Viewer".into(),
            kind: DeviceKind::LocalViewer,
            origin: DeviceOrigin::BuiltIn,
            status: DeviceStatus::Available,
            capabilities: DeviceCapabilities::launchable(),
            version: None,
            platform: None,
        };
        assert!(!state.remember_device(&built_in, Vec::new()));

        let devices = state.merge_runtime_devices([built_in.clone()]);

        assert_eq!(devices[&built_in.id], built_in);
        assert!(state.remembered_devices.is_empty());
    }

    #[test]
    fn live_discovery_overrides_an_offline_remembered_profile() {
        let mut state = GlobalDeviceState::default();
        let remembered = remote_device("remote:phone", DeviceOrigin::Discovered);
        state.remember_device(&remembered, vec!["192.0.2.1:8080".into()]);
        let mut discovered = remembered.clone();
        discovered.name = "Nigel's Phone".into();
        discovered.origin = DeviceOrigin::Discovered;
        discovered.status = DeviceStatus::Available;

        let devices = state.merge_runtime_devices([discovered.clone()]);

        assert_eq!(devices[&discovered.id], discovered);
    }
}
