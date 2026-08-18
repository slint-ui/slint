// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::project::ProjectRunTarget;

/// The stable ID of a Springboard device.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    /// Create a device ID.
    pub fn new(id: impl Into<String>) -> Result<Self, DeviceIdError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(DeviceIdError);
        }
        Ok(Self(id))
    }

    /// Return the device ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let id = String::deserialize(deserializer)?;
        Self::new(id).map_err(serde::de::Error::custom)
    }
}

/// An invalid empty device ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceIdError;

impl std::fmt::Display for DeviceIdError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("A device ID cannot be empty")
    }
}

impl std::error::Error for DeviceIdError {}

/// The driver used to reach a Springboard device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceKind {
    LocalViewer,
    RustApplication,
    RemoteViewer,
    IosSimulator,
    AndroidEmulator,
}

/// How Springboard learned about a device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceOrigin {
    BuiltIn,
    Remembered,
    Discovered,
    Manual,
}

/// The actions supported by a device driver.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceCapabilities {
    pub launch: bool,
    pub stop: bool,
    pub refresh: bool,
    pub reconnect: bool,
    pub rebuild: bool,
}

impl DeviceCapabilities {
    /// Capabilities for a target Springboard can launch and stop.
    pub const fn launchable() -> Self {
        Self { launch: true, stop: true, refresh: true, reconnect: false, rebuild: false }
    }
}

/// The current state of a Springboard device.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum DeviceStatus {
    Available,
    Unavailable,
    Resolving,
    Starting,
    Connecting,
    Reconnecting,
    Downloading { bytes_received: u64, total_bytes: Option<u64> },
    Compiling,
    Reloading,
    Rebuilding,
    Running,
    RunningWithError { message: String },
    Stopping,
    Failed { message: String },
    Incompatible { installed: String, required: String },
}

impl DeviceStatus {
    /// Return whether the device owns the session's active target slot.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Starting
                | Self::Connecting
                | Self::Reconnecting
                | Self::Downloading { .. }
                | Self::Compiling
                | Self::Reloading
                | Self::Rebuilding
                | Self::Running
                | Self::RunningWithError { .. }
                | Self::Stopping
        )
    }
}

/// A device visible in a Springboard session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub origin: DeviceOrigin,
    pub status: DeviceStatus,
    pub capabilities: DeviceCapabilities,
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

/// Work a driver must perform after a device-addressed session request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionAction {
    None,
    Launch,
    Stop,
    Refresh,
    Rebuild,
}

/// The severity of a structured Springboard diagnostic.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
}

/// The severity of a Springboard log event.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    Error,
    Warning,
    Information,
    Debug,
}

/// A state or output change produced by a Springboard session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "event")]
pub enum SessionEvent {
    DeviceChanged {
        device: Device,
    },
    DeviceRemoved {
        device_id: DeviceId,
    },
    ActiveDeviceChanged {
        device_id: Option<DeviceId>,
    },
    LastUsedDeviceChanged {
        device_id: Option<DeviceId>,
    },
    Log {
        device_id: Option<DeviceId>,
        level: LogLevel,
        message: String,
    },
    Diagnostic {
        device_id: DeviceId,
        severity: DiagnosticSeverity,
        message: String,
        file: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
    },
    Error {
        device_id: Option<DeviceId>,
        message: String,
    },
}

/// An invalid device-addressed session operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionError {
    UnknownDevice(DeviceId),
    TargetLimitReached { active: DeviceId, requested: DeviceId },
    Unsupported { device_id: DeviceId, operation: &'static str },
    InactiveDevice(DeviceId),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDevice(device_id) => write!(formatter, "Unknown device {device_id}"),
            Self::TargetLimitReached { active, requested } => write!(
                formatter,
                "Device {active} is already active; stop it before launching {requested}"
            ),
            Self::Unsupported { device_id, operation } => {
                write!(formatter, "Device {device_id} does not support {operation}")
            }
            Self::InactiveDevice(device_id) => {
                write!(formatter, "Device {device_id} is not the active target")
            }
        }
    }
}

impl std::error::Error for SessionError {}

/// The device state for one project development session.
#[derive(Debug)]
pub struct SpringboardSession {
    project: ProjectRunTarget,
    devices: BTreeMap<DeviceId, Device>,
    active_device: Option<DeviceId>,
}

impl SpringboardSession {
    /// Create an empty device session for a project.
    pub fn new(project: ProjectRunTarget) -> Self {
        Self { project, devices: BTreeMap::new(), active_device: None }
    }

    /// Return the project target managed by this session.
    pub fn project(&self) -> &ProjectRunTarget {
        &self.project
    }

    /// Return all registered devices, ordered by device ID.
    pub fn devices(&self) -> &BTreeMap<DeviceId, Device> {
        &self.devices
    }

    /// Return the active device ID.
    pub fn active_device(&self) -> Option<&DeviceId> {
        self.active_device.as_ref()
    }

