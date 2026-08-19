// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use anyhow::{Context as _, Result, bail};
use i_slint_live_preview::springboard_runtime::RuntimeEvent;
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStateStore, DeviceStatus,
    DiagnosticSeverity, GlobalDeviceState, LogLevel, ProjectSnapshot, RememberedDevice,
    SessionAction, SessionEvent, SpringboardSession,
    cargo::{ResolvedCargoApplication, resolve_cargo_application},
    project::ProjectRunTarget,
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::android_emulator::{
    ANDROID_EMULATOR_DEVICE_PREFIX, AndroidEmulator, AndroidEmulatorManager, AndroidLaunchProgress,
    AndroidLaunchResult, DEFAULT_ANDROID_VIEWER_PACKAGE,
};
use crate::artifacts::ArtifactSetupStatus;
use crate::cargo_driver::{
    CargoApplicationDriver, CargoApplicationOutput, CargoApplicationOutputSource,
};
use crate::discovery::{DiscoveredRemoteViewer, RemoteDiscoveryEvent, RemoteViewerDiscovery};
use crate::ios_simulator::{
    DEFAULT_IOS_VIEWER_BUNDLE_ID, IOS_SIMULATOR_DEVICE_PREFIX, IosLaunchProgress, IosLaunchResult,
    IosSimulator, IosSimulatorManager,
};
use crate::remote_driver::{RemoteDriverEvent, RemoteViewerDriver};

pub const LOCAL_VIEWER_DEVICE_ID: &str = "builtin:local-viewer";
pub const RUST_APPLICATION_DEVICE_ID: &str = "builtin:rust-app";

type CargoBuildTask = JoinHandle<(CargoApplicationDriver, std::result::Result<(), String>)>;
type AndroidLaunchTask = JoinHandle<std::result::Result<AndroidLaunchResult, String>>;
type IosLaunchTask = JoinHandle<std::result::Result<IosLaunchResult, String>>;
type AndroidRefreshTask =
    JoinHandle<std::result::Result<(Vec<AndroidEmulator>, ArtifactSetupStatus), String>>;
type IosRefreshTask =
    JoinHandle<std::result::Result<(Vec<IosSimulator>, ArtifactSetupStatus), String>>;

struct AndroidLaunchOperation {
    device_id: DeviceId,
    task: AndroidLaunchTask,
    progress: mpsc::UnboundedReceiver<AndroidLaunchProgress>,
}

