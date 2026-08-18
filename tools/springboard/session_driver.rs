// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use anyhow::{Context as _, Result, bail};
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStateStore, DeviceStatus,
    DiagnosticSeverity, GlobalDeviceState, LogLevel, ProjectSnapshot, RememberedDevice,
    SessionAction, SessionEvent, SpringboardSession, project::ProjectRunTarget,
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::discovery::{DiscoveredRemoteViewer, RemoteDiscoveryEvent, RemoteViewerDiscovery};
use crate::remote_driver::{RemoteDriverEvent, RemoteViewerDriver};

pub const LOCAL_VIEWER_DEVICE_ID: &str = "builtin:local-viewer";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputStream {
    StandardOutput,
    StandardError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChildOutput {
    stream: OutputStream,
    line: String,
}

#[derive(Clone, Debug)]
pub struct ViewerChildCommand {
    executable: PathBuf,
    prefix_args: Vec<OsString>,
    append_viewer_args: bool,
    environment: Vec<(OsString, OsString)>,
}

impl ViewerChildCommand {
    pub fn current_executable() -> Result<Self> {
        Ok(Self {
            executable: std::env::current_exe().context("Failed to locate slint-springboard")?,
            prefix_args: Vec::new(),
            append_viewer_args: true,
            environment: Vec::new(),
        })
    }

    #[cfg(test)]
    fn fake(executable: PathBuf, prefix_args: Vec<OsString>) -> Self {
        Self { executable, prefix_args, append_viewer_args: false, environment: Vec::new() }
    }

    #[cfg(test)]
    fn with_environment(mut self, key: &str, value: &str) -> Self {
        self.environment.push((key.into(), value.into()));
        self
    }
}

struct LocalViewerDriver {
    command: ViewerChildCommand,
    child: Option<Child>,
    output_sender: mpsc::UnboundedSender<ChildOutput>,
    output_receiver: mpsc::UnboundedReceiver<ChildOutput>,
    output_tasks: Vec<JoinHandle<()>>,
}

impl LocalViewerDriver {
    fn new(command: ViewerChildCommand) -> Self {
        let (output_sender, output_receiver) = mpsc::unbounded_channel();
        Self { command, child: None, output_sender, output_receiver, output_tasks: Vec::new() }
    }

    async fn launch(&mut self, target: &ProjectRunTarget, style: Option<&str>) -> Result<()> {
        if let Some(child) = &mut self.child
            && child.try_wait()?.is_none()
        {
            return Ok(());
        }

        let mut command = Command::new(&self.command.executable);
        command.args(&self.command.prefix_args);
        if self.command.append_viewer_args {
            command
                .arg("viewer-child")
                .arg("--entry")
                .arg(&target.entry_file)
                .arg("--component")
                .arg(&target.component);
            if let Some(style) = style {
                command.arg("--style").arg(style);
            }
        }
        command
            .envs(self.command.environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().with_context(|| {
            format!("Failed to launch local viewer with {}", self.command.executable.display())
        })?;
        let stdout = child.stdout.take().context("Local viewer stdout was not captured")?;
        let stderr = child.stderr.take().context("Local viewer stderr was not captured")?;
        self.output_tasks.push(spawn_output_reader(
            stdout,
            OutputStream::StandardOutput,
            self.output_sender.clone(),
        ));
        self.output_tasks.push(spawn_output_reader(
            stderr,
            OutputStream::StandardError,
            self.output_sender.clone(),
        ));
        self.child = Some(child);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().await.context("Failed to stop the local viewer")?;
            } else {
                let _ = child.wait().await;
            }
        }
        self.finish_output_tasks().await;
        Ok(())
    }

    fn poll_exit(&mut self) -> Result<Option<ExitStatus>> {
        let Some(child) = &mut self.child else { return Ok(None) };
        let Some(status) = child.try_wait()? else { return Ok(None) };
        self.child = None;
        Ok(Some(status))
    }

    fn drain_output(&mut self) -> Vec<ChildOutput> {
        let mut output = Vec::new();
        while let Ok(line) = self.output_receiver.try_recv() {
            output.push(line);
        }
        output
    }

    async fn finish_output_tasks(&mut self) {
        for task in self.output_tasks.drain(..) {
            let _ = task.await;
        }
    }
}

