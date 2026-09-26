// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    time::Duration,
};

use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::{LspToPreviews, Result, document_cache::OpenImportCallback};
use i_slint_live_preview::file_watcher::{FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewTarget, PreviewToLspMessage, SourceFileVersion,
    VersionedUrl,
};
use lsp_types::{MessageType, Url};
use slint::ComponentHandle;

#[cfg(target_os = "linux")]
mod flatpak;
mod preview;
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
}

#[derive(Default)]
struct RunPreviewState {
    requested: bool,
    highlight: Option<(Url, u32)>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    if cli.run_preview_child {
        return editor_preview::child_process::run();
    }

    // Set up the Slint backend (installing the macOS unified-title-bar hook)
    select_backend()?;

    let editor_ui = preview::ui::create_ui()?;

    // The updater needs to stay in scope for as long as the window is up.
    #[cfg(target_os = "macos")]
    let _updater = setup_macos_chrome(&editor_ui);
    #[cfg(target_os = "linux")]
    let _updater = flatpak::connect(&editor_ui);
    #[cfg(target_os = "windows")]
    let _updater = windows::connect(&editor_ui);

    let settings = startup::load_settings();
    let active_session = RefCell::new(None::<crossbeam_channel::Sender<EditorToSessionMessage>>);
    let editor_ui_weak = editor_ui.as_weak();
    let session_settings = settings.clone();
    let start_project = Rc::new(move |project| {
        let mut session_slot = active_session.borrow_mut();
        if let Some(session_sender) = session_slot.as_ref() {
            return session_sender.send(EditorToSessionMessage::SwitchProject(project)).is_ok();
        }
        let Some(editor_ui) = editor_ui_weak.upgrade() else {
            return false;
        };
        *session_slot = Some(start_editor_session(&editor_ui, project, session_settings.clone()));
        true
    });
    startup::setup(&editor_ui, &settings, start_project.clone());
    if let Some(file) = cli.file {
        let project = Project::from_file(file, cli.component)?;
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
    file: Option<String>,
    component: Option<String>,
}

fn select_backend() -> std::result::Result<(), slint::PlatformError> {
    let headless_requested = std::env::var("SLINT_BACKEND").is_ok_and(|backend| {
        i_slint_backend_selector::parse_backend_env_var(&backend.to_ascii_lowercase()).0
            == "headless"
    });
    if headless_requested {
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
    let preview_global =
        <preview::ui::Preview as slint::Global<'_, preview::ui::EditorUi>>::as_weak(
            &preview_global,
        );
    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;
    preview::ui::initialize_editor(editor_ui, &to_lsp, "");
    preview::initialize(editor_ui, to_lsp, settings);
    start_lsp_thread(vec![from_preview], from_editor_preview, project, preview_global);
    to_editor_session
}

fn start_lsp_thread(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    from_editor_preview: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    preview_global: slint::Weak<preview::ui::Preview<'static>>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set
            .block_on(&rt, lsp_main(from_previews, from_editor_preview, project, preview_global))
        {
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
    preview_global: slint::Weak<preview::ui::Preview<'static>>,
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

    let to_previews = vec![
        LspToPreviews::with_one(EditorLspToPreview),
        LspToPreviews::with_one(editor_preview::child_process::ChildProcessLspToPreview::new(
            std::env::current_exe()?,
            vec![std::ffi::OsString::from("--run-preview-child")],
            from_run_preview_sender,
        )),
    ];

    let mut session = new_editor_session(to_previews);

    assert_eq!(session.previews.len(), from_previews.len());

    let mut watch_paths_revision = None;
    let mut project_root = project.root;
    open_initial_preview(&mut session, &mut file_watcher, &project_root, project.preview).await?;
    sync_file_watcher_if_needed(
        &mut file_watcher,
        &session,
        &project_root,
        &mut watch_paths_revision,
    )?;

    const RECOMPILE_DELAY: Duration = Duration::from_millis(50);
    let mut recompile_deadline = None;
    let mut run_preview_state = RunPreviewState::default();
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
                    Some(event) => trigger_editor_file_watcher(&mut session, event).await?,
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
                            &mut run_preview_state,
                            &preview_global
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
                        run_preview(&mut session, &mut run_preview_state);
                    }
                    Some(EditorToSessionMessage::SwitchProject(project)) => {
                        watch_paths_revision = None;
                        if let Err(error) = switch_project(
                            &mut session,
                            &mut file_watcher,
                            &mut project_root,
                            project,
                        ).await {
                            tracing::warn!("Failed to switch project: {error}");
                        } else {
                            run_preview_state = RunPreviewState::default();
                        }
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

        sync_file_watcher_if_needed(
            &mut file_watcher,
            &session,
            &project_root,
            &mut watch_paths_revision,
        )?;
    }
}

fn new_editor_session(to_previews: Vec<Rc<LspToPreviews>>) -> editor_preview::EditorSession {
    use editor_preview::document_cache::CompilerConfiguration;

    let open_import_callback = {
        let to_previews = to_previews.clone();
        Rc::new(move |path: String| {
            let to_previews = to_previews.clone();
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read(&path);
                if let Ok(url) = Url::from_file_path(&path) {
                    for to_preview in &to_previews {
                        if let Ok(contents) = &contents {
                            to_preview.send(&LspToPreviewMessage::SetContents {
                                url: VersionedUrl::new(url.clone(), None),
                                contents: contents.clone(),
                            });
                        } else {
                            to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                        }
                    }
                }
                Some(
                    contents
                        .and_then(|c| String::from_utf8(c).map_err(std::io::Error::other))
                        .map(|c| (None, c)),
                )
            })
                as Pin<
                    Box<dyn Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>>,
                >
        }) as OpenImportCallback
    };
    let compiler_config = CompilerConfiguration {
        style: Some("fluent".into()),
        open_import_callback: Some(open_import_callback),
        format: editor_preview::ByteFormat::Utf8,
        ..Default::default()
    };

    editor_preview::EditorSession {
        document_cache: editor_preview::DocumentCache::new(compiler_config),
        preview_config: i_slint_live_preview::protocol::PreviewConfig {
            style: "fluent".into(),
            ..Default::default()
        },
        open_urls: Default::default(),
        previews: to_previews
            .into_iter()
            .map(|to_preview| editor_preview::PreviewConnection {
                to_preview,
                to_show: Default::default(),
            })
            .collect(),
        pending_recompile: Default::default(),
    }
}

