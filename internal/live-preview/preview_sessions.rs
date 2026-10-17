// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::source_path::SourcePath;
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
    Ready(CompiledPreview),
    CompilationError { message: String },
    ComponentNotFound,
    Unavailable,
}

/// A successful compilation, in the form that can leave the thread it was compiled on.
///
/// Call [`Self::component_definition()`] on the thread that instantiates the component.
pub struct CompiledPreview {
    compilation: Box<slint_interpreter::SendableCompilationResult>,
    component_name: String,
}

impl CompiledPreview {
    /// `None` only if the component is gone from the result, which the compiling side
    /// already ruled out: it reports [`PreviewCompilation::ComponentNotFound`] instead.
    pub fn component_definition(self) -> Option<slint_interpreter::ComponentDefinition> {
        slint_interpreter::CompilationResult::from(*self.compilation)
            .component(&self.component_name)
    }
}

enum PreviewSessionCommand {
    Message(Box<LspToPreviewMessage>),
    Reset(oneshot::Sender<()>),
}

/// The files and the compiler behind one preview, fed by an editor over any transport.
///
/// Lives on the thread that runs the preview: use [`PreviewSessionHandle`] to reach it from
/// the thread that owns the transport.
pub struct PreviewSession {
    /// File contents pushed by the editor, keyed by the `Url` the editor sent. Using the URL
    /// verbatim avoids platform-dependent path normalization (Windows backslashes,
    /// percent-encoding) — equality is structural.
    file_cache: RefCell<HashMap<Url, CacheEntry>>,
    /// Files the currently shown component depends on. `SetContents` for URLs outside this set
    /// does not rebuild, so unrelated edits in the user's editor don't disturb the preview.
    dependencies: RefCell<HashSet<Url>>,
    /// Taken for the duration of a compilation, which is how a second one is refused.
    compiler: RefCell<Option<slint_interpreter::Compiler>>,
    configuration: RefCell<Option<PreviewConfig>>,
    to_editor: Rc<dyn PreviewToLsp>,
}

/// Feeds messages to a [`PreviewSession`] from any thread.
#[derive(Clone)]
pub struct PreviewSessionHandle {
    command_sender: mpsc::UnboundedSender<PreviewSessionCommand>,
}

/// The receiving end of a [`PreviewSessionHandle`], which
/// [`PreviewSession::start_with()`] turns into a running session.
///
/// It exists so that the handle can be made before the session: the transport needs a
/// handle to feed, while the session needs the transport to answer file requests. Both
/// are `Send`, so the session can run on a thread of its own.
pub struct PreviewSessionCommands(mpsc::UnboundedReceiver<PreviewSessionCommand>);

impl PreviewSession {
    pub fn start(
        to_editor: Rc<dyn PreviewToLsp>,
        event_handler: impl Fn(PreviewSessionEvent) + 'static,
    ) -> (Rc<Self>, PreviewSessionHandle) {
        let (handle, commands) = PreviewSessionHandle::new();
        (Self::start_with(commands, to_editor, event_handler), handle)
    }

    /// Like [`Self::start()`], but for a handle that was made in advance, possibly on
    /// another thread. The session runs on the thread that calls this.
    pub fn start_with(
        commands: PreviewSessionCommands,
        to_editor: Rc<dyn PreviewToLsp>,
        event_handler: impl Fn(PreviewSessionEvent) + 'static,
    ) -> Rc<Self> {
        let session = Rc::new(Self {
            file_cache: Default::default(),
            dependencies: Default::default(),
            compiler: Default::default(),
            configuration: Default::default(),
            to_editor,
        });
        session.compiler.replace(Some(session.create_compiler()));
        tokio_spawn_local(session.clone().process_messages(commands.0, event_handler));
        session
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
                                *message,
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
                let removed = self.file_cache.borrow_mut().remove(&url);
                if let Some(CacheEntry::Loading(senders)) = removed {
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
                if i_slint_compiler::fileaccess::is_font_file(url.url().path()) {
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
                if self.configuration.borrow().as_ref() != Some(&config) {
                    self.set_configuration(config);
                    *debounce_deadline = Some(tokio::time::Instant::now() + REBUILD_DEBOUNCE);
                }
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

    fn manifest_contents(&self, path: &SourcePath) -> std::io::Result<String> {
        let url = path.to_url().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Not an absolute file path: {path}"),
            )
        })?;
        self.dependencies.borrow_mut().insert(url.clone());
        match self.file_cache.borrow().get(&url) {
            Some(CacheEntry::Ready(file_content)) => {
                Ok(String::from_utf8_lossy(&file_content.contents).to_string())
            }
            _ => Err(std::io::ErrorKind::NotFound.into()),
        }
    }