fn spawn_output_reader<Reader>(
    reader: Reader,
    stream: OutputStream,
    sender: mpsc::UnboundedSender<ChildOutput>,
) -> JoinHandle<()>
where
    Reader: tokio::io::AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if sender.send(ChildOutput { stream, line }).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(ChildOutput {
                        stream: OutputStream::StandardError,
                        line: format!("Failed to read local viewer output: {error}"),
                    });
                    break;
                }
            }
        }
    })
}

pub struct ProjectSessionController {
    session: SpringboardSession,
    global_state: GlobalDeviceState,
    store: DeviceStateStore,
    local_viewer: LocalViewerDriver,
    remote_viewer: Option<RemoteViewerDriver>,
    remote_discovery: Option<RemoteViewerDiscovery>,
    remote_viewers: BTreeMap<DeviceId, DiscoveredRemoteViewer>,
    pending_launch: Option<DeviceId>,
    pending_endpoint_update: Option<DeviceId>,
    events: VecDeque<SessionEvent>,
}

impl ProjectSessionController {
    pub fn new(
        project: ProjectRunTarget,
        store: DeviceStateStore,
        viewer_command: ViewerChildCommand,
    ) -> Self {
        let loaded = store.load();
        let mut session = SpringboardSession::new(project);
        let local_viewer = local_viewer_device();
        let mut remote_viewers = BTreeMap::new();
        let mut runtime_devices = vec![local_viewer];
        let mut events = VecDeque::new();
        for profile in loaded.state.remembered_devices.values().filter(|profile| profile.manual) {
            match manual_viewer_from_profile(profile) {
                Ok(viewer) => {
                    runtime_devices.push(viewer.to_device());
                    remote_viewers.insert(viewer.id.clone(), viewer);
                }
                Err(error) => events.push_back(SessionEvent::Error {
                    device_id: Some(profile.id.clone()),
                    message: format!("Ignoring invalid manual remote viewer: {error}"),
                }),
            }
        }
        for device in loaded.state.merge_runtime_devices(runtime_devices) {
            session.upsert_device(device.1);
        }
        if let Some(warning) = loaded.warning {
            events.push_back(SessionEvent::Log {
                device_id: None,
                level: LogLevel::Warning,
                message: warning,
            });
        }
        Self {
            session,
            global_state: loaded.state,
            store,
            local_viewer: LocalViewerDriver::new(viewer_command),
            remote_viewer: None,
            remote_discovery: None,
            remote_viewers,
            pending_launch: None,
            pending_endpoint_update: None,
            events,
        }
    }

    pub fn enable_remote_discovery(&mut self) {
        if self.remote_discovery.is_some() {
            return;
        }
        match RemoteViewerDiscovery::start() {
            Ok(discovery) => self.remote_discovery = Some(discovery),
            Err(error) => self.events.push_back(SessionEvent::Error {
                device_id: None,
                message: format!(
                    "Remote viewer discovery is unavailable: {}",
                    describe_remote_failure(&error.to_string())
                ),
            }),
        }
    }

    pub fn session(&self) -> &SpringboardSession {
        &self.session
    }

    pub fn last_used_device(&self) -> Option<&DeviceId> {
        self.global_state.last_used_device.as_ref()
    }

    pub fn snapshot(&self) -> ProjectSnapshot {
        ProjectSnapshot {
            project_root: self.session.project().project_root.clone(),
            entry_file: self.session.project().entry_file.clone(),
            component: self.session.project().component.clone(),
            devices: self.session.devices().values().cloned().collect(),
            active_device: self.session.active_device().cloned(),
            last_used_device: self.global_state.last_used_device.clone(),
        }
    }

