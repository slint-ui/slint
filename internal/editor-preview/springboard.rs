// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};

use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewTarget, PreviewToLspMessage, pairing,
};
use slint::ModelRc;
use tokio::sync::{mpsc, oneshot};

use crate::{
    LspToPreview,
    child_process::ChildProcessLspToPreview,
    remote_authentication::{PairingAnswer, PairingCredentials, PairingPrompt},
    springboard_ui as ui,
};

mod remote;
mod remote_connection;
use remote::{DiscoveredViewer, Discovery, DiscoveryEvent};
use remote_connection::{ConnectionEvent, RemoteConnection};

#[derive(Clone)]
pub struct LocalPreviewConfig {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}

type SpringboardAction = Box<dyn FnOnce(&mut SpringboardTask) + Send + 'static>;

#[derive(Clone)]
pub struct Springboard {
    task_sender: mpsc::UnboundedSender<SpringboardAction>,
    displayed_endpoints: Arc<Mutex<Vec<EndpointIdentity>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EndpointIdentity {
    Local,
    Remote(String),
}

enum Endpoint {
    Local {
        preview: ChildProcessLspToPreview,
        receiver: mpsc::UnboundedReceiver<PreviewToLspMessage>,
    },
    Remote {
        connection: RemoteConnection,
        pairing_answer: Option<oneshot::Sender<PairingAnswer>>,
    },
}

struct SpringboardTask {
    local_preview: LocalPreviewConfig,
    to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
    global: slint::Weak<ui::Springboard<'static>>,
    displayed_endpoints: Arc<Mutex<Vec<EndpointIdentity>>>,
    state: ui::SpringboardState,
    connection_state: ui::ConnectionState,
    error: String,
    preview: Option<Endpoint>,
    highlight: Option<LspToPreviewMessage>,
    discovery: Option<Discovery>,
    viewers: Vec<DiscoveredViewer>,
    advertised: HashSet<String>,
    selected: Option<EndpointIdentity>,
    credentials: HashMap<String, PairingCredentials>,
    #[cfg(test)]
    discovery_receiver: Option<mdns_sd::Receiver<mdns_sd::ServiceEvent>>,
}

impl Springboard {
    pub fn new(
        local_preview: LocalPreviewConfig,
        to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
        global: slint::Weak<ui::Springboard<'static>>,
    ) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel::<SpringboardAction>();
        let displayed_endpoints = Arc::new(Mutex::new(vec![EndpointIdentity::Local]));
        let handle = Self { task_sender, displayed_endpoints: displayed_endpoints.clone() };
        let close_handle = handle.clone();
        let select_handle = handle.clone();
        let code_handle = handle.clone();
        let consent_handle = handle.clone();
        let _ = global.upgrade_in_event_loop(move |global| {
            global.on_close(move || close_handle.close());
            global.on_select_endpoint(move |index| select_handle.select_displayed(index));
            global.on_submit_pairing_code(move |code| {
                let code = code.trim().to_owned();
                let _ = code_handle
                    .task_sender
                    .send(Box::new(move |task| task.answer_pairing(PairingAnswer::Code(code))));
            });
            global.on_accept_unpaired_connection(move || {
                let _ = consent_handle
                    .task_sender
                    .send(Box::new(|task| task.answer_pairing(PairingAnswer::AcceptUnpaired)));
            });
        });
        let task = SpringboardTask {
            local_preview,
            to_editor,
            global,
            displayed_endpoints,
            state: ui::SpringboardState::Stopped,
            connection_state: ui::ConnectionState::None,
            error: String::new(),
            preview: None,
            highlight: None,
            discovery: None,
            viewers: Vec::new(),
            advertised: HashSet::new(),
            selected: None,
            credentials: HashMap::new(),
            #[cfg(test)]
            discovery_receiver: None,
        };
        task.update_ui();
        crate::spawn_local(task.run(task_receiver));
        handle
    }

    fn select_displayed(&self, index: i32) {
        let endpoint = usize::try_from(index)
            .ok()
            .and_then(|index| self.displayed_endpoints.lock().unwrap().get(index).cloned());
        if let Some(endpoint) = endpoint {
            let _ = self.task_sender.send(Box::new(move |task| task.select_endpoint(endpoint)));
        }
    }

    pub fn start(&self) {
        let _ = self.task_sender.send(Box::new(|task| task.start()));
    }

