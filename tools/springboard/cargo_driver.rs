// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Build and process management for Rust live-preview applications.

use crate::runtime_control::RuntimeControlServer;
use anyhow::{Context as _, Result, bail};
use i_slint_live_preview::springboard_runtime::RuntimeEvent;
use i_slint_springboard::cargo::ResolvedCargoApplication;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// The process that produced one line of Cargo application output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CargoApplicationOutputSource {
    Cargo,
    ApplicationStandardOutput,
    ApplicationStandardError,
}

/// One captured line from Cargo or the launched application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoApplicationOutput {
    pub source: CargoApplicationOutputSource,
    pub line: String,
}

enum WorkspaceWatchMessage {
    Changed(Vec<PathBuf>),
    Error(String),
}

struct WorkspaceRebuildWatcher {
    _watcher: notify::RecommendedWatcher,
    events: mpsc::UnboundedReceiver<WorkspaceWatchMessage>,
    planner: RebuildPlanner,
    hot_reload_activity: bool,
}

impl WorkspaceRebuildWatcher {
    fn new(project_root: &Path) -> Result<Self> {
        use notify::Watcher as _;

        let project_root = project_root.canonicalize().with_context(|| {
            format!("Failed to resolve Cargo application root {}", project_root.display())
        })?;
        let (sender, events) = mpsc::unbounded_channel();
        let error_sender = sender.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) if is_rebuild_event(&event.kind) => {
                    sender.send(WorkspaceWatchMessage::Changed(event.paths)).ok();
                }
                Ok(_) => {}
                Err(error) => {
                    if !is_transient_watcher_error(&error) {
                        error_sender.send(WorkspaceWatchMessage::Error(error.to_string())).ok();
                    }
                }
            })
            .context("Failed to create the Cargo workspace watcher")?;
        watcher.watch(&project_root, notify::RecursiveMode::Recursive).with_context(|| {
            format!("Failed to watch Cargo workspace {}", project_root.display())
        })?;
        Ok(Self {
            _watcher: watcher,
            events,
            planner: RebuildPlanner::new(project_root),
            hot_reload_activity: false,
        })
    }

    fn update_hot_reload_paths(&mut self, paths: &[PathBuf]) {
        self.planner.update_hot_reload_paths(paths);
    }

    fn request_manual_rebuild(&mut self) {
        self.planner.request_manual_rebuild();
    }

    fn take_rebuild_request(&mut self) -> Result<bool> {
        let now = Instant::now();
        while let Ok(event) = self.events.try_recv() {
            match event {
                WorkspaceWatchMessage::Changed(paths) => {
                    for path in paths {
                        self.hot_reload_activity |= self.planner.record_change(path, now);
                    }
                }
                WorkspaceWatchMessage::Error(error) => {
                    bail!("Cargo workspace watcher failed: {error}");
                }
            }
        }
        Ok(self.planner.take_rebuild_request(now))
    }

    fn take_hot_reload_activity(&mut self) -> bool {
        std::mem::take(&mut self.hot_reload_activity)
    }
}

fn is_transient_watcher_error(error: &notify::Error) -> bool {
    match &error.kind {
        notify::ErrorKind::PathNotFound => true,
        notify::ErrorKind::Io(error) => error.kind() == std::io::ErrorKind::NotFound,
        _ => false,
    }
}

fn is_rebuild_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Any
            | notify::EventKind::Create(_)
            | notify::EventKind::Modify(_)
            | notify::EventKind::Remove(_)
            | notify::EventKind::Other
    )
}

struct RebuildPlanner {
    project_root: PathBuf,
    hot_reload_paths: BTreeSet<PathBuf>,
    pending_paths: BTreeSet<PathBuf>,
    last_change: Option<Instant>,
    manual_rebuild: bool,
}