    pub async fn launch(&mut self, device_id: &DeviceId) -> Result<()> {
        match self.session.launch(device_id)? {
            SessionAction::None => return Ok(()),
            SessionAction::Launch => {}
            action => bail!("Unexpected session action {action:?} for launch"),
        }
        self.emit_device(device_id);
        if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID {
            if let Err(error) =
                self.local_viewer.launch(self.session.project(), Some("fluent")).await
            {
                self.session.mark_failed(device_id, error.to_string())?;
                self.emit_device(device_id);
                return Err(error);
            }
        } else if let Some(viewer) = self.remote_viewers.get(device_id).cloned() {
            self.session.mark_connecting(device_id)?;
            self.pending_launch = Some(device_id.clone());
            self.emit_device(device_id);
            if let Err(error) = self.connect_remote_viewer(&viewer).await {
                self.pending_launch = None;
                let message = describe_remote_failure(&error.to_string());
                self.session.mark_failed(device_id, &message)?;
                self.emit_device(device_id);
                return Err(anyhow::anyhow!(message));
            }
            return Ok(());
        } else if self.global_state.remembered_devices.contains_key(device_id) {
            self.session.mark_connecting(device_id)?;
            self.pending_launch = Some(device_id.clone());
            self.emit_device(device_id);
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id.clone()),
                level: LogLevel::Information,
                message: "Waiting for the remembered remote viewer to appear".into(),
            });
            return Ok(());
        } else {
            let message = format!("Remote viewer {device_id} is not currently discoverable");
            self.session.mark_failed(device_id, &message)?;
            self.emit_device(device_id);
            bail!(message);
        }

        self.complete_launch(device_id)?;
        Ok(())
    }

    pub fn add_manual_device(&mut self, address: &str) -> Result<DeviceId> {
        let viewer = DiscoveredRemoteViewer::manual(address)?;
        let device_id = viewer.id.clone();
        let device = viewer.to_device();
        let mut global_state = self.global_state.clone();
        global_state.remember_device(&device, viewer.endpoint_strings());
        self.store.save(&global_state).context("Failed to remember the manual remote viewer")?;
        self.global_state = global_state;
        self.remote_viewers.insert(device_id.clone(), viewer);
        self.session.upsert_device(device);
        self.emit_device(&device_id);
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: "Manual remote viewer added".into(),
        });
        Ok(device_id)
    }

    async fn connect_remote_viewer(&mut self, viewer: &DiscoveredRemoteViewer) -> Result<()> {
        let mut driver = RemoteViewerDriver::new()?;
        driver.launch(viewer, self.session.project(), "fluent").await?;
        self.remote_viewer = Some(driver);
        Ok(())
    }

    fn complete_launch(&mut self, device_id: &DeviceId) -> Result<()> {
        self.pending_launch = None;
        if self.pending_endpoint_update.as_ref() == Some(device_id) {
            self.pending_endpoint_update = None;
        }
        self.session.mark_running(device_id)?;
        if let Some(viewer) = self.remote_viewers.get(device_id)
            && let Some(device) = self.session.devices().get(device_id)
        {
            self.global_state.remember_device(device, viewer.endpoint_strings());
        }
        self.global_state.last_used_device = Some(device_id.clone());
        self.emit_device(device_id);
        self.events
            .push_back(SessionEvent::LastUsedDeviceChanged { device_id: Some(device_id.clone()) });
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID {
                "Local viewer started".into()
            } else {
                "Remote viewer connected".into()
            },
        });
        if let Err(error) = self.store.save(&self.global_state) {
            self.events.push_back(SessionEvent::Error {
                device_id: Some(device_id.clone()),
                message: format!("Failed to remember the last-used device: {error}"),
            });
        }
        Ok(())
    }

    pub async fn stop(&mut self, device_id: &DeviceId) -> Result<()> {
        match self.session.stop(device_id)? {
            SessionAction::None => return Ok(()),
            SessionAction::Stop => {}
            action => bail!("Unexpected session action {action:?} for stop"),
        }
        self.emit_device(device_id);
        if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID {
            self.local_viewer.stop().await?;
        } else if let Some(mut driver) = self.remote_viewer.take() {
            driver.stop().await;
        }
        if self.pending_launch.as_ref() == Some(device_id) {
            self.pending_launch = None;
        }
        if self.pending_endpoint_update.as_ref() == Some(device_id) {
            self.pending_endpoint_update = None;
        }
        let idle_status = if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID
            || self.remote_viewers.contains_key(device_id)
        {
            DeviceStatus::Available
        } else {
            DeviceStatus::Unavailable
        };
        self.session.mark_stopped(device_id, idle_status)?;
        self.emit_device(device_id);
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID {
                "Local viewer stopped".into()
            } else {
                "Remote viewer disconnected".into()
            },
        });
        Ok(())
    }

    pub fn refresh(&mut self, device_id: &DeviceId) -> Result<()> {
        match self.session.refresh(device_id)? {
            SessionAction::Refresh => {
                if self.session.active_device() == Some(device_id)
                    && device_id.as_str() != LOCAL_VIEWER_DEVICE_ID
                    && self.pending_launch.as_ref() != Some(device_id)
                {
                    self.remote_viewer
                        .as_mut()
                        .context("The active remote viewer driver is unavailable")?
                        .refresh()?;
                }
                self.events.push_back(SessionEvent::Log {
                    device_id: Some(device_id.clone()),
                    level: LogLevel::Information,
                    message: "Device status refreshed".into(),
                });
                Ok(())
            }
            action => bail!("Unexpected session action {action:?} for refresh"),
        }
    }

    pub async fn poll(&mut self) -> Result<()> {
        let discovery_events = self
            .remote_discovery
            .as_mut()
            .map(RemoteViewerDiscovery::take_events)
            .unwrap_or_default();
        for event in discovery_events {
            self.apply_discovery_event(event);
        }
        self.reconnect_at_updated_endpoint().await;
        self.connect_pending_viewer().await;
        self.capture_remote_events()?;
        self.capture_output();
        if let Some(status) = self.local_viewer.poll_exit()? {
            let device_id = local_viewer_device_id();
            if self.session.active_device() == Some(&device_id) {
                let message = format!("Local viewer exited unexpectedly with {status}");
                self.session.mark_failed(&device_id, &message)?;
                self.emit_device(&device_id);
                self.events.push_back(SessionEvent::Error { device_id: Some(device_id), message });
            }
        }
        Ok(())
    }

    async fn connect_pending_viewer(&mut self) {
        let Some(device_id) = self.pending_launch.clone() else { return };
        if self.remote_viewer.is_some() {
            return;
        }
        let Some(viewer) = self.remote_viewers.get(&device_id).cloned() else { return };
        match self.connect_remote_viewer(&viewer).await {
            Ok(()) => {}
            Err(error) => {
                self.pending_launch = None;
                let message = describe_remote_failure(&error.to_string());
                if self.session.mark_failed(&device_id, &message).is_ok() {
                    self.emit_device(&device_id);
                }
                self.events.push_back(SessionEvent::Error { device_id: Some(device_id), message });
            }
        }
    }

    async fn reconnect_at_updated_endpoint(&mut self) {
        let Some(device_id) = self.pending_endpoint_update.clone() else { return };
        if self.session.active_device() != Some(&device_id)
            || !matches!(
                self.session.devices()[&device_id].status,
                DeviceStatus::Connecting | DeviceStatus::Reconnecting
            )
        {
            return;
        }
        let Some(viewer) = self.remote_viewers.get(&device_id).cloned() else { return };
        let Some(driver) = self.remote_viewer.as_mut() else { return };
        self.pending_endpoint_update = None;
        if let Err(error) = driver.reconnect(&viewer).await {
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id),
                level: LogLevel::Warning,
                message: describe_remote_failure(&error.to_string()),
            });
        }
    }

    pub fn take_events(&mut self) -> Vec<SessionEvent> {
        self.capture_output();
        self.events.drain(..).collect()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(device_id) = self.session.active_device().cloned() {
            self.stop(&device_id).await?;
        } else {
            self.local_viewer.stop().await?;
            if let Some(mut driver) = self.remote_viewer.take() {
                driver.stop().await;
            }
        }
        Ok(())
    }

    fn capture_output(&mut self) {
        let device_id = local_viewer_device_id();
        for output in self.local_viewer.drain_output() {
            let is_error = output.stream == OutputStream::StandardError
                && output.line.to_ascii_lowercase().contains("error");
            if is_error {
                self.events.push_back(SessionEvent::Diagnostic {
                    device_id: device_id.clone(),
                    severity: DiagnosticSeverity::Error,
                    message: output.line,
                    file: Some(self.session.project().entry_file.display().to_string()),
                    line: None,
                    column: None,
                });
            } else {
                self.events.push_back(SessionEvent::Log {
                    device_id: Some(device_id.clone()),
                    level: if output.stream == OutputStream::StandardError {
                        LogLevel::Warning
                    } else {
                        LogLevel::Information
                    },
                    message: output.line,
                });
            }
        }
    }

    fn capture_remote_events(&mut self) -> Result<()> {
        let events = self
            .remote_viewer
            .as_mut()
            .map(RemoteViewerDriver::poll)
            .transpose()?
            .unwrap_or_default();
        for event in events {
            match event {
                RemoteDriverEvent::Session(event) => self.events.push_back(event),
                RemoteDriverEvent::Connection(event) => {
                    let Some(device_id) = self.session.active_device().cloned() else {
                        continue;
                    };
                    use i_slint_live_preview::remote_client::RemoteClientState;
                    match event.state {
                        RemoteClientState::Connected => {
                            self.complete_launch(&device_id)?;
                        }
                        RemoteClientState::Connecting | RemoteClientState::Disconnected => {
                            if self.pending_launch.as_ref() == Some(&device_id) {
                                self.session.mark_connecting(&device_id)?;
                            } else {
                                self.session.mark_reconnecting(&device_id)?;
                            }
                            self.emit_device(&device_id);
                            if let Some(error) = event.error {
                                self.events.push_back(SessionEvent::Log {
                                    device_id: Some(device_id),
                                    level: LogLevel::Warning,
                                    message: describe_remote_failure(&error),
                                });
                            }
                        }
                        RemoteClientState::Failed => {
                            self.pending_launch = None;
                            self.pending_endpoint_update = None;
                            self.remote_viewer = None;
                            let message =
                                describe_remote_failure(&event.error.unwrap_or_else(|| {
                                    format!(
                                        "Failed to connect to remote viewer at {}",
                                        event.target
                                    )
                                }));
                            self.session.mark_failed(&device_id, &message)?;
                            self.emit_device(&device_id);
                            self.events.push_back(SessionEvent::Error {
                                device_id: Some(device_id),
                                message,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_device(&mut self, device_id: &DeviceId) {
        if let Some(device) = self.session.devices().get(device_id) {
            self.events.push_back(SessionEvent::DeviceChanged { device: device.clone() });
        }
        self.events.push_back(SessionEvent::ActiveDeviceChanged {
            device_id: self.session.active_device().cloned(),
        });
    }

    fn apply_discovery_event(&mut self, event: RemoteDiscoveryEvent) {
        match event {
            RemoteDiscoveryEvent::Upsert(viewer) => {
                let device_id = viewer.id.clone();
                let current_endpoints = viewer.endpoint_strings();
                let previous_endpoints = self
                    .remote_viewers
                    .get(&device_id)
                    .map(DiscoveredRemoteViewer::endpoint_strings)
                    .or_else(|| {
                        self.global_state
                            .remembered_devices
                            .get(&device_id)
                            .map(|profile| profile.addresses.clone())
                    });
                let endpoint_changed =
                    previous_endpoints.is_some_and(|previous| previous != current_endpoints);
                let device = viewer.to_device();
                self.remote_viewers.insert(device_id.clone(), viewer);
                self.session.upsert_device(device);
                if endpoint_changed
                    && self.session.active_device() == Some(&device_id)
                    && self.pending_launch.as_ref() != Some(&device_id)
                {
                    self.pending_endpoint_update = Some(device_id.clone());
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Information,
                        message: "The remote viewer advertised a new network address".into(),
                    });
                }
                self.emit_device(&device_id);
            }
            RemoteDiscoveryEvent::Removed(device_id) => {
                self.remote_viewers.remove(&device_id);
                if self.session.active_device() == Some(&device_id) {
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id),
                        level: LogLevel::Warning,
                        message: "The active remote viewer is no longer discoverable".into(),
                    });
                    return;
                }
                if let Some(remembered) = self.global_state.remembered_devices.get(&device_id) {
                    self.session.upsert_device(remembered.to_device());
                    self.emit_device(&device_id);
                } else if self.session.remove_device(&device_id).is_some() {
                    self.events.push_back(SessionEvent::DeviceRemoved { device_id });
                }
            }
            RemoteDiscoveryEvent::Warning(message) => {
                self.events.push_back(SessionEvent::Log {
                    device_id: None,
                    level: LogLevel::Warning,
                    message,
                });
            }
        }
    }
}

fn local_viewer_device_id() -> DeviceId {
    DeviceId::new(LOCAL_VIEWER_DEVICE_ID).unwrap()
}

fn local_viewer_device() -> Device {
    Device {
        id: local_viewer_device_id(),
        name: "Local Viewer".into(),
        kind: DeviceKind::LocalViewer,
        origin: DeviceOrigin::BuiltIn,
        status: DeviceStatus::Available,
        capabilities: DeviceCapabilities::launchable(),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        platform: Some(std::env::consts::OS.into()),
    }
}

fn manual_viewer_from_profile(profile: &RememberedDevice) -> Result<DiscoveredRemoteViewer> {
    let address = profile
        .addresses
        .first()
        .with_context(|| format!("Manual remote viewer {} has no saved address", profile.id))?;
    let mut viewer = DiscoveredRemoteViewer::manual(address)?;
    viewer.id = profile.id.clone();
    viewer.name = profile.name.clone();
    viewer.slint_version = profile.version.clone();
    viewer.platform = profile.platform.clone().unwrap_or_else(|| "manual".into());
    Ok(viewer)
}

fn describe_remote_failure(error: &str) -> String {
    let lowercase = error.to_ascii_lowercase();
    if lowercase.contains("version mismatch") || lowercase.contains("does not speak slint-preview")
    {
        return error.to_owned();
    }
    if lowercase.contains("permission denied") || lowercase.contains("operation not permitted") {
        return "Local-network access was denied. Allow Slint Springboard to access the local network in system settings, then retry.".into();
    }
    if lowercase.contains("timed out") || lowercase.contains("timeout") {
        return "The remote viewer connection timed out. Keep the Slint Viewer open and make sure both devices are on the same local network.".into();
    }
    if [
        "connection refused",
        "host unreachable",
        "network is unreachable",
        "no route to host",
        "dns",
        "failed to lookup",
        "name or service not known",
    ]
    .iter()
    .any(|needle| lowercase.contains(needle))
    {
        return "The remote viewer is offline or unreachable. Open the Slint Viewer and check that both devices are on the same local network.".into();
    }
    format!("Remote viewer connection failed: {error}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    #[ignore]
    fn fake_viewer_child() {
        println!("fake viewer stdout");
        eprintln!("error: fake viewer diagnostic");
        match std::env::var("SLINT_SPRINGBOARD_FAKE_MODE").as_deref() {
            Ok("wait") => std::thread::sleep(Duration::from_secs(30)),
            Ok("exit") => std::process::exit(17),
            _ => {}
        }
    }

    fn project(directory: &tempfile::TempDir) -> ProjectRunTarget {
        std::fs::write(
            directory.path().join("main.slint"),
            "export component App inherits Window {}",
        )
        .unwrap();
        ProjectRunTarget {
            project_root: directory.path().into(),
            manifest_path: directory.path().join("slint.toml"),
            entry_file: directory.path().join("main.slint"),
            component: "App".into(),
            app: None,
        }
    }

    fn store(directory: &tempfile::TempDir) -> DeviceStateStore {
        DeviceStateStore::new(directory.path().join("config/devices.json"))
    }

    fn fake_command(mode: &str) -> ViewerChildCommand {
        ViewerChildCommand::fake(
            std::env::current_exe().unwrap(),
            ["--exact", "session_driver::tests::fake_viewer_child", "--ignored", "--nocapture"]
                .into_iter()
                .map(Into::into)
                .collect(),
        )
        .with_environment("SLINT_SPRINGBOARD_FAKE_MODE", mode)
    }

    fn remote_viewer() -> DiscoveredRemoteViewer {
        DiscoveredRemoteViewer {
            id: DeviceId::new("remote:phone-id").unwrap(),
            name: "Nigel's iPhone".into(),
            origin: DeviceOrigin::Discovered,
            platform: "ios".into(),
            slint_version: Some(env!("CARGO_PKG_VERSION").into()),
            protocols: vec![i_slint_live_preview::protocol::PROTOCOL_SUBPROTOCOL.into()],
            addresses: vec!["192.0.2.10".into()],
            port: 41000,
        }
    }

    #[tokio::test]
    async fn successful_launch_sets_last_used_and_shutdown_stops_the_child() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        let mut controller =
            ProjectSessionController::new(project(&directory), store.clone(), fake_command("wait"));
        let device_id = local_viewer_device_id();

        controller.launch(&device_id).await.unwrap();

        assert_eq!(controller.last_used_device(), Some(&device_id));
        assert_eq!(store.load().state.last_used_device, Some(device_id.clone()));
        assert_eq!(controller.session().devices()[&device_id].status, DeviceStatus::Running);

        controller.shutdown().await.unwrap();
        assert_eq!(controller.session().active_device(), None);
    }

    #[tokio::test]
    async fn a_spawn_failure_does_not_replace_last_used() {
        let directory = tempfile::tempdir().unwrap();
        let store = store(&directory);
        let command = ViewerChildCommand::fake(directory.path().join("missing-viewer"), Vec::new());
        let mut controller =
            ProjectSessionController::new(project(&directory), store.clone(), command);
        let device_id = local_viewer_device_id();

        assert!(controller.launch(&device_id).await.is_err());

        assert_eq!(controller.last_used_device(), None);
        assert_eq!(store.load().state.last_used_device, None);
        assert!(matches!(
            controller.session().devices()[&device_id].status,
            DeviceStatus::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn child_output_and_unexpected_exit_become_session_events() {
        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("exit"),
        );
        let device_id = local_viewer_device_id();
        controller.launch(&device_id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        controller.poll().await.unwrap();
        let events = controller.take_events();

        assert!(events.iter().any(|event| matches!(
            event,
            SessionEvent::Diagnostic { message, .. }
                if message.contains("fake viewer diagnostic")
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            SessionEvent::Error { message, .. } if message.contains("exited unexpectedly")
        )));
        assert_eq!(controller.session().active_device(), None);
    }

    #[test]
    fn manual_viewers_survive_a_new_project_session() {
        let directory = tempfile::tempdir().unwrap();
        let state_store = store(&directory);
        let mut first = ProjectSessionController::new(
            project(&directory),
            state_store.clone(),
            fake_command("wait"),
        );

        let device_id = first.add_manual_device("viewer.local:41000").unwrap();
        drop(first);
        let second =
            ProjectSessionController::new(project(&directory), state_store, fake_command("wait"));
        let device = &second.session().devices()[&device_id];

        assert_eq!(device.origin, DeviceOrigin::Manual);
        assert_eq!(device.status, DeviceStatus::Available);
        assert_eq!(device.name, "viewer.local:41000");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_offline_last_used_viewer_connects_when_discovered_at_a_new_address() {
        use i_slint_live_preview::remote::Connection;

        let directory = tempfile::tempdir().unwrap();
        let state_store = store(&directory);
        let old_viewer = remote_viewer();
        let mut state = GlobalDeviceState::default();
        state.remember_device(&old_viewer.to_device(), vec!["192.0.2.10:41000".into()]);
        state.last_used_device = Some(old_viewer.id.clone());
        state_store.save(&state).unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            state_store.clone(),
            fake_command("wait"),
        );
        let device_id = old_viewer.id.clone();

        assert_eq!(controller.session().devices()[&device_id].status, DeviceStatus::Unavailable);
        controller.launch(&device_id).await.unwrap();
        assert_eq!(controller.session().devices()[&device_id].status, DeviceStatus::Connecting);

        let connection = Connection::listen(
            Some(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
            Some("Test Viewer".into()),
            |_| {},
        )
        .await
        .unwrap();
        let mut current_viewer = old_viewer;
        current_viewer.name = "Nigel's renamed iPhone".into();
        current_viewer.addresses = vec!["127.0.0.1".into()];
        current_viewer.port = connection.local_port();
        controller.apply_discovery_event(RemoteDiscoveryEvent::Upsert(current_viewer.clone()));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                controller.poll().await.unwrap();
                if controller.session().devices()[&device_id].status == DeviceStatus::Running {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the remembered remote viewer did not connect");

        assert_eq!(controller.session().devices()[&device_id].status, DeviceStatus::Running);
        let stored = state_store.load().state;
        let profile = &stored.remembered_devices[&device_id];
        assert_eq!(profile.name, "Nigel's renamed iPhone");
        assert_eq!(profile.addresses, current_viewer.endpoint_strings());
        assert_eq!(stored.last_used_device, Some(device_id.clone()));

        let replacement = Connection::listen(
            Some(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
            Some("Replacement Viewer".into()),
            |_| {},
        )
        .await
        .unwrap();
        controller.session.mark_reconnecting(&device_id).unwrap();
        current_viewer.port = replacement.local_port();
        controller.apply_discovery_event(RemoteDiscoveryEvent::Upsert(current_viewer.clone()));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                controller.poll().await.unwrap();
                if controller.session().devices()[&device_id].status == DeviceStatus::Running {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the remote viewer did not reconnect at its updated address");

        let stored = state_store.load().state;
        assert_eq!(
            stored.remembered_devices[&device_id].addresses,
            current_viewer.endpoint_strings()
        );

        controller.shutdown().await.unwrap();
    }

    #[test]
    fn expiring_an_unremembered_viewer_removes_it() {
        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let viewer = remote_viewer();
        controller.apply_discovery_event(RemoteDiscoveryEvent::Upsert(viewer.clone()));
        controller.take_events();

        controller.apply_discovery_event(RemoteDiscoveryEvent::Removed(viewer.id.clone()));

        assert!(!controller.session().devices().contains_key(&viewer.id));
        assert_eq!(
            controller.take_events(),
            [SessionEvent::DeviceRemoved { device_id: viewer.id }]
        );
    }

    #[test]
    fn remote_failure_messages_are_actionable_and_preserve_version_details() {
        assert!(describe_remote_failure("Operation not permitted").contains("Local-network"));
        assert!(describe_remote_failure("Connection attempt timed out").contains("timed out"));
        assert!(describe_remote_failure("Connection refused").contains("offline or unreachable"));

        let mismatch = "Version mismatch: viewer runs Slint 1.17.2; client uses Slint 1.18.0";
        assert_eq!(describe_remote_failure(mismatch), mismatch);
    }

    #[test]
    fn expiring_a_remembered_viewer_keeps_an_offline_row() {
        let directory = tempfile::tempdir().unwrap();
        let state_store = store(&directory);
        let viewer = remote_viewer();
        let mut state = GlobalDeviceState::default();
        state.remember_device(&viewer.to_device(), vec!["192.0.2.10:41000".into()]);
        state_store.save(&state).unwrap();
        let mut controller =
            ProjectSessionController::new(project(&directory), state_store, fake_command("wait"));
        controller.apply_discovery_event(RemoteDiscoveryEvent::Upsert(viewer.clone()));
        controller.take_events();

        controller.apply_discovery_event(RemoteDiscoveryEvent::Removed(viewer.id.clone()));

        let device = &controller.session().devices()[&viewer.id];
        assert_eq!(device.origin, DeviceOrigin::Remembered);
        assert_eq!(device.status, DeviceStatus::Unavailable);
        assert!(controller.take_events().iter().any(
            |event| matches!(event, SessionEvent::DeviceChanged { device } if device.id == viewer.id)
        ));
    }
}