async fn trigger_editor_file_watcher(
    session: &mut editor_preview::EditorSession,
    WatchEvent { path, kind }: WatchEvent,
) -> Result<()> {
    let Ok(url) = Url::from_file_path(&path) else {
        tracing::debug!("Ignoring file watcher event for non-file path: {}", path.display());
        return Ok(());
    };

    let _diagnostics = session.trigger_file_watcher(url, kind).await?;
    Ok(())
}

fn sync_file_watcher_if_needed(
    watcher: &mut FileWatcher,
    session: &editor_preview::EditorSession,
    root_path: &Path,
    watch_paths_revision: &mut Option<u64>,
) -> Result<()> {
    let current_revision = session.document_cache.revision();
    if watch_paths_revision.is_some_and(|rev| rev == current_revision) {
        return Ok(());
    }

    watcher.update_watched_paths(
        std::iter::once(root_path.to_path_buf())
            .chain(
                session
                    .document_cache
                    .all_urls_to_watch()
                    .into_iter()
                    // filter out builtins
                    .filter(|url| url.scheme() == "file")
                    .filter_map(|url| editor_preview::uri_to_file(&url)),
            )
            .chain(session.previews.iter().filter_map(|preview| {
                preview
                    .to_show
                    .as_ref()
                    .and_then(|component| editor_preview::uri_to_file(&component.url))
            })),
    )?;
    *watch_paths_revision = Some(current_revision);
    Ok(())
}

