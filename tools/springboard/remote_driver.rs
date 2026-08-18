// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Remote-viewer target driver and project source graph.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use i_slint_live_preview::file_watcher::{FileChangeKind, FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, PreviewToLspMessage, VersionedUrl,
    lsp_types,
};
use i_slint_live_preview::remote_client::{RemoteClientEvent, RemotePreviewClient};
use i_slint_springboard::{
    DeviceId, DiagnosticSeverity, LogLevel, SessionEvent, project::ProjectRunTarget,
};
use tokio::sync::mpsc;

use crate::discovery::DiscoveredRemoteViewer;

const PREVIEW_FILE_EXTENSIONS: &[&str] = &[
    "slint", "png", "jpg", "jpeg", "svg", "svgz", "gif", "webp", "bmp", "ico", "avif", "ttf",
    "ttc", "otf",
];

#[derive(Debug)]
pub enum RemoteDriverEvent {
    Connection(RemoteClientEvent),
    Session(SessionEvent),
}

struct ProjectSourceGraph {
    target: ProjectRunTarget,
    canonical_root: PathBuf,
    style: String,
    watched_paths: BTreeSet<PathBuf>,
}

pub struct RemoteViewerDriver {
    client: RemotePreviewClient,
    source_messages: mpsc::UnboundedReceiver<PreviewToLspMessage>,
    connection_events: mpsc::UnboundedReceiver<RemoteClientEvent>,
    watcher: FileWatcher,
    watch_events: mpsc::UnboundedReceiver<Result<WatchEvent, String>>,
    graph: Option<ProjectSourceGraph>,
    device_id: Option<DeviceId>,
}

impl RemoteViewerDriver {
    pub fn new() -> Result<Self> {
        let (source_sender, source_messages) = mpsc::unbounded_channel();
        let (connection_sender, connection_events) = mpsc::unbounded_channel();
        let client = RemotePreviewClient::new(
            move |message| {
                source_sender.send(message).ok();
            },
            move |event| {
                connection_sender.send(event).ok();
            },
        );
        let (watch_sender, watch_events) = mpsc::unbounded_channel();
        let error_sender = watch_sender.clone();
        let watcher = FileWatcher::start(
            move |event| {
                watch_sender.send(Ok(event)).ok();
            },
            move |error| {
                error_sender.send(Err(error.to_string())).ok();
            },
        )
        .context("Failed to start the remote project file watcher")?;
        Ok(Self {
            client,
            source_messages,
            connection_events,
            watcher,
            watch_events,
            graph: None,
            device_id: None,
        })
    }

    pub async fn launch(
        &mut self,
        viewer: &DiscoveredRemoteViewer,
        target: &ProjectRunTarget,
        style: &str,
    ) -> Result<()> {
        if !viewer.compatible() {
            bail!("Remote viewer {} is not protocol-compatible", viewer.name);
        }
        let canonical_root = target
            .project_root
            .canonicalize()
            .with_context(|| format!("Failed to resolve {}", target.project_root.display()))?;
        self.graph = Some(ProjectSourceGraph {
            target: target.clone(),
            canonical_root,
            style: style.into(),
            watched_paths: BTreeSet::new(),
        });
        self.device_id = Some(viewer.id.clone());
        self.watch_entry()?;
        if let Err(error) = self.client.connect(viewer.addresses.clone(), viewer.port).await {
            self.graph = None;
            self.device_id = None;
            return Err(error.into());
        }
        self.send_full_state()?;
        Ok(())
    }

    pub async fn stop(&mut self) {
        self.client.disconnect().await;
        self.graph = None;
        self.device_id = None;
        self.watcher.update_watched_paths(Vec::new()).ok();
        while self.source_messages.try_recv().is_ok() {}
        while self.connection_events.try_recv().is_ok() {}
        while self.watch_events.try_recv().is_ok() {}
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.send_full_state()
    }

    pub fn poll(&mut self) -> Result<Vec<RemoteDriverEvent>> {
        let mut events = Vec::new();
        while let Ok(event) = self.connection_events.try_recv() {
            events.push(RemoteDriverEvent::Connection(event));
        }
        while let Ok(message) = self.source_messages.try_recv() {
            self.handle_source_message(message, &mut events)?;
        }
        while let Ok(event) = self.watch_events.try_recv() {
            self.handle_watch_event(event, &mut events)?;
        }
        Ok(events)
    }

