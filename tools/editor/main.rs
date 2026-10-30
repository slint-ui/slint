// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use i_slint_compiler::source_path::SourcePath;
use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::{LspToPreviews, Result};
use i_slint_live_preview::file_watcher::{FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewTarget, PreviewToLspMessage,
};
use lsp_types::{MessageType, Url};
use slint::ComponentHandle;

#[cfg(not(target_arch = "wasm32"))]
mod file_dialog;
#[cfg(target_os = "linux")]
mod flatpak;
#[cfg(target_os = "macos")]
mod open_documents;
mod preview;
mod preview_window;
#[cfg(target_os = "macos")]
mod sparkle;
mod startup;
#[cfg(target_os = "windows")]
mod windows;

use preview::settings::{Project, TOOL_NAME};

const PRIMARY_PREVIEW_INDEX: usize = 0;
const RUN_PREVIEW_INDEX: usize = 1;

enum EditorToSessionMessage {
    RunPreview,
    SwitchProject(Project),
    CloseProject,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    if cli.run_preview_child {
        return preview_window::run(cli.preview_window_placement);
    }

    let headless = headless_requested();
    // Set up the Slint backend (installing the macOS unified-title-bar hook)
    select_backend(headless)?;

    let editor_ui = preview::ui::create_ui()?;
    editor_ui.on_quit_requested(|| {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = slint::quit_event_loop();
    });

    // The updater needs to stay in scope for as long as the window is up.
    #[cfg(target_os = "macos")]
    let _updater = setup_macos_chrome(&editor_ui);
    #[cfg(target_os = "linux")]
    let _updater = flatpak::connect(&editor_ui);
    #[cfg(target_os = "windows")]
    let _updater = windows::connect(&editor_ui);

    let settings = startup::load_settings();
    let active_session =
        Rc::new(RefCell::new(None::<crossbeam_channel::Sender<EditorToSessionMessage>>));
    let close_session = active_session.clone();
    let close_project = Rc::new(move || {
        if let Some(session_sender) = close_session.borrow().as_ref() {
            session_sender.send(EditorToSessionMessage::CloseProject).ok();
        }
    });
    let editor_ui_weak = editor_ui.as_weak();
    let start_project = Rc::new(move |project| {
        let mut session_slot = active_session.borrow_mut();
        if let Some(session_sender) = session_slot.as_ref() {
            return session_sender.send(EditorToSessionMessage::SwitchProject(project)).is_ok();
        }
        let Some(editor_ui) = editor_ui_weak.upgrade() else {
            return false;
        };
        *session_slot = Some(start_editor_session(&editor_ui, project, startup::load_settings()));
        true
    });
    startup::setup(&editor_ui, &settings, start_project.clone(), close_project);
    // Installing the handler connects to AppKit, which headless runs such as UI tests don't use.
    #[cfg(target_os = "macos")]
    if !headless {
        let start_project = start_project.clone();
        open_documents::install(move |file| {
            if preview::preview_in_open_project(&file) {
                return;
            }
            match Project::from_selected_file(&file, None) {
                Ok(project) => {
                    start_project(project);
                }
                Err(err) => tracing::warn!("Failed to open {}: {err}", file.display()),
            }
        });
    }
    if let Some(file) = cli.file {
        let project = Project::from_input(file, cli.component, cli.project_root.as_deref())?;
        start_project(project);
    }

    editor_ui.run()?;
    Ok(())
}

/// Set up the editor's macOS chrome: the unified title bar and the Sparkle
/// auto-updater driving the update section of the editor UI.
#[cfg(target_os = "macos")]
fn setup_macos_chrome(editor_ui: &preview::ui::EditorUi) -> Option<Rc<crate::sparkle::Sparkle>> {
    use slint::ComponentHandle;

    preview::macos_titlebar::setup(editor_ui.as_weak());
    crate::sparkle::connect(editor_ui)
}

/// Hands messages for the preview straight to the UI thread: the editor runs
/// the preview in-process, so there is nothing to serialize.
struct EditorLspToPreview;

impl editor_preview::LspToPreview for EditorLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        let message = message.clone();
        if let Err(err) = slint::invoke_from_event_loop(move || {
            preview::lsp_to_preview(message);
        }) {
            tracing::error!("Failed to queue message onto the event loop: {err}");
        }
    }

    // The variant `EmbeddedLspToPreview` used to report: despite its name it is not
    // wasm-specific, and here it only keys the single entry in `LspToPreviews`.
    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::EmbeddedWasm
    }
}

struct EmbeddedPreviewToLsp {
    sender: crossbeam_channel::Sender<PreviewToLspMessage>,
}

impl editor_preview::PreviewToLsp for EmbeddedPreviewToLsp {
    fn send(&self, message: &PreviewToLspMessage) -> editor_preview::Result<()> {
        self.sender.send(message.clone())?;
        Ok(())
    }
}

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, hide = true)]
    run_preview_child: bool,
    #[arg(long, hide = true, requires = "run_preview_child", value_parser = |value: &str| serde_json::from_str::<preview_window::Placement>(value))]
    preview_window_placement: Option<preview_window::Placement>,
    file: Option<String>,
    component: Option<String>,
    #[arg(short = 'p', long, requires = "file", help = "Use this directory as the project root")]
    project_root: Option<PathBuf>,
}

fn headless_requested() -> bool {
    std::env::var("SLINT_BACKEND").is_ok_and(|backend| {
        i_slint_backend_selector::parse_backend_env_var(&backend.to_ascii_lowercase()).0
            == "headless"
    })
}

fn select_backend(headless: bool) -> std::result::Result<(), slint::PlatformError> {
    if headless {
        return i_slint_backend_selector::with_platform(|_| Ok(()));
    }

    // See bug #10274 on macOS.
    let selector = slint::BackendSelector::new();
    // On macOS, request a unified title bar: the editor content extends underneath
    // a transparent title bar (see `preview::macos_titlebar`).
    #[cfg(target_os = "macos")]
    let selector =
        selector.with_winit_window_attributes_hook(preview::macos_titlebar::apply_unified_titlebar);
    selector.select()
}

