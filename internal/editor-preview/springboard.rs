// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{ffi::OsString, path::PathBuf, pin::Pin};

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget, PreviewToLspMessage};
use slint::ModelRc;
use tokio::sync::{mpsc, oneshot};

use crate::{LspToPreview, child_process::ChildProcessLspToPreview};

use crate::springboard_ui as ui;

#[derive(Clone)]
pub struct LocalPreviewConfig {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}

type SpringboardAction = Box<dyn FnOnce(&mut SpringboardTask) + Send + 'static>;

#[derive(Clone)]
pub struct Springboard {
    task_sender: mpsc::UnboundedSender<SpringboardAction>,
}

struct SpringboardTask {
    local_preview: LocalPreviewConfig,
    to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
    global: slint::Weak<ui::Springboard<'static>>,
    state: ui::SpringboardState,
    error: String,
    preview: Option<ChildProcessLspToPreview>,
    from_endpoint: Option<mpsc::UnboundedReceiver<PreviewToLspMessage>>,
    highlight: Option<LspToPreviewMessage>,
}

impl Springboard {
    pub fn new(
        local_preview: LocalPreviewConfig,
        to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
        global: slint::Weak<ui::Springboard<'static>>,
    ) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel::<SpringboardAction>();
        let handle = Self { task_sender };
        let close_handle = handle.clone();
        let select_handle = handle.clone();
        let _ = global.upgrade_in_event_loop(move |global| {
            global.set_endpoints(ModelRc::new(slint::VecModel::from(vec![ui::PreviewEndpoint {
                name: "Local".into(),
                kind: ui::PreviewEndpointKind::Local,
                addresses: ModelRc::default(),
                port: 0,
                unavailable_reason: "".into(),
            }])));
            global.on_close(move || close_handle.close());
            global.on_select_endpoint(move |index| {
                let _ = select_handle.task_sender.send(Box::new(move |task| task.select(index)));
            });
        });
        let task = SpringboardTask {
            local_preview,
            to_editor,
            global,
            state: ui::SpringboardState::Stopped,
            error: String::new(),
            preview: None,
            from_endpoint: None,
            highlight: None,
        };
        task.update_ui();
        crate::spawn_local(task.run(task_receiver));
        handle
    }

    pub fn start(&self) {
        let _ = self.task_sender.send(Box::new(|task| {
            if task.state == ui::SpringboardState::Stopped {
                task.state = ui::SpringboardState::Idle;
                task.update_ui();
            }
        }));
    }

    pub fn close(&self) {
        let _ = self.task_sender.send(Box::new(|task| {
            task.error.clear();
            task.close();
        }));
    }
}

impl LspToPreview for Springboard {
    fn send(&self, message: &LspToPreviewMessage) {
        let message = message.clone();
        let _ = self.task_sender.send(Box::new(move |task| task.send(message)));
    }

    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::ChildProcess
    }

    fn shutdown<'a>(&'a self) -> Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
        let (acknowledge, completion) = oneshot::channel();
        let _ = self.task_sender.send(Box::new(move |task| {
            task.error.clear();
            task.close();
            let _ = acknowledge.send(());
        }));
        Box::pin(async move {
            let _ = completion.await;
        })
    }
}