    fn handle_source_message(
        &mut self,
        message: PreviewToLspMessage,
        events: &mut Vec<RemoteDriverEvent>,
    ) -> Result<()> {
        let Some(device_id) = self.device_id.clone() else { return Ok(()) };
        match message {
            PreviewToLspMessage::RequestState { files, .. } => {
                if files.is_empty() {
                    self.send_full_state()?;
                } else {
                    for file in files {
                        self.send_requested_file(file)?;
                    }
                }
            }
            PreviewToLspMessage::Diagnostics { uri, diagnostics, .. } => {
                for diagnostic in diagnostics {
                    let file = uri
                        .to_file_path()
                        .ok()
                        .map(|path| path.display().to_string())
                        .or_else(|| Some(uri.to_string()));
                    events.push(RemoteDriverEvent::Session(SessionEvent::Diagnostic {
                        device_id: device_id.clone(),
                        severity: match diagnostic.severity {
                            Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
                            Some(lsp_types::DiagnosticSeverity::WARNING) => {
                                DiagnosticSeverity::Warning
                            }
                            _ => DiagnosticSeverity::Information,
                        },
                        message: diagnostic.message,
                        file,
                        line: Some(diagnostic.range.start.line + 1),
                        column: Some(diagnostic.range.start.character + 1),
                    }));
                }
            }
            PreviewToLspMessage::DebugMessage { location, message } => {
                let message = location.map_or(message.clone(), |(path, line, column)| {
                    format!("{}:{line}:{column}: {message}", path.display())
                });
                events.push(RemoteDriverEvent::Session(SessionEvent::Log {
                    device_id: Some(device_id),
                    level: LogLevel::Debug,
                    message,
                }));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_watch_event(
        &mut self,
        event: Result<WatchEvent, String>,
        events: &mut Vec<RemoteDriverEvent>,
    ) -> Result<()> {
        let Some(device_id) = self.device_id.clone() else { return Ok(()) };
        match event {
            Ok(event) => {
                let url = lsp_types::Url::from_file_path(&event.path).map_err(|()| {
                    anyhow::anyhow!("Cannot convert {} to a file URL", event.path.display())
                })?;
                if event.kind == FileChangeKind::Deleted {
                    self.client.send(&LspToPreviewMessage::ForgetFile { url });
                } else {
                    self.send_requested_file(url)?;
                }
                events.push(RemoteDriverEvent::Session(SessionEvent::Log {
                    device_id: Some(device_id),
                    level: LogLevel::Debug,
                    message: format!("Reloaded {}", event.path.display()),
                }));
            }
            Err(message) => events.push(RemoteDriverEvent::Session(SessionEvent::Error {
                device_id: Some(device_id),
                message: format!("Remote project watcher failed: {message}"),
            })),
        }
        Ok(())
    }

    fn watch_entry(&mut self) -> Result<()> {
        let entry =
            self.graph.as_ref().context("No remote project is active")?.target.entry_file.clone();
        self.add_watched_path(entry)
    }

    fn add_watched_path(&mut self, path: PathBuf) -> Result<()> {
        let Some(graph) = self.graph.as_mut() else { return Ok(()) };
        if graph.watched_paths.insert(path) {
            self.watcher
                .update_watched_paths(graph.watched_paths.iter().cloned())
                .context("Failed to update remote project file watches")?;
        }
        Ok(())
    }

    fn send_full_state(&mut self) -> Result<()> {
        let Some(graph) = self.graph.as_ref() else { return Ok(()) };
        let target = graph.target.clone();
        let style = graph.style.clone();
        self.client.send(&LspToPreviewMessage::SetConfiguration {
            config: PreviewConfig {
                style,
                include_paths: vec![target.project_root.clone()],
                format_utf8: true,
                ..Default::default()
            },
        });
        let entry_url = lsp_types::Url::from_file_path(&target.entry_file).map_err(|()| {
            anyhow::anyhow!("Cannot convert {} to a file URL", target.entry_file.display())
        })?;
        self.send_requested_file(entry_url.clone())?;
        self.client.send(&LspToPreviewMessage::ShowPreview(PreviewComponent {
            url: entry_url,
            component: Some(target.component),
        }));
        Ok(())
    }

    fn send_requested_file(&mut self, url: lsp_types::Url) -> Result<()> {
        let Some(graph) = self.graph.as_ref() else { return Ok(()) };
        let Ok(path) = url.to_file_path() else {
            self.client.send(&LspToPreviewMessage::ForgetFile { url });
            return Ok(());
        };
        let is_entry = path == graph.target.entry_file;
        let canonical_root = graph.canonical_root.clone();
        let canonical = match path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                self.client.send(&LspToPreviewMessage::ForgetFile { url });
                return Ok(());
            }
        };
        if !canonical.starts_with(&canonical_root) || (!is_entry && !is_preview_file(&canonical)) {
            tracing::warn!(
                "Refusing a remote-viewer request outside the project preview graph: {}",
                path.display()
            );
            self.client.send(&LspToPreviewMessage::ForgetFile { url });
            return Ok(());
        }
        let contents = std::fs::read(&canonical)
            .with_context(|| format!("Failed to read {}", canonical.display()))?;
        self.add_watched_path(canonical)?;
        self.client.send(&LspToPreviewMessage::SetContents {
            url: VersionedUrl::new(url, None),
            contents,
        });
        Ok(())
    }
}

fn is_preview_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        PREVIEW_FILE_EXTENSIONS.iter().any(|allowed| extension.eq_ignore_ascii_case(allowed))
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use i_slint_live_preview::protocol::{SourceFileVersion, lsp_types};
    use i_slint_live_preview::remote::{Connection, ConnectionMessage};

