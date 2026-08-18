// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use anyhow::{Context as _, Result, bail};
use i_slint_springboard::{
    Device, DeviceCapabilities, DeviceId, DeviceKind, DeviceOrigin, DeviceStateStore, DeviceStatus,
    DiagnosticSeverity, GlobalDeviceState, LogLevel, SessionAction, SessionEvent,
    SpringboardSession, project::ProjectRunTarget,
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

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
        for device in loaded.state.merge_runtime_devices([local_viewer]) {
            session.upsert_device(device.1);
        }
        let mut events = VecDeque::new();
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
            events,
        }
    }

    pub fn session(&self) -> &SpringboardSession {
        &self.session
    }

    pub fn last_used_device(&self) -> Option<&DeviceId> {
        self.global_state.last_used_device.as_ref()
    }

    pub async fn launch(&mut self, device_id: &DeviceId) -> Result<()> {
        match self.session.launch(device_id)? {
            SessionAction::None => return Ok(()),
            SessionAction::Launch => {}
            action => bail!("Unexpected session action {action:?} for launch"),
        }
        self.emit_device(device_id);
        if device_id.as_str() != LOCAL_VIEWER_DEVICE_ID {
            let message = format!("No target driver is available for {device_id}");
            self.session.mark_failed(device_id, &message)?;
            self.emit_device(device_id);
            bail!(message);
        }

        if let Err(error) = self.local_viewer.launch(self.session.project(), Some("fluent")).await {
            self.session.mark_failed(device_id, error.to_string())?;
            self.emit_device(device_id);
            return Err(error);
        }

        self.session.mark_running(device_id)?;
        self.global_state.last_used_device = Some(device_id.clone());
        self.emit_device(device_id);
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: "Local viewer started".into(),
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
        }
        self.session.mark_stopped(device_id, DeviceStatus::Available)?;
        self.emit_device(device_id);
        self.events.push_back(SessionEvent::Log {
            device_id: Some(device_id.clone()),
            level: LogLevel::Information,
            message: "Local viewer stopped".into(),
        });
        Ok(())
    }

    pub fn poll(&mut self) -> Result<()> {
        self.capture_output();
        let Some(status) = self.local_viewer.poll_exit()? else { return Ok(()) };
        let device_id = local_viewer_device_id();
        if self.session.active_device() == Some(&device_id) {
            let message = format!("Local viewer exited unexpectedly with {status}");
            self.session.mark_failed(&device_id, &message)?;
            self.emit_device(&device_id);
            self.events.push_back(SessionEvent::Error { device_id: Some(device_id), message });
        }
        Ok(())
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

    fn emit_device(&mut self, device_id: &DeviceId) {
        if let Some(device) = self.session.devices().get(device_id) {
            self.events.push_back(SessionEvent::DeviceChanged { device: device.clone() });
        }
        self.events.push_back(SessionEvent::ActiveDeviceChanged {
            device_id: self.session.active_device().cloned(),
        });
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
    }
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
        ProjectRunTarget {
            project_root: directory.path().into(),
            manifest_path: directory.path().join("slint.toml"),
            entry_file: directory.path().join("main.slint"),
            component: "App".into(),
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

        controller.poll().unwrap();
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
}