impl RebuildPlanner {
    fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            hot_reload_paths: BTreeSet::new(),
            pending_paths: BTreeSet::new(),
            last_change: None,
            manual_rebuild: false,
        }
    }

    fn update_hot_reload_paths(&mut self, paths: &[PathBuf]) {
        self.hot_reload_paths = paths.iter().map(|path| self.normalize(path)).collect();
        self.pending_paths.retain(|path| !self.hot_reload_paths.contains(path));
        if self.pending_paths.is_empty() {
            self.last_change = None;
        }
    }

    fn record_change(&mut self, path: PathBuf, now: Instant) -> bool {
        let path = self.normalize(&path);
        if self.is_ignored(&path) {
            return false;
        }
        if self.hot_reload_paths.contains(&path) {
            return true;
        }
        self.pending_paths.insert(path);
        self.last_change = Some(now);
        false
    }

    fn request_manual_rebuild(&mut self) {
        self.manual_rebuild = true;
    }

    fn take_rebuild_request(&mut self, now: Instant) -> bool {
        if std::mem::take(&mut self.manual_rebuild) {
            self.pending_paths.clear();
            self.last_change = None;
            return true;
        }
        if self.pending_paths.is_empty()
            || !self.last_change.is_some_and(|changed| {
                now.saturating_duration_since(changed) >= i_slint_live_preview::REBUILD_DEBOUNCE
            })
        {
            return false;
        }
        self.pending_paths.clear();
        self.last_change = None;
        true
    }

    fn normalize(&self, path: &Path) -> PathBuf {
        let path =
            if path.is_absolute() { path.to_path_buf() } else { self.project_root.join(path) };
        path.canonicalize().unwrap_or(path)
    }

    fn is_ignored(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.project_root) else { return true };
        relative.components().any(|component| {
            let std::path::Component::Normal(component) = component else { return false };
            component == ".git" || component == "target"
        })
    }
}

#[derive(Clone, Debug)]
struct CargoCommand {
    executable: PathBuf,
    prefix_arguments: Vec<OsString>,
    append_build_arguments: bool,
    application_prefix_arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl CargoCommand {
    fn from_environment() -> Self {
        Self {
            executable: std::env::var_os("CARGO")
                .map(PathBuf::from)
                .unwrap_or_else(|| "cargo".into()),
            prefix_arguments: Vec::new(),
            append_build_arguments: true,
            application_prefix_arguments: Vec::new(),
            environment: Vec::new(),
        }
    }
}

/// Builds and runs one resolved Cargo application target.
pub struct CargoApplicationDriver {
    target: ResolvedCargoApplication,
    working_directory: PathBuf,
    command: CargoCommand,
    application: Option<Child>,
    output_sender: mpsc::UnboundedSender<CargoApplicationOutput>,
    output_receiver: mpsc::UnboundedReceiver<CargoApplicationOutput>,
    application_output_tasks: Vec<JoinHandle<()>>,
    runtime_control: RuntimeControlServer,
    rebuild_watcher: WorkspaceRebuildWatcher,
}

impl CargoApplicationDriver {
    pub async fn new(target: ResolvedCargoApplication, working_directory: PathBuf) -> Result<Self> {
        Self::new_with_command(target, working_directory, CargoCommand::from_environment()).await
    }

    async fn new_with_command(
        target: ResolvedCargoApplication,
        working_directory: PathBuf,
        command: CargoCommand,
    ) -> Result<Self> {
        let runtime_control =
            RuntimeControlServer::bind().await.context("Failed to start Rust runtime control")?;
        let rebuild_watcher = WorkspaceRebuildWatcher::new(&working_directory)?;
        let (output_sender, output_receiver) = mpsc::unbounded_channel();
        Ok(Self {
            target,
            working_directory,
            command,
            application: None,
            output_sender,
            output_receiver,
            application_output_tasks: Vec::new(),
            runtime_control,
            rebuild_watcher,
        })
    }

    /// Build the configured target and replace the running application only after success.
    pub async fn build_and_launch(&mut self) -> Result<PathBuf> {
        let executable = self.build().await?;
        self.stop_application().await?;
        self.launch_application(&executable).await?;
        Ok(executable)
    }

    /// Stop the running application, if any.
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_application().await
    }

    /// Return the running application's operating-system process ID.
    pub fn application_id(&self) -> Option<u32> {
        self.application.as_ref().and_then(Child::id)
    }

