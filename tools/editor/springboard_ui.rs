// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::collections::BTreeMap;

use i_slint_editor_preview::preview;
use i_slint_springboard::{
    Device, DeviceId, DeviceKind, DeviceStatus, ProjectSnapshot, ProtocolErrorCode,
    ResponsePayload, ServerEvent, ServerMessage,
};
use slint::{ModelRc, SharedString, VecModel};

#[derive(Clone, Debug)]
pub enum SpringboardUiAction {
    ConfigureProject(String),
    Launch(String),
    Stop(String),
    Refresh(String),
    AddManualDevice(String),
}

pub fn setup(
    app_window: &preview::ui::AppWindow,
    sender: crossbeam_channel::Sender<SpringboardUiAction>,
) {
    let api = app_window.api();
    api.set_springboard_devices(ModelRc::new(VecModel::default()));
    api.set_springboard_session_status(preview::ui::SpringboardSessionStatus::Unavailable);

    let actions = sender.clone();
    api.on_springboard_configure_project(move |project_root| {
        send_action(&actions, SpringboardUiAction::ConfigureProject(project_root.into()));
    });

    let actions = sender.clone();
    api.on_springboard_launch_device(move |device_id| {
        send_action(&actions, SpringboardUiAction::Launch(device_id.into()));
    });
    let actions = sender.clone();
    api.on_springboard_stop_device(move |device_id| {
        send_action(&actions, SpringboardUiAction::Stop(device_id.into()));
    });
    let actions = sender.clone();
    api.on_springboard_refresh_device(move |device_id| {
        send_action(&actions, SpringboardUiAction::Refresh(device_id.into()));
    });
    api.on_springboard_add_manual_device(move |address| {
        send_action(&sender, SpringboardUiAction::AddManualDevice(address.into()));
    });
}

fn send_action(
    sender: &crossbeam_channel::Sender<SpringboardUiAction>,
    action: SpringboardUiAction,
) {
    if sender.send(action).is_err() {
        tracing::warn!("Ignoring a Springboard action after the editor host stopped");
    }
}

thread_local! {
    static STATE: RefCell<SpringboardUiState> = RefCell::new(SpringboardUiState::default());
}

pub fn apply_message(message: ServerMessage) {
    STATE.with_borrow_mut(|state| state.apply_message(message));
    sync_api();
}

pub fn set_connection_error(message: String) {
    STATE.with_borrow_mut(|state| {
        state.status = preview::ui::SpringboardSessionStatus::Error;
        state.error = message.clone();
        state.manager_error = message;
        state.reveal_manager = true;
    });
    sync_api();
}

pub fn open_device_manager() {
    STATE.with_borrow(|state| {
        preview::PREVIEW_STATE.with_borrow(|preview_state| {
            let Some(app_window) = &preview_state.app_window else { return };
            let api = app_window.api();
            api.set_springboard_device_manager_selected_device_id(
                state.last_used.as_ref().map(DeviceId::as_str).unwrap_or_default().into(),
            );
            api.set_springboard_device_manager_visible(true);
        });
    });
}

fn sync_api() {
    STATE.with_borrow_mut(|state| {
        preview::PREVIEW_STATE.with_borrow(|preview_state| {
            let Some(app_window) = &preview_state.app_window else { return };
            let api = app_window.api();
            api.set_springboard_devices(ModelRc::new(VecModel::from(state.rows())));
            api.set_springboard_session_status(state.status);
            api.set_springboard_session_error(state.error.clone().into());
            api.set_springboard_active_device_id(
                state.active.as_ref().map(DeviceId::as_str).unwrap_or_default().into(),
            );
            api.set_springboard_last_used_device_id(
                state.last_used.as_ref().map(DeviceId::as_str).unwrap_or_default().into(),
            );
            api.set_springboard_run_state(state.run_state());
            api.set_springboard_device_manager_error(state.manager_error.clone().into());
            if state.reveal_manager {
                api.set_springboard_device_manager_selected_device_id(
                    state.last_used.as_ref().map(DeviceId::as_str).unwrap_or_default().into(),
                );
                api.set_springboard_device_manager_visible(true);
                state.reveal_manager = false;
            }
        });
    });
}

struct SpringboardUiState {
    devices: BTreeMap<DeviceId, Device>,
    active: Option<DeviceId>,
    last_used: Option<DeviceId>,
    status: preview::ui::SpringboardSessionStatus,
    error: String,
    manager_error: String,
    reveal_manager: bool,
}