    pub fn close(&self) {
        let _ = self.task_sender.send(Box::new(|task| task.close()));
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
                action = actions.recv() => match action {
                    Some(action) => action(&mut self),
                    None => break,
                },
                event = receive_endpoint(&mut self.preview) => {
                    match event {
                        Some(event) => self.handle_connection(event),
                        None => self.fail_endpoint("Preview connection ended unexpectedly".into()),
                    }
                }
                event = async {
                    match &self.discovery {
                        Some(discovery) => discovery.next_event().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match event {
                        Ok(event) => self.handle_discovery(event),
                        Err(error) => {
                            self.discovery = None;
                            self.error = format!("Viewer discovery stopped: {error}");
                            self.update_ui();
                        }
                    }
                }
            }
        }
        self.close();
    }

    fn start(&mut self) {
        if self.state != ui::SpringboardState::Stopped {
            return;
        }
        self.state = ui::SpringboardState::Idle;
        #[cfg(test)]
        if let Some(receiver) = &self.discovery_receiver {
            self.discovery = Some(Discovery::from_receiver(receiver.clone()));
            self.update_ui();
            return;
        }
        match Discovery::start() {
            Ok(discovery) => self.discovery = Some(discovery),
            Err(error) => self.error = format!("Cannot discover viewers: {error}"),
        }
        self.update_ui();
    }

    fn handle_discovery(&mut self, event: DiscoveryEvent) {
        match event {
            DiscoveryEvent::Updated(viewer) => {
                self.advertised.insert(viewer.fullname.clone());
                if let Some(existing) =
                    self.viewers.iter_mut().find(|existing| existing.fullname == viewer.fullname)
                {
                    if existing == &viewer {
                        return;
                    }
                    *existing = viewer;
                } else {
                    self.viewers.push(viewer);
                }
            }
            DiscoveryEvent::Removed(fullname) => {
                self.advertised.remove(&fullname);
                if self.selected == Some(EndpointIdentity::Remote(fullname.clone())) {
                    if self.mark_unadvertised_selected() {
                        self.update_ui();
                    }
                    return;
                }
                self.viewers.retain(|viewer| viewer.fullname != fullname);
            }
        }
        self.update_ui();
    }

    fn mark_unadvertised_selected(&mut self) -> bool {
        let Some(EndpointIdentity::Remote(fullname)) = &self.selected else { return false };
        if self.preview.is_some() || self.advertised.contains(fullname) {
            return false;
        }
        let Some(viewer) = self.viewers.iter_mut().find(|viewer| &viewer.fullname == fullname)
        else {
            return false;
        };
        viewer.unavailable_reason = "Viewer is no longer advertised".into();
        true
    }

    #[cfg(test)]
    fn select(&mut self, index: i32) {
        let identity = if index == 0 {
            Some(EndpointIdentity::Local)
        } else {
            usize::try_from(index - 1)
                .ok()
                .and_then(|index| self.viewers.get(index))
                .map(|viewer| EndpointIdentity::Remote(viewer.fullname.clone()))
        };
        if let Some(identity) = identity {
            self.select_endpoint(identity);
        }
    }

    fn select_endpoint(&mut self, identity: EndpointIdentity) {
        if self.state == ui::SpringboardState::Stopped
            || (self.selected.as_ref() == Some(&identity)
                && self.connection_state != ui::ConnectionState::Failed)
        {
            return;
        }
        let viewer = match &identity {
            EndpointIdentity::Local => None,
            EndpointIdentity::Remote(fullname) => {
                let Some(viewer) = self.viewers.iter().find(|viewer| &viewer.fullname == fullname)
                else {
                    return;
                };
                if !viewer.unavailable_reason.is_empty() {
                    self.error = viewer.unavailable_reason.clone();
                    self.update_ui();
                    return;
                }
                Some(viewer.clone())
            }
        };
        self.preview = None;
        self.selected = Some(identity);
        self.error.clear();
        self.viewers.retain(|viewer| {
            self.advertised.contains(&viewer.fullname)
                || self.selected == Some(EndpointIdentity::Remote(viewer.fullname.clone()))
        });
        if let Some(viewer) = viewer {
            let credentials = self.credentials.get(&viewer.fullname).copied();
            self.preview = Some(Endpoint::Remote {
                connection: RemoteConnection::connect(viewer, credentials),
                pairing_answer: None,
            });
            self.state = ui::SpringboardState::EndpointSelected;
            self.connection_state = ui::ConnectionState::Connecting;
        } else {
            let (sender, receiver) = mpsc::unbounded_channel();
            let preview = ChildProcessLspToPreview::new(
                self.local_preview.executable.clone(),
                self.local_preview.arguments.clone(),
                sender,
            );
            match preview.start_preview() {
                Ok(()) => {
                    self.preview = Some(Endpoint::Local { preview, receiver });
                    self.state = ui::SpringboardState::EndpointSelected;
                }
                Err(error) => {
                    self.selected = None;
                    self.state = ui::SpringboardState::Idle;
                    self.error = format!("Failed to launch Local preview: {error}");
                }
            }
            self.connection_state = ui::ConnectionState::None;
        }
        self.update_ui();
    }

    fn handle_connection(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Pairing { prompt, answer } => {
                let Some(Endpoint::Remote { pairing_answer, .. }) = &mut self.preview else {
                    return;
                };
                *pairing_answer = Some(answer);
                match prompt {
                    PairingPrompt::Code { attempts_left, expires_in_seconds } => {
                        self.connection_state = ui::ConnectionState::PairingRequired;
                        self.error = if attempts_left < pairing::MAX_ATTEMPTS {
                            format!(
                                "Incorrect code. {attempts_left} attempts left, {expires_in_seconds}s remaining"
                            )
                        } else {
                            String::new()
                        };
                    }
                    PairingPrompt::Unpaired => {
                        self.connection_state = ui::ConnectionState::UnpairedConfirmationRequired;
                        self.error.clear();
                    }
                }
            }
            ConnectionEvent::CredentialsChanged(credentials) => {
                if let Some(EndpointIdentity::Remote(fullname)) = &self.selected {
                    match credentials {
                        Some(credentials) => {
                            self.credentials.insert(fullname.clone(), credentials);
                        }
                        None => {
                            self.credentials.remove(fullname);
                        }
                    }
                }
                return;
            }
            ConnectionEvent::Connected => {
                self.connection_state = ui::ConnectionState::Connected;
                if let Some(Endpoint::Remote { pairing_answer, .. }) = &mut self.preview {
                    *pairing_answer = None;
                }
                self.error.clear();
            }
            ConnectionEvent::Message(PreviewToLspMessage::Exited) => {
                self.close();
                return;
            }
            ConnectionEvent::Message(PreviewToLspMessage::SendShowMessage { message }) => {
                if message.typ == lsp_types::MessageType::ERROR {
                    self.fail_endpoint(message.message.clone());
                }
                let _ = self.to_editor.send(PreviewToLspMessage::SendShowMessage { message });
                return;
            }
            ConnectionEvent::Message(message) => {
                let _ = self.to_editor.send(message);
                return;
            }
            ConnectionEvent::Ended { error } => {
                if error.is_none() && self.connection_state == ui::ConnectionState::Connected {
                    self.close();
                } else {
                    self.fail_endpoint(
                        error.unwrap_or_else(|| "Viewer connection ended before connecting".into()),
                    );
                }
                return;
            }
        }
        self.update_ui();
    }

    fn fail_endpoint(&mut self, error: String) {
        self.preview = None;
        self.error = error;
        if matches!(self.selected, Some(EndpointIdentity::Remote(_))) {
            self.connection_state = ui::ConnectionState::Failed;
            self.mark_unadvertised_selected();
        } else {
            self.selected = None;
            self.state = ui::SpringboardState::Idle;
            self.connection_state = ui::ConnectionState::None;
        }
        self.update_ui();
    }

    fn answer_pairing(&mut self, answer: PairingAnswer) {
        match &answer {
            PairingAnswer::Code(code)
                if self.connection_state == ui::ConnectionState::PairingRequired =>
            {
                if !pairing::is_valid_code(code) {
                    self.error = "Enter four numeric digits".into();
                    self.update_ui();
                    return;
                }
            }
            PairingAnswer::AcceptUnpaired
                if self.connection_state == ui::ConnectionState::UnpairedConfirmationRequired => {}
            _ => return,
        }
        let Some(Endpoint::Remote { pairing_answer, .. }) = &mut self.preview else {
            return;
        };
        if let Some(sender) = pairing_answer.take() {
            let _ = sender.send(answer);
            self.connection_state = ui::ConnectionState::Connecting;
            self.error.clear();
            self.update_ui();
        }
    }

    fn close(&mut self) {
        self.error.clear();
        self.preview = None;
        self.discovery = None;
        self.viewers.clear();
        self.advertised.clear();
        self.selected = None;
        self.state = ui::SpringboardState::Stopped;
        self.connection_state = ui::ConnectionState::None;
        self.update_ui();
    }

    fn send(&mut self, message: LspToPreviewMessage) {
        if matches!(message, LspToPreviewMessage::Quit) {
            self.close();
            return;
        }
        if matches!(message, LspToPreviewMessage::HighlightFromEditor { .. }) {
            self.highlight = Some(message.clone());
        }
        if let Some(preview) = &self.preview {
            let send = |message| match preview {
                Endpoint::Local { preview, .. } => preview.send_running(&message),
                Endpoint::Remote { connection, .. }
                    if self.connection_state == ui::ConnectionState::Connected =>
                {
                    connection.send(message)
                }
                Endpoint::Remote { .. } => {}
            };
            let highlight = matches!(message, LspToPreviewMessage::ShowPreview(_))
                .then(|| self.highlight.clone())
                .flatten();
            send(message);
            if let Some(highlight) = highlight {
                send(highlight);
            }
        }
    }

    fn selected_index(&self) -> i32 {
        match &self.selected {
            Some(EndpointIdentity::Local) => 0,
            Some(EndpointIdentity::Remote(fullname)) => self
                .viewers
                .iter()
                .position(|viewer| &viewer.fullname == fullname)
                .map(|index| (index + 1) as i32)
                .unwrap_or(-1),
            None => -1,
        }
    }

    fn update_ui(&self) {
        let viewers = self.viewers.clone();
        let selected = self.selected_index();
        let mapping = self.displayed_endpoints.clone();
        let state = self.state;
        let connection = self.connection_state;
        let error = self.error.clone();
        let _ = self.global.upgrade_in_event_loop(move |global| {
            let mut identities = vec![EndpointIdentity::Local];
            identities.extend(
                viewers.iter().map(|viewer| EndpointIdentity::Remote(viewer.fullname.clone())),
            );
            *mapping.lock().unwrap() = identities;
            let mut endpoints = vec![ui::PreviewEndpoint {
                name: "Local".into(),
                kind: ui::PreviewEndpointKind::Local,
                addresses: ModelRc::default(),
                port: 0,
                unavailable_reason: "".into(),
            }];
            endpoints.extend(viewers.into_iter().map(|viewer| ui::PreviewEndpoint {
                name: viewer.name.into(),
                kind: ui::PreviewEndpointKind::Remote,
                addresses: ModelRc::new(slint::VecModel::from(
                    viewer.addresses.into_iter().map(Into::into).collect::<Vec<_>>(),
                )),
                port: i32::from(viewer.port),
                unavailable_reason: viewer.unavailable_reason.into(),
            }));
            global.set_endpoints(ModelRc::new(slint::VecModel::from(endpoints)));
            global.set_selected_endpoint_index(selected);
            global.set_state(state);
            global.set_connection_state(connection);
            global.set_error_message(error.into());
        });
    }
}

