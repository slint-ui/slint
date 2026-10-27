// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use i_slint_live_preview::protocol::{LspToPreviewMessage, PreviewTarget, PreviewToLspMessage};
use slint::ModelRc;
use tokio::sync::{mpsc, oneshot};

use crate::{
    LspToPreview,
    remote_authentication::{PairingAnswer, PairingCredentials},
    springboard_ui as ui,
};

mod connection;
mod discovery;
mod remote;
mod remote_connection;
mod settings;
use connection::{Connection, ManualProbe};
use discovery::StatefulDiscovery;
use remote::{Discovery, RemoteViewer};
use remote_connection::ConnectionEvent;
use settings::{SPRINGBOARD_SETTINGS_FILE, SpringboardSettings};

#[derive(Clone)]
pub struct LocalPreviewConfig {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}

type SpringboardAction = Box<dyn FnOnce(&mut SpringboardTask) + Send + 'static>;

#[derive(Clone)]
pub struct Springboard {
    task_sender: mpsc::UnboundedSender<SpringboardAction>,
    displayed_endpoints: Arc<RwLock<DisplayedEndpoints>>,
}

/// The identities behind the rows of the UI's endpoint lists, in row order.
/// `update_ui` replaces them together with the lists, on the UI thread.
// TODO: handle a poisoned lock instead of panicking on it.
struct DisplayedEndpoints {
    saved: Vec<EndpointIdentity>,
    discovered: Vec<EndpointIdentity>,
}

impl DisplayedEndpoints {
    fn saved(&self, row: i32) -> Option<EndpointIdentity> {
        self.saved.get(usize::try_from(row).ok()?).cloned()
    }

    fn discovered(&self, row: i32) -> Option<EndpointIdentity> {
        self.discovered.get(usize::try_from(row).ok()?).cloned()
    }
}

/// Identifies a preview endpoint: the Local child process, a discovered
/// viewer, or a viewer the user entered by address.
/// This is what the user picks, what `selected` tracks, and what is saved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum EndpointIdentity {
    #[default]
    Local,
    /// `host` is the name the viewer advertises, see `RemoteViewer::identity`.
    /// The mDNS fullname isn't part of the identity: it changes when mDNS
    /// resolves a name conflict, as "Pixel 8 (2)".
    // TODO: add the viewer's custom name, once it advertises one next to its host.
    Remote { host: String },
    /// Never matched with a discovered viewer, even at the same address.
    ///
    /// NOTE: Viewers listen on a port the OS assigns on every start, so a saved
    /// manual endpoint only reaches its viewer until that viewer restarts.
    /// It's saved anyway: after a restart, the user edits the port instead of
    /// entering the endpoint again.
    /// This is subject to change: a desktop viewer can already pin its address
    /// with `--remote-address`, a shorter `--port` would make that easier, and
    /// a reserved default port would also cover mobile viewers, which take no flags.
    // TODO: add an optional name the user gives it.
    Manual { host: String, port: u16 },
}

struct SpringboardTask {
    local_preview: LocalPreviewConfig,
    to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
    global: slint::Weak<ui::Springboard<'static>>,
    displayed_endpoints: Arc<RwLock<DisplayedEndpoints>>,
    /// Without Local, in the order the user saved them.
    saved_endpoints: Vec<EndpointIdentity>,
    selected_endpoint: EndpointIdentity,
    state: ui::SpringboardState,
    connection: Connection,
    highlight: Option<LspToPreviewMessage>,
    discovery: StatefulDiscovery,
    manual_probe: ManualProbe,
    credentials: HashMap<EndpointIdentity, PairingCredentials>,
}

impl Springboard {
    pub fn new(
        local_preview: LocalPreviewConfig,
        to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
        global: slint::Weak<ui::Springboard<'static>>,
    ) -> Self {
        Self::spawn(local_preview, to_editor, global, Discovery::start())
    }

