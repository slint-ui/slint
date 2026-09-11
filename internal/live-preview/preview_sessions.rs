// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use i_slint_core::InternalToken;
use i_slint_core::textlayout::sharedparley::fontique;
use i_slint_core::window::WindowInner;
use lsp_types::Url;
use slint_interpreter::ComponentHandle as _;
use tokio::sync::{mpsc, oneshot};

use crate::REBUILD_DEBOUNCE;
use crate::inspector::InspectorOverlay;
#[cfg(target_arch = "wasm32")]
use crate::protocol::wasm_prelude::*;
use crate::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, PreviewToLsp, PreviewToLspMessage,
    SourceFileVersion,
};

/// Small wrapper around tokio::spawn_local to silence the clippy warning that
/// you should just use `spawn_local`.
/// spawn_local is shadowed by `slint::spawn_local`, so make this explicit here.
#[allow(clippy::disallowed_methods)]
fn tokio_spawn_local<Future>(future: Future) -> tokio::task::JoinHandle<Future::Output>
where
    Future: std::future::Future + 'static,
    Future::Output: 'static,
{
    tokio::task::spawn_local(future)
}

#[derive(Clone, Debug)]
pub struct VersionedFileContent {
    pub version: SourceFileVersion,
    pub contents: Arc<[u8]>,
}

#[derive(Debug)]
enum CacheEntry {
    Loading(Vec<oneshot::Sender<std::io::Result<VersionedFileContent>>>),
    Ready(VersionedFileContent),
}

pub enum PreviewSessionEvent {
    SetUserSettings { name: String, contents: String },
    ShowPreview { component: PreviewComponent },
    ContentsChanged,
    HighlightFromEditor { url: Option<Url>, offset: u32 },
    RegisterFont { url: Url, contents: Arc<[u8]> },
}

pub enum PreviewCompilation {
    Ready(slint_interpreter::ComponentDefinition),
    CompilationError { message: String },
    ComponentNotFound,
    Unavailable,
}

enum PreviewSessionCommand {
    Message(LspToPreviewMessage),
    Reset(oneshot::Sender<()>),
}

pub struct PreviewSession {
    file_cache: RefCell<HashMap<Url, CacheEntry>>,
    dependencies: RefCell<HashSet<Url>>,
    compiler: RefCell<Option<slint_interpreter::Compiler>>,
    configuration: RefCell<Option<PreviewConfig>>,
    to_editor: Rc<dyn PreviewToLsp>,
}

#[derive(Clone)]
pub struct PreviewSessionHandle {
    command_sender: mpsc::UnboundedSender<PreviewSessionCommand>,
}

impl PreviewSession {
    pub fn start(
        to_editor: Rc<dyn PreviewToLsp>,
        event_handler: impl Fn(PreviewSessionEvent) + 'static,
    ) -> (Rc<Self>, PreviewSessionHandle) {
        let session = Rc::new(Self {
            file_cache: Default::default(),
            dependencies: Default::default(),
            compiler: Default::default(),
            configuration: Default::default(),
            to_editor,
        });
        session.compiler.replace(Some(session.create_compiler()));
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        tokio_spawn_local(session.clone().process_messages(command_receiver, event_handler));
        (session, PreviewSessionHandle { command_sender })
    }