    use super::*;

    fn project(directory: &tempfile::TempDir) -> ProjectRunTarget {
        let entry_file = directory.path().join("main.slint");
        std::fs::write(
            &entry_file,
            "import { Card } from \"card.slint\"; export component App inherits Card {}",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("card.slint"),
            "export component Card inherits Rectangle { background: blue; }",
        )
        .unwrap();
        ProjectRunTarget {
            project_root: directory.path().canonicalize().unwrap(),
            manifest_path: directory.path().join("slint.toml"),
            entry_file: entry_file.canonicalize().unwrap(),
            component: "App".into(),
        }
    }

    fn viewer(port: u16) -> DiscoveredRemoteViewer {
        DiscoveredRemoteViewer {
            id: DeviceId::new("remote:test-viewer").unwrap(),
            name: "Test Viewer".into(),
            platform: "test".into(),
            slint_version: Some(env!("CARGO_PKG_VERSION").into()),
            protocols: vec![i_slint_live_preview::protocol::PROTOCOL_SUBPROTOCOL.into()],
            addresses: vec!["127.0.0.1".into()],
            port,
        }
    }

    async fn poll_until(
        driver: &mut RemoteViewerDriver,
        mut predicate: impl FnMut(&RemoteDriverEvent) -> bool,
    ) -> Vec<RemoteDriverEvent> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut captured = Vec::new();
            loop {
                let events = driver.poll().unwrap();
                let done = events.iter().any(&mut predicate);
                captured.extend(events);
                if done {
                    return captured;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for a remote driver event")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn serves_requested_project_files_and_forwards_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let target = project(&directory);
        let (message_sender, mut messages) = mpsc::unbounded_channel();
        let connection = Connection::listen(
            Some(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
            Some("Test Viewer".into()),
            move |message| {
                message_sender.send(message).ok();
            },
        )
        .await
        .unwrap();
        let mut driver = RemoteViewerDriver::new().unwrap();

        driver.launch(&viewer(connection.local_port()), &target, "fluent").await.unwrap();

        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if matches!(messages.recv().await, Some(ConnectionMessage::ShowPreview { .. })) {
                    break;
                }
            }
        })
        .await
        .expect("the viewer did not receive ShowPreview");

        let entry_url = lsp_types::Url::from_file_path(&target.entry_file).unwrap();
        let entry = connection.request_file(entry_url.clone()).await.unwrap();
        assert!(String::from_utf8_lossy(&entry.contents).contains("card.slint"));

        let import_url =
            lsp_types::Url::from_file_path(directory.path().join("card.slint")).unwrap();
        let requested_import = connection.request_file(import_url);
        tokio::pin!(requested_import);
        let imported = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                tokio::select! {
                    result = &mut requested_import => break result.unwrap(),
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        driver.poll().unwrap();
                    }
                }
            }
        })
        .await
        .expect("Springboard did not serve the imported file");
        assert!(String::from_utf8_lossy(&imported.contents).contains("background: blue"));

        connection
            .send(PreviewToLspMessage::Diagnostics {
                uri: entry_url,
                version: SourceFileVersion::default(),
                diagnostics: vec![lsp_types::Diagnostic {
                    range: lsp_types::Range::default(),
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    message: "deliberate test error".into(),
                    ..Default::default()
                }],
            })
            .unwrap();
        let events = poll_until(&mut driver, |event| {
            matches!(
                event,
                RemoteDriverEvent::Session(SessionEvent::Diagnostic { message, .. })
                    if message == "deliberate test error"
            )
        })
        .await;
        assert!(events.iter().any(|event| matches!(
            event,
            RemoteDriverEvent::Session(SessionEvent::Diagnostic {
                severity: DiagnosticSeverity::Error,
                line: Some(1),
                column: Some(1),
                ..
            })
        )));

        driver.stop().await;
    }

    #[test]
    fn source_graph_only_accepts_preview_file_types() {
        assert!(is_preview_file(Path::new("ui/app.slint")));
        assert!(is_preview_file(Path::new("assets/logo.SVG")));
        assert!(is_preview_file(Path::new("fonts/ui.ttf")));
        assert!(!is_preview_file(Path::new(".env")));
        assert!(!is_preview_file(Path::new("src/main.rs")));
    }
}