impl Default for SpringboardUiState {
    fn default() -> Self {
        Self {
            devices: BTreeMap::new(),
            active: None,
            last_used: None,
            status: preview::ui::SpringboardSessionStatus::Unavailable,
            error: String::new(),
            manager_error: String::new(),
            reveal_manager: false,
        }
    }
}

impl SpringboardUiState {
    fn apply_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Response(response) => match response.response {
                ResponsePayload::Snapshot { snapshot } => self.apply_snapshot(snapshot),
                ResponsePayload::Error { code, message } => {
                    if code == ProtocolErrorCode::VersionMismatch {
                        self.status = preview::ui::SpringboardSessionStatus::Error;
                    }
                    self.error = message.clone();
                    self.manager_error = message;
                    self.reveal_manager = true;
                }
                ResponsePayload::Ok => {
                    self.error.clear();
                    self.manager_error.clear();
                }
            },
            ServerMessage::Event(event) => self.apply_event(event.event),
        }
    }

    fn apply_snapshot(&mut self, snapshot: ProjectSnapshot) {
        self.devices =
            snapshot.devices.into_iter().map(|device| (device.id.clone(), device)).collect();
        self.active = snapshot.active_device;
        self.last_used = snapshot.last_used_device;
        self.status = preview::ui::SpringboardSessionStatus::Ready;
        self.error.clear();
        self.manager_error.clear();
    }

    fn apply_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Snapshot { snapshot } => self.apply_snapshot(snapshot),
            ServerEvent::DeviceChanged { device } => {
                self.devices.insert(device.id.clone(), device);
            }
            ServerEvent::ActiveDeviceChanged { device_id } => self.active = device_id,
            ServerEvent::LastUsedDeviceChanged { device_id } => self.last_used = device_id,
            ServerEvent::Diagnostic { message, .. } | ServerEvent::Error { message, .. } => {
                self.error = message.clone();
                self.manager_error = message;
            }
            ServerEvent::Shutdown => {
                self.status = preview::ui::SpringboardSessionStatus::Unavailable;
                self.active = None;
            }
            ServerEvent::Log { .. } => {}
        }
    }

    fn rows(&self) -> Vec<preview::ui::SpringboardDevice> {
        let mut devices = self.devices.values().collect::<Vec<_>>();
        devices.sort_by_key(|device| self.last_used.as_ref() != Some(&device.id));
        devices
            .into_iter()
            .map(|device| device_to_ui(device, self.active.as_ref(), self.last_used.as_ref()))
            .collect()
    }

    fn run_state(&self) -> preview::ui::SpringboardRunState {
        if self.status == preview::ui::SpringboardSessionStatus::Error {
            return preview::ui::SpringboardRunState::Error;
        }
        let Some(last_used) = &self.last_used else {
            return preview::ui::SpringboardRunState::NoDevice;
        };
        let Some(device) = self.devices.get(last_used) else {
            return preview::ui::SpringboardRunState::Unavailable;
        };
        match device.status {
            DeviceStatus::Available | DeviceStatus::Failed { .. } => {
                preview::ui::SpringboardRunState::Ready
            }
            DeviceStatus::Starting => preview::ui::SpringboardRunState::Starting,
            DeviceStatus::Connecting => preview::ui::SpringboardRunState::Connecting,
            DeviceStatus::Running | DeviceStatus::Stopping => {
                preview::ui::SpringboardRunState::Running
            }
            DeviceStatus::Unavailable => preview::ui::SpringboardRunState::Unavailable,
            DeviceStatus::Incompatible { .. } => preview::ui::SpringboardRunState::Incompatible,
        }
    }
}