    fn spawn(
        local_preview: LocalPreviewConfig,
        to_editor: mpsc::UnboundedSender<PreviewToLspMessage>,
        global: slint::Weak<ui::Springboard<'static>>,
        browse: crate::Result<Discovery>,
    ) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel::<SpringboardAction>();
        let displayed_endpoints = Arc::new(RwLock::new(DisplayedEndpoints {
            saved: vec![EndpointIdentity::Local],
            discovered: Vec::new(),
        }));
        let handle = Self { task_sender, displayed_endpoints: displayed_endpoints.clone() };
        let close_handle = handle.clone();
        let select_handle = handle.clone();
        let pair_handle = handle.clone();
        let forget_handle = handle.clone();
        let code_handle = handle.clone();
        let consent_handle = handle.clone();
        let cancel_handle = handle.clone();
        let discover_handle = handle.clone();
        let probe_handle = handle.clone();
        let manual_handle = handle.clone();
        let _ = global.upgrade_in_event_loop(move |global| {
            global.on_close(move || close_handle.close());
            global.on_discover_devices(move || {
                let _ = discover_handle.task_sender.send(Box::new(|task| {
                    task.discovery.restart();
                    task.update_ui();
                }));
            });
            global.on_probe_address(move |host, port| {
                let host = host.trim().to_owned();
                let _ = probe_handle.task_sender.send(Box::new(move |task| {
                    task.manual_probe.start(host, port);
                    task.update_ui();
                }));
            });
            global.on_connect_manual_endpoint(move || {
                let _ =
                    manual_handle.task_sender.send(Box::new(|task| task.connect_manual_endpoint()));
            });
            global.on_select_endpoint(move |row| {
                if let Some(identity) = select_handle.displayed().saved(row) {
                    let _ = select_handle
                        .task_sender
                        .send(Box::new(move |task| task.select_endpoint(identity)));
                }
            });
            global.on_pair_endpoint(move |row| {
                if let Some(identity) = pair_handle.displayed().discovered(row) {
                    let _ = pair_handle
                        .task_sender
                        .send(Box::new(move |task| task.select_endpoint(identity)));
                }
            });
            global.on_forget_device(move |row| {
                if let Some(identity) = forget_handle.displayed().saved(row) {
                    let _ = forget_handle
                        .task_sender
                        .send(Box::new(move |task| task.forget_endpoint(identity)));
                }
            });
            global.on_submit_pairing_code(move |code| {
                let code = code.trim().to_owned();
                let _ = code_handle.task_sender.send(Box::new(move |task| {
                    task.connection.answer(PairingAnswer::Code(code));
                    task.update_ui();
                }));
            });
            global.on_cancel_pairing(move || {
                let _ = cancel_handle.task_sender.send(Box::new(|task| task.cancel_pairing()));
            });
            global.on_accept_unpaired_connection(move || {
                let _ = consent_handle.task_sender.send(Box::new(|task| {
                    task.connection.answer(PairingAnswer::AcceptUnpaired);
                    task.update_ui();
                }));
            });
        });
        let task = SpringboardTask {
            local_preview,
            to_editor,
            global,
            displayed_endpoints,
            saved_endpoints: Vec::new(),
            selected_endpoint: EndpointIdentity::Local,
            state: ui::SpringboardState::Stopped,
            connection: Connection::default(),
            highlight: None,
            discovery: StatefulDiscovery::new(browse),
            manual_probe: ManualProbe::default(),
            credentials: HashMap::new(),
        };
        task.request_settings();
        task.update_ui();
        crate::spawn_local(task.run(task_receiver));
        handle
    }

    fn displayed(&self) -> RwLockReadGuard<'_, DisplayedEndpoints> {
        self.displayed_endpoints.read().expect("displayed endpoints lock is poisoned")
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
        let _ = self.task_sender.send(Box::new(move |task| task.recv(message)));
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
                event = self.connection.next_event() => {
                    match event {
                        Some(event) => self.handle_connection(event),
                        None => {
                            self.connection.fail("Preview connection ended unexpectedly".into());
                            self.update_ui();
                        }
                    }
                }
                () = self.discovery.changed() => {
                    self.connect();
                    self.update_ui();
                }
                () = self.manual_probe.finished() => self.update_ui(),
            }
        }
        self.close();
    }

    fn start(&mut self) {
        if self.state != ui::SpringboardState::Stopped {
            return;
        }
        self.state = ui::SpringboardState::Running;
        self.connect();
        self.update_ui();
    }

    fn connect_manual_endpoint(&mut self) {
        if self.manual_probe.state != ui::ManualEndpointState::Reachable {
            return;
        }
        let ManualProbe { host, port, .. } = &self.manual_probe;
        self.select_endpoint(EndpointIdentity::Manual { host: host.clone(), port: *port });
    }

    /// What clicking row `index` of `saved-endpoints` does.
    #[cfg(test)]
    fn select_saved(&mut self, index: usize) {
        let saved =
            std::iter::once(EndpointIdentity::Local).chain(self.saved_endpoints.iter().cloned());
        if let Some(identity) = saved.into_iter().nth(index) {
            self.select_endpoint(identity);
        }
    }

    /// What clicking Pair on row `index` of `discovered-endpoints` does.
    #[cfg(test)]
    fn pair(&mut self, index: usize) {
        if let Some(identity) = self.discovery.discovered.get(index).map(RemoteViewer::identity) {
            self.select_endpoint(identity);
        }
    }

    fn select_endpoint(&mut self, identity: EndpointIdentity) {
        if self.selected_endpoint == identity
            && self.connection.state != ui::ConnectionState::Failed
        {
            return;
        }
        self.connection = Connection::default();
        self.selected_endpoint = identity;
        // A new endpoint is saved once it connects, see `ConnectionEvent::Connected`.
        if !self.is_unsaved(&self.selected_endpoint) {
            self.store_settings();
        }
        self.connect();
        self.update_ui();
    }

    fn is_unsaved(&self, identity: &EndpointIdentity) -> bool {
        identity != &EndpointIdentity::Local && !self.saved_endpoints.contains(identity)
    }

    /// Drop a connection that waits for the user to pair or to accept it.
    fn cancel_pairing(&mut self) {
        if matches!(
            self.connection.state,
            ui::ConnectionState::PairingRequired
                | ui::ConnectionState::UnpairedConfirmationRequired
        ) {
            self.connection.fail("Pairing cancelled".into());
            self.update_ui();
        }
    }

    /// Start the preview for `selected`, unless stopped, already started, or
    /// failed. A remote viewer is started once discovery reports it usable.
    fn connect(&mut self) {
        if self.state == ui::SpringboardState::Stopped
            || self.connection.is_started()
            || self.connection.state == ui::ConnectionState::Failed
        {
            return;
        }
        match &self.selected_endpoint {
            EndpointIdentity::Local => self.connection = Connection::local(&self.local_preview),
            EndpointIdentity::Remote { .. } => {
                let Some(viewer) = self.discovery.find(&self.selected_endpoint) else { return };
                if !viewer.unavailable_reason.is_empty() {
                    let reason = viewer.unavailable_reason.clone();
                    self.connection.fail(reason);
                    return;
                }
                let credentials = self.credentials.get(&self.selected_endpoint).copied();
                self.connection = Connection::remote(viewer.clone(), credentials);
            }
            EndpointIdentity::Manual { host, port } => {
                let credentials = self.credentials.get(&self.selected_endpoint).copied();
                self.connection =
                    Connection::remote(RemoteViewer::manual(host, *port), credentials);
            }
        }
    }

    fn handle_connection(&mut self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Pairing { prompt, answer } => self.connection.prompt(prompt, answer),
            ConnectionEvent::CredentialsChanged(credentials) => {
                if let selected @ (EndpointIdentity::Remote { .. }
                | EndpointIdentity::Manual { .. }) = &self.selected_endpoint
                {
                    match credentials {
                        Some(credentials) => {
                            self.credentials.insert(selected.clone(), credentials);
                        }
                        None => {
                            self.credentials.remove(selected);
                        }
                    }
                }
                return;
            }
            ConnectionEvent::Connected => {
                self.connection.connected();
                if self.is_unsaved(&self.selected_endpoint) {
                    self.saved_endpoints.push(self.selected_endpoint.clone());
                    self.store_settings();
                }
            }
            ConnectionEvent::Message(PreviewToLspMessage::Exited) => {
                self.close();
                return;
            }
            ConnectionEvent::Message(PreviewToLspMessage::SendShowMessage { message }) => {
                let error =
                    (message.typ == lsp_types::MessageType::ERROR).then(|| message.message.clone());
                let _ = self.to_editor.send(PreviewToLspMessage::SendShowMessage { message });
                let Some(error) = error else { return };
                self.connection.fail(error);
            }
            ConnectionEvent::Message(message) => {
                let _ = self.to_editor.send(message);
                return;
            }
            ConnectionEvent::Ended { error } => {
                if error.is_none() && self.connection.state == ui::ConnectionState::Connected {
                    self.close();
                    return;
                }
                self.connection.fail(
                    error.unwrap_or_else(|| "Viewer connection ended before connecting".into()),
                );
            }
        }
        self.update_ui();
    }

    fn close(&mut self) {
        self.connection = Connection::default();
        self.state = ui::SpringboardState::Stopped;
        self.update_ui();
    }

    fn forget_endpoint(&mut self, identity: EndpointIdentity) {
        self.saved_endpoints.retain(|saved| saved != &identity);
        self.credentials.remove(&identity);
        if self.selected_endpoint == identity {
            self.selected_endpoint = EndpointIdentity::Local;
            self.connection = Connection::default();
            self.state = ui::SpringboardState::Stopped;
        }
        self.store_settings();
        self.update_ui();
    }

    /// Ask the editor for the blob holding the saved endpoints, which it answers
    /// with a `SetUserSettings` handled in [`Self::recv`].
    fn request_settings(&self) {
        let _ = self.to_editor.send(PreviewToLspMessage::RequestState {
            files: Vec::new(),
            settings: vec![SPRINGBOARD_SETTINGS_FILE.into()],
        });
    }

    /// Persist the saved endpoints and the selection through the editor, which
    /// owns the filesystem.
    fn store_settings(&self) {
        let settings =
            SpringboardSettings::new(self.saved_endpoints.clone(), self.selected_endpoint.clone());
        let _ = self.to_editor.send(PreviewToLspMessage::UpdateUserSettings {
            name: SPRINGBOARD_SETTINGS_FILE.into(),
            contents: settings.serialize(),
        });
    }

    fn recv(&mut self, message: LspToPreviewMessage) {
        match message {
            LspToPreviewMessage::Quit => self.close(),
            LspToPreviewMessage::SetUserSettings { name, contents }
                if name == SPRINGBOARD_SETTINGS_FILE =>
            {
                self.handle_settings(&contents)
            }
            LspToPreviewMessage::HighlightFromEditor { .. } => {
                self.highlight = Some(message.clone());
                self.connection.send(message);
            }
            LspToPreviewMessage::ShowPreview(_) => {
                self.connection.send(message);
                if let Some(highlight) = self.highlight.clone() {
                    self.connection.send(highlight);
                }
            }
            message => self.connection.send(message),
        }
    }

    /// NOTE: Overwrites the saved endpoints and the selection without merging.
    /// The editor answers once its project is loaded, so an endpoint paired
    /// before that would be lost. Pairing takes longer than loading a project,
    /// so this isn't handled.
    fn handle_settings(&mut self, contents: &str) {
        let Some(settings) = SpringboardSettings::deserialize(contents) else { return };
        self.saved_endpoints = settings.saved;
        self.selected_endpoint = settings.selected;
        self.update_ui();
    }

    fn discovered_row(&self, viewer: &RemoteViewer) -> ui::PreviewEndpoint {
        ui::PreviewEndpoint {
            name: viewer.name.clone().into(),
            kind: ui::PreviewEndpointKind::Remote,
            address: format!("{}:{}", viewer.addresses.join(", "), viewer.port).into(),
            unavailable_reason: viewer.unavailable_reason.clone().into(),
            saved: self.saved_endpoints.contains(&viewer.identity()),
            ..Default::default()
        }
    }

    fn saved_row(&self, identity: &EndpointIdentity) -> ui::PreviewEndpoint {
        match identity {
            EndpointIdentity::Local => ui::PreviewEndpoint {
                name: "Local".into(),
                kind: ui::PreviewEndpointKind::Local,
                ..Default::default()
            },
            EndpointIdentity::Remote { host } => match self.discovery.find(identity) {
                Some(viewer) => self.discovered_row(viewer),
                // TODO: consider keeping unavailable saved endpoints selectable,
                // since `connect` already waits until the viewer is discovered.
                None => ui::PreviewEndpoint {
                    name: host.into(),
                    kind: ui::PreviewEndpointKind::Remote,
                    // A live connection outlasts the viewer's mDNS records.
                    unavailable_reason: if identity == &self.selected_endpoint
                        && self.connection.is_started()
                    {
                        Default::default()
                    } else {
                        "Viewer isn't nearby".into()
                    },
                    ..Default::default()
                },
            },
            // Never disabled: only dialing tells whether a manual endpoint answers.
            EndpointIdentity::Manual { host, port } => ui::PreviewEndpoint {
                name: format!("{host}:{port}").into(),
                kind: ui::PreviewEndpointKind::Manual,
                address: format!("{host}:{port}").into(),
                ..Default::default()
            },
        }
    }

    fn update_ui(&self) {
        let saved: Vec<_> = std::iter::once(EndpointIdentity::Local)
            .chain(self.saved_endpoints.iter().cloned())
            .collect();
        let saved_rows: Vec<_> = saved.iter().map(|identity| self.saved_row(identity)).collect();
        let discovered: Vec<_> =
            self.discovery.discovered.iter().map(RemoteViewer::identity).collect();
        let discovered_rows: Vec<_> =
            self.discovery.discovered.iter().map(|viewer| self.discovered_row(viewer)).collect();
        let selected =
            saved.iter().position(|row| row == &self.selected_endpoint).map_or(-1, |i| i as i32);
        let selected_row = self.saved_row(&self.selected_endpoint);
        let displayed = self.displayed_endpoints.clone();
        let state = self.state;
        let connection = ui::Connection {
            state: self.connection.state,
            pairing_code_digits: self.connection.pairing_code_digits.into(),
            error: self.connection.error.clone().into(),
        };
        let discovery_error = self.discovery.error.clone();
        let manual_endpoint = ui::ManualEndpoint {
            state: self.manual_probe.state,
            version: self.manual_probe.version.clone().into(),
        };
        let _ = self.global.upgrade_in_event_loop(move |global| {
            *displayed.write().expect("displayed endpoints lock is poisoned") =
                DisplayedEndpoints { saved, discovered };
            global.set_saved_endpoints(ModelRc::new(slint::VecModel::from(saved_rows)));
            global.set_discovered_endpoints(ModelRc::new(slint::VecModel::from(discovered_rows)));
            global.set_discovery_error(discovery_error.into());
            global.set_selected_endpoint_index(selected);
            global.set_selected_endpoint(selected_row);
            global.set_state(state);
            global.set_connection(connection);
            global.set_manual_endpoint(manual_endpoint);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::child_process::tests::{component, fixture_config, receive, started};
    use i_slint_live_preview::protocol::pairing;

    /// A springboard whose discovery reports what the test sends.
    /// It asks the editor for its settings right away; the request is consumed here.
    fn controller(
        name: &str,
    ) -> (
        Springboard,
        mpsc::UnboundedReceiver<PreviewToLspMessage>,
        flume::Sender<mdns_sd::ServiceEvent>,
    ) {
        let (executable, arguments) = fixture_config(name);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let (discovered, discovery_receiver) = flume::unbounded();
        let controller = Springboard::spawn(
            LocalPreviewConfig { executable, arguments },
            sender,
            Default::default(),
            Ok(Discovery::from_receiver(discovery_receiver)),
        );
        match receiver.try_recv() {
            Ok(PreviewToLspMessage::RequestState { settings, .. })
                if settings == [SPRINGBOARD_SETTINGS_FILE.to_string()] => {}
            other => panic!("Expected a request for the user settings, got {other:?}"),
        }
        (controller, receiver, discovered)
    }

    fn select_saved(controller: &Springboard, index: usize) {
        controller.task_sender.send(Box::new(move |task| task.select_saved(index))).unwrap();
    }

    fn pair(controller: &Springboard, index: usize) {
        controller.task_sender.send(Box::new(move |task| task.pair(index))).unwrap();
    }

    /// The identity of the viewer `remote::tests::service(host, …)` announces.
    fn remote(host: &str) -> EndpointIdentity {
        EndpointIdentity::Remote { host: host.into() }
    }

    /// The springboard stores the new selection through the editor, which owns
    /// the filesystem.
    async fn assert_user_setting_set(receiver: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>) {
        match receive(receiver).await {
            PreviewToLspMessage::UpdateUserSettings { name, .. }
                if name == SPRINGBOARD_SETTINGS_FILE => {}
            other => panic!("Expected the user settings to be stored, got {other:?}"),
        }
    }

    async fn stored_settings(
        receiver: &mut mpsc::UnboundedReceiver<PreviewToLspMessage>,
    ) -> SpringboardSettings {
        match receive(receiver).await {
            PreviewToLspMessage::UpdateUserSettings { name, contents }
                if name == SPRINGBOARD_SETTINGS_FILE =>
            {
                SpringboardSettings::deserialize(&contents).unwrap()
            }
            other => panic!("Expected the user settings to be stored, got {other:?}"),
        }
    }

    /// A port nothing listens on, so dialing a viewer there fails.
    async fn closed_port() -> u16 {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        listener.local_addr().unwrap().port()
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
                // Nothing is saved or discovered, so these select nothing.
                select_saved(&controller, 1);
                pair(&controller, 0);
                assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
                assert!(receiver.try_recv().is_err());
                controller.start();
                let first_process = started(&mut receiver).await;
                // Neither the selected endpoint nor a second start restarts it.
                select_saved(&controller, 0);
                controller.start();
                assert_eq!(state(&controller).await, ui::SpringboardState::Running);
                assert!(receiver.try_recv().is_err());
                let (old_sender, old_receiver) = mpsc::unbounded_channel();
                let queued_old_sender = old_sender.clone();
                controller
                    .task_sender
                    .send(Box::new(move |task| {
                        task.connection.replace_local_receiver(old_receiver);
                        queued_old_sender.send(PreviewToLspMessage::Pong).unwrap();
                        queued_old_sender.send(PreviewToLspMessage::Exited).unwrap();
                        task.close();
                    }))
                    .unwrap();
                controller.start();
                let second_process = started(&mut receiver).await;
                assert_ne!(first_process, second_process);
                assert!(old_sender.send(PreviewToLspMessage::Exited).is_err());
                assert_eq!(state(&controller).await, ui::SpringboardState::Running);
                assert!(receiver.try_recv().is_err());
                tokio::join!(controller.shutdown(), controller.shutdown());
                assert_eq!(state(&controller).await, ui::SpringboardState::Stopped);
                controller.shutdown().await;
                assert!(receiver.try_recv().is_err());
                controller.start();
                started(&mut receiver).await;
                assert_eq!(state(&controller).await, ui::SpringboardState::Running);
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
                let failed = |task: &SpringboardTask| {
                    task.state == ui::SpringboardState::Running
                        && task.connection.state == ui::ConnectionState::Failed
                        && !task.connection.error.is_empty()
                        && !task.connection.is_started()
                };
                assert!(inspect(&controller, failed).await);
                controller.send(&LspToPreviewMessage::ShowPreview(component()));
                controller.start();
                assert!(inspect(&controller, failed).await);
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        task.local_preview.executable = std::env::current_exe().unwrap();
                    }))
                    .unwrap();
                select_saved(&controller, 0);
                assert_user_setting_set(&mut receiver).await;
                started(&mut receiver).await;
                assert!(inspect(&controller, |task| task.connection.error.is_empty()).await);
                controller.shutdown().await;
            })
            .await;
    }

    #[tokio::test]
    async fn crash_persists_error_and_does_not_restart() {
        tokio::task::LocalSet::new().run_until(async {
            let (controller, mut receiver, _discovery) = controller("crashing_child");
            controller.start();
            started(&mut receiver).await;
            assert!(matches!(receive(&mut receiver).await, PreviewToLspMessage::SendShowMessage { message } if message.typ == lsp_types::MessageType::ERROR));
            assert!(inspect(&controller, |task| task.state == ui::SpringboardState::Running
                && task.connection.state == ui::ConnectionState::Failed
                && task.discovery.is_browsing() && !task.connection.error.is_empty()).await);
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            assert_eq!(state(&controller).await, ui::SpringboardState::Running);
            assert!(inspect(&controller, |task| !task.connection.error.is_empty() && !task.connection.is_started()).await);
            controller.task_sender.send(Box::new(|task| {
                task.local_preview.arguments = fixture_config("echo_child").1;
            })).unwrap();
            select_saved(&controller, 0);
            assert_user_setting_set(&mut receiver).await;
            started(&mut receiver).await;
            assert!(inspect(&controller, |task| task.connection.error.is_empty()).await);
            controller.close();
            assert!(inspect(&controller, |task| task.connection.error.is_empty()).await);
            controller.shutdown().await;
        }).await;
    }

    #[tokio::test]
    async fn endpoint_exit_stops_without_ending_upstream() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut receiver, _discovery) = controller("echo_child");
                controller.start();
                started(&mut receiver).await;
                controller
                    .task_sender
                    .send(Box::new(|task| {
                        let (sender, endpoint) = mpsc::unbounded_channel();
                        sender.send(PreviewToLspMessage::Exited).unwrap();
                        task.connection.replace_local_receiver(endpoint);
                    }))
                    .unwrap();
                wait_stopped(&controller).await;
                assert!(inspect(&controller, |task| task.connection.error.is_empty()).await);
                assert!(receiver.try_recv().is_err());
                controller.start();
                started(&mut receiver).await;
                assert_eq!(state(&controller).await, ui::SpringboardState::Running);
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
            task.discovery
                .discovered
                .iter()
                .any(|viewer| viewer.fullname == expected && viewer.port == port)
        })
        .await;
        fullname
    }

    async fn withdraw(
        controller: &Springboard,
        sender: &flume::Sender<mdns_sd::ServiceEvent>,
        fullname: &str,
    ) {
        sender
            .send(mdns_sd::ServiceEvent::ServiceRemoved(
                i_slint_live_preview::protocol::SERVICE_TYPE.into(),
                fullname.into(),
            ))
            .unwrap();
        let removed = fullname.to_owned();
        wait_condition(controller, move |task| {
            !task.discovery.discovered.iter().any(|viewer| viewer.fullname == removed)
        })
        .await;
    }

    fn answer(controller: &Springboard, answer: PairingAnswer) {
        controller
            .task_sender
            .send(Box::new(move |task| {
                task.connection.answer(answer);
                task.update_ui();
            }))
            .unwrap();
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
                overrides: i_slint_compiler::project_file::Overrides {
                    style: Some("fluent".into()),
                    ..Default::default()
                },
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

    #[tokio::test]
    async fn loopback_manifest_delivery_and_override_replacement_rebuild() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new().run_until(async {
            let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
            let (controller, mut upstream, discovered) = controller("echo_child");
            discover(&controller, &discovered, remote::tests::service("Manifest", viewer.connection.local_port())).await;
            pair(&controller, 0);
            controller.start();
            accept_plaintext(&controller).await;
            initial_request(&mut upstream).await;
            let directory = tempfile::tempdir().unwrap();
            let source = lsp_types::Url::from_file_path(directory.path().join("main.slint")).unwrap();
            let manifest = lsp_types::Url::from_file_path(directory.path().join("slint-project.json")).unwrap();
            controller.send(&LspToPreviewMessage::OpenProject { root: Some(manifest.join(".").unwrap()) });
            let target = i_slint_live_preview::protocol::PreviewComponent { url: source.clone(), component: None };
            controller.send(&LspToPreviewMessage::SetContents { url: i_slint_live_preview::protocol::VersionedUrl::new(source, None), contents: "export component Main inherits Window { ComponentContainer {} }".into() });
            controller.send(&LspToPreviewMessage::SetContents { url: i_slint_live_preview::protocol::VersionedUrl::new(manifest.clone(), None), contents: r#"{"slint-edition":"1.19","experimental-features":["component-container"]}"#.into() });
            controller.send(&LspToPreviewMessage::ShowPreview(target.clone()));
            viewer.preview_event(|event| matches!(event, PreviewSessionEvent::ShowPreview { .. })).await;
            assert!(matches!(viewer.session.compile_component(&target).await, PreviewCompilation::Ready(_)));
            controller.send(&LspToPreviewMessage::SetContents { url: i_slint_live_preview::protocol::VersionedUrl::new(manifest.clone(), None), contents: r#"{"slint-edition":"1.19","experimental-features":[]}"#.into() });
            viewer.preview_event(|event| matches!(event, PreviewSessionEvent::ContentsChanged)).await;
            assert!(matches!(viewer.session.compile_component(&target).await, PreviewCompilation::CompilationError { .. }));
            let component_container = i_slint_compiler::ExperimentalFeature::ComponentContainer.into();
            for (experimental_features, succeeds) in [(Some(component_container), true), (None, false)] {
                let mut config = i_slint_live_preview::protocol::PreviewConfig::default();
                config.overrides.project.experimental_features = experimental_features;
                config.overrides.debug_info = Some(true);
                controller.send(&LspToPreviewMessage::SetConfiguration { config });
                viewer.preview_event(|event| matches!(event, PreviewSessionEvent::ContentsChanged)).await;
                assert_eq!(matches!(viewer.session.compile_component(&target).await, PreviewCompilation::Ready(_)), succeeds);
            }
            while let Ok(message) = upstream.try_recv() {
                assert!(!matches!(message, PreviewToLspMessage::RequestState { files, .. } if !files.is_empty()));
            }
            controller.close();
        }).await;
    }

    async fn accept_plaintext(controller: &Springboard) {
        wait_condition(controller, |task| {
            task.connection.state == ui::ConnectionState::UnpairedConfirmationRequired
        })
        .await;
        answer(controller, PairingAnswer::AcceptUnpaired);
        wait_condition(controller, |task| task.connection.state == ui::ConnectionState::Connected)
            .await;
    }

    #[tokio::test]
    async fn discovery_updates_and_stale_display_indices_keep_identity() {
        tokio::task::LocalSet::new()
            .run_until(async {
                // Discovery runs before the preview is started.
                let (controller, _upstream, discovered) = controller("echo_child");
                let first = remote::tests::service("First", 1);
                let mut second = remote::tests::service("Second", 1);
                let first_id = discover(&controller, &discovered, first).await;
                discover(&controller, &discovered, second.clone()).await;
                discover(&controller, &discovered, second.clone()).await;
                assert!(inspect(&controller, |task| task.discovery.discovered.len() == 2).await);
                second.port = 2;
                discover(&controller, &discovered, second).await;
                let second = remote("Second");
                // The UI still shows the rows from before the removal below.
                controller.displayed_endpoints.write().unwrap().discovered =
                    vec![remote("First"), second.clone()];
                withdraw(&controller, &discovered, &first_id).await;
                // What clicking Pair on the second row does.
                let identity = controller.displayed().discovered(1).unwrap();
                controller
                    .task_sender
                    .send(Box::new(move |task| task.select_endpoint(identity)))
                    .unwrap();
                wait_condition(&controller, move |task| {
                    // It's saved once it connects, which this one never does.
                    task.selected_endpoint == second && task.saved_endpoints.is_empty()
                })
                .await;
                drop(discovered);
                wait_condition(&controller, |task| {
                    !task.discovery.is_browsing()
                        && task.discovery.discovered.is_empty()
                        && !task.discovery.error.is_empty()
                })
                .await;
                select_saved(&controller, 0);
                assert!(
                    inspect(&controller, |task| task.selected_endpoint == EndpointIdentity::Local
                        && task.state == ui::SpringboardState::Stopped)
                    .await
                );
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
            discover(&controller, &discovered, remote::tests::service("Paired", viewer.connection.local_port())).await;
            pair(&controller, 0);
            controller.start();
            let ConnectionMessage::PairingStarted { code, .. } = viewer.event(|event| matches!(event, ConnectionMessage::PairingStarted { .. })).await else { unreachable!() };
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::PairingRequired).await;
            answer(&controller, PairingAnswer::Code("12".into()));
            assert!(inspect(&controller, |task| task.connection.awaits_answer() && task.connection.error == "Enter 4 numeric digits").await);
            answer(&controller, PairingAnswer::Code(if code == "0000" { "0001" } else { "0000" }.into()));
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::PairingRequired && task.connection.error.contains("Incorrect code")).await;
            answer(&controller, PairingAnswer::Code(code));
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::Connected && task.credentials.len() == 1).await;
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
            let (connection, error) = inspect(&controller, |task| (task.connection.state, task.connection.error.clone())).await;
            assert_eq!(connection, ui::ConnectionState::Connected, "{error}");
            controller.send(&LspToPreviewMessage::ShowPreview(component()));
            let event = tokio::time::timeout(Duration::from_secs(5), viewer.session_events.recv()).await;
            if event.is_err() {
                let status = inspect(&controller, |task| (task.state, task.connection.state, task.connection.error.clone())).await;
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
            let first_id = discover(&controller, &discovered, remote::tests::service("First", first.connection.local_port())).await;
            discover(&controller, &discovered, remote::tests::service("Second", second.connection.local_port())).await;
            pair(&controller, 0);
            controller.start();
            let ConnectionMessage::PairingStarted { code, .. } = first.event(|event| matches!(event, ConnectionMessage::PairingStarted { .. })).await else { unreachable!() };
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::PairingRequired).await;
            answer(&controller, PairingAnswer::Code(code));
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut first, 11).await;
            pair(&controller, 1);
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            accept_plaintext(&controller).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut second, 22).await;
            pair(&controller, 0);
            second.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            let event = first.event(|event| matches!(event, ConnectionMessage::Connected { .. } | ConnectionMessage::PairingStarted { .. })).await;
            assert!(matches!(event, ConnectionMessage::Connected { .. }));
            compile_source(&controller, &mut first, 33).await;
            assert!(second.session_events.try_recv().is_err());
            // The established connection outlives the viewer's mDNS records.
            withdraw(&controller, &discovered, &first_id).await;
            assert!(inspect(&controller, |task| task.selected_endpoint == remote("First") && task.connection.state == ui::ConnectionState::Connected).await);
            select_saved(&controller, 0);
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            tokio::time::timeout(Duration::from_secs(5), async {
                loop { if matches!(upstream.recv().await.unwrap(), PreviewToLspMessage::DebugMessage { message, .. } if message.parse::<u32>().is_ok()) { break; } }
            }).await.unwrap();
            initial_request(&mut upstream).await;
            discover(&controller, &discovered, remote::tests::service("First", first.connection.local_port())).await;
            controller.task_sender.send(Box::new(move |task| task.select_endpoint(remote("First")))).unwrap();
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::Connected).await;
            initial_request(&mut upstream).await;
            compile_source(&controller, &mut first, 44).await;
            controller.close();
            first.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            assert!(inspect(&controller, |task| task.credentials.len() == 1).await);
            // Stopping keeps the selection, so the next start reconnects it.
            controller.start();
            wait_condition(&controller, |task| task.connection.state == ui::ConnectionState::Connected).await;
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
                discover(&controller, &discovered, remote::tests::service("Retry", port)).await;
                pair(&controller, 0);
                controller.start();
                wait_condition(&controller, |task| {
                    task.connection.state == ui::ConnectionState::Failed
                        && !task.connection.is_started()
                })
                .await;
                assert!(
                    inspect(&controller, move |task| task.state == ui::SpringboardState::Running
                        && task.selected_endpoint == remote("Retry"))
                    .await
                );
                let mut viewer = Viewer::start(PairingPolicy::Disabled, port).await;
                // Nothing is stored before it connects.
                assert!(upstream.try_recv().is_err());
                pair(&controller, 0);
                accept_plaintext(&controller).await;
                initial_request(&mut upstream).await;
                compile_source(&controller, &mut viewer, 77).await;
                drop(viewer);
                wait_stopped(&controller).await;
                assert!(
                    inspect(&controller, |task| !task.connection.is_started()
                        && task.selected_endpoint == remote("Retry")
                        && task.discovery.is_browsing()
                        && task.connection.error.is_empty())
                    .await
                );
                select_saved(&controller, 0);
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
                    discover(&controller, &discovered, remote::tests::service("Interrupted", port))
                        .await;
                    pair(&controller, 0);
                    controller.start();
                    accept_plaintext(&controller).await;
                    initial_request(&mut upstream).await;
                    disconnect.send(()).unwrap();
                    peer.await.unwrap();
                    wait_condition(&controller, |task| {
                        task.connection.state == ui::ConnectionState::Failed
                    })
                    .await;
                    assert!(
                        inspect(&controller, |task| task.state == ui::SpringboardState::Running
                            && task.discovery.is_browsing()
                            && !task.connection.is_started()
                            && !task.connection.error.is_empty())
                        .await
                    );
                    select_saved(&controller, 0);
                    assert_user_setting_set(&mut upstream).await;
                    started(&mut upstream).await;
                    assert!(
                        inspect(&controller, |task| task.selected_endpoint
                            == EndpointIdentity::Local
                            && task.connection.error.is_empty()
                            && task.discovery.is_browsing())
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
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Cancel", viewer.connection.local_port()),
                    )
                    .await;
                    pair(&controller, 0);
                    controller.start();
                    wait_condition(&controller, move |task| {
                        task.connection.state
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
                    assert!(
                        inspect(&controller, |task| task.state == ui::SpringboardState::Stopped
                            && task.saved_endpoints.is_empty())
                        .await
                    );
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
                    let mut first = Viewer::start(PairingPolicy::Generated, 0).await;
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Restarted", first.connection.local_port()),
                    )
                    .await;
                    pair(&controller, 0);
                    controller.start();
                    let ConnectionMessage::PairingStarted { code, .. } = first
                        .event(|event| matches!(event, ConnectionMessage::PairingStarted { .. }))
                        .await
                    else {
                        unreachable!()
                    };
                    wait_condition(&controller, |task| {
                        task.connection.state == ui::ConnectionState::PairingRequired
                    })
                    .await;
                    answer(&controller, PairingAnswer::Code(code));
                    wait_condition(&controller, |task| {
                        task.connection.state == ui::ConnectionState::Connected
                    })
                    .await;
                    initial_request(&mut upstream).await;
                    let old_credentials =
                        inspect(&controller, |task| task.credentials[&remote("Restarted")]).await;
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
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Restarted", replacement.connection.local_port()),
                    )
                    .await;
                    // The kept selection connects with the token of the replaced viewer.
                    controller.start();
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
                            task.connection.state == ui::ConnectionState::PairingRequired
                                && task.credentials.is_empty()
                        })
                        .await;
                        answer(&controller, PairingAnswer::Code(code));
                        wait_condition(&controller, |task| {
                            task.connection.state == ui::ConnectionState::Connected
                        })
                        .await;
                    } else {
                        wait_condition(&controller, |task| {
                            task.connection.state == ui::ConnectionState::Failed
                                && task.credentials.is_empty()
                        })
                        .await;
                        assert!(upstream.try_recv().is_err());
                        replacement
                            .event(|event| matches!(event, ConnectionMessage::Disconnected { .. }))
                            .await;
                        pair(&controller, 0);
                        accept_plaintext(&controller).await;
                    }
                    initial_request(&mut upstream).await;
                    if replacement_requires_pairing {
                        assert_ne!(
                            inspect(&controller, |task| task.credentials[&remote("Restarted")])
                                .await,
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
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Silent", listener.local_addr().unwrap().port()),
                )
                .await;
                pair(&controller, 0);
                controller.start();
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
                select_saved(&controller, 0);
                let read = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut bytes))
                    .await
                    .unwrap();
                assert!(matches!(read, Ok(0)) || read.is_err());
                assert!(
                    inspect(&controller, |task| task.selected_endpoint == EndpointIdentity::Local
                        && task.state == ui::SpringboardState::Running)
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
                            code_digits: 4,
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
                discover(&controller, &discovered, remote::tests::service("Expires", port)).await;
                pair(&controller, 0);
                controller.start();
                wait_condition(&controller, |task| {
                    task.connection.state == ui::ConnectionState::PairingRequired
                })
                .await;
                expire.send(()).unwrap();
                wait_condition(&controller, |task| {
                    task.connection.state == ui::ConnectionState::Failed
                        && task.connection.error.contains("expired")
                })
                .await;
                assert!(upstream.try_recv().is_err());
                assert_eq!(state(&controller).await, ui::SpringboardState::Running);
                controller.close();
                peer.await.unwrap();
            })
            .await;
    }

    #[tokio::test]
    async fn a_withdrawn_viewer_connects_once_it_is_rediscovered() {
        tokio::task::LocalSet::new()
            .run_until(async {
                for removal_before_failure in [true, false] {
                    let port = closed_port().await;
                    let (controller, mut upstream, discovered) = controller("echo_child");
                    let fullname = discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Withdrawn", port),
                    )
                    .await;
                    pair(&controller, 0);
                    wait_condition(&controller, |task| {
                        task.selected_endpoint == remote("Withdrawn")
                    })
                    .await;
                    if removal_before_failure {
                        withdraw(&controller, &discovered, &fullname).await;
                        controller.start();
                    } else {
                        controller.start();
                        wait_condition(&controller, |task| {
                            task.connection.state == ui::ConnectionState::Failed
                        })
                        .await;
                        withdraw(&controller, &discovered, &fullname).await;
                        // Picking it again waits for the viewer instead of failing.
                        controller
                            .task_sender
                            .send(Box::new(|task| task.select_endpoint(remote("Withdrawn"))))
                            .unwrap();
                    }
                    assert!(
                        inspect(&controller, |task| task.selected_endpoint == remote("Withdrawn")
                            && task.connection.state == ui::ConnectionState::None
                            && !task.connection.is_started())
                        .await
                    );
                    let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                    discover(
                        &controller,
                        &discovered,
                        remote::tests::service("Withdrawn", viewer.connection.local_port()),
                    )
                    .await;
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

    /// The editor replies to the request for the stored settings.
    fn reply_settings(
        controller: &Springboard,
        saved: Vec<EndpointIdentity>,
        selected: EndpointIdentity,
    ) {
        controller.send(&LspToPreviewMessage::SetUserSettings {
            name: SPRINGBOARD_SETTINGS_FILE.into(),
            contents: SpringboardSettings::new(saved, selected).serialize(),
        });
    }

    #[tokio::test]
    async fn stored_settings_are_restored_before_the_preview_starts() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, _discovery) = controller("echo_child");
                let saved = vec![remote("Remembered"), remote("Other")];
                reply_settings(&controller, saved.clone(), remote("Remembered"));
                assert!(
                    inspect(&controller, move |task| task.saved_endpoints == saved
                        && task.selected_endpoint == remote("Remembered")
                        && task.state == ui::SpringboardState::Stopped)
                    .await
                );
                // Restoring isn't a change, so nothing is stored back.
                assert!(upstream.try_recv().is_err());
                controller.close();
            })
            .await;
    }

    #[tokio::test]
    async fn a_remembered_viewer_is_connected_once_it_is_discovered() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, discovered) = controller("echo_child");
                let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                reply_settings(&controller, vec![remote("Remembered")], remote("Remembered"));
                controller.start();
                // Nothing to dial until the viewer is announced.
                assert!(
                    inspect(&controller, |task| task.state == ui::SpringboardState::Running
                        && !task.connection.is_started())
                    .await
                );
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Remembered", viewer.connection.local_port()),
                )
                .await;
                accept_plaintext(&controller).await;
                initial_request(&mut upstream).await;
                controller.close();
                viewer.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
            })
            .await;
    }

    #[tokio::test]
    async fn a_selection_made_while_stopped_survives_stop() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, discovered) = controller("echo_child");
                let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Chosen", viewer.connection.local_port()),
                )
                .await;
                pair(&controller, 0);
                assert!(inspect(&controller, |task| !task.connection.is_started()).await);
                for round in 0..2 {
                    controller.start();
                    accept_plaintext(&controller).await;
                    if round == 0 {
                        // Saved once it connects.
                        let settings = stored_settings(&mut upstream).await;
                        assert_eq!(settings.saved, [remote("Chosen")]);
                        assert_eq!(settings.selected, remote("Chosen"));
                    }
                    initial_request(&mut upstream).await;
                    controller.close();
                    viewer
                        .event(|event| matches!(event, ConnectionMessage::Disconnected { .. }))
                        .await;
                    assert!(
                        inspect(&controller, |task| task.selected_endpoint == remote("Chosen")
                            && task.state == ui::SpringboardState::Stopped)
                        .await
                    );
                }
            })
            .await;
    }

    #[tokio::test]
    async fn forgetting_the_selected_endpoint_stops_on_local() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, discovered) = controller("echo_child");
                let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Saved", viewer.connection.local_port()),
                )
                .await;
                pair(&controller, 0);
                controller.start();
                accept_plaintext(&controller).await;
                assert_eq!(stored_settings(&mut upstream).await.saved, [remote("Saved")]);
                initial_request(&mut upstream).await;
                controller
                    .task_sender
                    .send(Box::new(|task| task.forget_endpoint(remote("Saved"))))
                    .unwrap();
                let settings = stored_settings(&mut upstream).await;
                assert!(settings.saved.is_empty());
                assert_eq!(settings.selected, EndpointIdentity::Local);
                assert!(
                    inspect(&controller, |task| task.selected_endpoint == EndpointIdentity::Local
                        && task.state == ui::SpringboardState::Stopped
                        && task.connection.state == ui::ConnectionState::None
                        && !task.connection.is_started())
                    .await
                );
                viewer.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
                controller.close();
            })
            .await;
    }

    #[tokio::test]
    async fn a_probed_manual_endpoint_is_saved_and_dialed() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, _discovery) = controller("echo_child");
                let mut viewer = Viewer::start(PairingPolicy::Disabled, 0).await;
                let port = viewer.connection.local_port();
                let probe = |host: &'static str, port: i32| {
                    controller
                        .task_sender
                        .send(Box::new(move |task| task.manual_probe.start(host.into(), port)))
                        .unwrap();
                };
                probe("127.0.0.1", 0);
                assert!(
                    inspect(&controller, |task| task.manual_probe.state
                        == ui::ManualEndpointState::Unreachable)
                    .await
                );
                probe("127.0.0.1", closed_port().await.into());
                wait_condition(&controller, |task| {
                    task.manual_probe.state == ui::ManualEndpointState::Unreachable
                })
                .await;
                probe("127.0.0.1", port.into());
                wait_condition(&controller, |task| {
                    task.manual_probe.state == ui::ManualEndpointState::Reachable
                })
                .await;
                controller
                    .task_sender
                    .send(Box::new(|task| task.connect_manual_endpoint()))
                    .unwrap();
                let manual = EndpointIdentity::Manual { host: "127.0.0.1".into(), port };
                controller.start();
                accept_plaintext(&controller).await;
                let settings = stored_settings(&mut upstream).await;
                assert_eq!(settings.saved, [manual.clone()]);
                assert_eq!(settings.selected, manual);
                initial_request(&mut upstream).await;
                controller.close();
                viewer.event(|event| matches!(event, ConnectionMessage::Disconnected { .. })).await;
                probe("", 0);
                assert!(
                    inspect(&controller, |task| task.manual_probe.state
                        == ui::ManualEndpointState::Idle)
                    .await
                );
            })
            .await;
    }

    #[tokio::test]
    async fn cancelling_a_pairing_saves_nothing() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, mut upstream, discovered) = controller("echo_child");
                let mut viewer = Viewer::start(PairingPolicy::Generated, 0).await;
                discover(
                    &controller,
                    &discovered,
                    remote::tests::service("Cancelled", viewer.connection.local_port()),
                )
                .await;
                pair(&controller, 0);
                controller.start();
                wait_condition(&controller, |task| {
                    task.connection.state == ui::ConnectionState::PairingRequired
                })
                .await;
                controller.task_sender.send(Box::new(|task| task.cancel_pairing())).unwrap();
                viewer
                    .event(|event| {
                        matches!(event, ConnectionMessage::PairingFinished { accepted: false, .. })
                    })
                    .await;
                assert!(
                    inspect(&controller, |task| task.connection.state
                        == ui::ConnectionState::Failed
                        && task.selected_endpoint == remote("Cancelled")
                        && task.saved_endpoints.is_empty())
                    .await
                );
                assert!(upstream.try_recv().is_err());
                // Without a prompt there's nothing to cancel.
                controller.task_sender.send(Box::new(|task| task.cancel_pairing())).unwrap();
                assert!(
                    inspect(&controller, |task| task.connection.error == "Pairing cancelled").await
                );
                controller.close();
            })
            .await;
    }

    #[tokio::test]
    async fn restarting_discovery_drops_the_viewers_it_had() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (controller, _upstream, discovered) = controller("echo_child");
                discover(&controller, &discovered, remote::tests::service("Vanished", 1)).await;
                controller.task_sender.send(Box::new(|task| task.discovery.restart())).unwrap();
                assert!(inspect(&controller, |task| task.discovery.discovered.is_empty()).await);
                discover(&controller, &discovered, remote::tests::service("Answered", 1)).await;
                controller.close();
            })
            .await;
    }
}