    async fn process_messages(
        self: Rc<Self>,
        mut command_receiver: mpsc::UnboundedReceiver<PreviewSessionCommand>,
        event_handler: impl Fn(PreviewSessionEvent) + 'static,
    ) {
        let mut debounce_deadline = None;
        loop {
            let debounce = async {
                match debounce_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                biased;
                _ = debounce => {
                    debounce_deadline = None;
                    event_handler(PreviewSessionEvent::ContentsChanged);
                }
                command = command_receiver.recv() => {
                    let Some(command) = command else { break };
                    match command {
                        PreviewSessionCommand::Reset(reset_complete) => {
                            self.file_cache.borrow_mut().clear();
                            self.dependencies.borrow_mut().clear();
                            debounce_deadline = None;
                            reset_complete.send(()).ok();
                        }
                        PreviewSessionCommand::Message(message) => {
                            if !self.process_message(
                                message,
                                &event_handler,
                                &mut debounce_deadline,
                            ) {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn process_message(
        &self,
        message: LspToPreviewMessage,
        event_handler: &impl Fn(PreviewSessionEvent),
        debounce_deadline: &mut Option<tokio::time::Instant>,
    ) -> bool {
        match message {
            LspToPreviewMessage::InvalidateContents { url } => {
                if !is_supported(&url) {
                    return true;
                }
                self.file_cache.borrow_mut().remove(&url);
                self.schedule_rebuild(&url, debounce_deadline);
            }
            LspToPreviewMessage::ForgetFile { url } => {
                if !is_supported(&url) {
                    return true;
                }
                if let Some(CacheEntry::Loading(senders)) =
                    self.file_cache.borrow_mut().remove(&url)
                {
                    for sender in senders {
                        let _ = sender.send(Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "File not found",
                        )));
                    }
                }
                self.schedule_rebuild(&url, debounce_deadline);
            }
            LspToPreviewMessage::SetContents { url, contents } => {
                if !is_supported(url.url()) {
                    return true;
                }
                if i_slint_compiler::pathutils::is_font_file(url.url().path()) {
                    event_handler(PreviewSessionEvent::RegisterFont {
                        url: url.url().clone(),
                        contents: contents.into(),
                    });
                    return true;
                }
                let versioned_content =
                    VersionedFileContent { version: *url.version(), contents: contents.into() };
                let triggers_rebuild = self.dependencies.borrow().contains(url.url());
                match self.file_cache.borrow_mut().entry(url.url().clone()) {
                    std::collections::hash_map::Entry::Occupied(mut occupied) => {
                        if let CacheEntry::Loading(senders) = occupied.get_mut() {
                            for sender in senders.drain(..) {
                                let _ = sender.send(Ok(versioned_content.clone()));
                            }
                        }
                        occupied.insert(CacheEntry::Ready(versioned_content));
                    }
                    std::collections::hash_map::Entry::Vacant(vacant) => {
                        vacant.insert(CacheEntry::Ready(versioned_content));
                    }
                }
                if triggers_rebuild {
                    *debounce_deadline = Some(tokio::time::Instant::now() + REBUILD_DEBOUNCE);
                }
            }
            LspToPreviewMessage::SetConfiguration { config } => {
                self.set_configuration(config);
            }
            LspToPreviewMessage::SetUserSettings { name, contents } => {
                event_handler(PreviewSessionEvent::SetUserSettings { name, contents });
            }
            LspToPreviewMessage::ShowPreview(component) => {
                *debounce_deadline = None;
                event_handler(PreviewSessionEvent::ShowPreview { component });
            }
            LspToPreviewMessage::HighlightFromEditor { url, offset } => {
                event_handler(PreviewSessionEvent::HighlightFromEditor { url, offset });
            }
            LspToPreviewMessage::Quit => return false,
            LspToPreviewMessage::Ping => {
                self.send_to_editor(&PreviewToLspMessage::Pong).ok();
            }
            LspToPreviewMessage::RemoteConnectionState { .. } => {
                tracing::warn!("Ignoring unexpected RemoteConnectionState over WebSocket");
            }
            LspToPreviewMessage::OpenProject { .. } => {}
            LspToPreviewMessage::PairingHello { .. }
            | LspToPreviewMessage::PairingResponse { .. } => {
                tracing::warn!("Ignoring pairing message on an established session");
            }
        }
        true
    }

    fn schedule_rebuild(&self, url: &Url, debounce_deadline: &mut Option<tokio::time::Instant>) {
        if self.dependencies.borrow().contains(url) {
            *debounce_deadline = Some(tokio::time::Instant::now() + REBUILD_DEBOUNCE);
        }
    }

    async fn request_file(&self, url: Url) -> std::io::Result<VersionedFileContent> {
        if let Some(CacheEntry::Ready(entry)) = self.file_cache.borrow().get(&url) {
            return Ok(entry.clone());
        }
        let (sender, receiver) = oneshot::channel();
        let request_file;
        match self.file_cache.borrow_mut().entry(url.clone()) {
            std::collections::hash_map::Entry::Occupied(mut occupied) => match occupied.get_mut() {
                CacheEntry::Ready(entry) => return Ok(entry.clone()),
                CacheEntry::Loading(senders) => {
                    senders.push(sender);
                    request_file = false;
                }
            },
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(CacheEntry::Loading(vec![sender]));
                request_file = true;
            }
        }
        if request_file
            && let Err(error) = self.send_to_editor(&PreviewToLspMessage::RequestState {
                files: vec![url.clone()],
                settings: Vec::new(),
            })
        {
            self.file_cache.borrow_mut().remove(&url);
            return Err(std::io::Error::other(error.to_string()));
        }
        receiver.await.map_err(std::io::Error::other)?
    }

    fn create_compiler(self: &Rc<Self>) -> slint_interpreter::Compiler {
        let mut compiler = slint_interpreter::Compiler::new();

        let file_loader_session = Rc::downgrade(self);
        compiler.set_file_loader(move |path: &std::path::Path| {
            let url = Url::from_file_path(path);
            let path_display = path.display().to_string();
            let session = file_loader_session.clone();
            Box::pin(async move {
                let Some(session) = session.upgrade() else {
                    return Some(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Preview session is no longer available",
                    )));
                };
                let Ok(url) = url else {
                    return Some(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Not an absolute file path: {path_display}"),
                    )));
                };
                Some(session.request_file(url).await.map(|file_content| {
                    String::from_utf8_lossy(&file_content.contents).to_string()
                }))
            })
        });

        let mapper_session = Rc::downgrade(self);
        compiler.compiler_configuration(InternalToken).resource_url_mapper =
            Some(Rc::new(move |url: &Url| {
                let session = mapper_session.clone();
                let url = url.clone();
                Box::pin(async move {
                    if url.scheme() != "file" {
                        return None;
                    }
                    let session = session.upgrade()?;
                    let extension = std::path::Path::new(url.path())
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .unwrap_or("png");
                    let mime_type =
                        i_slint_core::graphics::image_mime_type_from_extension(extension)
                            .unwrap_or("application/octet-stream");
                    let file_content = session.request_file(url).await.ok()?;

                    use base64::Engine as _;
                    let encoded =
                        base64::engine::general_purpose::STANDARD.encode(&*file_content.contents);
                    Url::parse(&format!("data:{mime_type};base64,{encoded}")).ok()
                })
            }));

        compiler
    }