async fn receive_endpoint(preview: &mut Option<Endpoint>) -> Option<ConnectionEvent> {
    match preview {
        Some(Endpoint::Remote { connection, .. }) => connection.next_event().await,
        Some(Endpoint::Local { receiver, .. }) => {
            receiver.recv().await.map(ConnectionEvent::Message)
        }
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::child_process::tests::{component, fixture_config, receive, started};

    fn controller(
        name: &str,
    ) -> (
        Springboard,
        mpsc::UnboundedReceiver<PreviewToLspMessage>,
        flume::Sender<mdns_sd::ServiceEvent>,
    ) {
        let (executable, arguments) = fixture_config(name);
        let (sender, receiver) = mpsc::unbounded_channel();
        let controller = Springboard::new(
            LocalPreviewConfig { executable, arguments },
            sender,
            Default::default(),
        );
        let (discovered, discovery_receiver) = flume::unbounded();
        controller
            .task_sender
            .send(Box::new(move |task| task.discovery_receiver = Some(discovery_receiver)))
            .unwrap();
        (controller, receiver, discovered)
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
                let (controller, mut receiver, _discovery) = controller("echo_child");
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
                        let Some(Endpoint::Local { receiver, .. }) = &mut task.preview else {
                            panic!("Expected Local preview");
                        };
                        *receiver = old_receiver;
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
                let (controller, mut receiver, _discovery) = controller("echo_child");
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
            let (controller, mut receiver, _discovery) = controller("echo_child");
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
                let (controller, mut receiver, _discovery) = controller("echo_child");
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
            let (controller, mut receiver, _discovery) = controller("crashing_child");
            controller.start();
            select(&controller, 0);
            started(&mut receiver).await;
            assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::SendShowMessage { message } if message.typ == lsp_types::MessageType::ERROR));
            assert!(inspect(&controller, |task| task.state == ui::SpringboardState::Idle
                && task.discovery.is_some() && !task.error.is_empty()).await);
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            assert_eq!(state(&controller).await, ui::SpringboardState::Idle);
            assert!(inspect(&controller, |task| !task.error.is_empty() && task.preview.is_none()).await);
            controller.task_sender.send(Box::new(|task| {
                task.local_preview.arguments = fixture_config("echo_child").1;
            })).unwrap();
            select(&controller, 0);
            started(&mut receiver).await;
            assert!(inspect(&controller, |task| task.error.is_empty()).await);
            controller.close();
            assert!(inspect(&controller, |task| task.error.is_empty()).await);
            controller.shutdown().await;
        }).await;
    }

    #[tokio::test]
    async fn endpoint_exit_stops_without_ending_upstream() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver, _discovery) = controller("echo_child");
                controller.start();
                select(&controller, 0);
                started(&mut receiver).await;
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        let (sender, endpoint) = mpsc::unbounded_channel();
                        sender.send(PreviewToLspMessage::Exited).unwrap();
                        let Some(Endpoint::Local { receiver, .. }) = &mut task.preview else {
                            panic!("Expected Local preview");
                        };
                        *receiver = endpoint;
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

    use i_slint_live_preview::{
        preview_sessions::{
            PreviewCompilation, PreviewSession, PreviewSessionEvent, PreviewSessionHandle,
        },
        remote::{Connection, ConnectionMessage, PairingPolicy},
    };
    use std::rc::Rc;
    use std::time::Duration;

    struct Viewer {
        connection: Connection,
        connection_events: mpsc::UnboundedReceiver<ConnectionMessage>,
        session: Rc<PreviewSession>,
        session_events: mpsc::UnboundedReceiver<PreviewSessionEvent>,
    }

    impl Viewer {
        async fn start(policy: PairingPolicy, port: u16) -> Self {
            let (connection_sender, connection_events) = mpsc::unbounded_channel();
            let (session_sender, session_events) = mpsc::unbounded_channel();
            let (handle, commands) = PreviewSessionHandle::new();
            let connection = Connection::listen_with_session_handle(
                Some(([127, 0, 0, 1], port).into()),
                None,
                policy,
                move |event| {
                    let _ = connection_sender.send(event);
                },
                handle,
            )
            .await
            .unwrap();
            let session = PreviewSession::start_with(
                commands,
                Rc::new(connection.preview_to_lsp()),
                move |event| {
                    let _ = session_sender.send(event);
                },
            );
            Self { connection, connection_events, session, session_events }
        }

        async fn event(
            &mut self,
            predicate: impl Fn(&ConnectionMessage) -> bool,
        ) -> ConnectionMessage {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let event = self.connection_events.recv().await.unwrap();
                    if predicate(&event) {
                        return event;
                    }
                }
            })
            .await
            .unwrap()
        }

        async fn preview_event(
            &mut self,
            predicate: impl Fn(&PreviewSessionEvent) -> bool,
        ) -> PreviewSessionEvent {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let event = self.session_events.recv().await.unwrap();
                    if predicate(&event) {
                        return event;
                    }
                }
            })
            .await
            .unwrap()
        }
    }

    async fn wait_condition(
        controller: &Springboard,
        condition: impl Fn(&SpringboardTask) -> bool + Send + Sync + 'static,
    ) {
        let condition = Arc::new(condition);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let condition = condition.clone();
                if inspect(controller, move |task| condition(task)).await {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn discover(
        controller: &Springboard,
        sender: &flume::Sender<mdns_sd::ServiceEvent>,
        service: mdns_sd::ResolvedService,
    ) -> String {
        let fullname = service.fullname.clone();
        let port = service.port;
        sender.send(mdns_sd::ServiceEvent::ServiceResolved(Box::new(service))).unwrap();
        let expected = fullname.clone();
        wait_condition(controller, move |task| {
            task.viewers.iter().any(|viewer| viewer.fullname == expected && viewer.port == port)
        })
        .await;
        fullname
    }

    fn answer(controller: &Springboard, answer: PairingAnswer) {
        controller.task_sender.send(Box::new(move |task| task.answer_pairing(answer))).unwrap();
    }

    async fn initial_request(upstream: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if matches!(upstream.recv().await.unwrap(), PreviewToLspMessage::RequestState { files, settings } if files.is_empty() && settings.is_empty()) { break; }
            }
        }).await.unwrap();
    }

    async fn compile_source(controller: &Springboard, viewer: &mut Viewer, marker: i32) {
        let component = component();
        controller.send(&LspToPreviewMessage::SetConfiguration {
            config: i_slint_live_preview::protocol::PreviewConfig {
                style: "fluent".into(),
                ..Default::default()
            },
        });
        controller.send(&LspToPreviewMessage::SetContents {
            url: i_slint_live_preview::protocol::VersionedUrl::new(
                component.url.clone(),
                Some(marker),
            ),
            contents: format!(
                "export component Fixture inherits Window {{ out property <int> marker: {marker}; Rectangle {{}} }}"
            )
            .into_bytes(),
        });
        controller.send(&LspToPreviewMessage::ShowPreview(component.clone()));
        viewer
            .preview_event(|event| matches!(event, PreviewSessionEvent::ShowPreview { .. }))
            .await;
        let compilation = tokio::time::timeout(
            Duration::from_secs(5),
            viewer.session.compile_component(&component),
        )
        .await
        .unwrap();
        let compilation = match compilation {
            PreviewCompilation::Ready(compilation) => compilation,
            PreviewCompilation::CompilationError { message } => {
                panic!("Remote source did not compile: {message}")
            }
            PreviewCompilation::ComponentNotFound => panic!("Remote component was not found"),
            PreviewCompilation::Unavailable => panic!("Remote source was unavailable"),
        };
        let instance = compilation.component_definition().unwrap().create().unwrap();
        assert_eq!(i32::try_from(instance.get_property("marker").unwrap()).unwrap(), marker);
    }

    async fn accept_plaintext(controller: &Springboard) {
        wait_condition(controller, |task| {
            task.connection_state == ui::ConnectionState::UnpairedConfirmationRequired
        })
        .await;
        answer(controller, PairingAnswer::AcceptUnpaired);
        wait_condition(controller, |task| task.connection_state == ui::ConnectionState::Connected)
            .await;
    }

    #[tokio::test]
    async fn discovery_updates_and_stale_display_indices_keep_identity() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, _upstream, discovered) = controller("echo_child");
                controller.start();
                let mut first = remote::tests::service("First", 1);
                first.fullname = "first-viewer".into();
                first.host = "Duplicate".into();
                let mut second = remote::tests::service("Second", 1);
                second.fullname = "second-viewer".into();
                second.host = "Duplicate".into();
                let first_id = discover(&controller, &discovered, first).await;
                let second_id = discover(&controller, &discovered, second.clone()).await;
                discover(&controller, &discovered, second.clone()).await;
                assert!(
                    inspect(&controller, |task| task.viewers.len() == 2
                        && task.viewers[0].name == task.viewers[1].name)
                    .await
                );
                second.port = 2;
                discover(&controller, &discovered, second).await;
                *controller.displayed_endpoints.lock().unwrap() = vec![
                    EndpointIdentity::Local,
                    EndpointIdentity::Remote(first_id.clone()),
                    EndpointIdentity::Remote(second_id.clone()),
                ];
                controller
                    .task_sender
                    .send(Box::new(move |task| {
                        task.handle_discovery(DiscoveryEvent::Removed(first_id))
                    }))
                    .unwrap();
                controller.select_displayed(2);
                let expected = second_id.clone();
                wait_condition(&controller, move |task| {
                    task.selected == Some(EndpointIdentity::Remote(expected.clone()))
                        && task.selected_index() == 1
                })
                .await;
                drop(discovered);
                wait_condition(&controller, |task| task.discovery.is_none()).await;
                select(&controller, 0);
                assert_eq!(state(&controller).await, ui::SpringboardState::EndpointSelected);
                controller.close();
            })
            .await;
    }

    #[tokio::test]
    async fn paired_connection_compiles_and_replays_highlight_and_clear() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new().run_until(async {
            let mut viewer = Viewer::start(PairingPolicy::Generated, 0).await;
            let (controller, mut upstream, discovered) = controller("echo_child");
            controller.send(&LspToPreviewMessage::HighlightFromEditor { url: Some(component().url), offset: 17 });
            controller.start();
            discover(&controller, &discovered, remote::tests::service("Paired", viewer.connection.local_port())).await;
            select(&controller, 1);
            let ConnectionMessage::PairingStarted { code, .. } = viewer.event(|event| matches!(event, ConnectionMessage::PairingStarted { .. })).await else { unreachable!() };
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::PairingRequired).await;
            answer(&controller, PairingAnswer::Code("12".into()));
            assert!(inspect(&controller, |task| matches!(&task.preview, Some(Endpoint::Remote { pairing_answer: Some(_), .. })) && task.error == "Enter four numeric digits").await);
            answer(&controller, PairingAnswer::Code(if code == "0000" { "0001" } else { "0000" }.into()));
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::PairingRequired && task.error.contains("Incorrect code")).await;
            answer(&controller, PairingAnswer::Code(code));
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::Connected && task.credentials.len() == 1).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut viewer, 42).await;
            viewer.connection.send(()).unwrap();
            viewer.connection.send(PreviewToLspMessage::RequestPreview { component: component() }).unwrap();
            viewer.connection.send(PreviewToLspMessage::Exited).unwrap();
            viewer.connection.send(PreviewToLspMessage::Pong).unwrap();
            viewer.connection.send(PreviewToLspMessage::DebugMessage { location: None, message: "after malformed payload".into() }).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match upstream.recv().await.unwrap() {
                        PreviewToLspMessage::DebugMessage { message, .. } if message == "after malformed payload" => break,
                        PreviewToLspMessage::Diagnostics { .. } => {},
                        message => panic!("Unrelated remote control reached the editor: {message:?}"),
                    }
                }
            }).await.unwrap();

            assert!(matches!(viewer.preview_event(|event| matches!(event, PreviewSessionEvent::HighlightFromEditor { .. })).await, PreviewSessionEvent::HighlightFromEditor { url: Some(url), offset: 17 } if url == component().url));
            controller.send(&LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 });
            viewer.preview_event(|event| matches!(event, PreviewSessionEvent::HighlightFromEditor { url: None, .. })).await;
            let (connection, error) = inspect(&controller, |task| (task.connection_state, task.error.clone())).await;
            assert_eq!(connection, ui::ConnectionState::Connected, "{error}");
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            let event = tokio::time::timeout(Duration::from_secs(5), viewer.session_events.recv()).await;
            if event.is_err() {
                let status = inspect(&controller, |task| (task.state, task.connection_state, task.error.clone())).await;
                let mut events = Vec::new();
                while let Ok(event) = viewer.connection_events.try_recv() { events.push(event); }
                panic!("Follow-up preview timed out: {status:?}, viewer events: {events:?}");
            }
            assert!(matches!(event.unwrap(), Some(PreviewSessionEvent::ShowPreview { .. })));
            assert!(matches!(viewer.preview_event(|event| matches!(event, PreviewSessionEvent::HighlightFromEditor { .. })).await, PreviewSessionEvent::HighlightFromEditor { url: None, offset: 0 }));
            controller.close();
            viewer.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
        }).await;
    }

    #[tokio::test]
    async fn switching_viewers_and_local_reuses_tokens_without_quit() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new().run_until(async {
            let mut first = Viewer::start(PairingPolicy::Generated, 0).await;
            let mut second = Viewer::start(PairingPolicy::Disabled, 0).await;
            let (controller, mut upstream, discovered) = controller("echo_child");
            controller.start();
            let first_id = discover(&controller, &discovered, remote::tests::service("First", first.connection.local_port())).await;
            discover(&controller, &discovered, remote::tests::service("Second", second.connection.local_port())).await;
            select(&controller, 1);
            let ConnectionMessage::PairingStarted { code, .. } = first.event(|event| matches!(event, ConnectionMessage::PairingStarted { .. })).await else { unreachable!() };
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::PairingRequired).await;
            answer(&controller, PairingAnswer::Code(code));
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut first, 11).await;
            select(&controller, 2);
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            accept_plaintext(&controller).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut second, 22).await;
            select(&controller, 1);
            second.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            let event = first.event(|event| matches!(event, ConnectionMessage::Connected { .. } | ConnectionMessage::PairingStarted { .. })).await;
            assert!(matches!(event, ConnectionMessage::Connected { .. }));
            compile_source(&controller, &mut first, 33).await;
            assert!(second.session_events.try_recv().is_err());
            discovered.send(mdns_sd::ServiceEvent::ServiceRemoved(i_slint_live_preview::protocol::SERVICE_TYPE.into(), first_id.clone())).unwrap();
            let removed = first_id.clone();
            wait_condition(&controller, move |task| !task.advertised.contains(&removed) && task.selected == Some(EndpointIdentity::Remote(removed.clone())) && task.connection_state == ui::ConnectionState::Connected).await;
            select(&controller, 0);
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            tokio::time::timeout(Duration::from_secs(5), async {
                loop { if matches!(upstream.recv().await.unwrap(), PreviewToLspMessage::DebugMessage { message, .. } if message.parse::<u32>().is_ok()) { break; } }
            }).await.unwrap();
            initial_request(&mut upstream).await;
            discover(&controller, &discovered, remote::tests::service("First", first.connection.local_port())).await;
            let identity = EndpointIdentity::Remote(first_id.clone());
            controller.task_sender.send(Box::new(move |task| task.select_endpoint(identity))).unwrap();
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut first, 44).await;
            controller.close();
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            assert!(inspect(&controller, |task| task.credentials.len() == 1).await);
            controller.start();
            discover(&controller, &discovered, remote::tests::service("First", first.connection.local_port())).await;
            select(&controller, 1);
            wait_condition(&controller, |task| task.connection_state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut first, 55).await;
            controller.close();
        }).await;
    }

    #[tokio::test]
    async fn initial_failure_retries_explicitly_and_established_end_stops() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new()
            .run_until(async {
                let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let port = listener.local_addr().unwrap().port();
                drop(listener);
                let (controller, mut upstream, discovered) = controller("echo_child");
                controller.start();
                let identity =
                    discover(&controller, &discovered, remote::tests::service("Retry", port)).await;
                select(&controller, 1);
                wait_condition(&controller, |task| {
                    task.connection_state == ui::ConnectionState::Failed && task.preview.is_none()
                })
                .await;
                let expected = identity.clone();
                assert!(
                    inspect(&controller, move |task| task.state
                        == ui::SpringboardState::EndpointSelected
                        && task.selected == Some(EndpointIdentity::Remote(expected)))
                    .await
                );
                let mut viewer = Viewer::start(PairingPolicy::Disabled, port).await;
                assert!(upstream.try_recv().is_err());
                select(&controller, 1);
                accept_plaintext(&controller).await;
                initial_request(&mut upstream).await;
                compile_source(&controller, &mut viewer, 77).await;
                drop(viewer);
                wait_stopped(&controller).await;
                assert!(
                    inspect(&controller, |task| task.preview.is_none()
                        && task.selected.is_none()
                        && task.discovery.is_none()
                        && task.error.is_empty())
                    .await
                );
                select(&controller, 0);
                assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
            })
            .await;
    }

    #[tokio::test]
    async fn interrupted_remote_keeps_local_selection_available() {
        use crate::remote_authentication::tests::{raw_peer, receive_handshake, send_handshake};
        use futures_util::SinkExt as _;
        use tokio_tungstenite::tungstenite::{
            Message,
            protocol::{CloseFrame, frame::coding::CloseCode},
        };

        tokio::task::LocalSet::new()
            .run_until(async {
                for protocol_close in [false, true] {
                    let (disconnect, disconnected) = oneshot::channel();
                    let (port, peer) = raw_peer(move |mut socket| async move {
                        send_handshake(&mut socket, &PreviewToLspMessage::PairingReady).await;
                        assert!(matches!(
                            receive_handshake(&mut socket).await,
                            LspToPreviewMessage::PairingHello { .. }
                        ));
                        send_handshake(&mut socket, &PreviewToLspMessage::PairingAccepted).await;
                        send_handshake(
                            &mut socket,
                            &PreviewToLspMessage::RequestState {
                                files: Vec::new(),
                                settings: Vec::new(),
                            },
                        )
                        .await;
                        disconnected.await.unwrap();
                        if protocol_close {
                            socket
                                .send(Message::Close(Some(CloseFrame {
                                    code: CloseCode::Error,
                                    reason: "Viewer failed".into(),
                                })))
                                .await
                                .unwrap();
                        }
                    })
                    .await;
                    let (controller, mut upstream, discovered) = controller("echo_child");
                    controller.start();
                    discover(&controller, &discovered, remote::tests::service("Interrupted", port))
                        .await;
                    select(&controller, 1);
                    accept_plaintext(&controller).await;
                    initial_request(&mut upstream).await;
                    disconnect.send(()).unwrap();
                    peer.await.unwrap();
                    wait_condition(&controller, |task| {
                        task.connection_state == ui::ConnectionState::Failed
                    })
                    .await;
                    assert!(
                        inspect(&controller, |task| task.state
                            == ui::SpringboardState::EndpointSelected
                            && task.discovery.is_some()
                            && task.preview.is_none()
                            && !task.error.is_empty())
                        .await
                    );
                    select(&controller, 0);
                    started(&mut upstream).await;
                    assert!(
                        inspect(&controller, |task| task.selected == Some(EndpointIdentity::Local)
                            && task.error.is_empty()
                            && task.discovery.is_some())
                        .await
                    );
                    controller.close();
                }
            })
            .await;
    }

    #[tokio::test]
    async fn cancelling_pairing_or_consent_disconnects_without_state_leak() {
        tokio::task::LocalSet::new()
            .run_until(async {
                for policy in [PairingPolicy::Generated, PairingPolicy::Disabled] {
                    let generated = policy == PairingPolicy::Generated;
                    let mut viewer = Viewer::start(policy, 0).await;
                    let (controller, mut upstream, discovered) = controller("echo_child");
                    controller.start();
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Cancel", viewer.connection.local_port()),
                    )
                    .await;
                    select(&controller, 1);
                    wait_condition(&controller, move |task| {
                        task.connection_state
                            == if generated {
                                ui::ConnectionState::PairingRequired
                            } else {
                                ui::ConnectionState::UnpairedConfirmationRequired
                            }
                    })
                    .await;
                    assert!(upstream.try_recv().is_err());
                    controller.close();
                    viewer
                        .event(|event| {
                            if generated {
                                matches!(
                                    event,
                                    ConnectionMessage::PairingFinished { accepted: false, .. }
                                )
                            } else {
                                matches!(event, ConnectionMessage::Disconnected { .. })
                            }
                        })
                        .await;
                    assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
                    assert!(upstream.try_recv().is_err());
                }
            })
            .await;
    }

    #[tokio::test]
    async fn restarted_viewers_invalidate_tokens_and_allow_pairing_or_plaintext_retry() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new()
            .run_until(async {
                for replacement_requires_pairing in [true, false] {
                    let (controller, mut upstream, discovered) = controller("echo_child");
                    controller.start();
                    let mut first = Viewer::start(PairingPolicy::Generated, 0).await;
                    let fullname = discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Restarted", first.connection.local_port()),
                    )
                    .await;
                    select(&controller, 1);
                    let ConnectionMessage::PairingStarted { code, .. } = first
                        .event(|event| matches!(event, ConnectionMessage::PairingStarted { .. }))
                        .await
                    else {
                        unreachable!()
                    };
                    wait_condition(&controller, |task| {
                        task.connection_state == ui::ConnectionState::PairingRequired
                    })
                    .await;
                    answer(&controller, PairingAnswer::Code(code));
                    wait_condition(&controller, |task| {
                        task.connection_state == ui::ConnectionState::Connected
                    })
                    .await;
                    initial_request(&mut upstream).await;
                    let expected = fullname.clone();
                    let old_credentials =
                        inspect(&controller, move |task| task.credentials[&expected]).await;
                    controller.close();
                    first
                        .event(|event| matches!(event, ConnectionMessage::Disconnected { .. }))
                        .await;
                    drop(first);
                    let mut replacement = Viewer::start(
                        if replacement_requires_pairing {
                            PairingPolicy::Generated
                        } else {
                            PairingPolicy::Disabled
                        },
                        0,
                    )
                    .await;
                    controller.start();
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Restarted", replacement.connection.local_port()),
                    )
                    .await;
                    select(&controller, 1);
                    if replacement_requires_pairing {
                        let ConnectionMessage::PairingStarted { code, .. } = replacement
                            .event(|event| {
                                matches!(event, ConnectionMessage::PairingStarted { .. })
                            })
                            .await
                        else {
                            unreachable!()
                        };
                        wait_condition(&controller, |task| {
                            task.connection_state == ui::ConnectionState::PairingRequired
                                && task.credentials.is_empty()
                        })
                        .await;
                        answer(&controller, PairingAnswer::Code(code));
                        wait_condition(&controller, |task| {
                            task.connection_state == ui::ConnectionState::Connected
                        })
                        .await;
                    } else {
                        wait_condition(&controller, |task| {
                            task.connection_state == ui::ConnectionState::Failed
                                && task.credentials.is_empty()
                        })
                        .await;
                        assert!(upstream.try_recv().is_err());
                        replacement
                            .event(|event| matches!(event, ConnectionMessage::Disconnected { .. }))
                            .await;
                        select(&controller, 1);
                        accept_plaintext(&controller).await;
                    }
                    initial_request(&mut upstream).await;
                    if replacement_requires_pairing {
                        assert_ne!(
                            inspect(&controller, move |task| task.credentials[&fullname]).await,
                            old_credentials
                        );
                    }
                    compile_source(&controller, &mut replacement, 88).await;
                    controller.close();
                }
            })
            .await;
    }

    #[tokio::test]
    async fn switching_to_local_cancels_a_pending_websocket_upgrade() {
        use tokio::io::AsyncReadExt as _;
        tokio::task::LocalSet::new()
            .run_until(async {
                let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                let (controller, _upstream, discovered) = controller("echo_child");
                controller.start();
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Silent", listener.local_addr().unwrap().port()),
                )
                .await;
                select(&controller, 1);
                let (mut socket, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut bytes = [0u8; 4096];
                assert!(
                    tokio::time::timeout(Duration::from_secs(5), socket.read(&mut bytes))
                        .await
                        .unwrap()
                        .unwrap()
                        > 0
                );
                select(&controller, 0);
                let read = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut bytes))
                    .await
                    .unwrap();
                assert!(matches!(read, Ok(0)) || read.is_err());
                assert!(
                    inspect(&controller, |task| task.selected == Some(EndpointIdentity::Local)
                        && task.state == ui::SpringboardState::EndpointSelected)
                    .await
                );
                controller.close();
            })
            .await;
    }

    #[tokio::test]
    async fn viewer_expiry_rejection_leaves_initial_connection_failed() {
        use crate::remote_authentication::tests::{raw_peer, receive_handshake, send_handshake};
        tokio::task::LocalSet::new()
            .run_until(async {
                let (expire, expiration) = oneshot::channel();
                let (port, peer) = raw_peer(move |mut socket| async move {
                    send_handshake(&mut socket, &PreviewToLspMessage::PairingReady).await;
                    assert!(matches!(
                        receive_handshake(&mut socket).await,
                        LspToPreviewMessage::PairingHello { .. }
                    ));
                    let handshake = pairing::Handshake::with_code(pairing::Role::Viewer, "1234");
                    send_handshake(
                        &mut socket,
                        &PreviewToLspMessage::PairingRequired {
                            attempts_left: pairing::MAX_ATTEMPTS,
                            expires_in_seconds: 0,
                            element: handshake.element().clone(),
                        },
                    )
                    .await;
                    expiration.await.unwrap();
                    send_handshake(
                        &mut socket,
                        &PreviewToLspMessage::PairingRejected {
                            reason: i_slint_live_preview::protocol::PairingRejection::Expired,
                        },
                    )
                    .await;
                })
                .await;
                let (controller, mut upstream, discovered) = controller("echo_child");
                controller.start();
                discover(&controller, &discovered, remote::tests::service("Expires", port)).await;
                select(&controller, 1);
                wait_condition(&controller, |task| {
                    task.connection_state == ui::ConnectionState::PairingRequired
                })
                .await;
                expire.send(()).unwrap();
                wait_condition(&controller, |task| {
                    task.connection_state == ui::ConnectionState::Failed
                        && task.error.contains("expired")
                })
                .await;
                assert!(upstream.try_recv().is_err());
                assert_eq!(state(&controller).await, ui::SpringboardState::EndpointSelected);
                controller.close();
                peer.await.unwrap();
            })
            .await;
    }

    #[tokio::test]
    async fn withdrawn_failed_viewers_require_rediscovery_before_retry() {
        tokio::task::LocalSet::new()
            .run_until(async {
                for removal_before_failure in [true, false] {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
                    let port = listener.local_addr().unwrap().port();
                    drop(listener);
                    let (controller, mut upstream, discovered) = controller("echo_child");
                    controller.start();
                    let fullname = discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Withdrawn", port),
                    )
                    .await;
                    if removal_before_failure {
                        let selected = fullname.clone();
                        controller
                            .task_sender
                            .send(Box::new(move |task| {
                                task.select_endpoint(EndpointIdentity::Remote(selected.clone()));
                                task.handle_discovery(DiscoveryEvent::Removed(selected));
                            }))
                            .unwrap();
                    } else {
                        select(&controller, 1);
                        wait_condition(&controller, |task| {
                            task.connection_state == ui::ConnectionState::Failed
                        })
                        .await;
                        discovered
                            .send(mdns_sd::ServiceEvent::ServiceRemoved(
                                i_slint_live_preview::protocol::SERVICE_TYPE.into(),
                                fullname.clone(),
                            ))
                            .unwrap();
                    }
                    wait_condition(&controller, |task| {
                        task.connection_state == ui::ConnectionState::Failed
                            && !task.viewers[0].unavailable_reason.is_empty()
                    })
                    .await;
                    let (checked, result) = oneshot::channel();
                    let selected = fullname.clone();
                    controller
                        .task_sender
                        .send(Box::new(move |task| {
                            task.select_endpoint(EndpointIdentity::Remote(selected));
                            let _ = checked.send(
                                task.preview.is_none()
                                    && task.connection_state == ui::ConnectionState::Failed
                                    && task.selected_index() == 1,
                            );
                        }))
                        .unwrap();
                    assert!(result.await.unwrap());
                    let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Withdrawn", viewer.connection.local_port()),
                    )
                    .await;
                    assert!(
                        inspect(&controller, |task| task.viewers[0].unavailable_reason.is_empty()
                            && task.preview.is_none())
                        .await
                    );
                    select(&controller, 1);
                    accept_plaintext(&controller).await;
                    initial_request(&mut upstream).await;
                    controller.close();
                    viewer
                        .event(|event| matches!(event, ConnectionMessage::Disconnected { .. }))
                        .await;
                }
            })
            .await;
    }
}