struct IosLaunchOperation {
    device_id: DeviceId,
    task: IosLaunchTask,
    progress: mpsc::UnboundedReceiver<IosLaunchProgress>,
}

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
    cargo_target: Option<ResolvedCargoApplication>,
    cargo_application: Option<CargoApplicationDriver>,
    cargo_build: Option<CargoBuildTask>,
    cargo_rebuild_queued: bool,
    remote_viewer: Option<RemoteViewerDriver>,
    remote_discovery: Option<RemoteViewerDiscovery>,
    remote_viewers: BTreeMap<DeviceId, DiscoveredRemoteViewer>,
    android_manager: Option<AndroidEmulatorManager>,
    android_artifact_status: ArtifactSetupStatus,
    android_emulators: BTreeMap<DeviceId, AndroidEmulator>,
    android_preferred_order: Vec<DeviceId>,
    android_unavailable_reason: Option<String>,
    android_launch: Option<AndroidLaunchOperation>,
    android_refresh: Option<AndroidRefreshTask>,
    android_packages: BTreeMap<DeviceId, String>,
    ios_manager: Option<IosSimulatorManager>,
    ios_artifact_status: ArtifactSetupStatus,
    ios_simulators: BTreeMap<DeviceId, IosSimulator>,
    ios_preferred_order: Vec<DeviceId>,
    ios_unavailable_reason: Option<String>,
    ios_launch: Option<IosLaunchOperation>,
    ios_refresh: Option<IosRefreshTask>,
    ios_bundle_ids: BTreeMap<DeviceId, String>,
    managed_remote_devices: BTreeMap<DeviceId, DeviceId>,
    pending_launch: Option<DeviceId>,
    pending_remote_device: Option<DeviceId>,
    pending_android_viewer_name: Option<String>,
    pending_remote_deadline: Option<tokio::time::Instant>,
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
        let (cargo_target, cargo_resolution_error) = match resolve_cargo_application(&project) {
            Ok(target) => (target, None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut session = SpringboardSession::new(project);
        let local_viewer = local_viewer_device();
        let mut remote_viewers = BTreeMap::new();
        let mut runtime_devices = vec![local_viewer];
        let mut events = VecDeque::new();
        if let Some(target) = &cargo_target {
            runtime_devices.push(rust_application_device(target));
        }
        if let Some(error) = cargo_resolution_error {
            events.push_back(SessionEvent::Error {
                device_id: None,
                message: format!("The project Cargo application could not be resolved: {error}"),
            });
        }
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
            cargo_target,
            cargo_application: None,
            cargo_build: None,
            cargo_rebuild_queued: false,
            remote_viewer: None,
            remote_discovery: None,
            remote_viewers,
            android_manager: None,
            android_artifact_status: ArtifactSetupStatus::Ready,
            android_emulators: BTreeMap::new(),
            android_preferred_order: Vec::new(),
            android_unavailable_reason: None,
            android_launch: None,
            android_refresh: None,
            android_packages: BTreeMap::new(),
            ios_manager: None,
            ios_artifact_status: ArtifactSetupStatus::Ready,
            ios_simulators: BTreeMap::new(),
            ios_preferred_order: Vec::new(),
            ios_unavailable_reason: None,
            ios_launch: None,
            ios_refresh: None,
            ios_bundle_ids: BTreeMap::new(),
            managed_remote_devices: BTreeMap::new(),
            pending_launch: None,
            pending_remote_device: None,
            pending_android_viewer_name: None,
            pending_remote_deadline: None,
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

    pub async fn enable_android_emulators(&mut self) {
        let manager = match AndroidEmulatorManager::from_environment() {
            Ok(manager) => manager,
            Err(error) => {
                self.android_unavailable_reason = Some(error.to_string());
                return;
            }
        };
        self.android_manager = Some(manager.clone());
        self.android_artifact_status = manager.artifact_setup_status().await;
        match manager.discover().await {
            Ok(emulators) => {
                self.apply_android_emulators(emulators);
                self.android_unavailable_reason = None;
            }
            Err(error) => {
                self.android_unavailable_reason = Some(error.to_string());
            }
        }
    }

    fn apply_android_emulators(&mut self, emulators: Vec<AndroidEmulator>) {
        let discovered_ids = emulators
            .iter()
            .map(|emulator| emulator.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let stale = self
            .android_emulators
            .keys()
            .filter(|device_id| !discovered_ids.contains(*device_id))
            .cloned()
            .collect::<Vec<_>>();
        for device_id in stale {
            if self.session.active_device() == Some(&device_id) {
                continue;
            }
            self.android_emulators.remove(&device_id);
            if self.session.remove_device(&device_id).is_some() {
                self.events.push_back(SessionEvent::DeviceRemoved { device_id });
            }
        }
        self.android_preferred_order =
            emulators.iter().map(|emulator| emulator.id.clone()).collect();
        for emulator in emulators {
            let device_id = emulator.id.clone();
            let mut device = emulator.to_device();
            if self.session.active_device() == Some(&device_id)
                && let Some(current) = self.session.devices().get(&device_id)
            {
                device.status = current.status.clone();
            } else {
                device.status = artifact_device_status(&self.android_artifact_status);
            }
            self.session.upsert_device(device);
            self.android_emulators.insert(device_id.clone(), emulator);
            self.emit_device(&device_id);
        }
    }

    pub fn preferred_android_emulator(&self) -> Result<DeviceId> {
        if let Some(last) = self.last_used_device()
            && self.android_emulators.contains_key(last)
        {
            return Ok(last.clone());
        }
        self.android_preferred_order.first().cloned().with_context(|| {
            self.android_unavailable_reason.clone().unwrap_or_else(|| {
                "No Android Virtual Devices were found. Create one in Android Studio.".into()
            })
        })
    }

    pub async fn enable_ios_simulators(&mut self) {
        let manager = match IosSimulatorManager::from_environment() {
            Ok(manager) => manager,
            Err(error) => {
                self.ios_unavailable_reason = Some(error.to_string());
                return;
            }
        };
        self.ios_manager = Some(manager.clone());
        self.ios_artifact_status = manager.artifact_setup_status().await;
        match manager.discover().await {
            Ok(simulators) => {
                self.apply_ios_simulators(simulators);
                self.ios_unavailable_reason = None;
            }
            Err(error) => {
                let message = error.to_string();
                self.ios_unavailable_reason = Some(message.clone());
                self.events.push_back(SessionEvent::Log {
                    device_id: None,
                    level: LogLevel::Warning,
                    message,
                });
            }
        }
    }

    fn apply_ios_simulators(&mut self, simulators: Vec<IosSimulator>) {
        let discovered_ids = simulators
            .iter()
            .map(|simulator| simulator.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let stale = self
            .ios_simulators
            .keys()
            .filter(|device_id| !discovered_ids.contains(*device_id))
            .cloned()
            .collect::<Vec<_>>();
        for device_id in stale {
            if self.session.active_device() == Some(&device_id) {
                continue;
            }
            self.ios_simulators.remove(&device_id);
            if self.session.remove_device(&device_id).is_some() {
                self.events.push_back(SessionEvent::DeviceRemoved { device_id });
            }
        }
        self.ios_preferred_order =
            simulators.iter().map(|simulator| simulator.id.clone()).collect();
        for simulator in simulators {
            let device_id = simulator.id.clone();
            let mut device = simulator.to_device();
            if self.session.active_device() == Some(&device_id)
                && let Some(current) = self.session.devices().get(&device_id)
            {
                device.status = current.status.clone();
            } else {
                device.status = artifact_device_status(&self.ios_artifact_status);
            }
            self.session.upsert_device(device);
            self.ios_simulators.insert(device_id.clone(), simulator);
            self.emit_device(&device_id);
        }
    }

    pub fn preferred_ios_simulator(&self) -> Result<DeviceId> {
        if let Some(last) = self.last_used_device()
            && self.ios_simulators.contains_key(last)
        {
            return Ok(last.clone());
        }
        self.ios_preferred_order.first().cloned().with_context(|| {
            self.ios_unavailable_reason.clone().unwrap_or_else(|| {
                "No available iOS Simulators were found. Install an iOS runtime in Xcode.".into()
            })
        })
    }

    pub fn ensure_last_used_simulator_visible(&mut self) {
        let Some(device_id) = self.last_used_device().cloned() else { return };
        if self.session.devices().contains_key(&device_id) {
            return;
        }
        let (kind, name, platform) = if let Some(udid) =
            device_id.as_str().strip_prefix(IOS_SIMULATOR_DEVICE_PREFIX)
        {
            (DeviceKind::IosSimulator, format!("Missing iOS Simulator ({udid})"), "iOS Simulator")
        } else if let Some(avd_name) =
            device_id.as_str().strip_prefix(ANDROID_EMULATOR_DEVICE_PREFIX)
        {
            (DeviceKind::AndroidEmulator, avd_name.into(), "Android Emulator")
        } else {
            return;
        };
        self.session.upsert_device(Device {
            id: device_id.clone(),
            name,
            kind,
            origin: DeviceOrigin::Remembered,
            status: DeviceStatus::Unavailable,
            capabilities: DeviceCapabilities::launchable(),
            version: None,
            platform: Some(platform.into()),
        });
        self.emit_device(&device_id);
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
        let device_kind = self
            .session
            .devices()
            .get(device_id)
            .map(|device| device.kind)
            .with_context(|| format!("Unknown device {device_id}"))?;
        if matches!(device_kind, DeviceKind::AndroidEmulator | DeviceKind::IosSimulator) {
            match &self.session.devices()[device_id].status {
                DeviceStatus::SetupRequired { message } | DeviceStatus::Failed { message } => {
                    bail!("{message}");
                }
                DeviceStatus::Incompatible { installed, required } => {
                    bail!(
                        "Installed viewer support is {installed}; required support is {required}"
                    );
                }
                _ => {}
            }
        }
        if device_kind == DeviceKind::AndroidEmulator
            && !self.android_emulators.contains_key(device_id)
        {
            bail!(
                "Android emulator {device_id} is unavailable. Open Android Studio, restore the AVD, and refresh devices."
            );
        }
        if device_kind == DeviceKind::IosSimulator && !self.ios_simulators.contains_key(device_id) {
            bail!(
                "iOS Simulator {device_id} is unavailable. Restore its runtime in Xcode and refresh devices."
            );
        }
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
        } else if device_id.as_str() == RUST_APPLICATION_DEVICE_ID {
            let driver = match self.cargo_application.take() {
                Some(driver) => driver,
                None => {
                    let target = self
                        .cargo_target
                        .clone()
                        .context("The Rust application target is unavailable")?;
                    match CargoApplicationDriver::new(
                        target,
                        self.session.project().project_root.clone(),
                    )
                    .await
                    {
                        Ok(driver) => driver,
                        Err(error) => {
                            self.session.mark_failed(device_id, error.to_string())?;
                            self.emit_device(device_id);
                            return Err(error);
                        }
                    }
                }
            };
            self.start_cargo_build(driver, DeviceStatus::Compiling)?;
            return Ok(());
        } else if device_kind == DeviceKind::AndroidEmulator {
            let emulator =
                self.android_emulators.get(device_id).cloned().with_context(|| {
                    format!("Android emulator {device_id} is no longer available")
                })?;
            let manager = self
                .android_manager
                .clone()
                .context("Android emulator management is unavailable")?;
            let (progress_sender, progress) = mpsc::unbounded_channel();
            let launch_device_id = device_id.clone();
            let task = tokio::spawn(async move {
                manager
                    .launch(emulator, move |event| {
                        progress_sender.send(event).ok();
                    })
                    .await
                    .map_err(|error| error.to_string())
            });
            self.android_launch =
                Some(AndroidLaunchOperation { device_id: launch_device_id, task, progress });
            return Ok(());
        } else if device_kind == DeviceKind::IosSimulator {
            let simulator = self
                .ios_simulators
                .get(device_id)
                .cloned()
                .with_context(|| format!("iOS Simulator {device_id} is no longer available"))?;
            let manager =
                self.ios_manager.clone().context("iOS Simulator management is unavailable")?;
            let (progress_sender, progress) = mpsc::unbounded_channel();
            let launch_device_id = device_id.clone();
            let task = tokio::spawn(async move {
                manager
                    .launch(simulator, move |event| {
                        progress_sender.send(event).ok();
                    })
                    .await
                    .map_err(|error| error.to_string())
            });
            self.ios_launch =
                Some(IosLaunchOperation { device_id: launch_device_id, task, progress });
            return Ok(());
        } else if let Some(viewer) = self.remote_viewers.get(device_id).cloned() {
            self.session.mark_connecting(device_id)?;
            self.pending_launch = Some(device_id.clone());
            self.pending_remote_device = Some(device_id.clone());
            self.emit_device(device_id);
            if let Err(error) = self.connect_remote_viewer(&viewer).await {
                self.pending_launch = None;
                self.pending_remote_device = None;
                let message = describe_remote_failure(&error.to_string());
                self.session.mark_failed(device_id, &message)?;
                self.emit_device(device_id);
                return Err(anyhow::anyhow!(message));
            }
            return Ok(());
        } else if self.global_state.remembered_devices.contains_key(device_id) {
            self.session.mark_connecting(device_id)?;
            self.pending_launch = Some(device_id.clone());
            self.pending_remote_device = Some(device_id.clone());
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
        self.connect_remote_viewer_for_device(viewer, viewer.id.clone()).await
    }

    async fn connect_remote_viewer_for_device(
        &mut self,
        viewer: &DiscoveredRemoteViewer,
        device_id: DeviceId,
    ) -> Result<()> {
        let mut driver = RemoteViewerDriver::new()?;
        driver.launch_for_device(viewer, self.session.project(), "fluent", device_id).await?;
        self.remote_viewer = Some(driver);
        Ok(())
    }

    fn complete_launch(&mut self, device_id: &DeviceId) -> Result<()> {
        self.pending_launch = None;
        self.pending_remote_device = None;
        self.pending_android_viewer_name = None;
        self.pending_remote_deadline = None;
        if self.pending_endpoint_update.as_ref() == Some(device_id) {
            self.pending_endpoint_update = None;
        }
        let was_rebuild = self
            .session
            .devices()
            .get(device_id)
            .is_some_and(|device| matches!(device.status, DeviceStatus::Rebuilding));
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
            } else if device_id.as_str() == RUST_APPLICATION_DEVICE_ID {
                if was_rebuild {
                    "Rust application rebuilt and restarted".into()
                } else {
                    "Rust application started".into()
                }
            } else if self
                .session
                .devices()
                .get(device_id)
                .is_some_and(|device| device.kind == DeviceKind::IosSimulator)
            {
                "iOS Simulator viewer connected".into()
            } else if self
                .session
                .devices()
                .get(device_id)
                .is_some_and(|device| device.kind == DeviceKind::AndroidEmulator)
            {
                "Android emulator viewer connected".into()
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

    fn start_cargo_build(
        &mut self,
        driver: CargoApplicationDriver,
        status: DeviceStatus,
    ) -> Result<()> {
        let device_id = rust_application_device_id();
        self.session.mark_active_status(&device_id, status)?;
        self.emit_device(&device_id);
        self.cargo_build = Some(tokio::spawn(async move {
            let mut driver = driver;
            let result =
                driver.build_and_launch().await.map(|_| ()).map_err(|error| error.to_string());
            (driver, result)
        }));
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
        } else if device_id.as_str() == RUST_APPLICATION_DEVICE_ID {
            if let Some(task) = self.cargo_build.take() {
                task.abort();
                let _ = task.await;
            }
            if let Some(driver) = &mut self.cargo_application {
                driver.stop().await?;
            }
            self.cargo_rebuild_queued = false;
        } else if self.android_emulators.contains_key(device_id) {
            if let Some(operation) = self.android_launch.take() {
                operation.task.abort();
                let _ = operation.task.await;
            }
            if let Some(mut driver) = self.remote_viewer.take() {
                driver.stop().await;
            }
            if let (Some(manager), Some(emulator)) =
                (&self.android_manager, self.android_emulators.get(device_id))
            {
                let package = self
                    .android_packages
                    .get(device_id)
                    .map(String::as_str)
                    .unwrap_or(DEFAULT_ANDROID_VIEWER_PACKAGE);
                manager.stop(emulator, package).await?;
            }
        } else if self.ios_simulators.contains_key(device_id) {
            if let Some(operation) = self.ios_launch.take() {
                operation.task.abort();
                let _ = operation.task.await;
            }
            if let Some(mut driver) = self.remote_viewer.take() {
                driver.stop().await;
            }
            if let (Some(manager), Some(simulator)) =
                (&self.ios_manager, self.ios_simulators.get(device_id))
            {
                let bundle_id = self
                    .ios_bundle_ids
                    .get(device_id)
                    .map(String::as_str)
                    .unwrap_or(DEFAULT_IOS_VIEWER_BUNDLE_ID);
                manager.stop(simulator, bundle_id).await?;
            }
        } else if let Some(mut driver) = self.remote_viewer.take() {
            driver.stop().await;
        }
        if self.pending_launch.as_ref() == Some(device_id) {
            self.pending_launch = None;
        }
        self.pending_remote_device = None;
        self.pending_android_viewer_name = None;
        self.pending_remote_deadline = None;
        if self.pending_endpoint_update.as_ref() == Some(device_id) {
            self.pending_endpoint_update = None;
        }
        let idle_status = if self.android_emulators.contains_key(device_id) {
            artifact_device_status(&self.android_artifact_status)
        } else if self.ios_simulators.contains_key(device_id) {
            artifact_device_status(&self.ios_artifact_status)
        } else if device_id.as_str() == LOCAL_VIEWER_DEVICE_ID
            || device_id.as_str() == RUST_APPLICATION_DEVICE_ID
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
            } else if device_id.as_str() == RUST_APPLICATION_DEVICE_ID {
                "Rust application stopped".into()
            } else if self.android_emulators.contains_key(device_id) {
                "Android emulator viewer stopped".into()
            } else if self.ios_simulators.contains_key(device_id) {
                "iOS Simulator viewer stopped".into()
            } else {
                "Remote viewer disconnected".into()
            },
        });
        Ok(())
    }

    pub fn refresh(&mut self, device_id: &DeviceId) -> Result<()> {
        let device_kind = self
            .session
            .devices()
            .get(device_id)
            .map(|device| device.kind)
            .with_context(|| format!("Unknown device {device_id}"))?;
        match self.session.refresh(device_id)? {
            SessionAction::Refresh => {
                if device_kind == DeviceKind::AndroidEmulator {
                    if self.android_refresh.is_none()
                        && let Some(manager) = self.android_manager.clone()
                    {
                        self.android_refresh = Some(tokio::spawn(async move {
                            let (devices, setup) =
                                tokio::join!(manager.discover(), manager.artifact_setup_status());
                            devices
                                .map(|devices| (devices, setup))
                                .map_err(|error| error.to_string())
                        }));
                    }
                    if self.session.active_device() == Some(device_id)
                        && self.pending_launch.as_ref() != Some(device_id)
                        && let Some(driver) = self.remote_viewer.as_mut()
                    {
                        driver.refresh()?;
                    }
                } else if device_kind == DeviceKind::IosSimulator {
                    if self.ios_refresh.is_none()
                        && let Some(manager) = self.ios_manager.clone()
                    {
                        self.ios_refresh = Some(tokio::spawn(async move {
                            let (devices, setup) =
                                tokio::join!(manager.discover(), manager.artifact_setup_status());
                            devices
                                .map(|devices| (devices, setup))
                                .map_err(|error| error.to_string())
                        }));
                    }
                    if self.session.active_device() == Some(device_id)
                        && self.pending_launch.as_ref() != Some(device_id)
                        && let Some(driver) = self.remote_viewer.as_mut()
                    {
                        driver.refresh()?;
                    }
                } else if self.session.active_device() == Some(device_id)
                    && device_id.as_str() != LOCAL_VIEWER_DEVICE_ID
                    && device_id.as_str() != RUST_APPLICATION_DEVICE_ID
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

    pub fn rebuild(&mut self, device_id: &DeviceId) -> Result<()> {
        match self.session.rebuild(device_id)? {
            SessionAction::Rebuild => {}
            action => bail!("Unexpected session action {action:?} for rebuild"),
        }
        if self.cargo_build.is_some() {
            self.cargo_rebuild_queued = true;
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id.clone()),
                level: LogLevel::Information,
                message: "Queued a manual rebuild after the current build".into(),
            });
            return Ok(());
        }
        let driver =
            self.cargo_application.take().context("The Rust application driver is unavailable")?;
        self.start_cargo_build(driver, DeviceStatus::Rebuilding)?;
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: "Manual Cargo rebuild requested".into(),
        });
        Ok(())
    }

    pub async fn poll(&mut self) -> Result<()> {
        self.poll_android_launch().await?;
        self.poll_android_refresh().await;
        self.poll_ios_launch().await?;
        self.poll_ios_refresh().await;
        self.poll_cargo_build().await?;
        self.capture_cargo_events()?;
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
        self.expire_pending_remote_viewer()?;
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

    async fn poll_android_refresh(&mut self) {
        if !self.android_refresh.as_ref().is_some_and(|task| task.is_finished()) {
            return;
        }
        let task = self.android_refresh.take().unwrap();
        match task.await {
            Ok(Ok((emulators, setup))) => {
                self.android_artifact_status = setup;
                self.apply_android_emulators(emulators);
                self.events.push_back(SessionEvent::Log {
                    device_id: None,
                    level: LogLevel::Information,
                    message: "Android emulator devices refreshed".into(),
                });
            }
            Ok(Err(message)) => {
                self.events.push_back(SessionEvent::Error { device_id: None, message })
            }
            Err(error) => self.events.push_back(SessionEvent::Error {
                device_id: None,
                message: format!("The Android emulator refresh task failed: {error}"),
            }),
        }
    }

    async fn poll_android_launch(&mut self) -> Result<()> {
        let Some(operation) = &mut self.android_launch else { return Ok(()) };
        let device_id = operation.device_id.clone();
        let mut progress = Vec::new();
        while let Ok(event) = operation.progress.try_recv() {
            progress.push(event);
        }
        let finished = operation.task.is_finished();
        for event in progress {
            if self.session.active_device() != Some(&device_id) {
                continue;
            }
            let status = match event {
                AndroidLaunchProgress::Booting => Some(DeviceStatus::Booting),
                AndroidLaunchProgress::Artifact(
                    crate::artifacts::ArtifactCacheProgress::Importing {
                        bytes_copied,
                        total_bytes,
                    },
                ) => Some(DeviceStatus::Importing { bytes_copied, total_bytes }),
                AndroidLaunchProgress::Artifact(
                    crate::artifacts::ArtifactCacheProgress::UsingPrevious { reason },
                ) => {
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Warning,
                        message: format!(
                            "Using the previously cached Android viewer after an update failure: {reason}"
                        ),
                    });
                    None
                }
                AndroidLaunchProgress::Installing => Some(DeviceStatus::Installing),
                AndroidLaunchProgress::Launching => Some(DeviceStatus::Starting),
                AndroidLaunchProgress::WaitingForDiscovery => Some(DeviceStatus::Connecting),
                AndroidLaunchProgress::Artifact(_) => None,
            };
            if let Some(status) = status {
                self.session.mark_active_status(&device_id, status)?;
                self.emit_device(&device_id);
            }
        }
        if !finished {
            return Ok(());
        }

        let operation = self.android_launch.take().unwrap();
        let result = match operation.task.await {
            Ok(Ok(result)) => result,
            Ok(Err(message)) => {
                self.cleanup_failed_android_launch(&device_id).await;
                self.finish_managed_launch_error(&device_id, message)?;
                return Ok(());
            }
            Err(error) => {
                let message = format!("The Android emulator launch task failed: {error}");
                self.cleanup_failed_android_launch(&device_id).await;
                self.finish_managed_launch_error(&device_id, message)?;
                return Ok(());
            }
        };
        let simulator_id = result.emulator.id.clone();
        self.android_packages.insert(simulator_id.clone(), result.package);
        self.android_emulators.insert(simulator_id.clone(), result.emulator);
        let remote_id = self
            .managed_remote_devices
            .iter()
            .find_map(|(remote_id, managed_id)| {
                (managed_id == &simulator_id).then(|| remote_id.clone())
            })
            .or_else(|| {
                self.remote_viewers.iter().find_map(|(remote_id, viewer)| {
                    (viewer.platform.eq_ignore_ascii_case("android")
                        && viewer.name == result.viewer_name)
                        .then(|| remote_id.clone())
                })
            });
        if let Some(remote_id) = &remote_id {
            self.associate_managed_remote(remote_id.clone(), simulator_id.clone());
        }
        self.pending_launch = Some(simulator_id.clone());
        self.pending_remote_device = remote_id;
        self.pending_android_viewer_name = Some(result.viewer_name);
        self.pending_remote_deadline =
            Some(tokio::time::Instant::now() + std::time::Duration::from_secs(30));
        if self.session.active_device() == Some(&simulator_id) {
            self.session.mark_connecting(&simulator_id)?;
            self.emit_device(&simulator_id);
        }
        Ok(())
    }

    async fn cleanup_failed_android_launch(&mut self, device_id: &DeviceId) {
        let (Some(manager), Some(emulator)) =
            (&self.android_manager, self.android_emulators.get(device_id))
        else {
            return;
        };
        let package = self
            .android_packages
            .get(device_id)
            .map(String::as_str)
            .unwrap_or(DEFAULT_ANDROID_VIEWER_PACKAGE);
        if let Err(error) = manager.stop(emulator, package).await {
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id.clone()),
                level: LogLevel::Warning,
                message: format!(
                    "The partially launched Android emulator viewer could not be stopped: {error}"
                ),
            });
        }
    }

    async fn poll_ios_refresh(&mut self) {
        if !self.ios_refresh.as_ref().is_some_and(|task| task.is_finished()) {
            return;
        }
        let task = self.ios_refresh.take().unwrap();
        match task.await {
            Ok(Ok((simulators, setup))) => {
                self.ios_artifact_status = setup;
                self.apply_ios_simulators(simulators);
                self.events.push_back(SessionEvent::Log {
                    device_id: None,
                    level: LogLevel::Information,
                    message: "iOS Simulator devices refreshed".into(),
                });
            }
            Ok(Err(message)) => {
                self.events.push_back(SessionEvent::Error { device_id: None, message })
            }
            Err(error) => self.events.push_back(SessionEvent::Error {
                device_id: None,
                message: format!("The iOS Simulator refresh task failed: {error}"),
            }),
        }
    }

    async fn poll_ios_launch(&mut self) -> Result<()> {
        let Some(operation) = &mut self.ios_launch else { return Ok(()) };
        let device_id = operation.device_id.clone();
        let mut progress = Vec::new();
        while let Ok(event) = operation.progress.try_recv() {
            progress.push(event);
        }
        let finished = operation.task.is_finished();
        for event in progress {
            if self.session.active_device() != Some(&device_id) {
                continue;
            }
            let status = match event {
                IosLaunchProgress::Artifact(
                    crate::artifacts::ArtifactCacheProgress::Importing {
                        bytes_copied,
                        total_bytes,
                    },
                ) => Some(DeviceStatus::Importing { bytes_copied, total_bytes }),
                IosLaunchProgress::Artifact(
                    crate::artifacts::ArtifactCacheProgress::UsingPrevious { reason },
                ) => {
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Warning,
                        message: format!(
                            "Using the previously cached iOS viewer after an update failure: {reason}"
                        ),
                    });
                    None
                }
                IosLaunchProgress::Booting => Some(DeviceStatus::Booting),
                IosLaunchProgress::Installing => Some(DeviceStatus::Installing),
                IosLaunchProgress::Launching => Some(DeviceStatus::Starting),
                IosLaunchProgress::WaitingForDiscovery => Some(DeviceStatus::Connecting),
                IosLaunchProgress::Artifact(_) => None,
            };
            if let Some(status) = status {
                self.session.mark_active_status(&device_id, status)?;
                self.emit_device(&device_id);
            }
        }
        if !finished {
            return Ok(());
        }

        let operation = self.ios_launch.take().unwrap();
        let result = match operation.task.await {
            Ok(Ok(result)) => result,
            Ok(Err(message)) => {
                self.cleanup_failed_ios_launch(&device_id).await;
                self.finish_managed_launch_error(&device_id, message)?;
                return Ok(());
            }
            Err(error) => {
                let message = format!("The iOS Simulator launch task failed: {error}");
                self.cleanup_failed_ios_launch(&device_id).await;
                self.finish_managed_launch_error(&device_id, message)?;
                return Ok(());
            }
        };
        self.ios_bundle_ids.insert(result.simulator_id.clone(), result.bundle_id);
        self.associate_managed_remote(result.viewer_id.clone(), result.simulator_id.clone());
        self.pending_launch = Some(result.simulator_id.clone());
        self.pending_remote_device = Some(result.viewer_id);
        self.pending_remote_deadline =
            Some(tokio::time::Instant::now() + std::time::Duration::from_secs(30));
        if self.session.active_device() == Some(&result.simulator_id) {
            self.session.mark_connecting(&result.simulator_id)?;
            self.emit_device(&result.simulator_id);
        }
        Ok(())
    }

    async fn cleanup_failed_ios_launch(&mut self, device_id: &DeviceId) {
        let (Some(manager), Some(simulator)) =
            (&self.ios_manager, self.ios_simulators.get(device_id))
        else {
            return;
        };
        let bundle_id = self
            .ios_bundle_ids
            .get(device_id)
            .map(String::as_str)
            .unwrap_or(DEFAULT_IOS_VIEWER_BUNDLE_ID);
        if let Err(error) = manager.stop(simulator, bundle_id).await {
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id.clone()),
                level: LogLevel::Warning,
                message: format!(
                    "The partially launched iOS Simulator viewer could not be stopped: {error}"
                ),
            });
        }
    }

    fn finish_managed_launch_error(&mut self, device_id: &DeviceId, message: String) -> Result<()> {
        if self.session.active_device() == Some(device_id) {
            if let Some((installed, required)) = managed_artifact_incompatibility(&message) {
                self.session
                    .mark_stopped(device_id, DeviceStatus::Incompatible { installed, required })?;
            } else if managed_artifact_setup_required(&message) {
                self.session.mark_stopped(
                    device_id,
                    DeviceStatus::SetupRequired { message: message.clone() },
                )?;
            } else {
                self.session.mark_failed(device_id, &message)?;
            }
            self.emit_device(device_id);
        }
        self.events.push_back(SessionEvent::Error { device_id: Some(device_id.clone()), message });
        Ok(())
    }

    async fn poll_cargo_build(&mut self) -> Result<()> {
        if !self.cargo_build.as_ref().is_some_and(|task| task.is_finished()) {
            return Ok(());
        }
        let task = self.cargo_build.take().unwrap();
        let device_id = rust_application_device_id();
        let (driver, build_result) = match task.await {
            Ok(result) => result,
            Err(error) => {
                let message = format!("Cargo build task failed: {error}");
                if self.session.active_device() == Some(&device_id) {
                    self.session.mark_failed(&device_id, &message)?;
                    self.emit_device(&device_id);
                }
                self.events.push_back(SessionEvent::Error { device_id: Some(device_id), message });
                return Ok(());
            }
        };
        let previous_application_is_running = driver.application_id().is_some();
        self.cargo_application = Some(driver);
        match build_result {
            Ok(()) => {}
            Err(message) => {
                if self.session.active_device() == Some(&device_id) {
                    if previous_application_is_running {
                        self.session.mark_active_status(
                            &device_id,
                            DeviceStatus::RunningWithError { message: message.clone() },
                        )?;
                    } else {
                        self.session.mark_failed(&device_id, &message)?;
                    }
                    self.emit_device(&device_id);
                }
                self.events
                    .push_back(SessionEvent::Error { device_id: Some(device_id.clone()), message });
            }
        }

        if std::mem::take(&mut self.cargo_rebuild_queued)
            && self.session.active_device() == Some(&device_id)
            && let Some(driver) = self.cargo_application.take()
        {
            self.start_cargo_build(driver, DeviceStatus::Rebuilding)?;
        }
        Ok(())
    }

    fn capture_cargo_events(&mut self) -> Result<()> {
        if self.cargo_build.is_some() {
            return Ok(());
        }
        let Some(driver) = &mut self.cargo_application else { return Ok(()) };
        let mut runtime_events = Vec::new();
        while let Some(event) = driver.take_runtime_event() {
            runtime_events.push(event);
        }
        let rebuild_requested = driver.take_rebuild_request()?;
        let hot_reload_activity = driver.take_hot_reload_activity();
        let process_exit = driver.poll_exit()?;
        let output = driver.take_output();

        let device_id = rust_application_device_id();
        if hot_reload_activity
            && self.session.active_device() == Some(&device_id)
            && !matches!(
                self.session.devices()[&device_id].status,
                DeviceStatus::Compiling | DeviceStatus::Rebuilding | DeviceStatus::Stopping
            )
        {
            self.session.mark_active_status(&device_id, DeviceStatus::Reloading)?;
            self.emit_device(&device_id);
        }
        for event in runtime_events {
            match event {
                RuntimeEvent::Ready { .. } => {
                    if self.session.active_device() == Some(&device_id)
                        && matches!(
                            self.session.devices()[&device_id].status,
                            DeviceStatus::Compiling | DeviceStatus::Rebuilding
                        )
                    {
                        self.complete_launch(&device_id)?;
                    }
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Debug,
                        message: "Rust live-preview runtime is ready".into(),
                    });
                }
                RuntimeEvent::Reloaded { .. } => {
                    if self.session.active_device() == Some(&device_id) {
                        self.session.mark_running(&device_id)?;
                        self.emit_device(&device_id);
                    }
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Information,
                        message: "Slint implementation reloaded without Cargo".into(),
                    });
                }
                RuntimeEvent::CompileError { .. } => {
                    let message =
                        "Slint compilation failed; the previous application UI remains active";
                    if self.session.active_device() == Some(&device_id) {
                        self.session.mark_active_status(
                            &device_id,
                            DeviceStatus::RunningWithError { message: message.into() },
                        )?;
                        self.emit_device(&device_id);
                    }
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Warning,
                        message: message.into(),
                    });
                }
                RuntimeEvent::RebuildRequired { diff, .. } => {
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Information,
                        message: format!(
                            "The Slint Rust interface changed; rebuilding the application:\n{diff}"
                        ),
                    });
                }
                RuntimeEvent::Exiting => {
                    self.events.push_back(SessionEvent::Log {
                        device_id: Some(device_id.clone()),
                        level: LogLevel::Debug,
                        message: "Rust live-preview runtime is exiting".into(),
                    });
                }
            }
        }
        for output in output {
            self.capture_cargo_output(&device_id, output);
        }

        if let Some(status) = process_exit
            && self.session.active_device() == Some(&device_id)
        {
            let message = format!("Rust application exited unexpectedly with {status}");
            self.session.mark_failed(&device_id, &message)?;
            self.emit_device(&device_id);
            self.events
                .push_back(SessionEvent::Error { device_id: Some(device_id.clone()), message });
        }

        if rebuild_requested
            && self.session.active_device() == Some(&device_id)
            && let Some(driver) = self.cargo_application.take()
        {
            self.start_cargo_build(driver, DeviceStatus::Rebuilding)?;
        }
        Ok(())
    }

    fn capture_cargo_output(&mut self, device_id: &DeviceId, output: CargoApplicationOutput) {
        let lowercase = output.line.to_ascii_lowercase();
        let is_error = lowercase.contains("error")
            && matches!(
                output.source,
                CargoApplicationOutputSource::Cargo
                    | CargoApplicationOutputSource::ApplicationStandardError
            );
        if is_error {
            self.events.push_back(SessionEvent::Diagnostic {
                device_id: device_id.clone(),
                severity: DiagnosticSeverity::Error,
                message: output.line,
                file: None,
                line: None,
                column: None,
            });
        } else {
            self.events.push_back(SessionEvent::Log {
                device_id: Some(device_id.clone()),
                level: match output.source {
                    CargoApplicationOutputSource::Cargo => LogLevel::Information,
                    CargoApplicationOutputSource::ApplicationStandardOutput => LogLevel::Debug,
                    CargoApplicationOutputSource::ApplicationStandardError => LogLevel::Warning,
                },
                message: output.line,
            });
        }
    }

    async fn connect_pending_viewer(&mut self) {
        let Some(device_id) = self.pending_launch.clone() else { return };
        if self.remote_viewer.is_some() {
            return;
        }
        let Some(remote_device_id) = self.pending_remote_device.as_ref() else { return };
        let Some(viewer) = self.remote_viewers.get(remote_device_id).cloned() else { return };
        match self.connect_remote_viewer_for_device(&viewer, device_id.clone()).await {
            Ok(()) => {}
            Err(error) => {
                self.pending_launch = None;
                self.pending_remote_device = None;
                self.pending_android_viewer_name = None;
                self.pending_remote_deadline = None;
                let message = describe_remote_failure(&error.to_string());
                if self.session.mark_failed(&device_id, &message).is_ok() {
                    self.emit_device(&device_id);
                }
                self.events.push_back(SessionEvent::Error { device_id: Some(device_id), message });
            }
        }
    }

    fn expire_pending_remote_viewer(&mut self) -> Result<()> {
        let Some(deadline) = self.pending_remote_deadline else { return Ok(()) };
        if tokio::time::Instant::now() < deadline {
            return Ok(());
        }
        let Some(device_id) = self.pending_launch.take() else {
            self.pending_remote_deadline = None;
            return Ok(());
        };
        self.pending_remote_device = None;
        self.pending_android_viewer_name = None;
        self.pending_remote_deadline = None;
        self.remote_viewer = None;
        let message = "The launched simulator viewer was not discovered on the local network. Check the emulator network and retry.";
        if self.session.active_device() == Some(&device_id) {
            self.session.mark_failed(&device_id, message)?;
            self.emit_device(&device_id);
        }
        self.events
            .push_back(SessionEvent::Error { device_id: Some(device_id), message: message.into() });
        Ok(())
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
        if let Some(task) = self.android_refresh.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(task) = self.ios_refresh.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(device_id) = self.session.active_device().cloned() {
            self.stop(&device_id).await?;
        } else {
            self.local_viewer.stop().await?;
            if let Some(task) = self.cargo_build.take() {
                task.abort();
                let _ = task.await;
            }
            if let Some(driver) = &mut self.cargo_application {
                driver.stop().await?;
            }
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
                            self.pending_remote_device = None;
                            self.pending_android_viewer_name = None;
                            self.pending_remote_deadline = None;
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

    fn associate_managed_remote(&mut self, remote_id: DeviceId, simulator_id: DeviceId) {
        self.managed_remote_devices.insert(remote_id.clone(), simulator_id);
        if self.session.remove_device(&remote_id).is_some() {
            self.events.push_back(SessionEvent::DeviceRemoved { device_id: remote_id });
        }
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
                let pending_android_simulator =
                    self.pending_launch.clone().filter(|simulator_id| {
                        self.session
                            .devices()
                            .get(simulator_id)
                            .is_some_and(|device| device.kind == DeviceKind::AndroidEmulator)
                    });
                if pending_android_simulator.is_some()
                    && self.pending_remote_device.is_none()
                    && self.pending_android_viewer_name.as_deref()
                        == self.remote_viewers.get(&device_id).map(|viewer| viewer.name.as_str())
                    && self
                        .remote_viewers
                        .get(&device_id)
                        .is_some_and(|viewer| viewer.platform.eq_ignore_ascii_case("android"))
                {
                    let simulator_id = pending_android_simulator.unwrap();
                    self.pending_remote_device = Some(device_id.clone());
                    self.pending_android_viewer_name = None;
                    self.associate_managed_remote(device_id, simulator_id);
                    return;
                }
                if let Some(simulator_id) = self.managed_remote_devices.get(&device_id).cloned() {
                    if self.session.remove_device(&device_id).is_some() {
                        self.events.push_back(SessionEvent::DeviceRemoved {
                            device_id: device_id.clone(),
                        });
                    }
                    if endpoint_changed && self.session.active_device() == Some(&simulator_id) {
                        self.events.push_back(SessionEvent::Log {
                            device_id: Some(simulator_id),
                            level: LogLevel::Information,
                            message: "The iOS Simulator viewer advertised a new network address"
                                .into(),
                        });
                    }
                    return;
                }
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
                if let Some(simulator_id) = self.managed_remote_devices.get(&device_id).cloned() {
                    if self.session.active_device() == Some(&simulator_id) {
                        self.events.push_back(SessionEvent::Log {
                            device_id: Some(simulator_id),
                            level: LogLevel::Warning,
                            message: "The active iOS Simulator viewer is no longer discoverable"
                                .into(),
                        });
                    }
                    return;
                }
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

fn rust_application_device_id() -> DeviceId {
    DeviceId::new(RUST_APPLICATION_DEVICE_ID).unwrap()
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

fn rust_application_device(target: &ResolvedCargoApplication) -> Device {
    let mut capabilities = DeviceCapabilities::launchable();
    capabilities.rebuild = true;
    Device {
        id: rust_application_device_id(),
        name: format!("Rust Application ({})", target.binary),
        kind: DeviceKind::RustApplication,
        origin: DeviceOrigin::BuiltIn,
        status: DeviceStatus::Available,
        capabilities,
        version: None,
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

fn artifact_device_status(status: &ArtifactSetupStatus) -> DeviceStatus {
    match status {
        ArtifactSetupStatus::Ready => DeviceStatus::Available,
        ArtifactSetupStatus::SetupRequired { message } => {
            DeviceStatus::SetupRequired { message: message.clone() }
        }
        ArtifactSetupStatus::Incompatible { installed, required } => {
            DeviceStatus::Incompatible { installed: installed.clone(), required: required.clone() }
        }
        ArtifactSetupStatus::Failed { message } => {
            DeviceStatus::Failed { message: message.clone() }
        }
    }
}

fn managed_artifact_incompatibility(message: &str) -> Option<(String, String)> {
    if let Some(mismatch) = message.strip_prefix("Installed viewer artifact support is ") {
        let (installed, required) = mismatch.split_once(", expected ")?;
        return Some((installed.into(), required.into()));
    }
    let mismatch = message.strip_prefix("Viewer artifact manifest contains Slint ")?;
    let (installed, required) = mismatch.split_once(", expected ")?;
    Some((format!("Slint {installed}"), format!("Slint {required}")))
}

fn managed_artifact_setup_required(message: &str) -> bool {
    message.contains(i_slint_springboard::SPRINGBOARD_ARTIFACT_DIR_ENVIRONMENT_VARIABLE)
        || message.contains("does not contain the required")
        || message.contains("Cannot read local viewer artifact")
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

    fn add_cargo_application(directory: &tempfile::TempDir) {
        std::fs::create_dir_all(directory.path().join("src")).unwrap();
        std::fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"springboard-test-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(directory.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    }

    fn cargo_fixture_project(directory: &tempfile::TempDir) -> ProjectRunTarget {
        const FILES: &[(&str, &str)] = &[
            ("Cargo.toml", include_str!("tests/fixtures/cargo-application/Cargo.toml")),
            ("Cargo.lock", include_str!("tests/fixtures/cargo-application/Cargo.lock")),
            ("build.rs", include_str!("tests/fixtures/cargo-application/build.rs")),
            ("src/main.rs", include_str!("tests/fixtures/cargo-application/src/main.rs")),
            ("ui/app.slint", include_str!("tests/fixtures/cargo-application/ui/app.slint")),
            ("ui/resource.txt", include_str!("tests/fixtures/cargo-application/ui/resource.txt")),
            ("slint.toml", include_str!("tests/fixtures/cargo-application/slint.toml")),
        ];
        for (relative_path, contents) in FILES {
            let path = directory.path().join(relative_path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        i_slint_springboard::project::load_project_run_target(directory.path()).unwrap().unwrap()
    }

    fn cargo_fixture_pid(controller: &ProjectSessionController) -> Option<u32> {
        controller.cargo_application.as_ref().and_then(CargoApplicationDriver::application_id)
    }

    fn cargo_fixture_build_count(directory: &tempfile::TempDir) -> u32 {
        std::fs::read_to_string(directory.path().join("target/springboard-build-count"))
            .ok()
            .and_then(|count| count.parse().ok())
            .unwrap_or_default()
    }

    async fn wait_for_cargo_fixture(
        controller: &mut ProjectSessionController,
        description: &str,
        mut ready: impl FnMut(&ProjectSessionController) -> bool,
    ) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                controller.poll().await.unwrap();
                if ready(controller) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for {description}"));
    }

    async fn wait_for_cargo_fixture_log(
        controller: &mut ProjectSessionController,
        description: &str,
        needle: &str,
    ) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                controller.poll().await.unwrap();
                if controller.take_events().iter().any(|event| {
                    matches!(event, SessionEvent::Log { message, .. } if message.contains(needle))
                }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for {description}"));
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

    #[test]
    fn a_resolvable_cargo_binary_adds_the_rust_application_device() {
        let directory = tempfile::tempdir().unwrap();
        add_cargo_application(&directory);
        let controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let device_id = rust_application_device_id();
        let device = &controller.session().devices()[&device_id];

        assert_eq!(device.kind, DeviceKind::RustApplication);
        assert_eq!(device.status, DeviceStatus::Available);
        assert!(device.capabilities.rebuild);
        assert!(device.name.contains("springboard-test-app"));
    }

    #[test]
    fn simulator_refresh_preserves_active_state_and_removes_stale_devices() {
        use crate::ios_simulator::IosSimulatorState;

        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let active_id = DeviceId::new("simulator:ios:active").unwrap();
        let stale_id = DeviceId::new("simulator:ios:stale").unwrap();
        let simulator = |id: DeviceId, name: &str, state| IosSimulator {
            udid: id.as_str().trim_start_matches("simulator:ios:").into(),
            id,
            name: name.into(),
            runtime: "26.5".into(),
            state,
        };
        controller.apply_ios_simulators(vec![
            simulator(active_id.clone(), "iPhone 17", IosSimulatorState::Booted),
            simulator(stale_id.clone(), "iPhone 16", IosSimulatorState::Shutdown),
        ]);
        controller.session.launch(&active_id).unwrap();
        controller.session.mark_running(&active_id).unwrap();
        controller.take_events();

        controller.apply_ios_simulators(vec![simulator(
            active_id.clone(),
            "iPhone 17",
            IosSimulatorState::Booted,
        )]);

        assert_eq!(controller.preferred_ios_simulator().unwrap(), active_id);
        assert_eq!(controller.session.devices()[&active_id].status, DeviceStatus::Running);
        assert!(!controller.session.devices().contains_key(&stale_id));
        assert!(controller.take_events().iter().any(
            |event| matches!(event, SessionEvent::DeviceRemoved { device_id } if device_id == &stale_id)
        ));
    }

    #[tokio::test]
    async fn simulator_setup_errors_are_visible_and_prevent_launch() {
        use crate::ios_simulator::IosSimulatorState;

        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let device_id = DeviceId::new("simulator:ios:setup-required").unwrap();
        let message = format!(
            "Set {} to an absolute directory containing {} and the referenced iOS Simulator ZIP.",
            i_slint_springboard::SPRINGBOARD_ARTIFACT_DIR_ENVIRONMENT_VARIABLE,
            i_slint_springboard::MOBILE_VIEWER_ARTIFACT_MANIFEST_FILE
        );
        controller.ios_artifact_status =
            ArtifactSetupStatus::SetupRequired { message: message.clone() };
        controller.apply_ios_simulators(vec![IosSimulator {
            id: device_id.clone(),
            udid: "setup-required".into(),
            name: "iPhone 17".into(),
            runtime: "26.5".into(),
            state: IosSimulatorState::Shutdown,
        }]);

        assert_eq!(
            controller.session.devices()[&device_id].status,
            DeviceStatus::SetupRequired { message: message.clone() }
        );
        assert_eq!(controller.launch(&device_id).await.unwrap_err().to_string(), message);
        assert_eq!(controller.session.active_device(), None);
        assert!(controller.ios_launch.is_none());
    }

    #[test]
    fn android_discovery_is_associated_with_the_launched_emulator() {
        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let simulator_id = DeviceId::new("simulator:android:Pixel_9_API_36").unwrap();
        controller.apply_android_emulators(vec![AndroidEmulator {
            id: simulator_id.clone(),
            avd_name: "Pixel_9_API_36".into(),
            serial: Some("emulator-5554".into()),
        }]);
        controller.session.launch(&simulator_id).unwrap();
        controller.pending_launch = Some(simulator_id.clone());
        controller.pending_android_viewer_name = Some("Android SDK built for arm64".into());
        controller.take_events();
        let mut viewer = remote_viewer();
        viewer.name = "Android SDK built for arm64".into();
        viewer.platform = "android".into();
        let remote_id = viewer.id.clone();

        controller.apply_discovery_event(RemoteDiscoveryEvent::Upsert(viewer));

        assert_eq!(controller.pending_remote_device, Some(remote_id.clone()));
        assert_eq!(controller.managed_remote_devices.get(&remote_id), Some(&simulator_id));
        assert!(!controller.session.devices().contains_key(&remote_id));
        assert_eq!(controller.preferred_android_emulator().unwrap(), simulator_id);
    }

    #[tokio::test]
    async fn a_missing_last_used_simulator_remains_visible_without_taking_the_target_slot() {
        let directory = tempfile::tempdir().unwrap();
        let state_store = store(&directory);
        let missing_id = DeviceId::new("simulator:ios:deleted-udid").unwrap();
        let state =
            GlobalDeviceState { last_used_device: Some(missing_id.clone()), ..Default::default() };
        state_store.save(&state).unwrap();
        let mut controller =
            ProjectSessionController::new(project(&directory), state_store, fake_command("wait"));

        controller.ensure_last_used_simulator_visible();

        let missing = &controller.session.devices()[&missing_id];
        assert_eq!(missing.kind, DeviceKind::IosSimulator);
        assert_eq!(missing.status, DeviceStatus::Unavailable);
        assert!(controller.launch(&missing_id).await.unwrap_err().to_string().contains("Xcode"));
        assert_eq!(controller.session.active_device(), None);
    }

    #[test]
    fn managed_artifact_version_mismatches_have_an_incompatible_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut controller = ProjectSessionController::new(
            project(&directory),
            store(&directory),
            fake_command("wait"),
        );
        let simulator_id = DeviceId::new("simulator:android:Pixel").unwrap();
        controller.apply_android_emulators(vec![AndroidEmulator {
            id: simulator_id.clone(),
            avd_name: "Pixel".into(),
            serial: None,
        }]);
        controller.session.launch(&simulator_id).unwrap();

        controller
            .finish_managed_launch_error(
                &simulator_id,
                "Installed viewer artifact support is Slint 1.17.2, expected Slint 1.18.0".into(),
            )
            .unwrap();

        assert_eq!(
            controller.session.devices()[&simulator_id].status,
            DeviceStatus::Incompatible {
                installed: "Slint 1.17.2".into(),
                required: "Slint 1.18.0".into()
            }
        );
        assert_eq!(controller.session.active_device(), None);
    }

    #[tokio::test]
    async fn selective_cargo_rebuilds_preserve_the_last_successful_process() {
        let directory = tempfile::tempdir().unwrap();
        let state_directory = tempfile::tempdir().unwrap();
        let project = cargo_fixture_project(&directory);
        let entry_file = project.entry_file.clone();
        let resource_file = directory.path().join("ui/resource.txt");
        let rust_file = directory.path().join("src/main.rs");
        let original_rust = std::fs::read_to_string(&rust_file).unwrap();
        let mut controller =
            ProjectSessionController::new(project, store(&state_directory), fake_command("wait"));
        let device_id = rust_application_device_id();

        controller.launch(&device_id).await.unwrap();
        wait_for_cargo_fixture(&mut controller, "the initial Cargo application", |controller| {
            controller.session().devices()[&device_id].status == DeviceStatus::Running
                && cargo_fixture_pid(controller).is_some()
        })
        .await;
        wait_for_cargo_fixture_log(
            &mut controller,
            "the live-preview runtime handshake",
            "runtime is ready",
        )
        .await;
        let initial_pid = cargo_fixture_pid(&controller).unwrap();
        assert_eq!(cargo_fixture_build_count(&directory), 1);

        std::fs::write(
            &entry_file,
            "export component App inherits Window { Text { text: \"Layout edit\"; } }\n",
        )
        .unwrap();
        wait_for_cargo_fixture_log(
            &mut controller,
            "an implementation-only Slint reload",
            "implementation reloaded without Cargo",
        )
        .await;
        assert_eq!(cargo_fixture_pid(&controller), Some(initial_pid));
        assert_eq!(cargo_fixture_build_count(&directory), 1);

        std::fs::write(
            &entry_file,
            "export component App inherits Window { Text { color: #246; text: \"Style edit\"; } }\n",
        )
        .unwrap();
        wait_for_cargo_fixture_log(
            &mut controller,
            "a style-only Slint reload",
            "implementation reloaded without Cargo",
        )
        .await;
        assert_eq!(cargo_fixture_pid(&controller), Some(initial_pid));
        assert_eq!(cargo_fixture_build_count(&directory), 1);

        std::fs::write(&resource_file, "changed resource\n").unwrap();
        wait_for_cargo_fixture_log(
            &mut controller,
            "a Slint resource reload",
            "implementation reloaded without Cargo",
        )
        .await;
        assert_eq!(cargo_fixture_pid(&controller), Some(initial_pid));
        assert_eq!(cargo_fixture_build_count(&directory), 1);

        std::fs::write(
            &entry_file,
            "// springboard-fixture: interface-change\nexport component App inherits Window { in property <int> value; }\n",
        )
        .unwrap();
        wait_for_cargo_fixture(&mut controller, "the Slint interface rebuild", |controller| {
            controller.session().devices()[&device_id].status == DeviceStatus::Running
                && cargo_fixture_pid(controller).is_some_and(|pid| pid != initial_pid)
                && cargo_fixture_build_count(&directory) == 2
        })
        .await;
        let interface_pid = cargo_fixture_pid(&controller).unwrap();
        assert!(controller.take_events().iter().any(|event| {
            matches!(
                event,
                SessionEvent::Log { message, .. } if message.contains("exported property changed")
            )
        }));

        std::fs::write(&rust_file, format!("{original_rust}\n// Rust implementation edit\n"))
            .unwrap();
        wait_for_cargo_fixture(&mut controller, "the Rust source rebuild", |controller| {
            controller.session().devices()[&device_id].status == DeviceStatus::Running
                && cargo_fixture_pid(controller).is_some_and(|pid| pid != interface_pid)
                && cargo_fixture_build_count(&directory) == 3
        })
        .await;
        let rust_pid = cargo_fixture_pid(&controller).unwrap();

        std::fs::write(
            &entry_file,
            "// springboard-fixture: compile-error\nexport component App inherits Window {\n",
        )
        .unwrap();
        wait_for_cargo_fixture(&mut controller, "the Slint compile error", |controller| {
            matches!(
                controller.session().devices()[&device_id].status,
                DeviceStatus::RunningWithError { .. }
            )
        })
        .await;
        assert_eq!(cargo_fixture_pid(&controller), Some(rust_pid));
        assert_eq!(cargo_fixture_build_count(&directory), 3);

        std::fs::write(
            &entry_file,
            "export component App inherits Window { Text { text: \"Recovered\"; } }\n",
        )
        .unwrap();
        wait_for_cargo_fixture_log(
            &mut controller,
            "recovery from the Slint error",
            "implementation reloaded without Cargo",
        )
        .await;
        assert_eq!(controller.session().devices()[&device_id].status, DeviceStatus::Running);

        std::fs::write(&rust_file, format!("{original_rust}\nthis is not valid Rust;\n")).unwrap();
        wait_for_cargo_fixture(&mut controller, "the failed Rust rebuild", |controller| {
            cargo_fixture_build_count(&directory) == 4
                && cargo_fixture_pid(controller) == Some(rust_pid)
                && matches!(
                    controller.session().devices()[&device_id].status,
                    DeviceStatus::RunningWithError { .. }
                )
        })
        .await;

        std::fs::write(&rust_file, format!("{original_rust}\n// Recovered Rust edit\n")).unwrap();
        std::fs::write(
            &entry_file,
            "export component App inherits Window { Text { text: \"Mixed edit\"; } }\n",
        )
        .unwrap();
        std::fs::write(&resource_file, "rapid mixed resource edit\n").unwrap();
        wait_for_cargo_fixture(&mut controller, "the coalesced mixed rebuild", |controller| {
            controller.session().devices()[&device_id].status == DeviceStatus::Running
                && cargo_fixture_pid(controller).is_some_and(|pid| pid != rust_pid)
                && cargo_fixture_build_count(&directory) == 5
        })
        .await;
        let final_pid = cargo_fixture_pid(&controller).unwrap();
        tokio::time::sleep(i_slint_live_preview::REBUILD_DEBOUNCE * 2).await;
        controller.poll().await.unwrap();
        assert_eq!(cargo_fixture_build_count(&directory), 5);
        assert_eq!(cargo_fixture_pid(&controller), Some(final_pid));

        controller.shutdown().await.unwrap();
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