async fn handle_preview_message(
    message: PreviewToLspMessage,
    preview_index: usize,
    session: &mut editor_preview::EditorSession,
    project_root: &Path,
    run_preview_state: &mut RunPreviewState,
    preview_global: &slint::Weak<preview::ui::Preview<'static>>,
) {
    use PreviewToLspMessage::*;
    if session.preview(preview_index).is_none() {
        return;
    }

    // any message we receive from the preview that is not "Exited" means the preview is alive.
    if preview_index == RUN_PREVIEW_INDEX {
        let is_running = !matches!(&message, PreviewToLspMessage::Exited);
        if let Err(error) = preview_global.upgrade_in_event_loop(move |preview_global| {
            preview_global.set_is_running(is_running);
        }) {
            tracing::error!("Failed to update Run preview state: {error}");
        }
    }

    match &message {
        RequestState { files, settings } => {
            tracing::debug!("Preview requested state");
            if files.is_empty() {
                if let Ok(root) = Url::from_directory_path(project_root) {
                    session
                        .send_to_preview(preview_index, &LspToPreviewMessage::OpenProject { root });
                }
                session.send_state_to_preview(preview_index);
                if preview_index == RUN_PREVIEW_INDEX {
                    send_run_preview_highlight(session, run_preview_state);
                }
            } else {
                session.send_files_to_preview(preview_index, files, |_| true);
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
            let Some((component, path)) = canonical_preview_component(component) else {
                tracing::warn!("Ignoring preview request with an invalid path: {}", component.url);
                return;
            };
            if let Err(err) = open_preview(session, preview_index, component).await {
                tracing::error!("Failed to open preview for {}: {err}", path.display());
            }
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
            run_preview_state.highlight = Some((file.clone(), offset.into()));
            send_run_preview_highlight(session, run_preview_state);
        }

        Diagnostics { .. }
        | ShowDocument { .. }
        | PreviewTypeChanged { .. }
        | TelemetryEvent(..)
        | ConnectRemote { .. }
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
            let result = handle_workspace_edit(session, label.as_deref(), edit).await;
            preview::workspace_edit_finished(
                edit.clone(),
                result.applied,
                result.changed && !result.applied,
            );
        }
    }
}

async fn open_initial_preview(
    session: &mut editor_preview::EditorSession,
    watcher: &mut FileWatcher,
    project_root: &Path,
    component: PreviewComponent,
) -> Result<()> {
    watcher.update_watched_paths(
        std::iter::once(project_root.to_path_buf())
            .chain(editor_preview::uri_to_file(&component.url)),
    )?;
    open_project(session, PRIMARY_PREVIEW_INDEX, project_root)?;
    open_preview(session, PRIMARY_PREVIEW_INDEX, component).await
}

fn run_preview(
    session: &mut editor_preview::EditorSession,
    run_preview_state: &mut RunPreviewState,
) {
    let Some(component) = session.primary_preview().to_show.clone() else {
        tracing::warn!("Cannot run a preview before a component is open");
        return;
    };
    run_preview_state.requested = true;
    session.show_preview(RUN_PREVIEW_INDEX, component);
    send_run_preview_highlight(session, run_preview_state);
}

async fn switch_project(
    session: &mut editor_preview::EditorSession,
    file_watcher: &mut FileWatcher,
    project_root: &mut PathBuf,
    project: Project,
) -> Result<()> {
    let to_previews = session.previews.iter().map(|preview| preview.to_preview.clone()).collect();
    let mut next_session = new_editor_session(to_previews);
    let next_project_root = project.root;
    open_initial_preview(&mut next_session, file_watcher, &next_project_root, project.preview)
        .await?;

    session.send_to_preview(RUN_PREVIEW_INDEX, &LspToPreviewMessage::Quit);
    *session = next_session;
    *project_root = next_project_root;
    Ok(())
}

fn send_run_preview_highlight(
    session: &editor_preview::EditorSession,
    run_preview_state: &RunPreviewState,
) {
    if !run_preview_state.requested {
        return;
    }
    let Some((url, offset)) = &run_preview_state.highlight else { return };
    session.send_to_preview(
        RUN_PREVIEW_INDEX,
        &LspToPreviewMessage::HighlightFromEditor { url: Some(url.clone()), offset: *offset },
    );
}

async fn open_preview(
    session: &mut editor_preview::EditorSession,
    preview_index: usize,
    component: PreviewComponent,
) -> Result<()> {
    let _diagnostics = session.reload_document(component.url.clone()).await?;
    session.show_preview(preview_index, component);
    Ok(())
}