    fn create_compiler(self: &Rc<Self>) -> slint_interpreter::Compiler {
        let mut compiler = slint_interpreter::Compiler::new();
        let manifest_session = Rc::downgrade(self);
        compiler.set_project_file_loader(
            Rc::new(move |path| {
                let result = manifest_session
                    .upgrade()
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::BrokenPipe))
                    .and_then(|session| session.manifest_contents(&path));
                Box::pin(async move { Some(result) })
            }),
            InternalToken,
        );

        let file_loader_session = Rc::downgrade(self);
        compiler.compiler_configuration(InternalToken).open_import_callback =
            Some(Rc::new(move |path: SourcePath| {
                let url = path.to_url();
                let session = file_loader_session.clone();
                Box::pin(async move {
                    let Some(session) = session.upgrade() else {
                        return Some(Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Preview session is no longer available",
                        )));
                    };
                    let Some(url) = url else {
                        return Some(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("Not an absolute file path: {path}"),
                        )));
                    };
                    Some(session.request_file(url).await.map(|file_content| {
                        String::from_utf8_lossy(&file_content.contents).to_string()
                    }))
                })
            }));

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
        let path = SourcePath::from_url(&component.url);
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
        // A dropped compilation must not take the compiler with it: a session without one
        // refuses every build that follows.
        let compiler = scopeguard::guard(compiler, |compiler| self.restore_compiler(compiler));
        let source_code = String::from_utf8_lossy(&file.contents).into_owned();
        let resolved_source = crate::protocol::preview_source_path(&path, &source_code).await;
        let diagnostic_url = resolved_source
            .as_ref()
            .and_then(SourcePath::to_url)
            .unwrap_or_else(|| component.url.clone());
        let compilation_result =
            compiler.build_from_source_path(source_code, path, InternalToken).await;
        drop(compiler);
        // Set even on errors so edits to imported files still trigger a rebuild.
        *self.dependencies.borrow_mut() = compilation_result
            .watch_paths(InternalToken)
            .iter()
            .filter_map(SourcePath::to_url)
            .collect();

        if compilation_result.has_errors() {
            self.send_diagnostics(&compilation_result, &diagnostic_url);
            let message = compilation_result
                .diagnostics()
                .inspect(|diagnostic| tracing::warn!("Compiler diagnostic: {diagnostic}"))
                .map(|diagnostic| diagnostic.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            return PreviewCompilation::CompilationError { message };
        }

        let Some(component_name) = component
            .component
            .as_deref()
            .or_else(|| compilation_result.component_names().next())
            .filter(|name| compilation_result.component_names().any(|known| known == *name))
            .map(String::from)
        else {
            // No compile errors but no component: skip the diagnostics so they don't clobber
            // unrelated ones the editor holds for this URL.
            tracing::error!("Component not found");
            return PreviewCompilation::ComponentNotFound;
        };

        self.send_diagnostics(&compilation_result, &diagnostic_url);
        PreviewCompilation::Ready(CompiledPreview {
            compilation: Box::new(compilation_result.into_send()),
            component_name,
        })
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
    compiler.set_overrides(configuration.overrides.clone(), InternalToken);
}

impl PreviewSessionHandle {
    /// Makes a handle and the [`PreviewSessionCommands`] that
    /// [`PreviewSession::start_with()`] consumes.
    pub fn new() -> (Self, PreviewSessionCommands) {
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        (Self { command_sender }, PreviewSessionCommands(command_receiver))
    }

    pub fn handle_message(&self, message: LspToPreviewMessage) -> crate::protocol::Result<()> {
        self.command_sender.send(PreviewSessionCommand::Message(Box::new(message)))?;
        Ok(())
    }