    /// Report an application exit without waiting for it.
    pub fn poll_exit(&mut self) -> Result<Option<ExitStatus>> {
        let Some(application) = &mut self.application else { return Ok(None) };
        let Some(status) = application.try_wait()? else { return Ok(None) };
        self.application = None;
        Ok(Some(status))
    }

    /// Drain currently buffered Cargo and application output.
    pub fn take_output(&mut self) -> Vec<CargoApplicationOutput> {
        let mut output = Vec::new();
        while let Ok(line) = self.output_receiver.try_recv() {
            output.push(line);
        }
        output
    }

    /// Drain one runtime event from the launched application.
    pub fn take_runtime_event(&mut self) -> Option<RuntimeEvent> {
        let event = self.runtime_control.try_next_event()?;
        if let Some(paths) = event.hot_reload_paths() {
            self.rebuild_watcher.update_hot_reload_paths(paths);
        }
        if event.requires_rebuild() {
            self.rebuild_watcher.request_manual_rebuild();
        }
        Some(event)
    }

    /// Request a Cargo rebuild regardless of file changes.
    pub fn request_rebuild(&mut self) {
        self.rebuild_watcher.request_manual_rebuild();
    }

    /// Return whether coalesced workspace or runtime changes require a Cargo rebuild.
    pub fn take_rebuild_request(&mut self) -> Result<bool> {
        self.rebuild_watcher.take_rebuild_request()
    }

    /// Return whether a source or resource in the live graph changed.
    pub fn take_hot_reload_activity(&mut self) -> bool {
        self.rebuild_watcher.take_hot_reload_activity()
    }