fn open_project(
    session: &editor_preview::EditorSession,
    preview_index: usize,
    root: &Path,
) -> Result<()> {
    let root = std::fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }
    let url = Url::from_directory_path(&root)
        .map_err(|_| format!("Failed to convert {} to URL", root.display()))?;
    session.send_to_preview(preview_index, &LspToPreviewMessage::OpenProject { root: url });
    Ok(())
}

fn canonical_preview_component(
    component: &PreviewComponent,
) -> Option<(PreviewComponent, PathBuf)> {
    let path = editor_preview::uri_to_file(&component.url)?;
    let path = std::fs::canonicalize(path).ok()?;
    let url = Url::from_file_path(&path).ok()?;
    Some((PreviewComponent { url, component: component.component.clone() }, path))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WorkspaceEditApplication {
    applied: bool,
    changed: bool,
}

async fn handle_workspace_edit(
    session: &mut editor_preview::EditorSession,
    label: Option<&str>,
    edit: &lsp_types::WorkspaceEdit,
) -> WorkspaceEditApplication {
    let edited_texts = match editor_preview::editing::text_edit::apply_workspace_edit(
        &session.document_cache,
        edit,
    ) {
        Ok(edited_texts) => edited_texts,
        Err(err) => {
            tracing::error!(
                "Failed to compute workspace edit '{}': {err}",
                label.unwrap_or("(unnamed)")
            );
            return Default::default();
        }
    };
    let Some(paths) = edited_texts
        .iter()
        .map(|edited| {
            let path = editor_preview::uri_to_file(&edited.url);
            if path.is_none() {
                tracing::warn!("Cannot apply workspace edit to non-file URL: {}", edited.url);
            }
            path
        })
        .collect::<Option<Vec<_>>>()
    else {
        return Default::default();
    };

    let edit_count = edited_texts.len();
    let mut written = Vec::with_capacity(edit_count);
    for (edited, path) in edited_texts.into_iter().zip(paths) {
        if let Err(err) = std::fs::write(&path, &edited.contents) {
            tracing::error!(
                "Failed to apply workspace edit '{}' to {}: {err}",
                label.unwrap_or("(unnamed)"),
                path.display()
            );
            break;
        }
        written.push(edited);
    }

    let all_written = written.len() == edit_count;
    let mut synchronized = true;
    for editor_preview::editing::text_edit::EditedText { url, contents } in &written {
        if let Err(err) = session.load_document(contents.clone(), url.clone(), None).await {
            synchronized = false;
            tracing::error!("Failed to synchronize applied workspace edit for {url}: {err}");
        }
    }

    WorkspaceEditApplication { applied: all_written && synchronized, changed: !written.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_source_repair_is_watched_before_preview_is_published() {
        const REPAIRED: &str = "export component Initial inherits Window { width: 320px; }";
        struct RepairOnPreview(PathBuf);
        impl editor_preview::LspToPreview for RepairOnPreview {
            fn send(&self, message: &LspToPreviewMessage) {
                if matches!(message, LspToPreviewMessage::ShowPreview(_)) {
                    std::fs::write(&self.0, REPAIRED).unwrap();
                }
            }
            fn preview_target(&self) -> PreviewTarget {
                PreviewTarget::Dummy
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("Initial.slint");
        std::fs::write(&source, "export component Initial inherits Window { broken }").unwrap();
        let url = Url::from_file_path(&source).unwrap();
        let mut session = editor_preview::EditorSession {
            document_cache: editor_preview::DocumentCache::new(Default::default()),
            preview_config: Default::default(),
            open_urls: Default::default(),
            previews: vec![editor_preview::PreviewConnection {
                to_preview: LspToPreviews::with_one(RepairOnPreview(source.clone())),
                to_show: None,
            }],
            pending_recompile: Default::default(),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (error_tx, error_rx) = std::sync::mpsc::channel();
        let mut watcher = FileWatcher::start(
            move |event| {
                let _ = tx.send(event);
            },
            move |error| {
                let _ = error_tx.send(error);
            },
        )
        .unwrap();
        spin_on::spin_on(open_initial_preview(
            &mut session,
            &mut watcher,
            &root,
            PreviewComponent { url: url.clone(), component: None },
        ))
        .unwrap();
        sync_file_watcher_if_needed(&mut watcher, &session, &root, &mut None).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        loop {
            let event = rx
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap_or_else(|error| {
                    let errors = error_rx.try_iter().collect::<Vec<_>>();
                    panic!(
                        "repair made while publishing the initial preview must produce a watch event for {url}: {error}; received {seen:?}; watcher errors {errors:?}"
                    );
                });
            if Url::from_file_path(&event.path).ok().as_ref() == Some(&url) {
                spin_on::spin_on(trigger_editor_file_watcher(&mut session, event)).unwrap();
                break;
            }
            seen.push(event);
        }
        assert!(session.pending_recompile.remove(&url));
        spin_on::spin_on(session.reload_document(url.clone())).unwrap();
        let (_, node) =
            session.document_cache.all_url_documents().find(|(u, _)| u == &url).unwrap();
        assert_eq!(node.text().to_string(), REPAIRED);
    }

    fn session_with_recording_previews()
    -> (editor_preview::EditorSession, [editor_preview::test::CapturedPreviewMessages; 2]) {
        let captures = std::array::from_fn(|_| editor_preview::test::preview_capture());
        let previews = captures
            .iter()
            .map(|(to_preview, _)| editor_preview::PreviewConnection {
                to_preview: to_preview.clone(),
                to_show: None,
            })
            .collect();
        let messages = captures.map(|(_, messages)| messages);
        let session = editor_preview::EditorSession {
            document_cache: editor_preview::test::empty_document_cache(),
            preview_config: Default::default(),
            open_urls: Default::default(),
            previews,
            pending_recompile: Default::default(),
        };
        (session, messages)
    }

    #[test]
    fn workspace_edit_synchronizes_session_and_previews_before_returning() {
        const SOURCE: &str = "export component Main inherits Rectangle { width: 30px; }";
        const CHANGED: &str = "export component Main inherits Rectangle { width: 40px; }";

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.slint");
        std::fs::write(&path, SOURCE).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        let (mut session, messages) = session_with_recording_previews();
        spin_on::spin_on(session.load_document(SOURCE.into(), url.clone(), None)).unwrap();
        clear_messages(&messages);

        let edit = editor_preview::editing::create_workspace_edit(
            url.clone(),
            None,
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 50),
                    lsp_types::Position::new(0, 52),
                ),
                new_text: "40".into(),
            }],
        );
        let result =
            spin_on::spin_on(handle_workspace_edit(&mut session, Some("Change width"), &edit));

        assert_eq!(result, WorkspaceEditApplication { applied: true, changed: true });
        assert_eq!(std::fs::read_to_string(path).unwrap(), CHANGED);
        let document = session.document_cache.get_document(&url).unwrap();
        assert_eq!(document.node.as_ref().unwrap().text().to_string(), CHANGED);
        for preview_messages in messages {
            assert!(preview_messages.borrow().iter().any(|message| matches!(
                message,
                LspToPreviewMessage::SetContents { url: changed_url, contents }
                    if changed_url.url() == &url && contents.as_slice() == CHANGED.as_bytes()
            )));
        }
    }

    fn clear_messages(messages: &[editor_preview::test::CapturedPreviewMessages]) {
        for messages in messages {
            messages.borrow_mut().clear();
        }
    }

    fn component(file_name: &str, name: &str) -> PreviewComponent {
        PreviewComponent {
            url: Url::from_file_path(editor_preview::test::test_file_name(file_name)).unwrap(),
            component: Some(name.into()),
        }
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
        let (mut session, messages) = session_with_recording_previews();
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        assert!(session.document_cache.get_document(&old_url).is_some());

        let new_project = tempfile::tempdir().unwrap();
        let new_path = new_project.path().join("new.slint");
        std::fs::write(&new_path, "export component New {}").unwrap();
        let project = Project::from_file(&new_path, Some("New".into())).unwrap();
        let expected_root = project.root.clone();
        let expected_component = project.preview.clone();
        let mut project_root = old_project.path().to_path_buf();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);

        spin_on::spin_on(switch_project(&mut session, &mut watcher, &mut project_root, project))
            .unwrap();

        assert_eq!(project_root, expected_root);
        assert!(session.document_cache.get_document(&old_url).is_none());
        assert!(session.document_cache.get_document(&expected_component.url).is_some());
        let expected_root_url =
            Url::from_directory_path(std::fs::canonicalize(&project_root).unwrap()).unwrap();
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::OpenProject { root }
                if root == &expected_root_url)
        }));
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component)
                if component == &expected_component)
        }));
        assert!(
            messages[PRIMARY_PREVIEW_INDEX]
                .borrow()
                .iter()
                .all(|message| !matches!(message, LspToPreviewMessage::Quit))
        );
        assert!(
            messages[RUN_PREVIEW_INDEX]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::Quit))
        );
    }

    #[test]
    fn failed_project_switch_preserves_the_active_session() {
        let (mut session, messages) = session_with_recording_previews();
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        let mut project_root = old_project.path().to_path_buf();
        let expected_root = project_root.clone();
        let missing_root = old_project.path().join("missing");
        let project = Project {
            root: missing_root.clone(),
            preview: PreviewComponent {
                url: Url::from_file_path(missing_root.join("main.slint")).unwrap(),
                component: Some("Missing".into()),
            },
        };
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);

        assert!(
            spin_on::spin_on(switch_project(
                &mut session,
                &mut watcher,
                &mut project_root,
                project,
            ))
            .is_err()
        );

        assert_eq!(project_root, expected_root);
        assert!(session.document_cache.get_document(&old_url).is_some());
        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }

    #[test]
    fn preview_requests_are_answered_through_the_originating_connection() {
        let (mut session, messages) = session_with_recording_previews();
        let mut run_preview_state = RunPreviewState::default();
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
            &mut run_preview_state,
            &Default::default(),
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
            &mut run_preview_state,
            &Default::default(),
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
        assert!(session.primary_preview().to_show.is_none());
        assert_eq!(session.preview(1).unwrap().to_show, Some(component));
    }

    #[test]
    fn run_preview_uses_the_primary_preview_target() {
        let (mut session, messages) = session_with_recording_previews();
        let primary_component = component("primary.slint", "Primary");
        session.show_preview(PRIMARY_PREVIEW_INDEX, primary_component.clone());
        clear_messages(&messages);

        let mut run_preview_state = RunPreviewState::default();
        run_preview(&mut session, &mut run_preview_state);

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component) if component == &primary_component)
        }));
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(primary_component));
    }

    #[test]
    fn run_preview_target_changes_only_when_run_is_requested() {
        let (mut session, messages) = session_with_recording_previews();
        let first_component = component("first.slint", "First");
        let second_component = component("second.slint", "Second");
        session.show_preview(PRIMARY_PREVIEW_INDEX, first_component.clone());
        let mut run_preview_state = RunPreviewState::default();
        run_preview(&mut session, &mut run_preview_state);
        clear_messages(&messages);

        session.show_preview(PRIMARY_PREVIEW_INDEX, second_component.clone());

        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(first_component));
        assert!(messages[RUN_PREVIEW_INDEX].borrow().is_empty());

        run_preview(&mut session, &mut run_preview_state);
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(second_component));
    }

    #[test]
    fn primary_show_document_highlights_the_run_preview() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("main.slint");
        let url = Url::from_file_path(&path).unwrap();
        let source = "export component Main {\n    Text {}\n}";
        spin_on::spin_on(session.load_document_impl(source.into(), url.clone(), None));
        session.show_preview(
            PRIMARY_PREVIEW_INDEX,
            PreviewComponent { url: url.clone(), component: Some("Main".into()) },
        );
        let mut run_preview_state = RunPreviewState::default();
        run_preview(&mut session, &mut run_preview_state);
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
            &mut run_preview_state,
            &Default::default(),
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
            &mut run_preview_state,
            &Default::default(),
        ));
        let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
        let show_position = run_messages
            .iter()
            .position(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
            .unwrap();
        let highlight_position = run_messages.iter().position(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        });
        assert!(highlight_position.is_some_and(|position| position > show_position));
    }

    #[test]
    fn show_document_from_the_run_preview_is_ignored() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let mut run_preview_state = RunPreviewState { requested: true, highlight: None };

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: Url::from_file_path(editor_preview::test::test_file_name("run.slint"))
                    .unwrap(),
                selection: Default::default(),
                take_focus: false,
            },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
            &mut run_preview_state,
            &Default::default(),
        ));

        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
        assert!(run_preview_state.highlight.is_none());
    }
}