    pub async fn reset(&self) -> crate::protocol::Result<()> {
        let (reset_complete, reset_completed) = oneshot::channel();
        self.command_sender.send(PreviewSessionCommand::Reset(reset_complete))?;
        reset_completed.await?;
        Ok(())
    }
}

/// Whether the session can act on this URL. A preview only handles `file://` URLs; the editor
/// can legitimately produce others (e.g. `vscode-remote://`), but they're ignored on this side.
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
    let PreviewCompilation::Ready(compiled) = preview_session.compile_component(component).await
    else {
        return Ok(());
    };
    let Some(component_definition) = compiled.component_definition() else { return Ok(()) };

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

pub fn register_font(window: &i_slint_core::api::Window, contents: Arc<[u8]>) {
    // Wrap the already-Arc-backed bytes in a Blob without copying.
    let blob = fontique::Blob::new(Arc::new(contents));
    WindowInner::from_pub(window).context().font_context().borrow_mut().register_fonts(blob);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::VersionedUrl;

    #[derive(Default)]
    struct CaptureToEditor(RefCell<Vec<PreviewToLspMessage>>);

    impl PreviewToLsp for CaptureToEditor {
        fn send(&self, message: &PreviewToLspMessage) -> crate::protocol::Result<()> {
            self.0.borrow_mut().push(message.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_manifest_target_keeps_its_entry_separate_and_reports_a_missing_entry() {
        i_slint_backend_testing::init_no_event_loop();
        tokio::task::LocalSet::new()
            .run_until(async {
                let capture = Rc::new(CaptureToEditor::default());
                let session = Rc::new(PreviewSession {
                    file_cache: Default::default(),
                    dependencies: Default::default(),
                    compiler: Default::default(),
                    configuration: Default::default(),
                    to_editor: capture.clone(),
                });
                session.restore_compiler(session.create_compiler());
                let directory = tempfile::tempdir().unwrap();
                let manifest =
                    Url::from_file_path(directory.path().join("slint-project.json")).unwrap();
                let entry = Url::from_file_path(directory.path().join("main.slint")).unwrap();
                for (url, source) in [
                    (manifest.clone(), r#"{"slint-edition":"1.19","main":"main.slint"}"#),
                    (
                        entry.clone(),
                        "export component Main inherits Window { out property <int> marker: 91; }",
                    ),
                ] {
                    session.process_message(
                        LspToPreviewMessage::SetContents {
                            url: VersionedUrl::new(url, None),
                            contents: source.into(),
                        },
                        &|_| {},
                        &mut None,
                    );
                }
                let target = PreviewComponent { url: manifest.clone(), component: None };
                let PreviewCompilation::Ready(compiled) = session.compile_component(&target).await
                else {
                    panic!("manifest entry did not compile")
                };
                assert!(compiled.component_definition().is_some());
                assert_eq!(target.url, manifest);
                let mut instance = None;
                let mut inspector = None;
                show_component(
                    &session,
                    &target,
                    &mut instance,
                    &mut inspector,
                    None,
                    &mut Vec::new(),
                )
                .await
                .unwrap();
                assert!(instance.is_some());
            session.process_message(LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(target.url.join("main.slint").unwrap(), None),
                contents: "export component Broken inherits Missing {}".into(),
            }, &|_| {}, &mut None);
            assert!(matches!(session.compile_component(&target).await, PreviewCompilation::CompilationError { .. }));
            assert!(capture.0.borrow().iter().any(|message| matches!(message, PreviewToLspMessage::Diagnostics { uri, diagnostics, .. } if uri.path().ends_with("/main.slint") && !diagnostics.is_empty())));

                session.process_message(
                    LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(manifest, None),
                        contents: r#"{"slint-edition":"1.19"}"#.into(),
                    },
                    &|_| {},
                    &mut None,
                );
                assert!(matches!(
                    session.compile_component(&target).await,
                    PreviewCompilation::CompilationError { .. }
                ));
                show_component(
                    &session,
                    &target,
                    &mut instance,
                    &mut inspector,
                    None,
                    &mut Vec::new(),
                )
                .await
                .unwrap();
                assert!(instance.is_some());

                assert!(inspector.is_some());
                assert!(capture.0.borrow().iter().any(|message| matches!(message, PreviewToLspMessage::Diagnostics { uri, diagnostics, .. } if uri == &target.url && diagnostics.iter().any(|diagnostic| diagnostic.message.contains("has no 'main' to compile")))));
                assert!(
                    !capture
                        .0
                        .borrow()
                        .iter()
                        .any(|message| matches!(message, PreviewToLspMessage::RequestState { .. }))
                );
            })
            .await;
    }

    #[test]
    fn manifest_loader_uses_only_cache_and_reloads_on_updates() {
        let capture = Rc::new(CaptureToEditor::default());
        let session = Rc::new(PreviewSession {
            file_cache: Default::default(),
            dependencies: Default::default(),
            compiler: Default::default(),
            configuration: Default::default(),
            to_editor: capture.clone(),
        });
        let root = tempfile::tempdir().unwrap();
        let manifest_path = root.path().join(i_slint_compiler::project_file::FILE_NAME);
        std::fs::write(&manifest_path, r#"{"style":"disk"}"#).unwrap();
        let manifest = SourcePath::new(&manifest_path);
        let url = manifest.to_url().unwrap();
        let error = session.manifest_contents(&manifest).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(session.dependencies.borrow().contains(&url));
        assert!(capture.0.borrow().is_empty());
        for (contents, rebuild) in [
            (Some(r#"{"slint-edition":"1.19"}"#), true),
            (Some(r#"{"slint-edition":"1.19"}"#), true),
            (Some("{broken"), true),
            (None, true),
            (None, true),
        ] {
            let message = if let Some(contents) = contents {
                LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), None),
                    contents: contents.into(),
                }
            } else {
                LspToPreviewMessage::ForgetFile { url: url.clone() }
            };
            let mut deadline = None;
            assert!(session.process_message(message, &|_| {}, &mut deadline));
            assert_eq!(deadline.is_some(), rebuild);
            let loaded = session.manifest_contents(&manifest);
            if let Some(contents) = contents {
                assert_eq!(loaded.unwrap(), contents);
            } else {
                assert_eq!(loaded.unwrap_err().kind(), std::io::ErrorKind::NotFound);
            }
        }
        let unrelated = Url::from_file_path(root.path().join("unrelated.slint")).unwrap();
        let mut deadline = None;
        session.process_message(
            LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(unrelated, None),
                contents: b"export component Other {}".to_vec(),
            },
            &|_| {},
            &mut deadline,
        );
        assert!(deadline.is_none());
        assert!(capture.0.borrow().is_empty());
    }
    #[tokio::test]
    async fn empty_style_keeps_the_default_widget_style() {
        let mut compiler = slint_interpreter::Compiler::default();
        apply_configuration(&mut compiler, &PreviewConfig::default());
        let result = compiler
            .build_from_source(
                r#"
                import { Button } from "std-widgets.slint";
                export component App inherits Window { Button { text: "ok"; } }
                "#
                .into(),
                PathBuf::from("app.slint"),
            )
            .await;
        let diagnostics: Vec<_> = result.diagnostics().map(|d| d.to_string()).collect();
        assert!(!result.has_errors(), "{diagnostics:?}");
        assert!(result.component("App").is_some());
    }

    #[tokio::test]
    async fn preview_configuration_accepts_default_and_explicit_styles() {
        i_slint_backend_testing::init_no_event_loop();
        let mut compiler = slint_interpreter::Compiler::new();
        let source = r#"
            import { Button } from "std-widgets.slint";
            export component App inherits Window {
                Button { text: "Run"; }
            }
        "#;

        for (style, should_compile) in
            [("fluent", true), ("", true), ("unknown-preview-style", false), ("", true)]
        {
            apply_configuration(
                &mut compiler,
                &PreviewConfig {
                    overrides: i_slint_compiler::project_file::Overrides {
                        style: (!style.is_empty()).then(|| style.into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            let result = compiler.build_from_source(source.into(), "/preview.slint".into()).await;
            assert_eq!(
                result.component("App").is_some(),
                should_compile,
                "style {style:?}: {:?}",
                result.diagnostics().collect::<Vec<_>>()
            );
        }
    }
}