fn start_editor_session(
    editor_ui: &preview::ui::EditorUi,
    project: Project,
    settings: preview::settings::VisualEditorSettings,
) -> crossbeam_channel::Sender<EditorToSessionMessage> {
    let (to_lsp, from_preview) = crossbeam_channel::unbounded();
    let (to_editor_session, from_editor_preview) = crossbeam_channel::unbounded();
    let preview_global = editor_ui.global::<preview::ui::Preview>();
    let run_preview_sender = to_editor_session.clone();
    preview_global.on_run(move || {
        run_preview_sender.send(EditorToSessionMessage::RunPreview).ok();
    });
    let springboard_global = editor_ui.global::<editor_preview::springboard_ui::Springboard>();
    let springboard_global = <editor_preview::springboard_ui::Springboard as slint::Global<
        '_,
        preview::ui::EditorUi,
    >>::as_weak(&springboard_global);
    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;
    preview::ui::initialize_editor(editor_ui, &to_lsp, "");
    preview::initialize(editor_ui, to_lsp, settings);
    start_lsp_thread(
        vec![from_preview],
        from_editor_preview,
        project,
        springboard_global,
        preview_window::arguments_provider(editor_ui.as_weak()),
    );
    to_editor_session
}

fn start_lsp_thread(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    from_editor_preview: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    springboard_global: slint::Weak<editor_preview::springboard_ui::Springboard<'static>>,
    prepare_arguments: editor_preview::springboard::LocalPreviewArguments,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set.block_on(
            &rt,
            lsp_main(
                from_previews,
                from_editor_preview,
                project,
                springboard_global,
                prepare_arguments,
            ),
        ) {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
}

fn bridge_crossbeam_to_tokio(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
) -> Vec<tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage>> {
    from_previews.into_iter().map(bridge_crossbeam_receiver).collect()
}

fn bridge_crossbeam_receiver<Message: Send + 'static>(
    receiver: crossbeam_channel::Receiver<Message>,
) -> tokio::sync::mpsc::UnboundedReceiver<Message> {
    let (sender, tokio_receiver) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(message) = receiver.recv() {
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    tokio_receiver
}

async fn receive_preview_message(
    from_previews: &mut [tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage>],
) -> (usize, Option<PreviewToLspMessage>) {
    let receives = from_previews.iter_mut().map(|from_preview| Box::pin(from_preview.recv()));
    let (message, preview_index, _) = futures_util::future::select_all(receives).await;
    (preview_index, message)
}

async fn lsp_main(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    from_editor: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    springboard_global: slint::Weak<editor_preview::springboard_ui::Springboard<'static>>,
    prepare_arguments: editor_preview::springboard::LocalPreviewArguments,
) -> Result<()> {
    let mut from_previews = bridge_crossbeam_to_tokio(from_previews);
    let mut from_editor = bridge_crossbeam_receiver(from_editor);
    let (from_run_preview_sender, from_run_preview) = tokio::sync::mpsc::unbounded_channel();
    from_previews.push(from_run_preview);
    let (file_watcher_tx, mut file_watcher_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut file_watcher = FileWatcher::start(
        move |event| {
            if file_watcher_tx.send(event).is_err() {
                tracing::debug!("Ignoring file watcher event after editor shutdown");
            }
        },
        move |err| tracing::warn!("File watcher error: {err}"),
    )?;

    let local_preview = editor_preview::springboard::LocalPreviewConfig {
        executable: std::env::current_exe()?,
        arguments: preview_window::arguments(None),
        prepare_arguments: Some(prepare_arguments),
        #[cfg(target_os = "macos")]
        on_started: (!headless_requested())
            .then(|| std::sync::Arc::new(preview_window::yield_activation_to) as _),
        #[cfg(not(target_os = "macos"))]
        on_started: None,
    };
    let springboard = editor_preview::springboard::Springboard::new(
        local_preview,
        from_run_preview_sender,
        springboard_global,
    );
    let to_previews = vec![
        LspToPreviews::with_one(EditorLspToPreview),
        LspToPreviews::with_one(springboard.clone()),
    ];

    assert_eq!(to_previews.len(), from_previews.len());

    let mut watch_paths_revision = None;
    let mut project_root = project.root.clone();

    let mut session = activate_project(
        &mut file_watcher,
        &mut file_watcher_rx,
        &mut project_root,
        project,
        &to_previews,
    )
    .await;

    const RECOMPILE_DELAY: Duration = Duration::from_millis(50);
    let mut recompile_deadline = None;
    loop {
        if session.pending_recompile.is_empty() {
            recompile_deadline = None;
        } else {
            // Preview messages must not postpone a pending source update.
            recompile_deadline.get_or_insert_with(|| tokio::time::Instant::now() + RECOMPILE_DELAY);
        }
        tokio::select! {
            watcher_event = file_watcher_rx.recv() => {
                match watcher_event {
                    Some(event) => trigger_editor_file_watcher(&mut session, event).await,
                    None => break Err("File watcher channel closed".into()),
                }
            }
            preview_message = receive_preview_message(&mut from_previews) => {
                let (preview_index, message) = preview_message;
                match message {
                    Some(message) => {
                        handle_preview_message(
                            message,
                            preview_index,
                            &mut session,
                            &project_root,
                        ).await;
                    }
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
                    }
                }
            }
            editor_message = from_editor.recv() => {
                match editor_message {
                    Some(EditorToSessionMessage::RunPreview) => {
                        if run_preview(&mut session) {
                            springboard.start();
                        }
                    }
                    Some(EditorToSessionMessage::SwitchProject(project)) => {
                        watch_paths_revision = None;
                        recompile_deadline = None;
                        switch_project(
                            &mut session,
                            &mut file_watcher,
                            &mut file_watcher_rx,
                            &mut project_root,
                            project,
                            &to_previews,
                        ).await;
                    }
                    Some(EditorToSessionMessage::CloseProject) => {
                        stop_run_preview(&mut session).await;
                    }
                    None => break Ok(()),
                }
            }
            _ = async {
                match recompile_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                recompile_deadline = None;
                tracing::debug!("LSP recompiling");
                let pending_recompile = std::mem::take(&mut session.pending_recompile);

                for url in pending_recompile {
                    if let Err(err) = session.reload_document(url).await {
                        tracing::error!("Failed document reload: {err}");
                    }
                }
            }
        }

        if let Err(error) = sync_file_watcher_if_needed(
            &mut file_watcher,
            &session,
            &project_root,
            &mut watch_paths_revision,
        ) {
            tracing::warn!("Failed to watch project: {error}");
        }
    }
}

fn new_editor_session() -> editor_preview::EditorSession {
    use editor_preview::document_cache::CompilerConfiguration;
    let compiler_config =
        CompilerConfiguration { format: editor_preview::ByteFormat::Utf8, ..Default::default() };

    editor_preview::EditorSession::new(compiler_config)
}

async fn trigger_editor_file_watcher(
    session: &mut editor_preview::EditorSession,
    WatchEvent { path, kind }: WatchEvent,
) {
    let Ok(url) = Url::from_file_path(&path) else {
        tracing::debug!("Ignoring file watcher event for non-file path: {}", path.display());
        return;
    };

    if let Err(error) = session.trigger_file_watcher(url, kind).await {
        tracing::warn!("Failed to process file watcher event for {}: {error}", path.display());
    }
}

fn sync_file_watcher_if_needed(
    watcher: &mut FileWatcher,
    session: &editor_preview::EditorSession,
    project_root: &Path,
    watch_paths_revision: &mut Option<u64>,
) -> Result<()> {
    let current_revision = session.document_cache.revision();
    if watch_paths_revision.is_some_and(|rev| rev == current_revision) {
        return Ok(());
    }

    // Watching the project root as a target also watches its parent directory.
    // This is mostly unnecessary, as we don't expect the project root to change.
    // And on macOS the notify version 8.0.0 has a bug in the kqueue backend that
    // turns non-recursive watches recursive!
    //
    // So watching the project roots (and therefore its parent) could easily cause
    // us to watch all of /home or /tmp, which quickly exhausts the notify limit.
    //
    // Until notify 9.0 is released, let's not watch the project root.
    watcher.update_watched_paths(
        session
            .document_cache
            .all_paths_to_watch()
            .into_iter()
            .chain(
                session
                    .project_main_urls()
                    .filter_map(|url| SourcePath::from_url(&url).into_native_path()),
            )
            .chain(session.previews().iter().filter_map(|preview| {
                preview
                    .component()
                    .and_then(|component| SourcePath::from_url(&component.url).into_native_path())
            }))
            .chain(session.active_project_file_paths().map(Path::to_path_buf))
            // Missing and deleted project files aren't active, but their creation must be observed.
            .chain(std::iter::once(project_root.join(i_slint_compiler::project_file::FILE_NAME))),
    )?;
    *watch_paths_revision = Some(current_revision);
    Ok(())
}

async fn handle_preview_message(
    message: PreviewToLspMessage,
    preview_index: usize,
    session: &mut editor_preview::EditorSession,
    project_root: &Path,
) {
    use PreviewToLspMessage::*;
    if session.preview(preview_index).is_none() {
        return;
    }

    match &message {
        RequestState { files, settings } => {
            tracing::debug!("Preview requested state");
            if files.is_empty() {
                session.send_state_to_preview(preview_index);
            } else {
                session.send_files_to_preview(preview_index, files, &[project_root.to_path_buf()]);
            }
            for name in settings {
                if let Some(contents) =
                    i_slint_editor_preview::settings_store::load(TOOL_NAME, name)
                {
                    session.send_to_preview(
                        preview_index,
                        &LspToPreviewMessage::SetUserSettings { name: name.clone(), contents },
                    );
                }
            }
        }
        RequestPreview { component } => {
            let Some((component, _path)) = canonical_preview_component(component) else {
                tracing::warn!("Ignoring preview request with an invalid path: {}", component.url);
                return;
            };
            let root =
                session.preview(preview_index).and_then(|preview| preview.project_root().cloned());
            open_preview(session, preview_index, component, root).await;
        }
        UpdateUserSettings { name, contents } => {
            if let Err(error) =
                i_slint_editor_preview::settings_store::save(TOOL_NAME, name, contents)
            {
                tracing::warn!("Failed to save preview user settings: {error}");
            }
        }
        SendShowMessage { message } => {
            match message.typ {
                MessageType::ERROR => tracing::error!("Preview: {}", message.message),
                MessageType::WARNING => tracing::warn!("Preview: {}", message.message),
                MessageType::LOG => tracing::debug!("Preview: {}", message.message),
                _ => tracing::info!("Preview: {}", message.message),
            };
        }
        DebugMessage { location, message } => {
            eprintln!("{}", editor_preview::preview_log_message_to_string(location, message));
        }
        // If the editor preview requests to "showDocument" that should translate to a highlight in
        // the run preview.
        ShowDocument { file, selection, .. } if preview_index == PRIMARY_PREVIEW_INDEX => {
            let Some(document) = session
                .document_cache
                .get_document(file)
                .and_then(|document| document.node.as_ref())
            else {
                tracing::warn!("Cannot highlight a position in an unknown document: {file}");
                return;
            };
            let offset = editor_preview::util::lsp_position_to_text_size(
                &document.source_file,
                selection.start,
                session.document_cache.format,
            );
            session.send_to_preview(
                RUN_PREVIEW_INDEX,
                &LspToPreviewMessage::HighlightFromEditor {
                    url: Some(file.clone()),
                    offset: offset.into(),
                },
            );
        }
        ClearHighlight if preview_index == PRIMARY_PREVIEW_INDEX => {
            session.send_to_preview(
                RUN_PREVIEW_INDEX,
                &LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
            );
        }
        HighlightInRunPreview { url, offset } if preview_index == PRIMARY_PREVIEW_INDEX => {
            session.send_to_preview(
                RUN_PREVIEW_INDEX,
                &LspToPreviewMessage::HighlightFromEditor { url: url.clone(), offset: *offset },
            );
        }

        Diagnostics { .. }
        | ShowDocument { .. }
        | ClearHighlight
        | HighlightInRunPreview { .. }
        | PreviewTypeChanged { .. }
        | TelemetryEvent(..)
        | ConnectRemote { .. }
        | ConnectSerial { .. }
        | DisconnectRemote
        | SubmitPairingCode { .. }
        | CancelPairing
        | AcceptUnpairedConnection
        | Pong
        | PairingReady
        | PairingRequired { .. }
        | PairingTokenChallenge { .. }
        | PairingConfirm { .. }
        | PairingAccepted
        | PairingRejected { .. }
        | Exited => {
            tracing::debug!("Ignoring message from preview: {message:?}");
        }
        SendWorkspaceEdit { label, edit } => {
            let applied = handle_workspace_edit(session, label.as_deref(), edit).await;
            preview::workspace_edit_finished(edit.clone(), applied);
        }
    }
}

async fn activate_project(
    watcher: &mut FileWatcher,
    watcher_events: &mut tokio::sync::mpsc::UnboundedReceiver<WatchEvent>,
    project_root: &mut PathBuf,
    project: Project,
    to_previews: &[Rc<LspToPreviews>],
) -> editor_preview::EditorSession {
    let mut session = new_editor_session();

    *project_root = project.root;
    session.set_workspace_roots(vec![project_root.clone()]);
    let component = project.preview;
    if let Err(error) =
        watcher.update_watched_paths(SourcePath::from_url(&component.url).into_native_path())
    {
        tracing::warn!("Failed to watch project: {error}");
    }
    while watcher_events.try_recv().is_ok() {}
    let root = match Url::from_directory_path(project_root.as_path()) {
        Ok(root) => Some(root),
        Err(()) => {
            tracing::warn!("Failed to convert {} to URL", project_root.display());
            None
        }
    };
    if let Err(error) = session.reload_document(component.url.clone()).await {
        tracing::warn!("Failed to load {}: {error}", component.url);
    }

    // Attach the previews only after the project has been loaded into the EditorSession.
    // Otherwise, the editor sessions initialization will already send the files to the preview as
    // the EditorSession loads them.
    // However, the initial sync should happen via send_state_to_preview to ensure all data is
    // synched, which would send the file contents a second time.
    // So prevent the initial load. This means that send_state_to_preview should be called after
    // this function returns.
    // This happens either via the initial RequestState from the preview itself.
    // Or manually in switch_project.
    session.attach_preview(to_previews[PRIMARY_PREVIEW_INDEX].clone(), Some(component), root);
    session.attach_preview(to_previews[RUN_PREVIEW_INDEX].clone(), None, None);
    if let Err(error) = sync_file_watcher_if_needed(watcher, &session, project_root, &mut None) {
        tracing::warn!("Failed to watch project: {error}");
    }

    session
}

fn run_preview(session: &mut editor_preview::EditorSession) -> bool {
    let Some(component) = session.primary_preview().component().cloned() else {
        tracing::warn!("Cannot run a preview before a component is open");
        return false;
    };
    let root = session.primary_preview().project_root().cloned();
    session.show_preview(RUN_PREVIEW_INDEX, component, root);
    true
}

async fn switch_project(
    session: &mut editor_preview::EditorSession,
    file_watcher: &mut FileWatcher,
    watcher_events: &mut tokio::sync::mpsc::UnboundedReceiver<WatchEvent>,
    project_root: &mut PathBuf,
    project: Project,
    to_previews: &[Rc<LspToPreviews>],
) {
    stop_run_preview(session).await;
    *session =
        activate_project(file_watcher, watcher_events, project_root, project, to_previews).await;
    session.send_state_to_preview(PRIMARY_PREVIEW_INDEX);
}

async fn stop_run_preview(session: &mut editor_preview::EditorSession) {
    session.previews()[RUN_PREVIEW_INDEX].to_preview().shutdown().await;
    session.send_to_preview(
        RUN_PREVIEW_INDEX,
        &LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
    );
}

async fn open_preview(
    session: &mut editor_preview::EditorSession,
    preview_index: usize,
    component: PreviewComponent,
    root: Option<Url>,
) {
    if let Err(error) = session.reload_document(component.url.clone()).await {
        tracing::warn!("Failed to load {}: {error}", component.url);
    }
    session.show_preview(preview_index, component, root);
}

fn canonical_preview_component(
    component: &PreviewComponent,
) -> Option<(PreviewComponent, PathBuf)> {
    let path = SourcePath::from_url(&component.url).into_native_path()?;
    let path = std::fs::canonicalize(path).ok()?;
    let url = Url::from_file_path(&path).ok()?;
    Some((PreviewComponent { url, component: component.component.clone() }, path))
}

async fn handle_workspace_edit(
    session: &mut editor_preview::EditorSession,
    label: Option<&str>,
    edit: &lsp_types::WorkspaceEdit,
) -> bool {
    let urls = editor_preview::editing::text_edit::EditIterator::new(edit)
        .map(|(document, _)| document.uri)
        .collect::<std::collections::HashSet<_>>();
    let mut sources = std::collections::HashMap::new();
    for url in &urls {
        let path = SourcePath::from_url(url);
        let source_file =
            if path.as_native_path().is_some_and(i_slint_compiler::project_file::is_project_file) {
                match path.read_to_string() {
                    Ok(contents) => std::sync::Arc::new(
                        i_slint_compiler::diagnostics::SourceFileInner::new(path, contents),
                    ),
                    Err(error) => {
                        tracing::error!(%url, %error, "Failed to load a workspace edit document");
                        return false;
                    }
                }
            } else {
                if session
                    .document_cache
                    .get_document(url)
                    .and_then(|document| document.node.as_ref())
                    .is_none()
                    && let Err(error) = session.reload_document(url.clone()).await
                {
                    tracing::error!(%url, %error, "Failed to load a workspace edit document");
                    return false;
                }
                let Some(source_file) = session
                    .document_cache
                    .get_document(url)
                    .and_then(|document| document.node.as_ref())
                    .map(|node| node.source_file.clone())
                else {
                    tracing::warn!(%url, "Cannot apply a workspace edit with an unloaded document");
                    return false;
                };
                source_file
            };
        sources.insert(url.clone(), source_file);
    }
    match editor_preview::editing::text_edit::apply_workspace_edit_with_source(
        edit,
        session.document_cache.format,
        |url| sources.get(url).cloned(),
    ) {
        Ok(edited_texts) => {
            let mut applied = true;
            for editor_preview::editing::text_edit::EditedText { url, contents } in edited_texts {
                match SourcePath::from_url(&url).into_native_path() {
                    Some(path) => {
                        if let Err(err) = std::fs::write(&path, &contents) {
                            applied = false;
                            tracing::error!(
                                "Failed to apply workspace edit '{}' to {}: {err}",
                                label.unwrap_or("(unnamed)"),
                                path.display()
                            );
                        }
                    }
                    None => {
                        applied = false;
                        tracing::warn!("Cannot apply workspace edit to non-file URL: {url}");
                    }
                }
            }
            applied
        }
        Err(err) => {
            tracing::error!(
                "Failed to compute workspace edit '{}': {err}",
                label.unwrap_or("(unnamed)")
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn count_file_contents(messages: &[LspToPreviewMessage]) -> HashMap<Url, usize> {
        let mut counts = HashMap::new();
        for message in messages {
            if let LspToPreviewMessage::SetContents { url, .. } = message {
                *counts.entry(url.url().clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    #[test]
    fn initial_request_sends_each_prepared_file_once() {
        let directory = tempfile::tempdir().unwrap();
        let project_directory = directory.path().canonicalize().unwrap();
        let main_path = project_directory.join("main.slint");
        let import_path = project_directory.join("child.slint");
        let manifest_path = project_directory.join("slint-project.json");
        std::fs::write(
            &main_path,
            "import { Child } from \"child.slint\"; export component Main { Child {} }",
        )
        .unwrap();
        std::fs::write(&import_path, "export component Child inherits Rectangle {}").unwrap();
        std::fs::write(&manifest_path, r#"{"main":"main.slint", "edition": "1.19"}"#).unwrap();
        let project = Project::from_input(&project_directory, None, None).unwrap();
        let (to_previews, messages) = recording_previews();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        let mut root = PathBuf::new();
        let mut session = spin_on::spin_on(activate_project(
            &mut watcher,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut root,
            project,
            &to_previews,
        ));
        let request = PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() };
        spin_on::spin_on(handle_preview_message(
            request,
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            &root,
        ));
        for url in std::mem::take(&mut session.pending_recompile) {
            spin_on::spin_on(session.reload_document(url)).unwrap();
        }

        let contents_counts = count_file_contents(&messages[PRIMARY_PREVIEW_INDEX].borrow());
        let expected_urls = [&main_path, &import_path, &manifest_path]
            .map(|path| Url::from_file_path(path).unwrap())
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(contents_counts.keys().cloned().collect::<HashSet<_>>(), expected_urls);
        assert!(contents_counts.values().all(|count| *count == 1), "{contents_counts:?}");

        let late_import_path = project_directory.join("late.slint");
        std::fs::write(&late_import_path, "export component Late inherits Rectangle {}").unwrap();
        std::fs::write(
            &main_path,
            "import { Child } from \"child.slint\"; import { Late } from \"late.slint\"; export component Main { Child {} Late {} }",
        )
        .unwrap();
        messages[PRIMARY_PREVIEW_INDEX].borrow_mut().clear();
        let main_url = Url::from_file_path(&main_path).unwrap();
        spin_on::spin_on(session.trigger_file_watcher(
            main_url.clone(),
            i_slint_live_preview::file_watcher::FileChangeKind::Changed,
        ))
        .unwrap();
        for url in std::mem::take(&mut session.pending_recompile) {
            spin_on::spin_on(session.reload_document(url)).unwrap();
        }
        let late_import_url = Url::from_file_path(&late_import_path).unwrap();
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::SetContents { url, .. }
                if url.url() == &late_import_url)
        }));

        messages[PRIMARY_PREVIEW_INDEX].borrow_mut().clear();
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            &root,
        ));
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::SetContents { url, .. }
                if url.url() == &late_import_url)
        }));
    }

    #[test]
    fn initial_project_loads_through_the_session_after_opening_the_root() {
        let directory = tempfile::tempdir().unwrap();
        let project_directory = std::fs::canonicalize(directory.path()).unwrap();
        let source = project_directory.join("main.slint");
        std::fs::write(&source, "export component Main {}").unwrap();
        let manifest = project_directory.join("slint-project.json");
        std::fs::write(&manifest, r#"{"edition":"1.19","main":"main.slint"}"#).unwrap();
        let project = Project::from_input(&project_directory, None, None).unwrap();
        let component = project.preview.clone();
        let (to_previews, messages) = recording_previews();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        let mut root = PathBuf::new();
        let mut session = spin_on::spin_on(activate_project(
            &mut watcher,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut root,
            project,
            &to_previews,
        ));
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            &root,
        ));
        assert_eq!(session.primary_preview().component().cloned(), Some(component.clone()));
        assert!(
            session.document_cache.get_document(&Url::from_file_path(source).unwrap()).is_some()
        );
        assert_eq!(root, project_directory);
        let messages = messages[PRIMARY_PREVIEW_INDEX].borrow();
        assert!(messages.iter().any(|message| matches!(message,
            LspToPreviewMessage::OpenProject { root: Some(sent) }
                if sent == &Url::from_directory_path(&project_directory).unwrap()
        )));
        assert!(messages.iter().any(|message| matches!(message,
            LspToPreviewMessage::SetContents { url, .. }
                if url.url() == &Url::from_file_path(&manifest).unwrap()
        )));
        assert!(
            matches!(messages.last(), Some(LspToPreviewMessage::ShowPreview(target)) if target == &component)
        );
    }

    #[test]
    fn initial_source_loads_with_an_invalid_project_file() {
        let directory = tempfile::tempdir().unwrap();
        let project_directory = std::fs::canonicalize(directory.path()).unwrap();
        let source = project_directory.join("main.slint");
        std::fs::write(&source, "export component Main {}").unwrap();
        std::fs::write(project_directory.join("slint-project.json"), "{").unwrap();
        let project = Project::from_input(&source, None, None).unwrap();
        let (to_previews, _) = recording_previews();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        let mut root = PathBuf::new();
        let session = spin_on::spin_on(activate_project(
            &mut watcher,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut root,
            project,
            &to_previews,
        ));
        assert!(
            session.document_cache.get_document(&Url::from_file_path(source).unwrap()).is_some()
        );
    }

    fn recording_previews()
    -> (Vec<Rc<LspToPreviews>>, [editor_preview::test::CapturedPreviewMessages; 2]) {
        let captures = std::array::from_fn(|_| editor_preview::test::preview_capture());
        let previews = captures.iter().map(|(to_preview, _)| to_preview.clone()).collect();
        let messages = captures.map(|(_, messages)| messages);
        (previews, messages)
    }

    fn session_with_recording_previews()
    -> (editor_preview::EditorSession, [editor_preview::test::CapturedPreviewMessages; 2]) {
        let (previews, messages) = recording_previews();
        let session = editor_preview::EditorSession::with_previews(
            editor_preview::test::empty_document_cache(),
            previews,
        );
        (session, messages)
    }

    fn clear_messages(messages: &[editor_preview::test::CapturedPreviewMessages]) {
        for messages in messages {
            messages.borrow_mut().clear();
        }
    }

    fn component(file_name: &str, name: &str) -> PreviewComponent {
        PreviewComponent {
            url: editor_preview::test::test_file_name(file_name).to_url().unwrap(),
            component: Some(name.into()),
        }
    }

    #[test]
    fn workspace_edit_is_applied_during_a_file_watcher_reload() {
        for changed_file in ["Main.slint", "Child.slint"] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let path = root.join("Main.slint");
            let original = "import { Child } from \"Child.slint\"; export component Main inherits Window { width: 100px; Child {} }";
            let expected = original.replace("100px", "120px");
            std::fs::write(&path, original).unwrap();
            std::fs::write(
                root.join("Child.slint"),
                "export component Child inherits Rectangle {}",
            )
            .unwrap();
            let url = Url::from_file_path(&path).unwrap();
            let (mut session, _) = session_with_recording_previews();
            spin_on::spin_on(session.load_document_impl(original.into(), url.clone(), None));
            session.show_preview(
                PRIMARY_PREVIEW_INDEX,
                PreviewComponent { url: url.clone(), component: Some("Main".into()) },
                None,
            );
            let edit = editor_preview::editing::create_workspace_edit(
                url.clone(),
                None,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(0, 0),
                        lsp_types::Position::new(0, original.len() as u32),
                    ),
                    new_text: expected.clone(),
                }],
            );
            spin_on::spin_on(trigger_editor_file_watcher(
                &mut session,
                WatchEvent {
                    path: root.join(changed_file),
                    kind: i_slint_live_preview::file_watcher::FileChangeKind::Changed,
                },
            ));
            assert!(session.pending_recompile.contains(&url));
            assert!(
                session
                    .document_cache
                    .get_document(&url)
                    .and_then(|document| document.node.as_ref())
                    .is_none()
            );
            spin_on::spin_on(handle_preview_message(
                PreviewToLspMessage::SendWorkspaceEdit { label: Some("Resize".into()), edit },
                PRIMARY_PREVIEW_INDEX,
                &mut session,
                &root,
            ));
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                expected,
                "watcher event for {changed_file}"
            );
        }
    }

    #[test]
    fn file_watcher_read_failure_does_not_stop_later_events() {
        use i_slint_live_preview::file_watcher::FileChangeKind;
        for kind in [FileChangeKind::Created, FileChangeKind::Changed] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let main = root.join("main.slint");
            std::fs::write(&main, "export component Main {}").unwrap();
            let path = root.join("slint-project.json");
            let url = Url::from_file_path(&path).unwrap();
            let contents = r#"{"edition":"1.19","main":"main.slint"}"#;
            let (mut session, messages) = session_with_recording_previews();
            spin_on::spin_on(session.reload_document(Url::from_file_path(main).unwrap())).unwrap();
            std::fs::write(&path, contents).unwrap();
            std::fs::remove_file(&path).unwrap();
            spin_on::spin_on(trigger_editor_file_watcher(
                &mut session,
                WatchEvent { path: path.clone(), kind },
            ));
            clear_messages(&messages);
            std::fs::write(&path, contents).unwrap();
            spin_on::spin_on(trigger_editor_file_watcher(&mut session, WatchEvent { path, kind }));
            assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
                matches!(message, LspToPreviewMessage::SetContents { url: received, contents: received_contents }
                    if received.url() == &url && received_contents == contents.as_bytes())
            }));
        }
    }

    #[test]
    fn canvas_project_file_creation_and_recreation_reach_the_preview() {
        use i_slint_live_preview::file_watcher::FileChangeKind;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let main = root.join("main.slint");
        std::fs::write(&main, "export component Main {}").unwrap();
        let path = root.join("slint-project.json");
        let url = Url::from_file_path(&path).unwrap();
        let (mut session, messages) = session_with_recording_previews();
        spin_on::spin_on(session.reload_document(Url::from_file_path(main).unwrap())).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut watcher = FileWatcher::start(
            move |event| {
                let _ = sender.send(event);
            },
            |_| {},
        )
        .unwrap();
        sync_file_watcher_if_needed(&mut watcher, &session, &root, &mut None).unwrap();
        for contents in [
            Some(r#"{"edition":"1.19","metadata":{"visual-editor":{"canvas-presets":[]}}}"#),
            None,
            Some("{"),
            Some(r#"{"edition":"1.19","main":"main.slint"}"#),
        ] {
            clear_messages(&messages);
            while receiver.try_recv().is_ok() {}
            let expected_kind = if contents.is_some() {
                if path.exists() { FileChangeKind::Changed } else { FileChangeKind::Created }
            } else {
                FileChangeKind::Deleted
            };
            match contents {
                Some(contents) => std::fs::write(&path, contents).unwrap(),
                None => std::fs::remove_file(&path).unwrap(),
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let event = receiver
                    .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .unwrap();
                if event.path == path && event.kind == expected_kind {
                    spin_on::spin_on(trigger_editor_file_watcher(&mut session, event));
                    break;
                }
            }
            let messages = messages[PRIMARY_PREVIEW_INDEX].borrow();
            assert!(
                messages.iter().any(|message| match (contents, message) {
                    (
                        Some(expected),
                        LspToPreviewMessage::SetContents { url: received, contents },
                    ) => received.url() == &url && contents == expected.as_bytes(),
                    (None, LspToPreviewMessage::ForgetFile { url: received }) => received == &url,
                    _ => false,
                }),
                "{messages:?}"
            );
            sync_file_watcher_if_needed(&mut watcher, &session, &root, &mut None).unwrap();
        }
    }

    #[test]
    fn project_workspace_edits_apply_without_a_syntax_tree_and_ignore_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slint-project.json");
        let original = r#"{"edition":"1.19","main":"main.slint",/* keep */"metadata":{"visual-editor":{"canvas-presets":[{"width":100,"height":1080}]}}}"#;
        std::fs::write(&path, original).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let (mut session, _) = session_with_recording_previews();
        for (version, width) in [(Some(10), 720), (Some(1), 1920)] {
            let source = std::fs::read_to_string(&path).unwrap();
            let offset = source.find("100").or_else(|| source.find("720")).unwrap();
            let edit = editor_preview::editing::create_workspace_edit(
                url.clone(),
                version,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(0, offset as u32),
                        lsp_types::Position::new(0, offset as u32 + 3),
                    ),
                    new_text: width.to_string(),
                }],
            );
            assert!(spin_on::spin_on(handle_workspace_edit(&mut session, None, &edit)));
            assert!(session.document_cache.get_document(&url).is_none());
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                original.replace("100,", &format!("{width},"))
            );
        }
        std::fs::remove_file(&path).unwrap();
        let edit = editor_preview::editing::create_workspace_edit(
            url,
            None,
            vec![lsp_types::TextEdit { range: lsp_types::Range::default(), new_text: "{}".into() }],
        );
        assert!(!spin_on::spin_on(handle_workspace_edit(&mut session, None, &edit)));
        assert!(!path.exists());
    }

    #[test]
    fn missing_workspace_edit_document_prevents_partial_writes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("Main.slint");
        let original = "export component Main inherits Window { width: 100px; }";
        std::fs::write(&path, original).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let missing = Url::from_file_path(root.join("Missing.slint")).unwrap();
        let (mut session, _) = session_with_recording_previews();
        spin_on::spin_on(session.load_document_impl(original.into(), url.clone(), None));
        let edit = lsp_types::TextEdit {
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, original.len() as u32),
            ),
            new_text: original.replace("100px", "120px"),
        };
        let edit = editor_preview::editing::create_workspace_edit_from_text_document_edits(vec![
            editor_preview::editing::create_text_document_edit(url, None, vec![edit.clone()]),
            editor_preview::editing::create_text_document_edit(missing, None, vec![edit]),
        ]);
        assert!(!spin_on::spin_on(handle_workspace_edit(&mut session, Some("Resize"), &edit)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn separate_preview_channels_report_the_ready_receiver() {
        let (primary_sender, primary_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (secondary_sender, secondary_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut receivers = vec![primary_receiver, secondary_receiver];
        secondary_sender.send(PreviewToLspMessage::Pong).unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let (preview_index, message) = runtime.block_on(receive_preview_message(&mut receivers));

        assert_eq!(preview_index, 1);
        assert!(matches!(message, Some(PreviewToLspMessage::Pong)));
        drop(primary_sender);
    }

    #[test]
    fn switching_project_replaces_the_document_session_and_opens_the_new_preview() {
        let (to_previews, messages) = recording_previews();
        let mut session = editor_preview::EditorSession::with_previews(
            editor_preview::test::empty_document_cache(),
            to_previews.clone(),
        );
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        assert!(session.document_cache.get_document(&old_url).is_some());
        session.show_preview(
            PRIMARY_PREVIEW_INDEX,
            PreviewComponent { url: old_url.clone(), component: Some("Old".into()) },
            None,
        );
        assert!(run_preview(&mut session));

        let old_run_preview = session.previews()[RUN_PREVIEW_INDEX].to_preview().clone();
        let new_project = tempfile::tempdir().unwrap();
        let new_path = new_project.path().join("new.slint");
        std::fs::write(&new_path, "export component New {}").unwrap();
        let project = Project::from_input(&new_path, Some("New".into()), None).unwrap();
        let expected_root = project.root.clone();
        let expected_component = project.preview.clone();
        let mut project_root = old_project.path().to_path_buf();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);
        spin_on::spin_on(switch_project(
            &mut session,
            &mut watcher,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut project_root,
            project,
            &to_previews,
        ));

        assert_eq!(project_root, expected_root);
        assert!(session.document_cache.get_document(&old_url).is_none());
        assert!(session.document_cache.get_document(&expected_component.url).is_some());
        assert!(session.previews()[RUN_PREVIEW_INDEX].component().cloned().is_none());
        assert!(Rc::ptr_eq(session.previews()[RUN_PREVIEW_INDEX].to_preview(), &old_run_preview));
        let expected_root_url =
            Url::from_directory_path(std::fs::canonicalize(&project_root).unwrap()).unwrap();
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::OpenProject { root }
                if root.as_ref() == Some(&expected_root_url))
        }));
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component)
                if component == &expected_component)
        }));
        let contents_counts = count_file_contents(&messages[PRIMARY_PREVIEW_INDEX].borrow());
        assert_eq!(contents_counts.get(&expected_component.url), Some(&1));
        assert!(contents_counts.values().all(|count| *count == 1), "{contents_counts:?}");
        assert!(
            messages[PRIMARY_PREVIEW_INDEX]
                .borrow()
                .iter()
                .any(|message| { matches!(message, LspToPreviewMessage::SetConfiguration { .. }) })
        );
        assert!(
            messages[PRIMARY_PREVIEW_INDEX]
                .borrow()
                .iter()
                .all(|message| !matches!(message, LspToPreviewMessage::Quit))
        );
        assert_eq!(
            messages[RUN_PREVIEW_INDEX]
                .borrow()
                .iter()
                .filter(|message| matches!(message, LspToPreviewMessage::Quit))
                .count(),
            1
        );
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: None, .. })
        }));
        assert!(
            messages[RUN_PREVIEW_INDEX]
                .borrow()
                .iter()
                .all(|message| { !matches!(message, LspToPreviewMessage::ShowPreview(_)) })
        );

        clear_messages(&messages);
        assert!(run_preview(&mut session));
        assert_eq!(
            session.previews()[RUN_PREVIEW_INDEX].component().cloned(),
            Some(expected_component.clone())
        );
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component)
                if component == &expected_component)
        }));
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::OpenProject { root }
                if root.as_ref() == Some(&expected_root_url))
        }));
    }

    #[test]
    fn failed_project_switch_keeps_the_new_project_active() {
        let (to_previews, messages) = recording_previews();
        let mut session = editor_preview::EditorSession::with_previews(
            editor_preview::test::empty_document_cache(),
            to_previews.clone(),
        );
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        let old_component =
            PreviewComponent { url: old_url.clone(), component: Some("Old".into()) };
        session.show_preview(PRIMARY_PREVIEW_INDEX, old_component.clone(), None);
        assert!(run_preview(&mut session));
        let old_run_preview = session.previews()[RUN_PREVIEW_INDEX].to_preview().clone();
        let mut project_root = old_project.path().to_path_buf();
        let missing_root = old_project.path().join("missing");
        let project = Project {
            root: missing_root.clone(),
            selected_canvas_preset: 0,
            preview: PreviewComponent {
                url: Url::from_file_path(missing_root.join("main.slint")).unwrap(),
                component: Some("Missing".into()),
            },
        };
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);

        let target = project.preview.clone();
        spin_on::spin_on(switch_project(
            &mut session,
            &mut watcher,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut project_root,
            project,
            &to_previews,
        ));

        assert_eq!(project_root, missing_root);
        assert!(session.document_cache.get_document(&old_url).is_none());
        assert_eq!(session.primary_preview().component().cloned(), Some(target));
        assert!(session.previews()[RUN_PREVIEW_INDEX].component().cloned().is_none());
        assert!(Rc::ptr_eq(session.previews()[RUN_PREVIEW_INDEX].to_preview(), &old_run_preview));
        assert!(matches!(
            messages[PRIMARY_PREVIEW_INDEX].borrow().first(),
            Some(LspToPreviewMessage::OpenProject { .. })
        ));
    }

    #[test]
    fn preview_requests_are_answered_through_the_originating_connection() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("secondary.slint");
        std::fs::write(&path, "export component Secondary {}").unwrap();
        let path = std::fs::canonicalize(path).unwrap();
        let component = PreviewComponent {
            url: Url::from_file_path(&path).unwrap(),
            component: Some("Secondary".into()),
        };

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            1,
            &mut session,
            project.path(),
        ));

        assert!(messages[0].borrow().is_empty());
        assert!(
            messages[1]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::OpenProject { .. }))
        );

        clear_messages(&messages);

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestPreview { component: component.clone() },
            1,
            &mut session,
            project.path(),
        ));

        assert!(
            !messages[0]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
        );
        assert!(messages[1].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(current) if current == &component)
        }));
        assert!(session.primary_preview().component().cloned().is_none());
        assert_eq!(session.preview(1).unwrap().component().cloned(), Some(component));
    }

    #[test]
    fn run_without_a_component_does_not_start_springboard() {
        let (mut session, messages) = session_with_recording_previews();
        assert!(!run_preview(&mut session));
        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }

    #[test]
    fn run_preview_uses_the_primary_preview_target() {
        let (mut session, messages) = session_with_recording_previews();
        let primary_component = component("primary.slint", "Primary");
        session.show_preview(PRIMARY_PREVIEW_INDEX, primary_component.clone(), None);
        clear_messages(&messages);

        run_preview(&mut session);

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component) if component == &primary_component)
        }));
        assert_eq!(
            session.preview(RUN_PREVIEW_INDEX).unwrap().component().cloned(),
            Some(primary_component)
        );
    }

    #[test]
    fn run_preview_target_changes_only_when_run_is_requested() {
        let (mut session, messages) = session_with_recording_previews();
        let first_component = component("first.slint", "First");
        let second_component = component("second.slint", "Second");
        session.show_preview(PRIMARY_PREVIEW_INDEX, first_component.clone(), None);
        run_preview(&mut session);
        clear_messages(&messages);

        session.show_preview(PRIMARY_PREVIEW_INDEX, second_component.clone(), None);

        assert_eq!(
            session.preview(RUN_PREVIEW_INDEX).unwrap().component().cloned(),
            Some(first_component)
        );
        assert!(messages[RUN_PREVIEW_INDEX].borrow().is_empty());

        run_preview(&mut session);
        assert_eq!(
            session.preview(RUN_PREVIEW_INDEX).unwrap().component().cloned(),
            Some(second_component)
        );
    }

    #[test]
    fn editor_forwards_highlight_without_replaying_it_in_state_response() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("main.slint");
        let url = Url::from_file_path(&path).unwrap();
        let source = "export component Main {\n    Text {}\n}";
        spin_on::spin_on(session.load_document_impl(source.into(), url.clone(), None));
        session.show_preview(
            PRIMARY_PREVIEW_INDEX,
            PreviewComponent { url: url.clone(), component: Some("Main".into()) },
            None,
        );
        run_preview(&mut session);
        clear_messages(&messages);

        let expected_offset = u32::try_from(source.find("Text").unwrap()).unwrap();
        let document = session.document_cache.get_document(&url).unwrap().node.as_ref().unwrap();
        let position = editor_preview::util::text_size_to_lsp_position(
            &document.source_file,
            expected_offset.into(),
            session.document_cache.format,
        );
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: url.clone(),
                selection: lsp_types::Range::new(position, position),
                take_focus: false,
            },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            project.path(),
        ));

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        }));

        clear_messages(&messages);
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
        ));
        let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
        assert!(
            run_messages
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
        );
        let highlight_position = run_messages.iter().position(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        });
        assert!(highlight_position.is_none());
    }

    #[test]
    fn clear_highlight_only_clears_the_run_preview_from_the_primary_preview() {
        for preview_index in [PRIMARY_PREVIEW_INDEX, RUN_PREVIEW_INDEX] {
            let (mut session, messages) = session_with_recording_previews();
            let project = tempfile::tempdir().unwrap();

            spin_on::spin_on(handle_preview_message(
                PreviewToLspMessage::ClearHighlight,
                preview_index,
                &mut session,
                project.path(),
            ));

            assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
            let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
            if preview_index == PRIMARY_PREVIEW_INDEX {
                assert!(matches!(
                    run_messages.as_slice(),
                    [LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 }]
                ));
            } else {
                assert!(run_messages.is_empty());
            }
        }
    }

    #[test]
    fn hover_highlight_only_reaches_the_run_preview_from_the_primary_preview() {
        let url = editor_preview::test::test_file_name("hover.slint").to_url().unwrap();
        for preview_index in [PRIMARY_PREVIEW_INDEX, RUN_PREVIEW_INDEX] {
            let (mut session, messages) = session_with_recording_previews();
            let project = tempfile::tempdir().unwrap();

            spin_on::spin_on(handle_preview_message(
                PreviewToLspMessage::HighlightInRunPreview { url: Some(url.clone()), offset: 42 },
                preview_index,
                &mut session,
                project.path(),
            ));

            assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
            let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
            if preview_index == PRIMARY_PREVIEW_INDEX {
                assert!(matches!(
                    run_messages.as_slice(),
                    [LspToPreviewMessage::HighlightFromEditor { url: Some(u), offset: 42 }] if *u == url
                ));
            } else {
                assert!(run_messages.is_empty());
            }
        }
    }

    #[test]
    fn show_document_from_the_run_preview_is_ignored() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: editor_preview::test::test_file_name("run.slint").to_url().unwrap(),
                selection: Default::default(),
                take_focus: false,
            },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
        ));

        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }

    /// Remote viewers reach the editor through the Springboard of the run preview (#14132).
    #[test]
    fn the_run_preview_can_only_request_files_of_the_project() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let inside = project.path().join("logo.png");
        let outside = elsewhere.path().join("logo.png");
        std::fs::write(&inside, []).unwrap();
        std::fs::write(&outside, []).unwrap();
        let [inside, outside] = [inside, outside].map(|path| Url::from_file_path(path).unwrap());

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState {
                files: vec![inside.clone(), outside.clone()],
                settings: Vec::new(),
            },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
        ));

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
        assert!(
            matches!(
                run_messages.as_slice(),
                [
                    LspToPreviewMessage::SetContents { url: served, .. },
                    LspToPreviewMessage::ForgetFile { url: refused },
                ] if served.url() == &inside && refused == &outside
            ),
            "{run_messages:?}"
        );
    }
}