    /// Add a device or replace its non-active definition.
    pub fn upsert_device(&mut self, mut device: Device) {
        if self.active_device.as_ref() == Some(&device.id)
            && let Some(existing) = self.devices.get(&device.id)
        {
            device.status = existing.status.clone();
        }
        self.devices.insert(device.id.clone(), device);
    }

    /// Remove an inactive device from this project session.
    pub fn remove_device(&mut self, device_id: &DeviceId) -> Option<Device> {
        if self.active_device.as_ref() == Some(device_id) {
            return None;
        }
        self.devices.remove(device_id)
    }

    /// Begin launching a device.
    pub fn launch(&mut self, device_id: &DeviceId) -> Result<SessionAction, SessionError> {
        let Some(device) = self.devices.get(device_id) else {
            return Err(SessionError::UnknownDevice(device_id.clone()));
        };
        if self.active_device.as_ref() == Some(device_id) {
            return Ok(SessionAction::None);
        }
        if let Some(active) = &self.active_device {
            return Err(SessionError::TargetLimitReached {
                active: active.clone(),
                requested: device_id.clone(),
            });
        }
        if !device.capabilities.launch {
            return Err(SessionError::Unsupported {
                device_id: device_id.clone(),
                operation: "launch",
            });
        }

        self.active_device = Some(device_id.clone());
        self.devices.get_mut(device_id).unwrap().status = DeviceStatus::Starting;
        Ok(SessionAction::Launch)
    }

    /// Mark an active device as connecting.
    pub fn mark_connecting(&mut self, device_id: &DeviceId) -> Result<(), SessionError> {
        self.active_device_mut(device_id)?.status = DeviceStatus::Connecting;
        Ok(())
    }

    /// Mark an active device as reconnecting after a connection loss.
    pub fn mark_reconnecting(&mut self, device_id: &DeviceId) -> Result<(), SessionError> {
        self.active_device_mut(device_id)?.status = DeviceStatus::Reconnecting;
        Ok(())
    }

    /// Mark an active device as running.
    pub fn mark_running(&mut self, device_id: &DeviceId) -> Result<(), SessionError> {
        self.active_device_mut(device_id)?.status = DeviceStatus::Running;
        Ok(())
    }

    /// Replace an active device's status without changing its target slot.
    pub fn mark_active_status(
        &mut self,
        device_id: &DeviceId,
        status: DeviceStatus,
    ) -> Result<(), SessionError> {
        if !status.is_active() {
            return Err(SessionError::InactiveDevice(device_id.clone()));
        }
        self.active_device_mut(device_id)?.status = status;
        Ok(())
    }

    /// Begin stopping a device.
    pub fn stop(&mut self, device_id: &DeviceId) -> Result<SessionAction, SessionError> {
        let Some(device) = self.devices.get(device_id) else {
            return Err(SessionError::UnknownDevice(device_id.clone()));
        };
        if self.active_device.as_ref() != Some(device_id) {
            return Ok(SessionAction::None);
        }
        if matches!(device.status, DeviceStatus::Stopping) {
            return Ok(SessionAction::None);
        }
        if !device.capabilities.stop {
            return Err(SessionError::Unsupported {
                device_id: device_id.clone(),
                operation: "stop",
            });
        }

        self.devices.get_mut(device_id).unwrap().status = DeviceStatus::Stopping;
        Ok(SessionAction::Stop)
    }

    /// Finish stopping a device and release the active target slot.
    pub fn mark_stopped(
        &mut self,
        device_id: &DeviceId,
        idle_status: DeviceStatus,
    ) -> Result<(), SessionError> {
        if idle_status.is_active() {
            return Err(SessionError::InactiveDevice(device_id.clone()));
        }
        self.active_device_mut(device_id)?.status = idle_status;
        self.active_device = None;
        Ok(())
    }

    /// Mark a launch or running target as failed and release the active target slot.
    pub fn mark_failed(
        &mut self,
        device_id: &DeviceId,
        message: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.active_device_mut(device_id)?.status =
            DeviceStatus::Failed { message: message.into() };
        self.active_device = None;
        Ok(())
    }

    /// Request a device refresh.
    pub fn refresh(&self, device_id: &DeviceId) -> Result<SessionAction, SessionError> {
        let Some(device) = self.devices.get(device_id) else {
            return Err(SessionError::UnknownDevice(device_id.clone()));
        };
        if !device.capabilities.refresh {
            return Err(SessionError::Unsupported {
                device_id: device_id.clone(),
                operation: "refresh",
            });
        }
        Ok(SessionAction::Refresh)
    }

    /// Request a rebuild from an active device.
    pub fn rebuild(&self, device_id: &DeviceId) -> Result<SessionAction, SessionError> {
        let Some(device) = self.devices.get(device_id) else {
            return Err(SessionError::UnknownDevice(device_id.clone()));
        };
        if self.active_device.as_ref() != Some(device_id) {
            return Err(SessionError::InactiveDevice(device_id.clone()));
        }
        if !device.capabilities.rebuild {
            return Err(SessionError::Unsupported {
                device_id: device_id.clone(),
                operation: "rebuild",
            });
        }
        Ok(SessionAction::Rebuild)
    }