fn device_to_ui(
    device: &Device,
    active: Option<&DeviceId>,
    last_used: Option<&DeviceId>,
) -> preview::ui::SpringboardDevice {
    let kind = match device.kind {
        DeviceKind::LocalViewer => preview::ui::SpringboardDeviceKind::LocalViewer,
        DeviceKind::RustApplication => preview::ui::SpringboardDeviceKind::RustApplication,
        DeviceKind::RemoteViewer => preview::ui::SpringboardDeviceKind::RemoteViewer,
        DeviceKind::IosSimulator => preview::ui::SpringboardDeviceKind::IosSimulator,
        DeviceKind::AndroidEmulator => preview::ui::SpringboardDeviceKind::AndroidEmulator,
    };
    let (status, status_detail) = match &device.status {
        DeviceStatus::Available => (preview::ui::SpringboardDeviceStatus::Available, String::new()),
        DeviceStatus::Unavailable => {
            (preview::ui::SpringboardDeviceStatus::Unavailable, String::new())
        }
        DeviceStatus::Starting => (preview::ui::SpringboardDeviceStatus::Starting, String::new()),
        DeviceStatus::Connecting => {
            (preview::ui::SpringboardDeviceStatus::Connecting, String::new())
        }
        DeviceStatus::Running => (preview::ui::SpringboardDeviceStatus::Running, String::new()),
        DeviceStatus::Stopping => (preview::ui::SpringboardDeviceStatus::Stopping, String::new()),
        DeviceStatus::Failed { message } => {
            (preview::ui::SpringboardDeviceStatus::Failed, message.clone())
        }
        DeviceStatus::Incompatible { installed, required } => (
            preview::ui::SpringboardDeviceStatus::Incompatible,
            format!("Installed {installed}; requires {required}"),
        ),
    };
    preview::ui::SpringboardDevice {
        id: SharedString::from(device.id.as_str()),
        name: SharedString::from(&device.name),
        kind,
        status,
        status_detail: status_detail.into(),
        version: device.version.as_deref().unwrap_or_default().into(),
        is_active: active == Some(&device.id),
        is_last_used: last_used == Some(&device.id),
        can_launch: device.capabilities.launch,
        can_stop: device.capabilities.stop,
        can_refresh: device.capabilities.refresh,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use i_slint_springboard::{
        DeviceCapabilities, DeviceOrigin, EventEnvelope, SPRINGBOARD_PROTOCOL_VERSION,
    };

    use super::*;

    fn device(status: DeviceStatus) -> Device {
        Device {
            id: DeviceId::new("builtin:local-viewer").unwrap(),
            name: "Local Viewer".into(),
            kind: DeviceKind::LocalViewer,
            origin: DeviceOrigin::BuiltIn,
            status,
            capabilities: DeviceCapabilities::launchable(),
            version: Some("1.18.0".into()),
        }
    }

    #[test]
    fn snapshot_conversion_marks_active_and_last_used() {
        let target = device(DeviceStatus::Running);
        let mut state = SpringboardUiState::default();

        state.apply_snapshot(ProjectSnapshot {
            project_root: PathBuf::from("/project"),
            entry_file: PathBuf::from("/project/main.slint"),
            component: "App".into(),
            devices: vec![target.clone()],
            active_device: Some(target.id.clone()),
            last_used_device: Some(target.id.clone()),
        });

        let rows = state.rows();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_active);
        assert!(rows[0].is_last_used);
        assert_eq!(rows[0].status, preview::ui::SpringboardDeviceStatus::Running);
    }

    #[test]
    fn incremental_events_update_device_and_session_markers() {
        let mut state = SpringboardUiState::default();
        let mut target = device(DeviceStatus::Starting);
        state.apply_message(ServerMessage::Event(EventEnvelope {
            protocol_version: SPRINGBOARD_PROTOCOL_VERSION,
            event: ServerEvent::DeviceChanged { device: target.clone() },
        }));
        target.status = DeviceStatus::Running;
        state.apply_event(ServerEvent::DeviceChanged { device: target.clone() });
        state.apply_event(ServerEvent::ActiveDeviceChanged { device_id: Some(target.id.clone()) });
        state
            .apply_event(ServerEvent::LastUsedDeviceChanged { device_id: Some(target.id.clone()) });

        let rows = state.rows();
        assert_eq!(rows[0].status, preview::ui::SpringboardDeviceStatus::Running);
        assert!(rows[0].is_active);
        assert!(rows[0].is_last_used);
    }

    #[test]
    fn run_state_follows_the_last_used_device() {
        let mut state = SpringboardUiState::default();
        assert_eq!(state.run_state(), preview::ui::SpringboardRunState::NoDevice);

        let target = device(DeviceStatus::Connecting);
        state.devices.insert(target.id.clone(), target.clone());
        state.last_used = Some(target.id.clone());
        state.status = preview::ui::SpringboardSessionStatus::Ready;
        assert_eq!(state.run_state(), preview::ui::SpringboardRunState::Connecting);

        state.devices.get_mut(&target.id).unwrap().status = DeviceStatus::Unavailable;
        assert_eq!(state.run_state(), preview::ui::SpringboardRunState::Unavailable);
    }

    #[test]
    fn a_successful_retry_clears_the_device_manager_error() {
        let mut state = SpringboardUiState::default();
        state.manager_error = "Launch failed".into();
        state.apply_message(ServerMessage::Response(i_slint_springboard::ResponseEnvelope::new(
            i_slint_springboard::RequestId(7),
            ResponsePayload::Ok,
        )));

        assert!(state.manager_error.is_empty());
    }
}