    async fn build(&mut self) -> Result<PathBuf> {
        let mut command = Command::new(&self.command.executable);
        command.args(&self.command.prefix_arguments);
        if self.command.append_build_arguments {
            command.args(build_arguments(&self.target));
        }
        command
            .envs(self.command.environment.iter().cloned())
            .env("SLINT_LIVE_PREVIEW", "1")
            .current_dir(&self.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().with_context(|| {
            format!("Failed to start Cargo with {}", self.command.executable.display())
        })?;
        let stdout = child.stdout.take().context("Cargo stdout was not captured")?;
        let stderr = child.stderr.take().context("Cargo stderr was not captured")?;
        let stderr_task = spawn_output_reader(
            stderr,
            CargoApplicationOutputSource::Cargo,
            self.output_sender.clone(),
        );

        let mut executable = None;
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await.context("Failed to read Cargo output")? {
            match parse_cargo_message(&line, &self.target.binary) {
                ParsedCargoMessage::Executable(path) => executable = Some(path),
                ParsedCargoMessage::Output(message) => {
                    send_output(&self.output_sender, CargoApplicationOutputSource::Cargo, &message);
                }
                ParsedCargoMessage::None => {}
            }
        }
        let status = child.wait().await.context("Failed to wait for Cargo")?;
        let _ = stderr_task.await;
        if !status.success() {
            bail!("Cargo build failed with {status}");
        }
        executable.context("Cargo did not report an executable compiler artifact")
    }

    async fn launch_application(&mut self, executable: &Path) -> Result<()> {
        let mut command = Command::new(executable);
        command
            .args(&self.command.application_prefix_arguments)
            .envs(self.runtime_control.environment())
            .current_dir(&self.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().with_context(|| {
            format!("Failed to launch Cargo application {}", executable.display())
        })?;
        let stdout = child.stdout.take().context("Application stdout was not captured")?;
        let stderr = child.stderr.take().context("Application stderr was not captured")?;
        self.application_output_tasks.push(spawn_output_reader(
            stdout,
            CargoApplicationOutputSource::ApplicationStandardOutput,
            self.output_sender.clone(),
        ));
        self.application_output_tasks.push(spawn_output_reader(
            stderr,
            CargoApplicationOutputSource::ApplicationStandardError,
            self.output_sender.clone(),
        ));
        self.application = Some(child);
        Ok(())
    }

    async fn stop_application(&mut self) -> Result<()> {
        if let Some(mut application) = self.application.take() {
            if application.try_wait()?.is_none() {
                application.kill().await.context("Failed to stop the Cargo application")?;
            } else {
                let _ = application.wait().await;
            }
        }
        for task in self.application_output_tasks.drain(..) {
            let _ = task.await;
        }
        Ok(())
    }
}

fn build_arguments(target: &ResolvedCargoApplication) -> Vec<OsString> {
    let mut arguments = vec![
        "build".into(),
        "--message-format=json".into(),
        "--manifest-path".into(),
        target.manifest_path.as_os_str().into(),
        "--package".into(),
        target.package.as_str().into(),
        "--bin".into(),
        target.binary.as_str().into(),
    ];
    let mut features = target.features.iter().map(String::as_str).collect::<BTreeSet<_>>();
    features.insert(&target.live_preview_feature);
    arguments.push("--features".into());
    arguments.push(features.into_iter().collect::<Vec<_>>().join(",").into());
    arguments
}

enum ParsedCargoMessage {
    Executable(PathBuf),
    Output(String),
    None,
}

fn parse_cargo_message(line: &str, selected_binary: &str) -> ParsedCargoMessage {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
        return ParsedCargoMessage::Output(line.into());
    };
    match message.get("reason").and_then(serde_json::Value::as_str) {
        Some("compiler-artifact")
            if message.pointer("/target/name").and_then(serde_json::Value::as_str)
                == Some(selected_binary)
                && message
                    .pointer("/target/kind")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin"))) =>
        {
            message
                .get("executable")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
                .map(ParsedCargoMessage::Executable)
                .unwrap_or(ParsedCargoMessage::None)
        }
        Some("compiler-message") => message
            .pointer("/message/rendered")
            .and_then(serde_json::Value::as_str)
            .or_else(|| message.pointer("/message/message").and_then(serde_json::Value::as_str))
            .map(str::to_owned)
            .map(ParsedCargoMessage::Output)
            .unwrap_or(ParsedCargoMessage::None),
        Some(_) => ParsedCargoMessage::None,
        None => ParsedCargoMessage::Output(line.into()),
    }
}

fn spawn_output_reader<Reader>(
    reader: Reader,
    source: CargoApplicationOutputSource,
    sender: mpsc::UnboundedSender<CargoApplicationOutput>,
) -> JoinHandle<()>
where
    Reader: tokio::io::AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if sender.send(CargoApplicationOutput { source, line }).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(CargoApplicationOutput {
                        source,
                        line: format!("Failed to read process output: {error}"),
                    });
                    break;
                }
            }
        }
    })
}