impl SpringboardTask {
    async fn run(mut self, mut actions: mpsc::UnboundedReceiver<SpringboardAction>) {
        loop {
            tokio::select! {
                action = actions.recv() => {
                    match action {
                        Some(action) => action(&mut self),
                        None => break,
                    }
                }
                message = async {
                    match &mut self.from_endpoint {
                        Some(receiver) => receiver.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match message {
                        Some(PreviewToLspMessage::Exited) | None => self.close(),
                        Some(PreviewToLspMessage::SendShowMessage { message }) => {
                            if message.typ == lsp_types::MessageType::ERROR {
                                self.error = message.message.clone();
                                self.update_ui();
                            }
                            let _ = self.to_editor.send(PreviewToLspMessage::SendShowMessage { message });
                        }
                        Some(message) => { let _ = self.to_editor.send(message); }
                    }
                }
            }
        }
        self.close();
    }

    fn select(&mut self, index: i32) {
        if index != 0 || self.state != ui::SpringboardState::Idle {
            return;
        }
        let (to_endpoint, from_endpoint) = mpsc::unbounded_channel();
        let transport = ChildProcessLspToPreview::new(
            self.local_preview.executable.clone(),
            self.local_preview.arguments.clone(),
            to_endpoint,
        );
        match transport.start_preview() {
            Ok(()) => {
                self.preview = Some(transport);
                self.from_endpoint = Some(from_endpoint);
                self.state = ui::SpringboardState::EndpointSelected;
                self.error.clear();
            }
            Err(error) => self.error = format!("Failed to launch Local preview: {error}"),
        }
        self.update_ui();
    }

    fn close(&mut self) {
        self.from_endpoint = None;
        self.preview = None;
        self.state = ui::SpringboardState::Stopped;
        self.update_ui();
    }

    fn send(&mut self, message: LspToPreviewMessage) {
        if matches!(message, LspToPreviewMessage::Quit) {
            self.error.clear();
            self.close();
            return;
        }
        if matches!(message, LspToPreviewMessage::HighlightFromEditor { .. }) {
            self.highlight = Some(message.clone());
        }
        if let Some(transport) = &self.preview {
            transport.send_running(&message);
            if matches!(message, LspToPreviewMessage::ShowPreview(_)) {
                if let Some(highlight) = &self.highlight {
                    transport.send_running(highlight);
                }
            }
        }
    }

    fn update_ui(&self) {
        let state = self.state;
        let error = self.error.clone();
        let _ = self.global.upgrade_in_event_loop(move |global| {
            global.set_selected_endpoint_index(
                if state == ui::SpringboardState::EndpointSelected { 0 } else { -1 },
            );
            global.set_state(state);
            global.set_connection_state(ui::ConnectionState::None);
            global.set_error_message(error.into());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::child_process::tests::{component, fixture_config, receive, started};

    fn controller(name: &str) -> (Springboard, mpsc::UnboundedReceiver<PreviewToLspMessage>) {
        let (executable, arguments) = fixture_config(name);
        let (sender, receiver) = mpsc::unbounded_channel();
        (
            Springboard::new(
                LocalPreviewConfig { executable, arguments },
                sender,
                Default::default(),
            ),
            receiver,
        )
    }

    fn select(controller: &Springboard, index: i32) {
        controller.task_sender.send(Box::new(move |task| task.select(index))).unwrap();
    }

    async fn inspect<Result: Send + 'static>(
        controller: &Springboard,
        inspect: impl FnOnce(&SpringboardTask) -> Result + Send + 'static,
    ) -> Result {
        let (sender, receiver) = oneshot::channel();
        controller
            .task_sender
            .send(Box::new(move |task| {
                let _ = sender.send(inspect(task));
            }))
            .unwrap();
        receiver.await.unwrap()
    }

    async fn state(controller: &Springboard) -> ui::SpringboardState {
        inspect(controller, |task| task.state).await
    }

    async fn wait_stopped(controller: &Springboard) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while state(controller).await != ui::SpringboardState::Stopped {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn echo(
        receiver: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) -> LspToPreviewMessage {
        let PreviewToLspMessage::DebugMessage { message, .. } = receive(receiver).await else {
            panic!("Expected protocol echo");
        };
        serde_json::from_str(&message).unwrap()
    }

    #[tokio::test]
    async fn start_selection_restart_and_stale_events() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver) = controller("echo_child");
                select(&controller, 0);
                assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
                controller.start();
                controller.send(&LspToPreviewMessage::ShowPreview(component()));
                for index in [-1, 1] {
                    select(&controller, index);
                }
                assert_eq!(state(&controller).await, ui::SpringboardState::Idle);
                assert!(inspect(&controller, |task| task.preview.is_none()).await);
                assert!(receiver.try_recv().is_err());
                select(&controller, 0);
                let first_process = started(&mut receiver).await;
                select(&controller, 0);
                controller.start();
                assert_eq!(state(&controller).await, ui::SpringboardState::EndpointSelected);
                assert!(receiver.try_recv().is_err());
                let (old_sender, old_receiver) = mpsc::unbounded_channel();
                let queued_old_sender = old_sender.clone();
                controller
                    .task_sender
                    .send(Box::new(move |task| {
                        task.from_endpoint = Some(old_receiver);
                        queued_old_sender.send(PreviewToLspMessage::Pong).unwrap();
                        queued_old_sender.send(PreviewToLspMessage::Exited).unwrap();
                        task.close();
                    }))
                    .unwrap();
                controller.start();
                select(&controller, 0);
                let second_process = started(&mut receiver).await;
                assert_ne!(first_process, second_process);
                assert!(old_sender.send(PreviewToLspMessage::Exited).is_err());
                assert_eq!(state(&controller).await, ui::SpringboardState::EndpointSelected);
                assert!(receiver.try_recv().is_err());
                tokio::join!(controller.shutdown(), controller.shutdown());
                assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
                controller.shutdown().await;
                assert!(receiver.try_recv().is_err());
                controller.start();
                assert_eq!(state(&controller).await, ui::SpringboardState::Idle);
                select(&controller, 0);
                started(&mut receiver).await;
                assert_eq!(state(&controller).await, ui::SpringboardState::EndpointSelected);
                controller.shutdown().await;
            })
            .await;
    }