    fn set_configuration(&self, configuration: PreviewConfig) {
        if let Some(compiler) = self.compiler.borrow_mut().as_mut() {
            apply_configuration(compiler, &configuration);
        }
        self.configuration.replace(Some(configuration));
    }

    pub async fn compile_component(&self, component: &PreviewComponent) -> PreviewCompilation {
        let Ok(path) = component.url.to_file_path() else {
            tracing::error!("Not a file URL: {}", component.url);
            return PreviewCompilation::Unavailable;
        };
        let file = match self.request_file(component.url.clone()).await {
            Ok(file) => file,
            Err(error) => {
                tracing::error!("Failed fetching {}: {error}", component.url);
                return PreviewCompilation::Unavailable;
            }
        };
        let Some(compiler) = self.compiler.borrow_mut().take() else {
            tracing::error!("Preview session is already compiling a component");
            return PreviewCompilation::Unavailable;
        };
        let compilation_result = compiler
            .build_from_source(String::from_utf8_lossy(&file.contents).into_owned(), path)
            .await;
        self.restore_compiler(compiler);
        *self.dependencies.borrow_mut() = compilation_result
            .watch_paths(InternalToken)
            .iter()
            .filter_map(|path| Url::from_file_path(path).ok())
            .collect();

        if compilation_result.has_errors() {
            self.send_diagnostics(&compilation_result, &component.url);
            let message = compilation_result
                .diagnostics()
                .inspect(|diagnostic| tracing::warn!("Compiler diagnostic: {diagnostic}"))
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            return PreviewCompilation::CompilationError { message };
        }

        let Some(component_definition) = component
            .component
            .as_deref()
            .or_else(|| compilation_result.component_names().next())
            .and_then(|name| compilation_result.component(name))
        else {
            tracing::error!("Component not found");
            return PreviewCompilation::ComponentNotFound;
        };

        self.send_diagnostics(&compilation_result, &component.url);
        PreviewCompilation::Ready(component_definition)
    }

    fn restore_compiler(&self, mut compiler: slint_interpreter::Compiler) {
        if let Some(configuration) = self.configuration.borrow().as_ref() {
            apply_configuration(&mut compiler, configuration);
        }
        self.compiler.replace(Some(compiler));
    }

    pub fn send_to_editor(&self, message: &PreviewToLspMessage) -> crate::protocol::Result<()> {
        self.to_editor.send(message)
    }

    fn send_diagnostics(
        &self,
        compilation_result: &slint_interpreter::CompilationResult,
        uri: &Url,
    ) {
        let message = PreviewToLspMessage::Diagnostics {
            uri: uri.clone(),
            version: None,
            diagnostics: compilation_result
                .diagnostics()
                .map(|diagnostic| {
                    crate::protocol::to_lsp_diagnostic(
                        &diagnostic,
                        i_slint_compiler::diagnostics::ByteFormat::Utf8,
                    )
                })
                .collect(),
        };
        self.send_to_editor(&message).ok();
    }
}

fn apply_configuration(compiler: &mut slint_interpreter::Compiler, configuration: &PreviewConfig) {
    compiler.set_style(configuration.style.clone());
    compiler.compiler_configuration(InternalToken).enable_experimental =
        configuration.enable_experimental;
}

impl PreviewSessionHandle {
    pub fn handle_message(&self, message: LspToPreviewMessage) -> crate::protocol::Result<()> {
        self.command_sender.send(PreviewSessionCommand::Message(message))?;
        Ok(())
    }

    pub async fn reset(&self) -> crate::protocol::Result<()> {
        let (reset_complete, reset_completed) = oneshot::channel();
        self.command_sender.send(PreviewSessionCommand::Reset(reset_complete))?;
        reset_completed.await?;
        Ok(())
    }
}