fn send_output(
    sender: &mpsc::UnboundedSender<CargoApplicationOutput>,
    source: CargoApplicationOutputSource,
    message: &str,
) {
    for line in message.lines() {
        let _ = sender.send(CargoApplicationOutput { source, line: line.into() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_live_preview::springboard_runtime::SpringboardRuntimeReporter;
    use std::time::Duration;

    #[test]
    #[ignore]
    fn fake_cargo_child() {
        assert_eq!(std::env::var("SLINT_LIVE_PREVIEW").as_deref(), Ok("1"));
        eprintln!("fake Cargo stderr");
        match std::env::var("SLINT_SPRINGBOARD_FAKE_CARGO_MODE").as_deref() {
            Ok("fail") => std::process::exit(23),
            Ok("missing-artifact") => return,
            _ => {}
        }
        println!("fake Cargo stdout");
        let executable = std::env::var("SLINT_SPRINGBOARD_FAKE_EXECUTABLE").unwrap();
        println!(
            "{}",
            serde_json::json!({
                "reason": "compiler-artifact",
                "target": { "name": "demo", "kind": ["bin"] },
                "executable": executable,
            })
        );
    }

    #[test]
    #[ignore]
    fn fake_application_child() {
        println!("fake application stdout");
        eprintln!("fake application stderr");
        let mut reporter = SpringboardRuntimeReporter::from_environment().unwrap().unwrap();
        reporter.report(RuntimeEvent::Ready { hot_reload_paths: Vec::new() }).unwrap();
        std::thread::sleep(Duration::from_secs(30));
    }

    fn target(directory: &tempfile::TempDir) -> ResolvedCargoApplication {
        ResolvedCargoApplication {
            manifest_path: directory.path().join("Cargo.toml"),
            package: "demo-package".into(),
            binary: "demo".into(),
            features: vec!["logging".into()],
            live_preview_feature: "preview-ui".into(),
        }
    }

    fn fake_command(mode: &str) -> CargoCommand {
        CargoCommand {
            executable: std::env::current_exe().unwrap(),
            prefix_arguments: [
                "--exact",
                "cargo_driver::tests::fake_cargo_child",
                "--ignored",
                "--nocapture",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            append_build_arguments: false,
            application_prefix_arguments: [
                "--exact",
                "cargo_driver::tests::fake_application_child",
                "--ignored",
                "--nocapture",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            environment: vec![
                ("SLINT_SPRINGBOARD_FAKE_CARGO_MODE".into(), mode.into()),
                (
                    "SLINT_SPRINGBOARD_FAKE_EXECUTABLE".into(),
                    std::env::current_exe().unwrap().into_os_string(),
                ),
            ],
        }
    }

    #[test]
    fn cargo_build_arguments_select_the_target_and_features() {
        let directory = tempfile::tempdir().unwrap();
        let arguments = build_arguments(&target(&directory));
        let arguments =
            arguments.iter().map(|argument| argument.to_string_lossy()).collect::<Vec<_>>();

        assert_eq!(arguments[0], "build");
        assert!(arguments.iter().any(|argument| argument == "--message-format=json"));
        assert!(arguments.windows(2).any(|args| args == ["--package", "demo-package"]));
        assert!(arguments.windows(2).any(|args| args == ["--bin", "demo"]));
        assert!(arguments.windows(2).any(|args| args == ["--features", "logging,preview-ui"]));
    }

    #[test]
    fn compiler_messages_fall_back_to_the_plain_diagnostic() {
        let message = serde_json::json!({
            "reason": "compiler-message",
            "message": { "rendered": null, "message": "plain diagnostic" },
        });

        let ParsedCargoMessage::Output(output) = parse_cargo_message(&message.to_string(), "demo")
        else {
            panic!("the diagnostic should become Cargo output")
        };

        assert_eq!(output, "plain diagnostic");
    }

    #[test]
    fn rebuild_planner_separates_hot_reload_and_cargo_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let hot_path = root.join("ui/app.slint");
        let rust_path = root.join("src/main.rs");
        let mut planner = RebuildPlanner::new(root.clone());
        planner.update_hot_reload_paths(std::slice::from_ref(&hot_path));
        let now = Instant::now();

        assert!(planner.record_change(hot_path, now));
        assert!(!planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));

        assert!(!planner.record_change(rust_path, now));
        assert!(!planner.take_rebuild_request(now));
        assert!(planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));
        assert!(!planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));

        assert!(!planner.record_change(root.join(".git/index"), now));
        assert!(!planner.record_change(root.join("target/debug/demo"), now));
        assert!(!planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));
    }

    #[test]
    fn new_hot_paths_cancel_pending_cargo_rebuilds() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let imported = root.join("ui/new-component.slint");
        let mut planner = RebuildPlanner::new(root);
        let now = Instant::now();
        assert!(!planner.record_change(imported.clone(), now));

        planner.update_hot_reload_paths(std::slice::from_ref(&imported));

        assert!(!planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));
    }

    #[test]
    fn rapid_changes_coalesce_and_manual_rebuild_is_immediate() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let mut planner = RebuildPlanner::new(root.clone());
        let now = Instant::now();
        let half_debounce = i_slint_live_preview::REBUILD_DEBOUNCE / 2;
        assert!(!planner.record_change(root.join("src/first.rs"), now));
        assert!(!planner.record_change(root.join("src/second.rs"), now + half_debounce));

        assert!(!planner.take_rebuild_request(now + i_slint_live_preview::REBUILD_DEBOUNCE));
        assert!(
            planner
                .take_rebuild_request(now + half_debounce + i_slint_live_preview::REBUILD_DEBOUNCE)
        );

        planner.request_manual_rebuild();
        assert!(planner.take_rebuild_request(now));
    }

    #[test]
    fn a_disappearing_watched_path_is_transient_but_other_errors_are_not() {
        assert!(is_transient_watcher_error(&notify::Error::path_not_found()));
        assert!(is_transient_watcher_error(&notify::Error::io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "gone",
        ))));
        assert!(!is_transient_watcher_error(&notify::Error::generic("watch failed")));
    }

    #[tokio::test]
    async fn workspace_watcher_ignores_live_graph_edits_and_reports_rust_edits() {
        let directory = tempfile::tempdir().unwrap();
        let hot_path = directory.path().join("ui/app.slint");
        let rust_path = directory.path().join("src/main.rs");
        std::fs::create_dir_all(hot_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(rust_path.parent().unwrap()).unwrap();
        std::fs::write(&hot_path, "export component App {}\n").unwrap();
        std::fs::write(&rust_path, "fn main() {}\n").unwrap();
        let mut watcher = WorkspaceRebuildWatcher::new(directory.path()).unwrap();
        watcher.update_hot_reload_paths(std::slice::from_ref(&hot_path));

        std::fs::write(&hot_path, "export component App { property <int> count; }\n").unwrap();
        tokio::time::sleep(i_slint_live_preview::REBUILD_DEBOUNCE * 2).await;
        assert!(!watcher.take_rebuild_request().unwrap());

        std::fs::write(&rust_path, "fn main() { println!(\"changed\"); }\n").unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if watcher.take_rebuild_request().unwrap() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the Rust source change did not request a rebuild");
    }

    #[tokio::test]
    async fn builds_launches_and_separates_process_output() {
        let directory = tempfile::tempdir().unwrap();
        let mut driver = CargoApplicationDriver::new_with_command(
            target(&directory),
            directory.path().into(),
            fake_command("success"),
        )
        .await
        .unwrap();

        driver.build_and_launch().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if driver.take_runtime_event()
                    == Some(RuntimeEvent::Ready { hot_reload_paths: Vec::new() })
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let output = driver.take_output();

        assert!(driver.application_id().is_some());
        assert!(output.iter().any(|line| {
            line.source == CargoApplicationOutputSource::Cargo
                && line.line.contains("fake Cargo stdout")
        }));
        assert!(output.iter().any(|line| {
            line.source == CargoApplicationOutputSource::Cargo
                && line.line.contains("fake Cargo stderr")
        }));
        assert!(output.iter().any(|line| {
            line.source == CargoApplicationOutputSource::ApplicationStandardOutput
                && line.line.contains("fake application stdout")
        }));
        assert!(output.iter().any(|line| {
            line.source == CargoApplicationOutputSource::ApplicationStandardError
                && line.line.contains("fake application stderr")
        }));
        driver.stop().await.unwrap();
    }

    #[tokio::test]
    async fn a_failed_rebuild_keeps_the_previous_application_running() {
        let directory = tempfile::tempdir().unwrap();
        let mut driver = CargoApplicationDriver::new_with_command(
            target(&directory),
            directory.path().into(),
            fake_command("success"),
        )
        .await
        .unwrap();
        driver.build_and_launch().await.unwrap();
        let original_id = driver.application_id();
        driver.command.environment[0].1 = "fail".into();

        assert!(driver.build_and_launch().await.is_err());
        assert_eq!(driver.application_id(), original_id);
        assert_eq!(driver.poll_exit().unwrap(), None);
        driver.stop().await.unwrap();
    }

    #[tokio::test]
    async fn a_successful_build_must_report_an_executable() {
        let directory = tempfile::tempdir().unwrap();
        let mut driver = CargoApplicationDriver::new_with_command(
            target(&directory),
            directory.path().into(),
            fake_command("missing-artifact"),
        )
        .await
        .unwrap();

        let error = driver.build_and_launch().await.unwrap_err().to_string();

        assert!(error.contains("executable compiler artifact"));
        assert_eq!(driver.application_id(), None);
    }
}