    fn active_device_mut(&mut self, device_id: &DeviceId) -> Result<&mut Device, SessionError> {
        if self.active_device.as_ref() != Some(device_id) {
            return Err(SessionError::InactiveDevice(device_id.clone()));
        }
        self.devices
            .get_mut(device_id)
            .ok_or_else(|| SessionError::UnknownDevice(device_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn device(id: &str) -> Device {
        Device {
            id: DeviceId::new(id).unwrap(),
            name: id.into(),
            kind: DeviceKind::LocalViewer,
            origin: DeviceOrigin::BuiltIn,
            status: DeviceStatus::Available,
            capabilities: DeviceCapabilities::launchable(),
            version: None,
            platform: None,
        }
    }

    fn session() -> SpringboardSession {
        SpringboardSession::new(ProjectRunTarget {
            project_root: PathBuf::from("/project"),
            manifest_path: PathBuf::from("/project/slint.toml"),
            entry_file: PathBuf::from("/project/main.slint"),
            component: "App".into(),
            app: None,
        })
    }

    #[test]
    fn launch_and_stop_are_idempotent() {
        let mut session = session();
        let first = device("first");
        let id = first.id.clone();
        session.upsert_device(first);

        assert_eq!(session.launch(&id), Ok(SessionAction::Launch));
        assert_eq!(session.launch(&id), Ok(SessionAction::None));
        session.mark_running(&id).unwrap();
        assert_eq!(session.stop(&id), Ok(SessionAction::Stop));
        assert_eq!(session.stop(&id), Ok(SessionAction::None));
        session.mark_stopped(&id, DeviceStatus::Available).unwrap();
        assert_eq!(session.stop(&id), Ok(SessionAction::None));
        assert_eq!(session.active_device(), None);
    }

    #[test]
    fn one_active_target_is_enforced() {
        let mut session = session();
        let first = device("first");
        let first_id = first.id.clone();
        let second = device("second");
        let second_id = second.id.clone();
        session.upsert_device(first);
        session.upsert_device(second);
        session.launch(&first_id).unwrap();

        assert_eq!(
            session.launch(&second_id),
            Err(SessionError::TargetLimitReached { active: first_id, requested: second_id })
        );
    }

    #[test]
    fn rebuild_requires_an_active_capable_device() {
        let mut session = session();
        let mut target = device("rust");
        target.capabilities.rebuild = true;
        let id = target.id.clone();
        session.upsert_device(target);

        assert_eq!(session.rebuild(&id), Err(SessionError::InactiveDevice(id.clone())));
        session.launch(&id).unwrap();
        assert_eq!(session.rebuild(&id), Ok(SessionAction::Rebuild));
    }

    #[test]
    fn failures_release_the_active_target() {
        let mut session = session();
        let target = device("first");
        let id = target.id.clone();
        session.upsert_device(target);
        session.launch(&id).unwrap();

        session.mark_failed(&id, "viewer exited").unwrap();

        assert_eq!(session.active_device(), None);
        assert_eq!(
            session.devices()[&id].status,
            DeviceStatus::Failed { message: "viewer exited".into() }
        );
    }

    #[test]
    fn every_operation_requires_a_known_device_id() {
        let mut session = session();
        let missing = DeviceId::new("missing").unwrap();

        assert_eq!(session.launch(&missing), Err(SessionError::UnknownDevice(missing.clone())));
        assert_eq!(session.stop(&missing), Err(SessionError::UnknownDevice(missing.clone())));
        assert_eq!(session.refresh(&missing), Err(SessionError::UnknownDevice(missing)));
    }

    #[test]
    fn refreshing_checks_device_capabilities() {
        let mut session = session();
        let mut target = device("first");
        target.capabilities.refresh = false;
        let id = target.id.clone();
        session.upsert_device(target);

        assert_eq!(
            session.refresh(&id),
            Err(SessionError::Unsupported { device_id: id, operation: "refresh" })
        );
    }

    #[test]
    fn active_status_survives_discovery_updates() {
        let mut session = session();
        let target = device("first");
        let id = target.id.clone();
        session.upsert_device(target);
        session.launch(&id).unwrap();
        session.mark_running(&id).unwrap();

        let mut update = device("first");
        update.name = "Renamed".into();
        update.status = DeviceStatus::Unavailable;
        session.upsert_device(update);

        assert_eq!(session.devices()[&id].name, "Renamed");
        assert_eq!(session.devices()[&id].status, DeviceStatus::Running);
    }

    #[test]
    fn reconnecting_keeps_the_active_target_slot() {
        let mut session = session();
        let target = device("first");
        let id = target.id.clone();
        session.upsert_device(target);
        session.launch(&id).unwrap();

        session.mark_reconnecting(&id).unwrap();

        assert_eq!(session.devices()[&id].status, DeviceStatus::Reconnecting);
        assert_eq!(session.active_device(), Some(&id));
    }

    #[test]
    fn empty_device_ids_are_rejected_by_construction_and_deserialization() {
        assert_eq!(DeviceId::new("  "), Err(DeviceIdError));
        assert!(serde_json::from_str::<DeviceId>(r#"""#).is_err());
    }
}