fn is_supported(url: &Url) -> bool {
    if url.scheme() != "file" {
        tracing::warn!("Ignoring message for unsupported URL scheme: {url}");
        return false;
    }
    true
}

pub async fn run_with_channels(
    mut from_editor: mpsc::UnboundedReceiver<LspToPreviewMessage>,
    to_editor: Rc<dyn PreviewToLsp>,
) -> anyhow::Result<()> {
    let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
    let (preview_session, session_handle) =
        PreviewSession::start(to_editor.clone(), move |event| {
            event_sender.send(event).ok();
        });
    preview_session
        .send_to_editor(&PreviewToLspMessage::RequestState {
            files: Vec::new(),
            settings: Vec::new(),
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    tokio_spawn_local(async move {
        while let Some(message) = from_editor.recv().await {
            let should_quit = matches!(message, LspToPreviewMessage::Quit);
            if session_handle.handle_message(message).is_err() || should_quit {
                break;
            }
        }
    });

    install_debug_handler(Rc::downgrade(&to_editor))?;

    let mut current_component = None;
    let mut current_highlight = None;
    let mut component_instance = None;
    let mut inspector = None;
    let mut registered_fonts = HashSet::new();
    let mut pending_fonts = Vec::new();

    while let Some(event) = event_receiver.recv().await {
        match event {
            PreviewSessionEvent::SetUserSettings { .. } => {}
            PreviewSessionEvent::ShowPreview { component } => {
                show_component(
                    &preview_session,
                    &component,
                    &mut component_instance,
                    &mut inspector,
                    current_highlight.as_ref(),
                    &mut pending_fonts,
                )
                .await?;
                current_component = Some(component);
            }
            PreviewSessionEvent::ContentsChanged => {
                let Some(component) = current_component.as_ref() else { continue };
                show_component(
                    &preview_session,
                    component,
                    &mut component_instance,
                    &mut inspector,
                    current_highlight.as_ref(),
                    &mut pending_fonts,
                )
                .await?;
            }
            PreviewSessionEvent::HighlightFromEditor { url, offset } => {
                current_highlight = url.map(|url| (url, offset));
                if let Some(inspector) = inspector.as_ref() {
                    inspector.update(component_instance.as_ref(), current_highlight.as_ref());
                }
            }
            PreviewSessionEvent::RegisterFont { url, contents } => {
                if !registered_fonts.insert(url.clone()) {
                    tracing::debug!("Font {url} already registered, skipping");
                    continue;
                }
                if let Some(component_instance) = component_instance.as_ref() {
                    register_font(component_instance.window(), contents);
                } else {
                    pending_fonts.push(contents);
                }
            }
        }
    }

    Ok(())
}

async fn show_component(
    preview_session: &PreviewSession,
    component: &PreviewComponent,
    component_instance: &mut Option<slint_interpreter::ComponentInstance>,
    inspector: &mut Option<InspectorOverlay>,
    current_highlight: Option<&(Url, u32)>,
    pending_fonts: &mut Vec<Arc<[u8]>>,
) -> anyhow::Result<()> {
    let PreviewCompilation::Ready(component_definition) =
        preview_session.compile_component(component).await
    else {
        return Ok(());
    };

    let new_instance = if let Some(component_instance) = component_instance.as_ref() {
        component_definition.create_with_existing_window(component_instance.window())?
    } else {
        component_definition.create()?
    };
    for contents in pending_fonts.drain(..) {
        register_font(new_instance.window(), contents);
    }
    if inspector.is_none() {
        *inspector = Some(InspectorOverlay::new(new_instance.window()).await?);
    }
    new_instance.show()?;
    *component_instance = Some(new_instance);
    let inspector = inspector.as_ref().unwrap();
    inspector.attach()?;
    inspector.update(component_instance.as_ref(), current_highlight);
    Ok(())
}

fn register_font(window: &i_slint_core::api::Window, contents: Arc<[u8]>) {
    let blob = fontique::Blob::new(Arc::new(contents));
    WindowInner::from_pub(window)
        .context()
        .font_context()
        .borrow_mut()
        .collection
        .register_fonts(blob, None);
}

fn install_debug_handler(
    to_editor: std::rc::Weak<dyn PreviewToLsp>,
) -> Result<(), slint_interpreter::PlatformError> {
    i_slint_backend_selector::with_global_context(|context| {
        context.set_log_message_handler(Some(Box::new(move |message| {
            let Some(to_editor) = to_editor.upgrade() else { return };
            let location = message
                .location()
                .map(|location| (PathBuf::from(location.path), location.line, location.column));
            to_editor
                .send(&PreviewToLspMessage::DebugMessage {
                    location,
                    message: message.message_arguments().to_string(),
                })
                .ok();
        })))
    })?;
    Ok(())
}