    #[tokio::test]
    async fn dropping_handles_disconnects_and_ends_task() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver) = controller("echo_child");
                controller.start();
                select(&controller, 0);
                started(&mut receiver).await;
                drop(controller);
                assert!(
                    tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
                        .await
                        .unwrap()
                        .is_none()
                );
            })
            .await;
    }

    #[tokio::test]
    async fn synchronization_highlight_and_explicit_clear() {
        tokio::task::LocalSet::new().run_until(async {
            let (controller, mut receiver) = controller("echo_child");
            let url = component().url;
            controller.send(&LspToPreviewMessage::HighlightFromEditor { url: Some(url.clone()), offset: 17 });
            controller.start();
            select(&controller, 0);
            started(&mut receiver).await;
            let configuration = LspToPreviewMessage::SetConfiguration { config: Default::default() };
            controller.send(&configuration);
            let source = b"export component Fixture { Text { text: \"synced\"; } }".to_vec();
            controller.send(&LspToPreviewMessage::SetContents {
                url: i_slint_live_preview::protocol::VersionedUrl::new(url.clone(), Some(1)),
                contents: source.clone(),
            });
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::SetConfiguration { .. }));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::SetContents { url: current, contents } if current.url() == &url && current.version() == &Some(1) && contents == source));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::ShowPreview(_)));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::HighlightFromEditor { url: Some(current), offset: 17 } if current == url));
            controller.send(&LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 });
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::HighlightFromEditor { url: None, .. }));
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::ShowPreview(_)));
            assert!(matches!(echo(&mut receiver).await, LspToPreviewMessage::HighlightFromEditor { url: None, .. }));
            controller.send(&LspToPreviewMessage::Quit);
            assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
            controller.shutdown().await;
        }).await;
    }

    #[tokio::test]
    async fn launch_failure_requires_explicit_retry() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver) = controller("echo_child");
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        task.local_preview.executable =
                            PathBuf::from("/missing/springboard-preview")
                    }))
                    .unwrap();
                controller.start();
                select(&controller, 0);
                assert_eq!(state(&controller).await, ui::SpringboardState::Idle);
                assert!(
                    inspect(&controller, |task| !task.error.is_empty() && task.preview.is_none())
                        .await
                );
                controller.send(&LspToPreviewMessage::ShowPreview(component()));
                controller.start();
                assert!(
                    inspect(&controller, |task| task.preview.is_none() && !task.error.is_empty())
                        .await
                );
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        task.local_preview.executable = std::env::current_exe().unwrap();
                    }))
                    .unwrap();
                select(&controller, 0);
                started(&mut receiver).await;
                assert!(inspect(&controller, |task| task.error.is_empty()).await);
                controller.shutdown().await;
            })
            .await;
    }

    #[tokio::test]
    async fn crash_persists_error_and_does_not_restart() {
        tokio::task::LocalSet::new().run_until(async {
            let (controller, mut receiver) = controller("crashing_child");
            controller.start();
            select(&controller, 0);
            started(&mut receiver).await;
            assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::SendShowMessage { message } if message.typ == lsp_types::MessageType::ERROR));
            wait_stopped(&controller).await;
            assert!(inspect(&controller, |task| !task.error.is_empty()).await);
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
            controller.start();
            assert!(inspect(&controller, |task| !task.error.is_empty() && task.preview.is_none()).await);
            controller.close();
            assert!(inspect(&controller, |task| task.error.is_empty()).await);
            controller.shutdown().await;
        }).await;
    }

    #[tokio::test]
    async fn endpoint_exit_stops_without_ending_upstream() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver) = controller("echo_child");
                controller.start();
                select(&controller, 0);
                started(&mut receiver).await;
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        let (sender, endpoint) = mpsc::unbounded_channel();
                        sender.send(PreviewToLspMessage::Exited).unwrap();
                        task.from_endpoint = Some(endpoint);
                    }))
                    .unwrap();
                wait_stopped(&controller).await;
                assert!(inspect(&controller, |task| task.error.is_empty()).await);
                assert!(receiver.try_recv().is_err());
                controller.start();
                assert_eq!(state(&controller).await, ui::SpringboardState::Idle);
                controller.shutdown().await;
            })
            .await;
    }
}
